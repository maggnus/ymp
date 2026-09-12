use anyhow::{bail, Context, Result};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use ymp_core::*;
mod authority;
mod budget;
mod provenance;
#[cfg(test)]
mod provenance_tests;
mod usage;

/// The exact FTS expression used by memory lookup and recorded by the runtime.
pub fn memory_search_query(query: &str) -> String {
    query
        .split_whitespace()
        .take(12)
        .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    pub home: PathBuf,
}

impl Store {
    /// Open an existing, current-schema store without creating files, migrating
    /// records, discovering providers, or changing configuration.
    pub fn open_read_only(home: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            home.join("state.sqlite"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 3 {
            bail!("This database was created by a newer ymp version");
        }
        if version != 3 {
            bail!("Structured read-only export requires schema 3; found {version}. Open the store normally to migrate an older schema.");
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            home: home.into(),
        })
    }

    pub fn open(home: &Path) -> Result<Self> {
        std::fs::create_dir_all(home)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700))?;
        }
        let mut conn = Connection::open(home.join("state.sqlite"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 3 {
            bail!("This database was created by a newer ymp version");
        }
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        if version == 0 {
            let tx = conn.transaction()?;
            tx.execute_batch(include_str!("schema.sql"))?;
            tx.execute_batch("PRAGMA user_version=1")?;
            tx.commit()?;
        }
        if version < 2 {
            usage::migrate(&mut conn)?;
        }
        if version < 3 {
            provenance::migrate(&mut conn)?;
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            home: home.to_owned(),
        })
    }
    fn db(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock poisoned"))
    }
    pub fn project(&self, path: &Path) -> Result<Project> {
        let path = path.canonicalize()?;
        let db = self.db()?;
        let found: Option<String> = db
            .query_row(
                "SELECT data FROM projects WHERE path=?",
                [path.to_string_lossy().as_ref()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(data) = found {
            return Ok(serde_json::from_str(&data)?);
        }
        let p = Project {
            id: new_id(),
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            path,
        };
        db.execute(
            "INSERT INTO projects(id,path,data) VALUES (?,?,?)",
            params![p.id, p.path.to_string_lossy(), serde_json::to_string(&p)?],
        )?;
        Ok(p)
    }
    pub fn get_project(&self, id: &str) -> Result<Project> {
        let data: String =
            self.db()?
                .query_row("SELECT data FROM projects WHERE id=?", [id], |r| r.get(0))?;
        Ok(serde_json::from_str(&data)?)
    }
    pub fn relocate_project(&self, id: &str, path: &Path) -> Result<()> {
        let mut project = self.get_project(id)?;
        project.path = path.canonicalize()?;
        self.db()?.execute(
            "UPDATE projects SET path=?,data=? WHERE id=?",
            params![
                project.path.to_string_lossy(),
                serde_json::to_string(&project)?,
                id
            ],
        )?;
        Ok(())
    }
    pub fn save_session(&self, s: &Session) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old: Option<String> = tx
            .query_row("SELECT data FROM sessions WHERE id=?", [&s.id], |r| {
                r.get(0)
            })
            .optional()?;
        let old: Option<Session> = old.map(|raw| serde_json::from_str(&raw)).transpose()?;
        if old
            .as_ref()
            .is_some_and(|old| old.project_id != s.project_id)
        {
            bail!("Session belongs to another project");
        }
        let mut current = s.clone();
        let historical = old.as_ref().map_or(0, |old| old.turns_used as u64);
        let count = usage::invocation_count(&tx, &s.id, historical.max(s.turns_used as u64))?;
        current.turns_used =
            usize::try_from(count).context("Invocation count exceeds platform capacity")?;
        usage::advance_invocation_count(&tx, &s.id, count)?;
        let data = serde_json::to_string(&current)?;
        tx.execute("INSERT INTO sessions(id,project_id,data) VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data",params![s.id,s.project_id,data])?;
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'session',?,?)",
            params![s.id, data, now()],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn session(&self, id: &str) -> Result<Session> {
        let data: String =
            self.db()?
                .query_row("SELECT data FROM sessions WHERE id=?", [id], |r| r.get(0))?;
        Ok(serde_json::from_str(&data)?)
    }
    pub fn sessions(&self, project: Option<&str>) -> Result<Vec<Session>> {
        let db = self.db()?;
        let mut q = db.prepare(
            "SELECT data FROM sessions WHERE (?1 IS NULL OR project_id=?1) ORDER BY rowid DESC",
        )?;
        let values = q
            .query_map([project], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn session_dir(&self, s: &Session) -> PathBuf {
        self.home
            .join("projects")
            .join(&s.project_id)
            .join("sessions")
            .join(&s.id)
    }
    pub fn lock_session(&self, s: &Session) -> Result<File> {
        let dir = self.session_dir(s);
        std::fs::create_dir_all(&dir)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join("session.lock"))?;
        file.try_lock_exclusive()
            .context("This session is already running in another ymp process")?;
        Ok(file)
    }
    pub fn lock_project(&self, project_id: &str) -> Result<File> {
        let dir = self.home.join("projects").join(project_id);
        std::fs::create_dir_all(&dir)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join("workspace.lock"))?;
        file.try_lock_exclusive()
            .context("Another ymp run is already using this working directory")?;
        Ok(file)
    }
    pub fn message(
        &self,
        session: &str,
        author: &str,
        recipient: Option<&str>,
        kind: &str,
        text: &str,
    ) -> Result<Message> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let created = now();
        tx.execute("INSERT INTO messages(session_id,author,recipient,kind,text,created_at) VALUES (?,?,?,?,?,?)",params![session,author,recipient,kind,text,created])?;
        let msg = Message {
            seq: tx.last_insert_rowid(),
            session_id: session.into(),
            author: author.into(),
            recipient: recipient.map(str::to_owned),
            kind: kind.into(),
            text: text.into(),
            created_at: created,
        };
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,?,?,?)",
            params![session, "message", serde_json::to_string(&msg)?, now()],
        )?;
        tx.commit()?;
        Ok(msg)
    }
    pub fn messages(&self, session: &str, after: i64, limit: usize) -> Result<Vec<Message>> {
        let db = self.db()?;
        let mut q=db.prepare("SELECT seq,session_id,author,recipient,kind,text,created_at FROM messages WHERE session_id=? AND seq>? ORDER BY seq LIMIT ?")?;
        let items = q
            .query_map(params![session, after, limit.min(10000)], |r| {
                Ok(Message {
                    seq: r.get(0)?,
                    session_id: r.get(1)?,
                    author: r.get(2)?,
                    recipient: r.get(3)?,
                    kind: r.get(4)?,
                    text: r.get(5)?,
                    created_at: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(items)
    }
    pub fn event(&self, session: &str, kind: &str, data: &serde_json::Value) -> Result<()> {
        self.db()?.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,?,?,?)",
            params![session, kind, data.to_string(), now()],
        )?;
        Ok(())
    }

    /// Read the actual latest user input, without depending on a history window.
    pub fn last_user_request(&self, session: &str) -> Result<Option<String>> {
        Ok(self.db()?.query_row(
            "SELECT text FROM messages WHERE session_id=? AND author='you' AND kind='user' ORDER BY seq DESC LIMIT 1",
            [session], |r| r.get(0),
        ).optional()?)
    }

    /// Acceptance commands the runtime ran for this session, oldest first.
    ///
    /// This is a read of the session log, which is the only place the commands and their
    /// results exist. A record is returned as it was written: a field the event does not
    /// carry stays absent, so a view cannot present an unknown result as a pass.
    pub fn checks(&self, session: &str) -> Result<Vec<CheckRun>> {
        let db = self.db()?;
        let mut q = db.prepare(
            "SELECT seq,data,created_at FROM events WHERE session_id=? AND kind='check' ORDER BY seq",
        )?;
        let rows = q
            .query_map([session], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .map(|(seq, data, recorded_at)| {
                let data: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
                CheckRun {
                    seq,
                    command: recorded_text(&data["command"]),
                    task: recorded_task(&data["task"]),
                    directory: recorded_text(&data["cwd"]),
                    outcome: match data["success"].as_bool() {
                        Some(true) => CheckOutcome::Passed,
                        Some(false) => CheckOutcome::Failed,
                        None => CheckOutcome::Unrecorded,
                    },
                    output: recorded_text(&data["output"]),
                    recorded_at,
                }
            })
            .collect())
    }
    pub fn save_task(&self, t: &Task) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        write_task(&tx, t)?;
        tx.commit()?;
        Ok(())
    }
    pub fn tasks(&self, session: &str) -> Result<Vec<Task>> {
        let db = self.db()?;
        let mut q = db.prepare("SELECT data FROM tasks WHERE session_id=? ORDER BY rowid")?;
        let values = q
            .query_map([session], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    /// Publish a complete accepted plan atomically. Recovery must never see only
    /// the first half of a dependency graph.
    pub fn save_plan(&self, tasks: &[Task]) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        write_plan(&tx, tasks)?;
        tx.commit()?;
        Ok(())
    }
    pub fn proposed_memory(&self, session: &str) -> Result<Vec<MemoryEntry>> {
        let db = self.db()?;
        let mut q = db.prepare("SELECT data FROM memory WHERE status='proposed' AND json_extract(data,'$.source_session')=? ORDER BY rowid LIMIT 5")?;
        let values = q
            .query_map([session], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn put_value(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        self.db()?.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value.to_string()])?;
        Ok(())
    }
    pub fn value(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let data: Option<String> = self
            .db()?
            .query_row("SELECT value FROM kv WHERE key=?", [key], |r| r.get(0))
            .optional()?;
        data.map(|s| serde_json::from_str(&s).map_err(Into::into))
            .transpose()
    }
    pub fn observe(&self, o: &Observation) -> Result<bool> {
        Ok(self.db()?.execute("INSERT OR IGNORE INTO observations(id,agent_version,competence,difficulty,success,data) VALUES (?,?,?,?,?,?)",params![o.id,o.agent_version,o.competence,o.difficulty,o.success,serde_json::to_string(o)?])?==1)
    }
    pub fn reputation(
        &self,
        version: &str,
        competence: &str,
        difficulty: &str,
    ) -> Result<Reputation> {
        let db = self.db()?;
        let mut q=db.prepare("SELECT success FROM observations WHERE agent_version=? AND competence=? AND difficulty=? ORDER BY rowid DESC LIMIT 100")?;
        let values = q
            .query_map(params![version, competence, difficulty], |r| {
                r.get::<_, bool>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Reputation {
            successes: values.iter().filter(|&&v| v).count() as u32,
            failures: values.iter().filter(|&&v| !v).count() as u32,
        })
    }
    pub fn observations(&self) -> Result<Vec<Observation>> {
        let db = self.db()?;
        let mut q = db.prepare("SELECT data FROM observations ORDER BY rowid DESC LIMIT 500")?;
        let values = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn save_memory(&self, m: &MemoryEntry) -> Result<()> {
        if m.project_id.is_none()
            && m.status == "active"
            && (m.reviewer.is_none() || m.reviewer.as_deref() == Some(&m.author))
        {
            bail!("Global memory requires independent review");
        }
        let mut db = self.db()?;
        let tx = db.transaction()?;
        tx.execute("INSERT INTO memory(id,project_id,status,data) VALUES (?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,data=excluded.data",params![m.id,m.project_id,m.status,serde_json::to_string(m)?])?;
        tx.execute("DELETE FROM memory_search WHERE id=?", [&m.id])?;
        tx.execute(
            "INSERT INTO memory_search(id,title,content) VALUES (?,?,?)",
            params![m.id, m.title, m.content],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn memory(&self, project: Option<&str>, query: &str) -> Result<Vec<MemoryEntry>> {
        let db = self.db()?;
        read_memory(&db, project, query)
    }
    pub fn forget_memory(&self, id: &str) -> Result<()> {
        let data: String =
            self.db()?
                .query_row("SELECT data FROM memory WHERE id=?", [id], |r| r.get(0))?;
        let mut m: MemoryEntry = serde_json::from_str(&data)?;
        m.status = "retired".into();
        self.save_memory(&m)
    }
}

fn write_task(tx: &rusqlite::Transaction<'_>, task: &Task) -> Result<()> {
    let old: Option<String> = tx
        .query_row("SELECT session_id FROM tasks WHERE id=?", [&task.id], |r| {
            r.get(0)
        })
        .optional()?;
    if old
        .as_ref()
        .is_some_and(|session| session != &task.session_id)
    {
        bail!("Task belongs to another session");
    }
    // A task transition invalidates authority over the prior result/attempt in
    // the same transaction, including cancellation and runtime reassignment.
    if old.is_some() {
        let prior: Task = provenance::record(tx, "tasks", &task.id)?;
        if prior.attempts != task.attempts
            || prior.assignee != task.assignee
            || prior.state != task.state
        {
            for assignment in
                provenance::records::<AssignmentRecord>(tx, "assignments", &task.session_id)?
            {
                if assignment.state == InvocationState::Running
                    && !assignment.grant_ids.is_empty()
                    && assignment
                        .task
                        .as_ref()
                        .is_some_and(|reference| reference.task_id == task.id)
                {
                    let invocations = provenance::records::<InvocationRecord>(
                        tx,
                        "invocations",
                        &task.session_id,
                    )?;
                    if let Some(invocation) = invocations
                        .iter()
                        .find(|v| v.assignment_id == assignment.id)
                    {
                        provenance::finish(
                            tx,
                            &task.session_id,
                            &invocation.id,
                            InvocationState::Interrupted,
                            Some("Task authority changed"),
                        )?;
                    }
                }
            }
        }
    }
    let data = serde_json::to_string(task)?;
    tx.execute("INSERT INTO tasks(id,session_id,data) VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data",params![task.id,task.session_id,data])?;
    tx.execute(
        "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'task',?,?)",
        params![task.session_id, data, now()],
    )?;
    Ok(())
}

fn write_plan(tx: &rusqlite::Transaction<'_>, tasks: &[Task]) -> Result<()> {
    for task in tasks {
        let data = serde_json::to_string(task)?;
        tx.execute(
            "INSERT INTO tasks(id,session_id,data) VALUES (?,?,?)",
            params![task.id, task.session_id, data],
        )?;
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'task',?,?)",
            params![task.session_id, data, now()],
        )?;
    }
    Ok(())
}

/// The task attempt a record names, or nothing. A field that is absent, null or not a usable
/// reference leaves the run unscoped, which is what the final pass and every record written
/// before runs carried a task look like.
fn recorded_task(value: &serde_json::Value) -> Option<TaskAttemptRef> {
    serde_json::from_value::<TaskAttemptRef>(value.clone())
        .ok()
        .filter(|task| !task.task_id.trim().is_empty())
}

/// A recorded string, or nothing. A field that is absent, blank or not a string carries no
/// information and must not become an empty value a reader could mistake for one.
fn recorded_text(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim_end)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
}

fn read_memory(db: &Connection, project: Option<&str>, query: &str) -> Result<Vec<MemoryEntry>> {
    let terms = memory_search_query(query);
    let sql = if terms.is_empty() {
        "SELECT data FROM memory WHERE status='active' AND (project_id IS NULL OR project_id=?1) ORDER BY rowid DESC LIMIT 20"
    } else {
        "SELECT m.data FROM memory m JOIN memory_search f ON f.id=m.id WHERE m.status='active' AND (m.project_id IS NULL OR m.project_id=?1) AND memory_search MATCH ?2 ORDER BY rank LIMIT 10"
    };
    let mut q = db.prepare(sql)?;
    let values = if terms.is_empty() {
        q.query_map([project], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        q.query_map(params![project, terms], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    values
        .into_iter()
        .map(|s| Ok(serde_json::from_str(&s)?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_ids_cannot_be_rebound_to_another_project() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("state")).unwrap();
        let first = store.project(temp.path()).unwrap();
        let other = store.project(&store.home).unwrap();
        let mut session = Session {
            id: new_id(),
            project_id: first.id.clone(),
            title: "goal".into(),
            status: "created".into(),
            created_at: now(),
            team: vec![],
            turns_used: 0,
        };
        store.save_session(&session).unwrap();
        session.project_id = other.id;
        assert!(store.save_session(&session).is_err());
        assert_eq!(store.session(&session.id).unwrap().project_id, first.id);
    }
    #[test]
    fn observations_are_idempotent_and_memory_is_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let o = Observation {
            id: "attempt".into(),
            agent_version: "v".into(),
            agent_name: "a".into(),
            competence: "analysis".into(),
            difficulty: "simple".into(),
            success: true,
            evidence: "checks".into(),
            created_at: now(),
        };
        assert!(store.observe(&o).unwrap());
        assert!(!store.observe(&o).unwrap());
        assert_eq!(
            store
                .reputation("v", "analysis", "simple")
                .unwrap()
                .successes,
            1
        );
        let m = MemoryEntry {
            id: new_id(),
            project_id: Some("a".into()),
            kind: "procedure".into(),
            title: "Rust".into(),
            content: "Run cargo test".into(),
            source_session: "s".into(),
            author: "a".into(),
            reviewer: Some("b".into()),
            status: "active".into(),
            created_at: now(),
            supersedes: None,
        };
        store.save_memory(&m).unwrap();
        assert_eq!(store.memory(Some("a"), "cargo").unwrap().len(), 1);
        assert!(store.memory(Some("b"), "cargo").unwrap().is_empty());
        store.forget_memory(&m.id).unwrap();
        assert!(store.memory(Some("a"), "").unwrap().is_empty());
    }
    #[test]
    fn recorded_checks_are_read_back_in_order_without_completing_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        assert!(store.checks("absent-session").unwrap().is_empty());
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"cwd":"/project","command":"cargo test","success":true,"output":"exit: 0\n"}),
            )
            .unwrap();
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"command":"cargo test","success":false,"output":"exit: 101"}),
            )
            .unwrap();
        // An event written without a result, and one written with nothing usable at all.
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"command":"npm run build"}),
            )
            .unwrap();
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"command":"   ","output":""}),
            )
            .unwrap();
        store
            .event(
                "s",
                "message",
                &serde_json::json!({"command":"not a check"}),
            )
            .unwrap();
        store
            .event("other", "check", &serde_json::json!({"command":"foreign"}))
            .unwrap();

        // The shapes a task reference arrives in: scoped, explicitly null, absent, and
        // unusable. Only the first names a task.
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"command":"scoped","task":{"task_id":"task-1","attempt":2},"success":true}),
            )
            .unwrap();
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"command":"final pass","task":null,"success":true}),
            )
            .unwrap();
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"command":"blank task","task":{"task_id":"  ","attempt":1},"success":true}),
            )
            .unwrap();
        store
            .event(
                "s",
                "check",
                &serde_json::json!({"command":"partial task","task":{"task_id":"task-1"},"success":true}),
            )
            .unwrap();

        let checks = store.checks("s").unwrap();
        assert_eq!(checks.len(), 8, "{checks:?}");
        assert_eq!(
            checks[4].task,
            Some(TaskAttemptRef {
                task_id: "task-1".into(),
                attempt: 2
            })
        );
        assert_eq!(checks[5].task, None, "an explicit null became a task");
        assert_eq!(checks[6].task, None, "a blank task id became a task");
        assert_eq!(
            checks[7].task, None,
            "a reference missing its attempt became a task"
        );
        assert_eq!(
            checks[0].task, None,
            "a record written without the field became a task"
        );
        assert!(checks.windows(2).all(|pair| pair[0].seq < pair[1].seq));
        assert_eq!(checks[0].command.as_deref(), Some("cargo test"));
        assert_eq!(checks[0].directory.as_deref(), Some("/project"));
        assert_eq!(checks[0].outcome, CheckOutcome::Passed);
        assert_eq!(checks[0].output.as_deref(), Some("exit: 0"));
        assert!(!checks[0].recorded_at.is_empty());
        assert_eq!(checks[1].outcome, CheckOutcome::Failed);
        assert_eq!(checks[1].directory, None, "a directory was invented");
        assert_eq!(
            checks[2].outcome,
            CheckOutcome::Unrecorded,
            "a missing result became a pass"
        );
        assert_eq!(checks[2].output, None);
        assert_eq!(checks[3].command, None, "a blank command became a command");
    }

    #[test]
    fn only_one_session_may_own_a_project_writer_lock() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("state")).unwrap();
        let first = store.lock_project("project").unwrap();
        assert!(store.lock_project("project").is_err());
        assert!(store.lock_project("other-project").is_ok());
        drop(first);
        assert!(store.lock_project("project").is_ok());
    }
}
