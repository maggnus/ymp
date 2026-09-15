//! Durable [`Journal`] adapter backed by an embedded SQLite database.
//!
//! [`SqliteJournal`] implements the kernel's `Journal` port over one SQLite
//! database file (`<root>/journal.db`) opened through `rusqlite` with the
//! bundled SQLite build. It owns a narrow journal schema identified by the
//! database header's application identifier and schema version, keeps a full
//! `u64` revision in two nonnegative 32-bit columns, appends each batch as one
//! `BEGIN IMMEDIATE` transaction, and reads each stream as one coherent
//! snapshot while validating the payload version, a per-row content checksum,
//! strict payload decoding, revision continuity and head agreement.
//!
//! # Durability and filesystem assumptions
//!
//! The database runs in WAL mode with `synchronous=FULL` and a bounded busy
//! timeout (5 seconds): every commit flushes to the storage device before it
//! is acknowledged, and competing writers serialize on the write lock instead
//! of failing immediately. This assumes an honest local filesystem whose
//! `fsync` actually persists data; network filesystems and storage stacks that
//! acknowledge writes before they are durable void the durability claim.
//!
//! Copying a database file while a journal handle is open is unsafe: the WAL
//! and shared-memory side files may hold committed frames the main file lacks.
//! Only a quiescent database (every handle closed) may be copied, or a
//! coherent backup taken through SQLite's own backup API, which remains safe
//! while the database is in use.
//!
//! The invariants, failure mapping and acceptance criteria are fixed by the
//! DEV-0005 work order (`ymp-docs/tasks/records/persistence/DEV-0005.json`);
//! the standalone durable Journal contract document is DEV-0004 work and is
//! deliberately not part of this crate.

#![forbid(unsafe_code)]

mod payload;

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};
use ymp_domain::SessionId;
use ymp_kernel::{Journal, JournalEntry, JournalError, Revision, SessionEvent};

use payload::{MAX_PAYLOAD_BYTES, PAYLOAD_VERSION};

/// The journal's application identifier, stored in the database header
/// (`0x594D504A`, the ASCII bytes `YMPJ`).
pub const APPLICATION_ID: i32 = 0x594D_504A;

/// The journal schema version this build writes and reads.
pub const SCHEMA_VERSION: u16 = 1;

/// A batch appends at most this many events.
pub const MAX_BATCH_EVENTS: usize = 256;

/// The database file name inside the caller-selected root.
const DATABASE_FILE_NAME: &str = "journal.db";

/// Upper bound both nonnegative 32-bit revision columns must respect.
const U32_MAX_I64: i64 = u32::MAX as i64;

/// How long a database operation waits for a busy database before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// A one-shot fault-injection point for acceptance tests of atomicity and
/// indeterminate-commit behavior.
///
/// Arming a fault affects exactly the next `append` on the journal (any clone)
/// and is consumed by it; reads are never affected. The seam exists only to
/// make failure states reproducible in tests — no other code path uses it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultPoint {
    /// Fails the next append after the batch rows are written but before the
    /// transaction commits. The rollback is executed and confirmed; the
    /// failure is reported as [`JournalError::AdapterFailure`].
    BeforeCommit,
    /// Commits the next append, then reports
    /// [`JournalError::IndeterminateCommit`] even though the batch is
    /// committed: the visible indeterminate case.
    IndeterminateCommitted,
    /// Rolls the next append back, then reports
    /// [`JournalError::IndeterminateCommit`] with the batch absent: the absent
    /// indeterminate case.
    IndeterminateRolledBack,
}

/// A durable `Journal` over one SQLite database file.
///
/// All clones share one connection and one fault-injection state. Opening
/// validates an existing database's application identifier and schema version
/// before changing the journal mode or executing DDL, then runs with WAL,
/// `synchronous=FULL`, a bounded busy timeout and enforced foreign keys.
#[derive(Clone)]
pub struct SqliteJournal {
    inner: Arc<Inner>,
}

struct Inner {
    database_path: PathBuf,
    connection: Mutex<Connection>,
    fault: Mutex<Option<FaultPoint>>,
}

impl fmt::Debug for SqliteJournal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteJournal")
            .field("database_path", &self.inner.database_path)
            .finish_non_exhaustive()
    }
}

