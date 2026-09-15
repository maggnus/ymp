//! Honest typed failures without mutation (DEV-0005 acceptance criteria 6, 7
//! and 8): unknown schema/payload versions, malformed JSON, checksum
//! mismatches — including altered-but-valid JSON — revision gaps, head
//! disagreement and a non-database input file all fail with typed errors and
//! are never served as empty, shorter or valid histories, and other sessions
//! stay readable.

#![forbid(unsafe_code)]

mod support;

use std::fs;

use support::{TempRoot, open_dispatcher, open_journal, sample_task, session_id};
use ymp_kernel::{Journal, JournalError, Revision};
use ymp_storage::SqliteJournal;

fn opened_root(tag: &str) -> (TempRoot, ymp_domain::SessionId) {
    let root = TempRoot::new(tag);
    let sid = session_id(tag);
    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid.clone(), sample_task(tag))
        .expect("session opens");
    (root, sid)
}

#[test]
fn unknown_schema_version_fails_without_mutation() {
    let (root, sid) = opened_root("schema-version");

    // Take the database out of WAL and to an unknown schema version; all
    // adapter handles are dropped first.
    drop(open_journal(root.path()));
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute_batch("PRAGMA journal_mode=DELETE;")
            .expect("journal mode switches");
        connection
            .pragma_update(None, "user_version", 99_i64)
            .expect("version is set");
    }

    // Opening fails with the typed variant naming the version, before any
    // journal-mode change or DDL.
    assert_eq!(
        SqliteJournal::open(root.path()).unwrap_err(),
        JournalError::UnsupportedFormat { version: 99 }
    );

    // The database is unchanged: same version, same mode, same rows.
    let connection = support::direct_connection(root.path());
    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version reads");
    assert_eq!(user_version, 99);
    let mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("mode reads");
    assert_eq!(mode, "delete", "the failed open changed no journal mode");
    let rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM journal_entries WHERE session_id = ?1",
            [sid.as_str()],
            |row| row.get(0),
        )
        .expect("rows count");
    assert_eq!(rows, 1, "the failed open rewrote nothing");
}

#[test]
fn foreign_application_identifier_is_corruption() {
    let (root, _sid) = opened_root("foreign-app");
    drop(open_journal(root.path()));
    {
        let connection = support::direct_connection(root.path());
        connection
            .pragma_update(None, "application_id", 0x1234_5678_i64)
            .expect("identifier is set");
    }
    match SqliteJournal::open(root.path()).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(message.contains("application_id"), "message: {message}")
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn unknown_payload_version_fails_as_unsupported_format() {
    let (root, sid) = opened_root("payload-version");
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute(
                "UPDATE journal_entries SET payload_version = 2 WHERE session_id = ?1",
                [sid.as_str()],
            )
            .expect("payload version is altered");
    }
    assert_eq!(
        open_journal(root.path()).read(&sid).unwrap_err(),
        JournalError::UnsupportedFormat { version: 2 }
    );
}

