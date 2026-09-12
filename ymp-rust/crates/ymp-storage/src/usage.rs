use super::Store;
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use ymp_core::{InvocationObservation, InvocationState, SessionUsage, TokenCounts, UsageSnapshot};

impl Store {
    pub fn begin_usage(&self, session: &str, turn: u64, agent: &str) -> Result<()> {
        let db = self.db()?;
        let existing: Option<(String, String)> = db
            .query_row(
                "SELECT agent,status FROM token_usage WHERE session_id=? AND turn=?",
                params![session, turn],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((prior, state)) = existing {
            ensure!(
                prior == agent && state == "running",
                "Usage ordinal is already attributed or closed"
            );
            return Ok(());
        }
        db.execute(
            "INSERT INTO token_usage(session_id,turn,agent,status) VALUES (?,?,?,'running')",
            params![session, turn, agent],
        )?;
        Ok(())
    }
    pub fn update_usage(&self, session: &str, turn: u64, snapshot: &UsageSnapshot) -> Result<()> {
        if let Some(id) = self.usage_invocation(session, turn)? {
            return self.observe_invocation(
                session,
                &id,
                &InvocationObservation {
                    usage: Some(snapshot.clone()),
                    ..Default::default()
                },
            );
        }
        let db = self.db()?;
        let old: Option<String> = db
            .query_row(
                "SELECT snapshot FROM token_usage WHERE session_id=? AND turn=?",
                params![session, turn],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        if let Some(old) = old {
            let prior: UsageSnapshot = serde_json::from_str(&old)?;
            if prior.finalized && !snapshot.finalized {
                return Ok(());
            }
        }
        db.execute(
            "UPDATE token_usage SET snapshot=? WHERE session_id=? AND turn=?",
            params![serde_json::to_string(snapshot)?, session, turn],
        )?;
        Ok(())
    }
    pub fn finish_usage(&self, session: &str, turn: u64, status: &str) -> Result<()> {
        if let Some(id) = self.usage_invocation(session, turn)? {
            let state: InvocationState = serde_json::from_value(Value::String(status.into()))?;
            return self.finish_invocation(session, &id, state, Some(state.as_str()));
        }
        self.db()?.execute(
            "UPDATE token_usage SET status=? WHERE session_id=? AND turn=?",
            params![status, session, turn],
        )?;
        Ok(())
    }
    pub fn session_usage(&self, session: &str) -> Result<SessionUsage> {
        session_usage(&*self.db()?, session)
    }
    fn usage_invocation(&self, session: &str, turn: u64) -> Result<Option<String>> {
        Ok(self
            .db()?
            .query_row(
                "SELECT id FROM invocations WHERE session_id=? AND turn=?",
                params![session, turn],
                |r| r.get(0),
            )
            .optional()?)
    }
}

/// Invocation ordinals are cumulative. Gaps retain unknown historical calls;
/// a new detailed row beyond the saved history cannot replace those calls.
pub(super) fn invocation_count(db: &Connection, session: &str, saved: u64) -> Result<u64> {
    let durable: Option<String> = db
        .query_row(
            "SELECT value FROM kv WHERE key=?",
            [format!("turns:{session}")],
            |r| r.get(0),
        )
        .optional()?;
    let durable = durable
        .map(|raw| {
            raw.parse::<u64>()
                .context("Invalid durable invocation count")
        })
        .transpose()?
        .unwrap_or(0);
    let highest: u64 = db.query_row(
        "SELECT COALESCE(MAX(turn),0) FROM token_usage WHERE session_id=?",
        [session],
        |r| r.get(0),
    )?;
    Ok(saved.max(durable).max(highest))
}

pub(super) fn advance_invocation_count(db: &Connection, session: &str, count: u64) -> Result<()> {
    db.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value WHERE CAST(excluded.value AS INTEGER)>CAST(kv.value AS INTEGER)", params![format!("turns:{session}"), count.to_string()])?;
    Ok(())
}

pub(super) fn session_usage(db: &Connection, session: &str) -> Result<SessionUsage> {
    let mut q = db.prepare(
        "SELECT agent,status,snapshot FROM token_usage WHERE session_id=? ORDER BY turn",
    )?;
    let rows = q.query_map([session], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut usage = SessionUsage::default();
    for row in rows {
        let (agent, status, raw) = row?;
        let snapshot = raw
            .map(|v| serde_json::from_str::<UsageSnapshot>(&v))
            .transpose()?;
        usage.total.include(snapshot.as_ref(), status == "running");
        usage
            .agents
            .entry(agent)
            .or_default()
            .include(snapshot.as_ref(), status == "running");
    }
    // A diagnostic or an older incomplete trace can have a recorded turn count
    // but no per-invocation events. Such usage is unavailable, never free.
    let raw_session: Option<String> = db
        .query_row("SELECT data FROM sessions WHERE id=?", [session], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(raw) = raw_session {
        let record: Value = serde_json::from_str(&raw)?;
        let expected = invocation_count(db, session, record["turns_used"].as_u64().unwrap_or(0))?;
        if expected > usage.total.calls {
            let missing = expected - usage.total.calls;
            usage.total.calls = expected;
            if let Some(team) = record["team"].as_array().filter(|t| t.len() == 1) {
                if let Some(id) = team[0]["id"].as_str() {
                    usage.agents.entry(id.into()).or_default().calls += missing;
                }
            }
        }
    }
    Ok(usage)
}

pub(super) fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TABLE token_usage(session_id TEXT NOT NULL,turn INTEGER NOT NULL,agent TEXT NOT NULL,status TEXT NOT NULL,snapshot TEXT,PRIMARY KEY(session_id,turn)); CREATE INDEX token_usage_agent ON token_usage(session_id,agent);")?;
    let mut q=tx.prepare("SELECT session_id,kind,data FROM events WHERE kind IN ('turn_started','turn_completed','turn_failed') ORDER BY seq")?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(q);
    for (session, kind, raw) in rows {
        let Ok(event) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let (Some(turn), Some(agent)) = (event["turn"].as_u64(), event["agent"].as_str()) else {
            continue;
        };
        let status = match kind.as_str() {
            "turn_completed" => "completed",
            "turn_failed" => "failed",
            _ => "interrupted",
        };
        tx.execute("INSERT INTO token_usage(session_id,turn,agent,status) VALUES (?,?,?,?) ON CONFLICT(session_id,turn) DO UPDATE SET status=excluded.status",params![session,turn,agent,status])?;
        if let Some(snapshot) = legacy(&event["usage"]) {
            tx.execute(
                "UPDATE token_usage SET snapshot=? WHERE session_id=? AND turn=?",
                params![serde_json::to_string(&snapshot)?, session, turn],
            )?;
        }
    }
    tx.execute_batch("PRAGMA user_version=2")?;
    tx.commit()?;
    Ok(())
}

/// Old traces lack native baselines and whole-tree totals. Preserve known partial
/// contributions instead of summing overlapping cumulative counters.
fn legacy(raw: &Value) -> Option<UsageSnapshot> {
    let c = if raw.get("last").is_some() {
        let v = &raw["last"];
        TokenCounts {
            input: v["inputTokens"].as_u64(),
            output: v["outputTokens"].as_u64(),
            cache_read: v["cachedInputTokens"].as_u64(),
            cache_write: v["cacheWriteInputTokens"].as_u64(),
            reasoning: v["reasoningOutputTokens"].as_u64(),
        }
    } else if raw.get("input_tokens").is_some() {
        let input = raw["input_tokens"].as_u64().map(|n| {
            n.saturating_add(raw["cache_read_input_tokens"].as_u64().unwrap_or(0))
                .saturating_add(raw["cache_creation_input_tokens"].as_u64().unwrap_or(0))
        });
        TokenCounts {
            input,
            output: raw["output_tokens"].as_u64(),
            cache_read: raw["cache_read_input_tokens"].as_u64(),
            cache_write: raw["cache_creation_input_tokens"].as_u64(),
            reasoning: raw
                .pointer("/output_tokens_details/thinking_tokens")
                .and_then(Value::as_u64),
        }
    } else if raw.get("inputTokens").is_some() {
        TokenCounts {
            input: raw["inputTokens"].as_u64(),
            output: raw["outputTokens"].as_u64(),
            cache_read: raw["cachedReadTokens"].as_u64(),
            cache_write: raw["cachedWriteTokens"].as_u64(),
            reasoning: raw["thoughtTokens"].as_u64(),
        }
    } else {
        return None;
    };
    Some(UsageSnapshot {
        counts: c,
        finalized: true,
        partial: true,
        note: Some("Partial historical provider data".into()),
        native_total: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn snapshots_replace_and_agents_are_not_grouped_by_provider() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path()).unwrap();
        for (turn, agent) in [(1, "writer"), (2, "reviewer")] {
            store.begin_usage("s", turn, agent).unwrap();
        }
        let mut snap = UsageSnapshot {
            counts: TokenCounts {
                input: Some(100),
                output: Some(10),
                ..Default::default()
            },
            ..Default::default()
        };
        store.update_usage("s", 1, &snap).unwrap();
        store.update_usage("s", 1, &snap).unwrap();
        assert_eq!(
            store.session_usage("s").unwrap().total.known_total(),
            Some(110)
        );
        snap.counts.output = Some(20);
        snap.finalized = true;
        store.update_usage("s", 1, &snap).unwrap();
        store.finish_usage("s", 1, "completed").unwrap();
        store.update_usage("s", 2, &snap).unwrap();
        store.finish_usage("s", 2, "failed").unwrap();
        let mut stale = snap.clone();
        stale.counts.output = Some(10);
        stale.finalized = false;
        store.update_usage("s", 1, &stale).unwrap();
        let summary = store.session_usage("s").unwrap();
        assert_eq!(summary.agents.len(), 2);
        assert_eq!(summary.total.known_total(), Some(240));
        assert!(!summary.total.is_partial());
        let reopened = Store::open(temp.path()).unwrap();
        assert_eq!(reopened.session_usage("s").unwrap(), summary);
    }
    #[test]
    fn recorded_turns_without_events_are_unknown_not_zero() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path()).unwrap();
        let record = json!({"turns_used":2,"team":[{"id":"probe-agent"}]});
        let project = store.project(temp.path()).unwrap();
        store
            .db()
            .unwrap()
            .execute(
                "INSERT INTO sessions(id,project_id,data) VALUES ('probe',?,?)",
                params![project.id, record.to_string()],
            )
            .unwrap();
        let summary = store.session_usage("probe").unwrap();
        assert_eq!(summary.total.calls, 2);
        assert_eq!(summary.total.known_total(), None);
        assert!(summary.total.is_partial());
        assert_eq!(summary.agents["probe-agent"].calls, 2);
        assert_eq!(summary.agents["probe-agent"].known_total(), None);
    }
    #[test]
    fn migration_does_not_add_cumulative_history_again() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.sqlite");
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(include_str!("schema.sql")).unwrap();
        conn.execute_batch("PRAGMA user_version=1").unwrap();
        for turn in 1..=2 {
            let event = json!({"turn":turn,"agent":"a","usage":{"total":{"inputTokens":10000*turn,"outputTokens":1000*turn},"last":{"inputTokens":100,"outputTokens":10}}});
            conn.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES ('old','turn_completed',?,'now')",[event.to_string()]).unwrap();
        }
        drop(conn);
        let store = Store::open(temp.path()).unwrap();
        let usage = store.session_usage("old").unwrap();
        assert_eq!(usage.total.known_total(), Some(220));
        assert!(usage.total.is_partial());
    }
}