impl SqliteJournal {
    /// Opens (creating if absent) the journal database `<root>/journal.db`.
    ///
    /// The root directory is created if missing. An existing database is
    /// validated before any journal-mode change or DDL: a journal of this
    /// application with an unknown schema version fails as
    /// [`JournalError::UnsupportedFormat`] without being rewritten, migrated
    /// or repaired; a foreign or corrupted database fails as
    /// [`JournalError::Corruption`].
    pub fn open(root: &Path) -> Result<Self, JournalError> {
        std::fs::create_dir_all(root).map_err(|error| JournalError::AdapterFailure {
            message: format!("journal root {:?} cannot be created: {error}", root),
        })?;
        let database_path = root.join(DATABASE_FILE_NAME);
        let mut connection = Connection::open(&database_path).map_err(sqlite_to_adapter)?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(sqlite_to_adapter)?;
        prepare(&mut connection)?;
        Ok(Self {
            inner: Arc::new(Inner {
                database_path,
                connection: Mutex::new(connection),
                fault: Mutex::new(None),
            }),
        })
    }

    /// The database file path this journal uses.
    pub fn database_path(&self) -> &Path {
        &self.inner.database_path
    }

    /// Arms a one-shot fault consumed by the next `append` (see
    /// [`FaultPoint`]). Test seam only.
    pub fn inject_fault(&self, point: FaultPoint) {
        if let Ok(mut fault) = self.inner.fault.lock() {
            *fault = Some(point);
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, JournalError> {
        self.inner
            .connection
            .lock()
            .map_err(|_| JournalError::AdapterFailure {
                message: "journal connection lock is poisoned".to_owned(),
            })
    }

    fn take_fault(&self) -> Option<FaultPoint> {
        self.inner
            .fault
            .lock()
            .ok()
            .and_then(|mut fault| fault.take())
    }
}

impl Journal for SqliteJournal {
    fn read(&self, session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError> {
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(map_sqlite_error)?;

        let head = read_head(&transaction, session_id)?;
        let raw_entries = read_raw_entries(&transaction, session_id)?;
        transaction
            .commit()
            .map_err(|error| JournalError::AdapterFailure {
                message: format!("journal read transaction failed: {error}"),
            })?;
        drop(connection);

        validate_entries(session_id, head, raw_entries)
    }

    fn append(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        if events.is_empty() {
            return Err(JournalError::EmptyBatch);
        }
        if events.len() > MAX_BATCH_EVENTS {
            return Err(JournalError::AdapterFailure {
                message: format!("append batch exceeds the {MAX_BATCH_EVENTS}-event limit"),
            });
        }

        // All decisions that need no storage happen before any storage access.
        let mut next_revision = expected_revision;
        let mut encoded = Vec::with_capacity(events.len());
        for event in &events {
            next_revision = next_revision
                .checked_next()
                .ok_or(JournalError::RevisionOverflow)?;
            encoded.push((next_revision, payload::encode_event(event)?));
        }
        let attempted = next_revision;

        let fault = self.take_fault();
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;

        let head = read_head(&transaction, session_id)?;
        let actual_revision = match head {
            Some((head_hi, head_lo)) => compose_revision(head_hi, head_lo)?,
            None => Revision::INITIAL,
        };
        if actual_revision != expected_revision {
            return finish_with_confirmed_rollback(
                transaction,
                expected_revision,
                attempted,
                JournalError::StaleRevision {
                    expected: expected_revision,
                    actual: actual_revision,
                },
            );
        }

        if let Err(error) = write_batch(
            &transaction,
            session_id,
            head.is_some(),
            attempted,
            &encoded,
        ) {
            return Err(statement_failure(
                transaction,
                error,
                expected_revision,
                attempted,
            ));
        }

        match fault {
            Some(FaultPoint::BeforeCommit) => finish_with_confirmed_rollback(
                transaction,
                expected_revision,
                attempted,
                JournalError::AdapterFailure {
                    message: "injected failure before the commit point".to_owned(),
                },
            ),
            Some(FaultPoint::IndeterminateRolledBack) => finish_with_confirmed_rollback(
                transaction,
                expected_revision,
                attempted,
                JournalError::IndeterminateCommit {
                    expected: expected_revision,
                    attempted,
                    message: "injected unproven commit: the batch is absent".to_owned(),
                },
            ),
            Some(FaultPoint::IndeterminateCommitted) => {
                transaction
                    .commit()
                    .map_err(|error| JournalError::IndeterminateCommit {
                        expected: expected_revision,
                        attempted,
                        message: format!("commit outcome is not proven: {error}"),
                    })?;
                Err(JournalError::IndeterminateCommit {
                    expected: expected_revision,
                    attempted,
                    message: "injected unproven commit: the batch is committed".to_owned(),
                })
            }
            None => {
                transaction
                    .commit()
                    .map_err(|error| JournalError::IndeterminateCommit {
                        expected: expected_revision,
                        attempted,
                        message: format!("commit outcome is not proven: {error}"),
                    })?;
                Ok(attempted)
            }
        }
    }
}

/// Connection setup shared by every open: identity validation (before any
/// journal-mode change or DDL), initialization of a fresh database, structural
/// check, then the durability settings.
fn prepare(connection: &mut Connection) -> Result<(), JournalError> {
    // First contact with the file happens here: a non-database input fails
    // these reads with a not-a-database error, mapped to Corruption, before
    // anything is written.
    let application_id: i32 = connection
        .query_row("PRAGMA application_id", [], |row| row.get(0))
        .map_err(map_sqlite_error)?;
    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(map_sqlite_error)?;
    let user_objects: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite_error)?;

    if application_id == 0 && user_version == 0 && user_objects == 0 {
        initialize(connection)?;
    } else if application_id == APPLICATION_ID {
        if user_version == i64::from(SCHEMA_VERSION) {
            require_journal_tables(connection)?;
        } else if user_version == 0 {
            return Err(JournalError::Corruption {
                message: "journal application identifier is set but the schema version is \
                          missing"
                    .to_owned(),
            });
        } else {
            return Err(JournalError::UnsupportedFormat {
                version: u16::try_from(user_version).unwrap_or(u16::MAX),
            });
        }
    } else {
        return Err(JournalError::Corruption {
            message: format!(
                "database is not a journal of this application (application_id {application_id})"
            ),
        });
    }

    run_quick_check(connection)?;
    set_durability_pragmas(connection)
}

/// Initializes an empty database as schema version 1 in one transaction.
///
/// The `payload_version` column is deliberately not pinned by a CHECK
/// constraint: an unknown payload version must be readable so the read
/// protocol can report it as [`JournalError::UnsupportedFormat`]; pinning the
/// column would make that typed failure unreachable. Representation bounds
/// (nonnegative 32-bit halves, `NOT NULL`) stay enforced by the schema.
fn initialize(connection: &mut Connection) -> Result<(), JournalError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sqlite_error)?;
    transaction
        .execute_batch(
            "CREATE TABLE journal_streams (
                 session_id TEXT PRIMARY KEY,
                 head_hi    INTEGER NOT NULL CHECK (head_hi BETWEEN 0 AND 4294967295),
                 head_lo    INTEGER NOT NULL CHECK (head_lo BETWEEN 0 AND 4294967295)
             );
             CREATE TABLE journal_entries (
                 session_id      TEXT NOT NULL
                                 REFERENCES journal_streams (session_id),
                 revision_hi     INTEGER NOT NULL
                                 CHECK (revision_hi BETWEEN 0 AND 4294967295),
                 revision_lo     INTEGER NOT NULL
                                 CHECK (revision_lo BETWEEN 0 AND 4294967295),
                 payload_version INTEGER NOT NULL,
                 payload         BLOB NOT NULL,
                 checksum        INTEGER NOT NULL
                                 CHECK (checksum BETWEEN 0 AND 4294967295),
                 PRIMARY KEY (session_id, revision_hi, revision_lo)
             );",
        )
        .map_err(map_sqlite_error)?;
    transaction
        .pragma_update(None, "application_id", APPLICATION_ID)
        .map_err(map_sqlite_error)?;
    transaction
        .pragma_update(None, "user_version", i64::from(SCHEMA_VERSION))
        .map_err(map_sqlite_error)?;
    transaction
        .commit()
        .map_err(|error| JournalError::AdapterFailure {
            message: format!("journal initialization commit failed: {error}"),
        })
}

