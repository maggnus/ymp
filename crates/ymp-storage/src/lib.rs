//! Embedded SQLite persistence for the current Journal and immutable content.

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use ymp_domain::{Denial, Digest, Result};

pub mod content;
pub mod journal;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;

/// Allocation limits are checked before loading stored values; oversize is a refusal.
pub const MAX_CONTENT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_JOURNAL_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_JOURNAL_EVENTS: usize = 32_768;
const APPLICATION_ID: i64 = 0x594d_504e;
const FORMAT_VERSION: i64 = 1;

const SCHEMA: &str = "CREATE TABLE content_values (
                digest TEXT PRIMARY KEY NOT NULL CHECK(length(digest)=64), bytes BLOB NOT NULL
            ) STRICT;
            CREATE TABLE journal_heads (
                session TEXT PRIMARY KEY NOT NULL, last_seq BLOB NOT NULL CHECK(length(last_seq)=8 AND last_seq!=zeroblob(8)), chain TEXT NOT NULL CHECK(length(chain)=64)
            ) STRICT;
            CREATE TABLE journal_events (
                session TEXT NOT NULL, seq BLOB NOT NULL CHECK(length(seq)=8),
                payload TEXT NOT NULL CHECK(length(payload)=64), prior TEXT NOT NULL CHECK(length(prior)=64), chain TEXT NOT NULL CHECK(length(chain)=64),
                PRIMARY KEY(session,seq),
                FOREIGN KEY(session) REFERENCES journal_heads(session) DEFERRABLE INITIALLY DEFERRED,
                FOREIGN KEY(payload) REFERENCES content_values(digest)
            ) STRICT;
            CREATE TABLE event_content (
                session TEXT NOT NULL, seq BLOB NOT NULL CHECK(length(seq)=8), digest TEXT NOT NULL CHECK(length(digest)=64),
                PRIMARY KEY(session,seq,digest),
                FOREIGN KEY(session,seq) REFERENCES journal_events(session,seq),
                FOREIGN KEY(digest) REFERENCES content_values(digest)
            ) STRICT;";

#[derive(Clone)]
pub(crate) struct Database {
    path: PathBuf,
}

pub(crate) fn sql_error(error: rusqlite::Error) -> Denial {
    use rusqlite::ErrorCode;
    let code = match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => "storage_busy",
        Some(ErrorCode::SystemIoFailure | ErrorCode::CannotOpen | ErrorCode::DiskFull) => {
            "storage_io"
        }
        _ => "storage_corrupt",
    };
    Denial::new(
        code,
        format!("SQLite operation failed ({:?})", error.sqlite_error_code()),
    )
}
pub(crate) fn io_error(_: std::io::Error) -> Denial {
    Denial::new(
        "storage_io",
        "Cannot access or synchronize the storage path",
    )
}

