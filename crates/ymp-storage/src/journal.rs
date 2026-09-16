//! A durable Journal: content, references, ordered rows and head share one transaction.
use crate::{
    Database, MAX_CONTENT_BYTES, MAX_JOURNAL_BYTES, MAX_JOURNAL_EVENTS, content::SqliteContent,
    put_content, read_content, sql_error,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{collections::BTreeSet, path::Path};
use ymp_domain::{
    Denial, Digest, Id, Result,
    journal::{Envelope, decode, encode},
};
use ymp_kernel::{
    events::Event,
    journal::{AppendResolution, Journal, JournalRead, ParameterSchemas, validate_append},
};

pub struct SqliteJournal {
    database: Database,
    schemas: ParameterSchemas,
    #[cfg(test)]
    fault: std::sync::atomic::AtomicU8,
    #[cfg(test)]
    pub(crate) event_limit: usize,
}
impl SqliteJournal {
    pub fn open(path: impl AsRef<Path>, schemas: ParameterSchemas) -> Result<Self> {
        Ok(Self {
            database: Database::open(path.as_ref())?,
            schemas,
            #[cfg(test)]
            fault: std::sync::atomic::AtomicU8::new(0),
            #[cfg(test)]
            event_limit: MAX_JOURNAL_EVENTS,
        })
    }
    pub fn content_store(&self) -> SqliteContent {
        SqliteContent {
            database: self.database.clone(),
        }
    }

    fn write(
        &self,
        session: &Id,
        expected: u64,
        events: &[Envelope<Event>],
        attempted: &mut bool,
    ) -> Result<u64> {
        let mut connection = self.database.connect(true)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        let current = read_journal(&tx, session, &self.schemas)?;
        let validated = validate_append(&current, session, expected, events, &self.schemas)?;
        for event in events {
            if let Event::WorkspaceBound { binding, .. } = &event.payload
                && binding.journal != self.database.binding_identity()?
            {
                return Err(Denial::new(
                    "workspace_binding",
                    "Root binding targets another journal",
                ));
            }
        }
        if ymp_kernel::workspace_locks::touches_ownership(events) {
            let inventory = read_workspace_inventory(&tx, session, &self.schemas)?;
            ymp_kernel::journal::validate_workspace_append(
                &inventory,
                session,
                expected,
                events,
                &self.schemas,
            )?;
        }
        let (reserved_events, reserved_bytes) =
            ymp_kernel::workspace_locks::control_reserve(&validated);
        #[cfg(test)]
        let event_limit = self.event_limit;
        #[cfg(not(test))]
        let event_limit = MAX_JOURNAL_EVENTS;
        if validated.revision() + reserved_events as u64 > event_limit as u64 {
            return Err(Denial::new(
                "journal_limit",
                "Journal exceeds its event limit",
            ));
        }
        let mut prior = match current.events.last() {
            None => chain_start(session)?,
            Some(_) => {
                head(&tx, session)?
                    .ok_or_else(|| Denial::new("storage_corrupt", "Journal head is missing"))?
                    .1
            }
        };
        for event in events {
            let bytes = encode(event)?;
            let payload = Digest::of(&bytes);
            put_content(&tx, &payload, &bytes)?;
            let content = event.payload.contents()?;
            for (digest, value) in &content.attached {
                put_content(&tx, digest, value)?;
            }
            for digest in &content.required {
                read_content(&tx, digest, MAX_CONTENT_BYTES)?;
            }
            let chain = chain_digest(session, event.seq, &payload, &prior)?;
            tx.execute("INSERT INTO journal_events(session,seq,payload,prior,chain) VALUES(?1,?2,?3,?4,?5)",params![session.as_str(),event.seq.to_be_bytes().as_slice(),payload.as_str(),prior.as_str(),chain.as_str()]).map_err(sql_error)?;
            for digest in &content.required {
                tx.execute(
                    "INSERT INTO event_content(session,seq,digest) VALUES(?1,?2,?3)",
                    params![
                        session.as_str(),
                        event.seq.to_be_bytes().as_slice(),
                        digest.as_str()
                    ],
                )
                .map_err(sql_error)?;
            }
            prior = chain;
        }
        tx.execute("INSERT INTO journal_heads(session,last_seq,chain) VALUES(?1,?2,?3) ON CONFLICT(session) DO UPDATE SET last_seq=excluded.last_seq,chain=excluded.chain",params![session.as_str(),validated.revision().to_be_bytes().as_slice(),prior.as_str()]).map_err(sql_error)?;
        // Apply read limits to the resulting state before making it durable.
        check_limits(&tx, session, reserved_bytes)?;
        #[cfg(test)]
        match self.fault.load(std::sync::atomic::Ordering::SeqCst) {
            1 => {
                return Err(Denial::new(
                    "injected_before_commit",
                    "Test interruption before commit",
                ));
            }
            3 => std::process::exit(81),
            _ => {}
        }
        *attempted = true;
        tx.commit().map_err(sql_error)?;
        #[cfg(test)]
        match self.fault.load(std::sync::atomic::Ordering::SeqCst) {
            2 => {
                return Err(Denial::new(
                    "injected_after_commit",
                    "Test acknowledgement loss",
                ));
            }
            4 => std::process::exit(82),
            _ => {}
        }
        Ok(validated.revision())
    }
}
impl Journal for SqliteJournal {
    fn binding_identity(&self) -> Result<ymp_domain::workspace::JournalIdentity> {
        self.database.binding_identity()
    }

    fn workspace_binding(
        &self,
        root: &ymp_domain::workspace::WorkspaceLocation,
    ) -> Result<Option<ymp_domain::workspace::WorkspaceBinding>> {
        let mut connection = self.database.connect(false)?;
        let tx = connection.transaction().map_err(sql_error)?;
        let mut found = None;
        visit_journals(&tx, &self.schemas, |id, read| {
            let view = read.view_with_schemas(&id, None, &self.schemas)?;
            for binding in view.workspace_bindings().values().filter(|b| {
                (b.root.device == root.device && b.root.inode == root.inode)
                    || root_paths_overlap(&b.root.root, &root.root)
            }) {
                if found.as_ref().is_some_and(|known| known != binding) {
                    return Err(Denial::new(
                        "workspace_binding",
                        "Conflicting persisted root bindings",
                    ));
                }
                found = Some(binding.clone());
            }
            Ok(())
        })?;
        tx.commit().map_err(sql_error)?;
        Ok(found)
    }
    fn schemas(&self) -> &ParameterSchemas {
        &self.schemas
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        let mut connection = self.database.connect(false)?;
        let tx = connection.transaction().map_err(sql_error)?;
        let read = read_journal(&tx, session, &self.schemas)?;
        tx.commit().map_err(sql_error)?;
        Ok(read)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<ymp_kernel::journal::WorkspaceInventory> {
        let mut connection = self.database.connect(false)?;
        let tx = connection.transaction().map_err(sql_error)?;
        let inventory = read_workspace_inventory(&tx, session, &self.schemas)?;
        tx.commit().map_err(sql_error)?;
        Ok(inventory)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        let mut attempted = false;
        let result = self.write(session, expected, events, &mut attempted);
        if !attempted || result.is_ok() {
            return result;
        }
        // write() has dropped its transaction and connection. Resolve from a new
        // read transaction; uncommitted changes cannot masquerade as durable data.
        match self.resolve_append(session, expected, events) {
            Ok(AppendResolution::Committed(revision)) => Ok(revision),
            Ok(AppendResolution::Absent) => Err(Denial::new(
                "storage_not_committed",
                "Commit failed; the exact append is absent and may be retried at its original revision",
            )),
            Ok(AppendResolution::Conflict { .. }) => Err(Denial::new(
                "stale_revision",
                "Another append advanced the journal while commit was being resolved",
            )),
            Err(_) => Err(Denial::new(
                "storage_indeterminate",
                "Cannot determine whether the append committed; preserve the original request and resolve it before retrying",
            )),
        }
    }
}

fn root_paths_overlap(left: &str, right: &str) -> bool {
    let left = left.to_ascii_lowercase();
    let right = right.to_ascii_lowercase();
    left == "/"
        || right == "/"
        || left == right
        || left
            .strip_prefix(&right)
            .is_some_and(|tail| tail.starts_with('/'))
        || right
            .strip_prefix(&left)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn visit_journals(
    connection: &Connection,
    schemas: &ParameterSchemas,
    mut visit: impl FnMut(Id, JournalRead) -> Result<()>,
) -> Result<()> {
    // Refuse malformed keys before loading any key into a Rust String. Never
    // filter them out: their events might retain unresolved ownership.
    for table in ["journal_heads", "journal_events", "event_content"] {
        let query = format!(
            "SELECT EXISTS(SELECT 1 FROM {table} WHERE typeof(session)!='text' OR length(CAST(session AS BLOB)) NOT BETWEEN 1 AND 128)"
        );
        let invalid: bool = connection
            .query_row(&query, [], |row| row.get(0))
            .map_err(sql_error)?;
        if invalid {
            return Err(Denial::new(
                "storage_corrupt",
                "Invalid session key in ownership inventory",
            ));
        }
    }
    // The union deliberately includes orphan events and links. A missing head
    // must not make that session's active ownership disappear from coordination.
    let mut statement = connection.prepare("SELECT session FROM journal_heads UNION SELECT session FROM journal_events UNION SELECT session FROM event_content ORDER BY session").map_err(sql_error)?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error)?;
    for id in ids {
        let id = Id::new(id.map_err(sql_error)?)?;
        let read = read_journal(connection, &id, schemas)?;
        visit(id, read)?;
    }
    Ok(())
}

fn read_workspace_inventory(
    connection: &Connection,
    session: &Id,
    schemas: &ParameterSchemas,
) -> Result<ymp_kernel::journal::WorkspaceInventory> {
    let mut current = JournalRead {
        revision: 0,
        events: vec![],
    };
    let mut other = vec![];
    let mut weight = (0usize, 0usize);
    visit_journals(connection, schemas, |id, read| {
        if id == *session {
            current = read;
        } else {
            let owner = ymp_kernel::workspace_locks::WorkspaceOwnership::from_view(
                &read.view_with_schemas(&id, None, schemas)?,
            );
            if !owner.is_empty() {
                let (bytes, count) = owner.retained_weight()?;
                weight.0 += bytes;
                weight.1 += count;
                ymp_kernel::workspace_locks::validate_inventory_size(weight.0, weight.1)?;
                other.push(owner);
            }
        }
        Ok(())
    })?;
    Ok(ymp_kernel::journal::WorkspaceInventory { current, other })
}

fn chain_start(session: &Id) -> Result<Digest> {
    Digest::of_value(&("ymp journal chain", 1_u32, session))
}
fn chain_digest(session: &Id, seq: u64, payload: &Digest, prior: &Digest) -> Result<Digest> {
    Digest::of_value(&("ymp journal row", 1_u32, session, seq, payload, prior))
}
fn sequence(bytes: Vec<u8>) -> Result<u64> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        Denial::new(
            "storage_corrupt",
            "A stored sequence must be exactly eight bytes",
        )
    })?;
    Ok(u64::from_be_bytes(bytes))
}
fn digest(text: String) -> Result<Digest> {
    Digest::try_from(text)
        .map_err(|_| Denial::new("storage_corrupt", "A stored digest is malformed"))
}
fn head(connection: &Connection, session: &Id) -> Result<Option<(u64, Digest)>> {
    let row:Option<(Vec<u8>,String)>=connection.query_row("SELECT length(last_seq),length(chain),last_seq,chain FROM journal_heads WHERE session=?1",[session.as_str()],|r| {
        if r.get::<_,i64>(0)?!=8 || r.get::<_,i64>(1)?!=64 { return Err(rusqlite::Error::InvalidQuery); }
        Ok((r.get(2)?,r.get(3)?))
    }).optional().map_err(sql_error)?;
    row.map(|(seq, chain)| Ok((sequence(seq)?, digest(chain)?)))
        .transpose()
}
fn check_limits(connection: &Connection, session: &Id, reserved_bytes: usize) -> Result<usize> {
    let (count,bytes):(i64,i64)=connection.query_row("SELECT count(*),coalesce(sum(length(v.bytes)),0) FROM journal_events e LEFT JOIN content_values v ON v.digest=e.payload WHERE e.session=?1",[session.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).map_err(sql_error)?;
    let (links,linked_bytes):(i64,i64)=connection.query_row("SELECT count(*),coalesce(sum(length(v.bytes)),0) FROM event_content c LEFT JOIN content_values v ON v.digest=c.digest WHERE c.session=?1",[session.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).map_err(sql_error)?;
    let count = usize::try_from(count)
        .map_err(|_| Denial::new("storage_corrupt", "Invalid event count"))?;
    let links = usize::try_from(links)
        .map_err(|_| Denial::new("storage_corrupt", "Invalid content-link count"))?;
    let total = bytes
        .checked_add(linked_bytes)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| {
            Denial::new(
                "journal_limit",
                "Journal byte count exceeds addressable memory",
            )
        })?;
    if count > MAX_JOURNAL_EVENTS
        || links > MAX_JOURNAL_EVENTS * 16
        || total > MAX_JOURNAL_BYTES.saturating_sub(reserved_bytes)
    {
        return Err(Denial::new(
            "journal_limit",
            "Journal or its referenced content exceeds the bounded read limit",
        ));
    }
    let malformed:i64=connection.query_row("SELECT count(*) FROM journal_events WHERE session=?1 AND (length(seq)!=8 OR length(payload)!=64 OR length(prior)!=64 OR length(chain)!=64)",[session.as_str()],|r|r.get(0)).map_err(sql_error)?;
    let malformed_links:i64=connection.query_row("SELECT count(*) FROM event_content WHERE session=?1 AND (length(seq)!=8 OR length(digest)!=64)",[session.as_str()],|r|r.get(0)).map_err(sql_error)?;
    if malformed != 0 || malformed_links != 0 {
        return Err(Denial::new("storage_corrupt", "Malformed journal metadata"));
    }
    Ok(count)
}
fn read_journal(
    connection: &Connection,
    session: &Id,
    schemas: &ParameterSchemas,
) -> Result<JournalRead> {
    let count = check_limits(connection, session, 0)?;
    let orphan_links: i64 = connection.query_row("SELECT count(*) FROM event_content c LEFT JOIN journal_events e ON e.session=c.session AND e.seq=c.seq WHERE c.session=?1 AND e.session IS NULL", [session.as_str()], |r| r.get(0)).map_err(sql_error)?;
    if orphan_links != 0 {
        return Err(Denial::new(
            "storage_corrupt",
            "Content links name absent journal events",
        ));
    }
    let stored_head = head(connection, session)?;
    let Some((revision, last_chain)) = stored_head else {
        if count != 0 {
            return Err(Denial::new(
                "storage_corrupt",
                "Events exist without a journal head",
            ));
        }
        let links: i64 = connection
            .query_row(
                "SELECT count(*) FROM event_content WHERE session=?1",
                [session.as_str()],
                |r| r.get(0),
            )
            .map_err(sql_error)?;
        if links != 0 {
            return Err(Denial::new(
                "storage_corrupt",
                "Content links exist without a journal",
            ));
        }
        return Ok(JournalRead {
            revision: 0,
            events: vec![],
        });
    };
    if revision == 0 || revision != count as u64 {
        return Err(Denial::new(
            "storage_corrupt",
            "Journal head and event count disagree",
        ));
    }
    let mut statement = connection
        .prepare("SELECT seq,payload,prior,chain FROM journal_events WHERE session=?1 ORDER BY seq")
        .map_err(sql_error)?;
    let rows = statement
        .query_map([session.as_str()], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(sql_error)?;
    let mut events = Vec::with_capacity(count);
    let mut preceding = chain_start(session)?;
    for row in rows {
        let (seq, payload, prior, chain) = row.map_err(sql_error)?;
        let seq = sequence(seq)?;
        if seq != events.len() as u64 + 1 {
            return Err(Denial::new(
                "storage_corrupt",
                "Journal sequence has a gap or is out of order",
            ));
        }
        let payload = digest(payload)?;
        let prior = digest(prior)?;
        let chain = digest(chain)?;
        if prior != preceding || chain != chain_digest(session, seq, &payload, &prior)? {
            return Err(Denial::new(
                "storage_corrupt",
                "Journal hash chain disagrees with its records",
            ));
        }
        let bytes = read_content(connection, &payload, MAX_CONTENT_BYTES)?;
        let event: Envelope<Event> = decode(&bytes)?;
        if event.session != *session || event.seq != seq || encode(&event)? != bytes {
            return Err(Denial::new(
                "storage_corrupt",
                "Journal row does not match its canonical event",
            ));
        }
        let required = event.payload.contents()?;
        let mut link_statement = connection
            .prepare("SELECT digest FROM event_content WHERE session=?1 AND seq=?2 ORDER BY digest")
            .map_err(sql_error)?;
        let links = link_statement
            .query_map(
                params![session.as_str(), seq.to_be_bytes().as_slice()],
                |r| r.get::<_, String>(0),
            )
            .map_err(sql_error)?
            .map(|r| digest(r.map_err(sql_error)?))
            .collect::<Result<BTreeSet<_>>>()?;
        if links != required.required {
            return Err(Denial::new(
                "storage_corrupt",
                "Event content links differ from its required content",
            ));
        }
        for reference in &links {
            let bytes = read_content(connection, reference, MAX_CONTENT_BYTES)?;
            if required
                .attached
                .get(reference)
                .is_some_and(|expected| expected != &bytes)
            {
                return Err(Denial::new(
                    "content_digest",
                    "Event parameters differ from their retained bytes",
                ));
            }
        }
        events.push(event);
        preceding = chain;
    }
    if preceding != last_chain {
        return Err(Denial::new(
            "storage_corrupt",
            "Journal head hash does not match the last event",
        ));
    }
    let read = JournalRead { revision, events };
    read.view_with_schemas(session, None, schemas)?;
    Ok(read)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as support;
    use std::{
        process::{Command, Stdio},
        sync::atomic::Ordering,
    };
    use ymp_kernel::journal::ContentStore;

    #[test]
    fn commit_error_resolution_observes_only_committed_rows() {
        let directory = support::Directory::new();
        let journal =
            SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
        let session = Id::new("fault").unwrap();
        let events = [support::opening(&session)];
        journal.fault.store(1, Ordering::SeqCst);
        assert_eq!(
            journal.append(&session, 0, &events).unwrap_err().code,
            "injected_before_commit"
        );
        assert_eq!(
            journal.resolve_append(&session, 0, &events).unwrap(),
            AppendResolution::Absent
        );
        assert!(
            journal
                .content_store()
                .get(&support::policy("SoloWithVerifier").policy.params, 4096)
                .is_err()
        );
        journal.fault.store(2, Ordering::SeqCst);
        assert_eq!(journal.append(&session, 0, &events).unwrap(), 1);
        assert_eq!(
            journal.resolve_append(&session, 0, &events).unwrap(),
            AppendResolution::Committed(1)
        );
        assert_eq!(journal.read(&session).unwrap().revision, 1);
    }

    #[test]
    fn crash_child() {
        let Some(path) = std::env::var_os("YMP_STORAGE_CRASH_DATABASE") else {
            return;
        };
        let phase = std::env::var("YMP_STORAGE_CRASH_PHASE")
            .unwrap()
            .parse::<u8>()
            .unwrap();
        let journal = SqliteJournal::open(path, ParameterSchemas::default()).unwrap();
        journal.fault.store(phase, Ordering::SeqCst);
        let session = Id::new("crash").unwrap();
        journal
            .append(&session, 0, &[support::opening(&session)])
            .unwrap();
        panic!("The crash checkpoint must exit before returning");
    }
    #[test]
    fn abrupt_process_exit_before_or_after_commit_preserves_atomicity() {
        for (phase, exit_code, revision) in [(3, 81, 0), (4, 82, 1)] {
            let directory = support::Directory::new();
            SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
            let status = Command::new(std::env::current_exe().unwrap())
                .args(["journal::tests::crash_child", "--exact", "--nocapture"])
                .env("YMP_STORAGE_CRASH_DATABASE", directory.database())
                .env("YMP_STORAGE_CRASH_PHASE", phase.to_string())
                .stdout(Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(exit_code));
            let journal =
                SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
            let session = Id::new("crash").unwrap();
            assert_eq!(journal.read(&session).unwrap().revision, revision);
            let resolved = journal
                .resolve_append(&session, 0, &[support::opening(&session)])
                .unwrap();
            assert_eq!(
                resolved,
                if revision == 0 {
                    AppendResolution::Absent
                } else {
                    AppendResolution::Committed(1)
                }
            );
            let content = journal
                .content_store()
                .get(&support::policy("SoloWithVerifier").policy.params, 4096);
            assert_eq!(content.is_ok(), revision == 1);
        }
    }
    #[test]
    fn unsigned_sequence_encoding_preserves_order_and_refuses_invalid_width() {
        let values = [
            0,
            1,
            255,
            256,
            u32::MAX as u64,
            u32::MAX as u64 + 1,
            u64::MAX,
        ];
        let mut bytes: Vec<_> = values.iter().map(|n| n.to_be_bytes().to_vec()).collect();
        bytes.sort();
        assert_eq!(
            bytes
                .into_iter()
                .map(|b| sequence(b).unwrap())
                .collect::<Vec<_>>(),
            values
        );
        assert!(sequence(vec![0; 7]).is_err());
    }
    #[test]
    fn unsupported_event_version_with_intact_hashes_is_refused() {
        let directory = support::Directory::new();
        let journal =
            SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
        let session = Id::new("version").unwrap();
        journal
            .append(&session, 0, &[support::opening(&session)])
            .unwrap();
        let mut event = support::opening(&session);
        if let Event::SessionOpened { version, .. } = &mut event.payload {
            *version = 999;
        }
        let body = encode(&event).unwrap();
        let payload = Digest::of(&body);
        let prior = chain_start(&session).unwrap();
        let chain = chain_digest(&session, 1, &payload, &prior).unwrap();
        let connection = journal.database.connect(true).unwrap();
        put_content(&connection, &payload, &body).unwrap();
        connection
            .execute(
                "UPDATE journal_events SET payload=?1,chain=?2 WHERE session=?3",
                params![payload.as_str(), chain.as_str(), session.as_str()],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE journal_heads SET chain=?1 WHERE session=?2",
                params![chain.as_str(), session.as_str()],
            )
            .unwrap();
        assert_eq!(journal.read(&session).unwrap_err().code, "event_version");
    }
}