/// Verifies that both journal tables exist in a database claiming version 1.
fn require_journal_tables(connection: &Connection) -> Result<(), JournalError> {
    let found: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name IN ('journal_streams', 'journal_entries')",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite_error)?;
    if found == 2 {
        Ok(())
    } else {
        Err(JournalError::Corruption {
            message: "journal schema is incomplete: journal tables are missing".to_owned(),
        })
    }
}

fn run_quick_check(connection: &Connection) -> Result<(), JournalError> {
    let mut statement = connection
        .prepare("PRAGMA quick_check")
        .map_err(map_sqlite_error)?;
    let findings = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(map_sqlite_error)?;
    for finding in findings {
        let finding = finding.map_err(map_sqlite_error)?;
        if finding != "ok" {
            return Err(JournalError::Corruption {
                message: format!("quick_check failed: {finding}"),
            });
        }
    }
    Ok(())
}

fn set_durability_pragmas(connection: &mut Connection) -> Result<(), JournalError> {
    let mode: String = connection
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .map_err(sqlite_to_adapter)?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(JournalError::AdapterFailure {
            message: format!("journal mode WAL was refused ({mode})"),
        });
    }
    connection
        .execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .map_err(sqlite_to_adapter)
}

type HeadColumns = (i64, i64);