#[test]
fn malformed_json_with_a_valid_checksum_is_corruption() {
    let (root, sid) = opened_root("malformed-json");
    // Replace the payload with bytes that carry a correct checksum for the
    // new content, so the failure is the JSON itself, not the checksum.
    let broken = b"{not json";
    {
        let connection = support::direct_connection(root.path());
        let checksum = support::row_checksum(1, 0, 1, sid.as_str(), broken) as i64;
        connection
            .execute(
                "UPDATE journal_entries SET payload = ?1, checksum = ?2 WHERE session_id = ?3",
                rusqlite::params![broken, checksum, sid.as_str()],
            )
            .expect("payload is altered");
    }
    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(message.contains("payload"), "message: {message}")
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn altered_but_valid_json_fails_the_content_checksum() {
    let (root, sid) = opened_root("altered-json");
    // A still-valid cancelled payload for a DIFFERENT session id, written
    // without updating the checksum: structural checks alone would not catch
    // it, the application-level content checksum must.
    let altered = br#"{"type":"session_cancelled","session_id":"someone-else"}"#;
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute(
                "UPDATE journal_entries SET payload = ?1 WHERE session_id = ?2",
                rusqlite::params![altered, sid.as_str()],
            )
            .expect("payload is altered");
    }
    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(message.contains("checksum"), "message: {message}")
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn flipped_payload_byte_fails_the_checksum() {
    let (root, sid) = opened_root("flipped");
    {
        let connection = support::direct_connection(root.path());
        // CAST keeps the value a BLOB: SQLite's `||` alone would store TEXT,
        // which exercises the storage-class check instead of the checksum.
        connection
            .execute(
                "UPDATE journal_entries SET payload = CAST(payload || X'00' AS BLOB) \
                 WHERE session_id = ?1",
                [sid.as_str()],
            )
            .expect("payload byte is appended");
    }
    assert!(matches!(
        open_journal(root.path()).read(&sid).unwrap_err(),
        JournalError::Corruption { .. }
    ));
}

#[test]
fn payload_column_of_the_wrong_storage_class_is_corruption() {
    let (root, sid) = opened_root("storage-class");
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute(
                "UPDATE journal_entries SET payload = CAST(payload AS TEXT) WHERE session_id = ?1",
                [sid.as_str()],
            )
            .expect("payload is stored as TEXT");
    }
    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(message.contains("column type"), "message: {message}")
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn revision_gap_is_corruption_with_the_exact_revisions() {
    let root = TempRoot::new("gap");
    let sid = session_id("gap");
    let journal = open_journal(root.path());
    journal
        .append(
            &sid,
            Revision::INITIAL,
            vec![support::opened_event(&sid, &sample_task("gap"))],
        )
        .expect("first batch commits");
    journal
        .append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)])
        .expect("second batch commits");
    journal
        .append(&sid, Revision::new(2), vec![support::cancelled_event(&sid)])
        .expect("third batch commits");
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute(
                "DELETE FROM journal_entries WHERE session_id = ?1 AND revision_lo = 2",
                [sid.as_str()],
            )
            .expect("middle row is removed");
    }
    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(
                message.contains("expected 2, actual 3"),
                "message names the exact gap: {message}"
            );
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn head_disagreement_is_corruption() {
    let (root, sid) = opened_root("head");
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute(
                "UPDATE journal_streams SET head_lo = head_lo + 1 WHERE session_id = ?1",
                [sid.as_str()],
            )
            .expect("head is altered");
    }
    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(message.contains("disagrees"), "message: {message}")
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn a_non_database_file_fails_on_open_without_mutation() {
    let root = TempRoot::new("not-a-db");
    let content =
        b"this is definitely not an SQLite database, padded past the header size\n".repeat(4);
    fs::write(root.path().join("journal.db"), &content).expect("file writes");

    match SqliteJournal::open(root.path()).unwrap_err() {
        JournalError::Corruption { .. } => {}
        other => panic!("expected Corruption, got {other:?}"),
    }
    assert_eq!(
        fs::read(root.path().join("journal.db")).expect("file reads"),
        content,
        "the failed open modified nothing"
    );
    assert_eq!(
        fs::read_dir(root.path()).unwrap().count(),
        1,
        "the failed open created no side files"
    );
}

#[test]
fn other_sessions_stay_readable_around_a_corrupted_stream() {
    let root = TempRoot::new("neighbors");
    let healthy = session_id("healthy");
    let damaged = session_id("damaged");
    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(healthy.clone(), sample_task("healthy"))
        .expect("healthy session opens");
    dispatcher
        .open(damaged.clone(), sample_task("damaged"))
        .expect("damaged session opens");
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute(
                "UPDATE journal_entries SET checksum = 0 WHERE session_id = ?1",
                [damaged.as_str()],
            )
            .expect("checksum is broken");
    }
    let journal = open_journal(root.path());
    assert!(matches!(
        journal.read(&damaged).unwrap_err(),
        JournalError::Corruption { .. }
    ));
    assert_eq!(
        journal.read(&healthy).unwrap().len(),
        1,
        "the healthy stream still reads"
    );
}

#[test]
fn orphan_entries_without_a_stream_row_are_not_an_empty_history() {
    let (root, sid) = opened_root("orphan");
    {
        let connection = support::direct_connection(root.path());
        connection
            .execute_batch(&format!(
                "PRAGMA foreign_keys=OFF; DELETE FROM journal_streams WHERE session_id = '{}';",
                sid.as_str().replace('\'', "''")
            ))
            .expect("stream row is removed");
    }
    assert!(matches!(
        open_journal(root.path()).read(&sid).unwrap_err(),
        JournalError::Corruption { .. }
    ));
}