fn header(connection: &Connection) -> Result<(i64, i64, i64)> {
    let app = connection
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(sql_error)?;
    let version = connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(sql_error)?;
    let objects = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    Ok((app, version, objects))
}
fn validate_format(connection: &Connection) -> Result<()> {
    let (app, version, objects) = header(connection)?;
    if app != APPLICATION_ID {
        return Err(Denial::new(
            "storage_format",
            "This database is not a current ymp journal",
        ));
    }
    if version != FORMAT_VERSION {
        return Err(Denial::new(
            "storage_version",
            "Unsupported ymp storage format version",
        ));
    }
    if objects != 4 {
        return Err(Denial::new(
            "storage_format",
            "Unexpected storage schema objects",
        ));
    }
    for declaration in SCHEMA
        .split(';')
        .map(str::trim)
        .filter(|sql| !sql.is_empty())
    {
        let name = declaration
            .split_whitespace()
            .nth(2)
            .ok_or_else(|| Denial::new("storage_format", "Invalid built-in schema"))?;
        let actual: Option<String> = connection
            .query_row(
                "SELECT length(sql),sql FROM sqlite_schema WHERE type='table' AND name=?1",
                [name],
                |r| {
                    if r.get::<_, i64>(0)? > 16_384 {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    r.get(1)
                },
            )
            .optional()
            .map_err(sql_error)?;
        let normalized = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        if actual.as_deref().map(normalized) != Some(normalized(declaration)) {
            return Err(Denial::new(
                "storage_format",
                "Storage table definitions do not match format version 1",
            ));
        }
    }
    let mut statement=connection.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").map_err(sql_error)?;
    let tables = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(sql_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    if tables
        != [
            "content_values",
            "event_content",
            "journal_events",
            "journal_heads",
        ]
    {
        return Err(Denial::new(
            "storage_format",
            "Storage tables do not match format version 1",
        ));
    }
    let extra: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type IN ('trigger','view')",
            [],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if extra != 0 {
        return Err(Denial::new(
            "storage_format",
            "Unexpected storage triggers or views",
        ));
    }
    Ok(())
}
fn quick_check(connection: &Connection) -> Result<()> {
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(sql_error)?;
    if result != "ok" {
        return Err(Denial::new(
            "storage_corrupt",
            "SQLite integrity check failed",
        ));
    }
    Ok(())
}
fn local_settings(connection: &Connection, write: bool) -> Result<()> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(sql_error)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(sql_error)?;
    if write {
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(sql_error)?;
        connection
            .pragma_update(None, "fullfsync", "ON")
            .map_err(sql_error)?;
    }
    Ok(())
}
fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            Denial::new("storage_path", "The database path must be a regular file"),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}
impl Database {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let absolute = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir().map_err(io_error)?.join(path)
        };
        let parent = absolute
            .parent()
            .ok_or_else(|| Denial::new("storage_path", "A database file needs a parent directory"))?
            .canonicalize()
            .map_err(io_error)?;
        let name = absolute
            .file_name()
            .ok_or_else(|| Denial::new("storage_path", "A database file name is required"))?;
        let path = parent.join(name);
        reject_symlink(&path)?;
        // Inspect existing data read-only before any persistent PRAGMA or DDL.
        if fs::metadata(&path).is_ok_and(|m| m.len() > 0) {
            let mut read = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(sql_error)?;
            local_settings(&read, false)?;
            let tx = read.transaction().map_err(sql_error)?;
            let identity = header(&tx)?;
            if identity != (0, 0, 0) {
                validate_format(&tx)?;
                quick_check(&tx)?;
            }
        }
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(sql_error)?;
        local_settings(&connection, true)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        // Recheck under the write transaction: another opener may have initialized it.
        if header(&tx)? == (0, 0, 0) {
            tx.execute_batch(SCHEMA).map_err(sql_error)?;
            tx.pragma_update(None, "application_id", APPLICATION_ID)
                .map_err(sql_error)?;
            tx.pragma_update(None, "user_version", FORMAT_VERSION)
                .map_err(sql_error)?;
        } else {
            validate_format(&tx)?;
        }
        tx.commit().map_err(sql_error)?;
        let mode: String = connection
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .map_err(sql_error)?;
        if mode != "wal" {
            let mode: String = connection
                .query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))
                .map_err(sql_error)?;
            if mode != "wal" {
                return Err(Denial::new(
                    "storage_mode",
                    "SQLite could not enable WAL journaling",
                ));
            }
        }
        quick_check(&connection)?;
        fs::File::open(&parent)
            .and_then(|f| f.sync_all())
            .map_err(io_error)?;
        Ok(Self { path })
    }
    pub(crate) fn connect(&self, write: bool) -> Result<Connection> {
        reject_symlink(&self.path)?;
        let flags = if write {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        };
        let connection =
            Connection::open_with_flags(&self.path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)
                .map_err(sql_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(sql_error)?;
        validate_format(&connection)?;
        let mode: String = connection
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .map_err(sql_error)?;
        if mode != "wal" {
            return Err(Denial::new(
                "storage_mode",
                "Storage journaling mode changed; reopen the database explicitly",
            ));
        }
        local_settings(&connection, write)?;
        Ok(connection)
    }
}

pub(crate) fn read_content(
    connection: &Connection,
    digest: &Digest,
    limit: usize,
) -> Result<Vec<u8>> {
    let length: Option<i64> = connection
        .query_row(
            "SELECT length(bytes) FROM content_values WHERE digest=?1",
            [digest.as_str()],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    let length = length.ok_or_else(|| {
        Denial::new(
            "content_missing",
            format!("Missing immutable content {digest}"),
        )
    })?;
    let size = usize::try_from(length)
        .map_err(|_| Denial::new("storage_corrupt", "Invalid content length"))?;
    if size > limit.min(MAX_CONTENT_BYTES) {
        return Err(Denial::new(
            "content_limit",
            "Content exceeds the reader's allocation limit",
        ));
    }
    let bytes: Vec<u8> = connection
        .query_row(
            "SELECT bytes FROM content_values WHERE digest=?1",
            [digest.as_str()],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if bytes.len() != size || Digest::of(&bytes) != *digest {
        return Err(Denial::new(
            "content_digest",
            format!("Immutable content failed its digest check: {digest}"),
        ));
    }
    Ok(bytes)
}
pub(crate) fn put_content(connection: &Connection, digest: &Digest, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_CONTENT_BYTES {
        return Err(Denial::new(
            "content_limit",
            "Content exceeds the storage limit",
        ));
    }
    if Digest::of(bytes) != *digest {
        return Err(Denial::new(
            "content_digest",
            "Attached bytes do not match their declared digest",
        ));
    }
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM content_values WHERE digest=?1)",
            [digest.as_str()],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if exists {
        if read_content(connection, digest, MAX_CONTENT_BYTES)? != bytes {
            return Err(Denial::new(
                "content_conflict",
                "An immutable digest already identifies different bytes",
            ));
        }
    } else {
        connection
            .execute(
                "INSERT INTO content_values(digest,bytes) VALUES(?1,?2)",
                rusqlite::params![digest.as_str(), bytes],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}