fn read_head(
    transaction: &Transaction<'_>,
    session_id: &SessionId,
) -> Result<Option<HeadColumns>, JournalError> {
    transaction
        .query_row(
            "SELECT head_hi, head_lo FROM journal_streams WHERE session_id = ?1",
            [session_id.as_str()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(map_sqlite_error)
}

struct RawEntry {
    revision_hi: i64,
    revision_lo: i64,
    payload_version: i64,
    payload: Vec<u8>,
    checksum: i64,
}

fn read_raw_entries(
    transaction: &Transaction<'_>,
    session_id: &SessionId,
) -> Result<Vec<RawEntry>, JournalError> {
    let mut statement = transaction
        .prepare(
            "SELECT revision_hi, revision_lo, payload_version, payload, checksum \
             FROM journal_entries WHERE session_id = ?1 \
             ORDER BY revision_hi, revision_lo",
        )
        .map_err(map_sqlite_error)?;
    let rows = statement
        .query_map([session_id.as_str()], |row| {
            Ok(RawEntry {
                revision_hi: row.get(0)?,
                revision_lo: row.get(1)?,
                payload_version: row.get(2)?,
                payload: row.get(3)?,
                checksum: row.get(4)?,
            })
        })
        .map_err(map_sqlite_error)?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(map_sqlite_error)?);
    }
    Ok(entries)
}

/// Validates one collected snapshot and rebuilds the history, following the
/// read protocol's fixed order: payload bounds, payload version, content
/// checksum, strict payload decoding, revision continuity, head agreement.
fn validate_entries(
    session_id: &SessionId,
    head: Option<HeadColumns>,
    raw_entries: Vec<RawEntry>,
) -> Result<Vec<JournalEntry>, JournalError> {
    let mut entries = Vec::with_capacity(raw_entries.len());
    let mut previous_revision = Revision::INITIAL;
    for raw in raw_entries {
        if raw.payload.len() > MAX_PAYLOAD_BYTES {
            return Err(JournalError::Corruption {
                message: format!(
                    "stored payload at revision {} exceeds the {MAX_PAYLOAD_BYTES}-byte limit",
                    preview_revision(raw.revision_hi, raw.revision_lo)
                ),
            });
        }
        if raw.payload_version != i64::from(PAYLOAD_VERSION) {
            return Err(JournalError::UnsupportedFormat {
                version: u16::try_from(raw.payload_version).unwrap_or(u16::MAX),
            });
        }
        let stored_checksum =
            u32::try_from(raw.checksum).map_err(|_| JournalError::Corruption {
                message: "stored checksum column is out of range".to_owned(),
            })?;
        let revision = compose_revision(raw.revision_hi, raw.revision_lo)?;
        let (revision_hi, revision_lo) = split_revision(revision.value());
        let computed = content_checksum(
            PAYLOAD_VERSION,
            revision_hi,
            revision_lo,
            session_id.as_str(),
            &raw.payload,
        );
        if computed != stored_checksum {
            return Err(JournalError::Corruption {
                message: format!("content checksum mismatch at revision {revision}"),
            });
        }
        let event =
            payload::decode_payload(&raw.payload).map_err(|reason| JournalError::Corruption {
                message: format!("at revision {revision}: {reason}"),
            })?;
        let expected_revision =
            previous_revision
                .checked_next()
                .ok_or(JournalError::Corruption {
                    message: format!(
                        "session history revision is not contiguous: expected above \
                 {previous_revision}, actual {revision}"
                    ),
                })?;
        if revision != expected_revision {
            return Err(JournalError::Corruption {
                message: format!(
                    "session history revision is not contiguous: expected \
                     {expected_revision}, actual {revision}"
                ),
            });
        }
        entries.push(JournalEntry::new(revision, event));
        previous_revision = revision;
    }

    let computed_head = entries
        .last()
        .map_or(Revision::INITIAL, JournalEntry::revision);
    let stored_head = match head {
        Some((head_hi, head_lo)) => compose_revision(head_hi, head_lo)?,
        None => Revision::INITIAL,
    };
    if stored_head != computed_head {
        return Err(JournalError::Corruption {
            message: format!(
                "stream head {stored_head} disagrees with the last entry {computed_head}"
            ),
        });
    }
    Ok(entries)
}

fn write_batch(
    transaction: &Transaction<'_>,
    session_id: &SessionId,
    stream_exists: bool,
    attempted: Revision,
    encoded: &[(Revision, Vec<u8>)],
) -> Result<(), rusqlite::Error> {
    let (head_hi, head_lo) = split_revision(attempted.value());
    if stream_exists {
        transaction.execute(
            "UPDATE journal_streams SET head_hi = ?1, head_lo = ?2 WHERE session_id = ?3",
            rusqlite::params![i64::from(head_hi), i64::from(head_lo), session_id.as_str()],
        )?;
    } else {
        transaction.execute(
            "INSERT INTO journal_streams (session_id, head_hi, head_lo) VALUES (?1, ?2, ?3)",
            rusqlite::params![session_id.as_str(), i64::from(head_hi), i64::from(head_lo)],
        )?;
    }
    for (revision, payload_bytes) in encoded {
        let (revision_hi, revision_lo) = split_revision(revision.value());
        let checksum = content_checksum(
            PAYLOAD_VERSION,
            revision_hi,
            revision_lo,
            session_id.as_str(),
            payload_bytes,
        );
        transaction.execute(
            "INSERT INTO journal_entries \
             (session_id, revision_hi, revision_lo, payload_version, payload, checksum) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                session_id.as_str(),
                i64::from(revision_hi),
                i64::from(revision_lo),
                i64::from(PAYLOAD_VERSION),
                payload_bytes,
                i64::from(checksum)
            ],
        )?;
    }
    Ok(())
}

/// Commits nothing: rolls the transaction back, confirming the rollback. A
/// rollback that cannot be confirmed leaves the outcome unproven and is
/// reported as an indeterminate commit instead of the intended failure.
fn finish_with_confirmed_rollback(
    transaction: Transaction<'_>,
    expected: Revision,
    attempted: Revision,
    intended: JournalError,
) -> Result<Revision, JournalError> {
    match transaction.rollback() {
        Ok(()) => Err(intended),
        Err(rollback_error) => Err(JournalError::IndeterminateCommit {
            expected,
            attempted,
            message: format!("rollback could not be confirmed: {rollback_error}"),
        }),
    }
}

/// Maps a failed statement inside an append: the rollback is attempted first;
/// a confirmed rollback returns the mapped statement failure, an
/// unconfirmable rollback is an indeterminate commit.
fn statement_failure(
    transaction: Transaction<'_>,
    error: rusqlite::Error,
    expected: Revision,
    attempted: Revision,
) -> JournalError {
    let mapped = map_sqlite_error(error);
    match transaction.rollback() {
        Ok(()) => mapped,
        Err(rollback_error) => JournalError::IndeterminateCommit {
            expected,
            attempted,
            message: format!("rollback could not be confirmed: {rollback_error}"),
        },
    }
}

fn sqlite_to_adapter(error: rusqlite::Error) -> JournalError {
    JournalError::AdapterFailure {
        message: format!("sqlite operation failed: {error}"),
    }
}

/// Maps a SQLite error to the contract's typed failures: corruption-class
/// result codes and stored column-type mismatches are `Corruption`; anything
/// else (busy, ordinary I/O, lock) is `AdapterFailure`.
fn map_sqlite_error(error: rusqlite::Error) -> JournalError {
    if let rusqlite::Error::SqliteFailure(code, _) = &error
        && matches!(
            code.code,
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
        )
    {
        return JournalError::Corruption {
            message: format!("database operation failed: {error}"),
        };
    }
    // A stored value whose storage class does not match the journal schema
    // (for example a payload stored as TEXT instead of a BLOB) is altered
    // storage, not an adapter malfunction.
    if matches!(error, rusqlite::Error::InvalidColumnType { .. }) {
        return JournalError::Corruption {
            message: format!("stored column type does not match the journal schema: {error}"),
        };
    }
    sqlite_to_adapter(error)
}

fn preview_revision(hi: i64, lo: i64) -> String {
    match (u32::try_from(hi), u32::try_from(lo)) {
        (Ok(hi), Ok(lo)) => Revision::new((u64::from(hi) << 32) | u64::from(lo)).to_string(),
        _ => format!("{hi}:{lo}"),
    }
}

/// Splits a `u64` revision into two 32-bit columns. Both halves always fit
/// the nonnegative 32-bit SQL columns; the full value is never narrowed.
fn split_revision(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, (value & 0xFFFF_FFFF) as u32)
}

/// Composes two nonnegative 32-bit columns back into the full `u64` revision.
/// Out-of-range columns are corruption.
fn compose_revision(hi: i64, lo: i64) -> Result<Revision, JournalError> {
    if !(0..=U32_MAX_I64).contains(&hi) || !(0..=U32_MAX_I64).contains(&lo) {
        return Err(JournalError::Corruption {
            message: format!("revision columns are out of range: hi {hi}, lo {lo}"),
        });
    }
    let hi = u32::try_from(hi).expect("bounded above");
    let lo = u32::try_from(lo).expect("bounded above");
    Ok(Revision::new((u64::from(hi) << 32) | u64::from(lo)))
}

/// The per-row content checksum: CRC-32/ISO-HDLC over the payload version,
/// the full revision, the session identity and the exact serialized payload
/// bytes, each 32-bit part as a little-endian word and the session identity
/// length-prefixed.
fn content_checksum(
    payload_version: u32,
    revision_hi: u32,
    revision_lo: u32,
    session_id: &str,
    payload_bytes: &[u8],
) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&payload_version.to_le_bytes());
    hasher.update(&revision_hi.to_le_bytes());
    hasher.update(&revision_lo.to_le_bytes());
    hasher.update(
        &u32::try_from(session_id.len())
            .expect("session ID length fits u32")
            .to_le_bytes(),
    );
    hasher.update(session_id.as_bytes());
    hasher.update(payload_bytes);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_compose_round_trips_the_full_u64_range() {
        for value in [
            0u64,
            1,
            2,
            u64::from(u32::MAX),
            u64::from(u32::MAX) + 1,
            (1u64 << 32) + 123,
            u64::MAX - 1,
            u64::MAX,
        ] {
            let (hi, lo) = split_revision(value);
            assert_eq!(
                compose_revision(i64::from(hi), i64::from(lo))
                    .unwrap()
                    .value(),
                value
            );
        }
    }

    #[test]
    fn compose_rejects_out_of_range_columns() {
        assert!(compose_revision(-1, 0).is_err());
        assert!(compose_revision(0, -1).is_err());
        assert!(compose_revision(U32_MAX_I64 + 1, 0).is_err());
        assert!(compose_revision(0, U32_MAX_I64 + 1).is_err());
    }

    #[test]
    fn checksum_covers_every_protected_value() {
        let session = SessionId::new("s1").expect("valid session ID");
        let base = content_checksum(1, 0, 1, session.as_str(), b"{\"x\":1}");
        assert_ne!(
            base,
            content_checksum(2, 0, 1, session.as_str(), b"{\"x\":1}")
        );
        assert_ne!(
            base,
            content_checksum(1, 1, 1, session.as_str(), b"{\"x\":1}")
        );
        assert_ne!(
            base,
            content_checksum(1, 0, 2, session.as_str(), b"{\"x\":1}")
        );
        let other = SessionId::new("s2").expect("valid session ID");
        assert_ne!(
            base,
            content_checksum(1, 0, 1, other.as_str(), b"{\"x\":1}")
        );
        assert_ne!(
            base,
            content_checksum(1, 0, 1, session.as_str(), b"{\"x\":2}")
        );
        // The session length prefix keeps the input unambiguous.
        assert_ne!(
            content_checksum(1, 0, 1, "ab", b"c"),
            content_checksum(1, 0, 1, "a", b"bc")
        );
    }

    #[test]
    fn crc32fast_is_iso_hdlc() {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(b"123456789");
        assert_eq!(hasher.finalize(), 0xCBF4_3926);
    }
}
