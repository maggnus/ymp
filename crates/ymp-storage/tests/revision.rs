//! Full `u64` revision representation (DEV-0005 acceptance criterion 9): two
//! nonnegative 32-bit columns, checked high/low arithmetic with a carry
//! across the 2^32 boundary, the full range without `i64` narrowing, and
//! `RevisionOverflow` at the top — all via direct row manipulation.

#![forbid(unsafe_code)]

mod support;

use support::{TempRoot, open_journal, session_id};
use ymp_kernel::{Journal, JournalError, Revision};

/// Creates the journal schema in `root` by opening and closing the adapter
/// once, so the direct row manipulation below targets an initialized database.
fn initialize_schema(root: &std::path::Path) {
    drop(open_journal(root));
}

/// Inserts a stream head row directly (no entries).
fn set_head(root: &std::path::Path, sid: &str, hi: u32, lo: u32) {
    let connection = support::direct_connection(root);
    connection
        .execute(
            "INSERT OR REPLACE INTO journal_streams (session_id, head_hi, head_lo) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![sid, i64::from(hi), i64::from(lo)],
        )
        .expect("head row inserts");
}

/// Inserts one entry row directly with a payload and checksum that satisfy
/// every read validation before revision continuity.
fn insert_cancelled_entry(root: &std::path::Path, sid: &str, hi: u32, lo: u32) {
    let payload = format!("{{\"type\":\"session_cancelled\",\"session_id\":\"{sid}\"}}");
    let payload = payload.as_bytes();
    let checksum = support::row_checksum(1, hi, lo, sid, payload);
    let connection = support::direct_connection(root);
    connection
        .execute(
            "INSERT INTO journal_entries \
             (session_id, revision_hi, revision_lo, payload_version, payload, checksum) \
             VALUES (?1, ?2, ?3, 1, ?4, ?5)",
            rusqlite::params![
                sid,
                i64::from(hi),
                i64::from(lo),
                payload,
                i64::from(checksum)
            ],
        )
        .expect("entry row inserts");
}

#[test]
fn append_carries_across_the_2p32_boundary_into_the_high_column() {
    let root = TempRoot::new("carry");
    let sid = session_id("carry");
    initialize_schema(root.path());
    // The stream rests at revision 2^32 - 1: low column saturated.
    set_head(root.path(), sid.as_str(), 0, u32::MAX);
    let journal = open_journal(root.path());

    let boundary = u64::from(u32::MAX);
    let committed = journal
        .append(
            &sid,
            Revision::new(boundary),
            vec![support::cancelled_event(&sid)],
        )
        .expect("append carries");
    assert_eq!(committed, Revision::new(boundary + 1));
    assert_eq!(committed.value(), 1 << 32, "the checked sum crosses 2^32");

    // The stored row carries into the high column: (hi, lo) = (1, 0).
    let connection = support::direct_connection(root.path());
    let (hi, lo): (i64, i64) = connection
        .query_row(
            "SELECT revision_hi, revision_lo FROM journal_entries WHERE session_id = ?1",
            [sid.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("entry reads");
    assert_eq!((hi, lo), (1, 0), "the carry reaches revision_hi");
    let (head_hi, head_lo): (i64, i64) = connection
        .query_row(
            "SELECT head_hi, head_lo FROM journal_streams WHERE session_id = ?1",
            [sid.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("head reads");
    assert_eq!((head_hi, head_lo), (1, 0));
}

#[test]
fn revisions_above_i64_max_round_trip_without_narrowing() {
    let root = TempRoot::new("above-i64");
    let sid = session_id("above-i64");
    initialize_schema(root.path());
    // 2^63 + 1: impossible to represent as a nonnegative i64.
    let above = (1u64 << 63) + 1;
    set_head(
        root.path(),
        sid.as_str(),
        (above >> 32) as u32,
        above as u32,
    );
    insert_cancelled_entry(
        root.path(),
        sid.as_str(),
        (above >> 32) as u32,
        above as u32,
    );

    // The read reaches revision continuity with the full value: the typed
    // failure names exactly 9223372036854775809, not a narrowed or negative
    // number.
    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(
                message.contains("9223372036854775809"),
                "the full u64 appears: {message}"
            );
            assert!(
                !message.contains('-'),
                "no narrowed/negative value: {message}"
            );
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn entries_are_ordered_by_the_column_pair_not_insertion_order() {
    let root = TempRoot::new("ordering");
    let sid = session_id("ordering");
    initialize_schema(root.path());
    // The stream row must exist before the entries (foreign keys are enforced
    // even on direct connections). The higher (hi, lo) entry row is inserted
    // first: the read must still report the lower pair as the first entry.
    set_head(root.path(), sid.as_str(), 1, 0);
    insert_cancelled_entry(root.path(), sid.as_str(), 1, 0);
    insert_cancelled_entry(root.path(), sid.as_str(), 0, u32::MAX);

    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(
                message.contains("expected 1, actual 4294967295"),
                "ordering follows (revision_hi, revision_lo): {message}"
            );
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn head_at_u64_max_reads_as_the_exact_value() {
    let root = TempRoot::new("head-max");
    let sid = session_id("head-max");
    initialize_schema(root.path());
    set_head(root.path(), sid.as_str(), u32::MAX, u32::MAX);
    // No entries: the head disagreement must name u64::MAX exactly.
    match open_journal(root.path()).read(&sid).unwrap_err() {
        JournalError::Corruption { message } => {
            assert!(
                message.contains("18446744073709551615"),
                "the full u64::MAX appears: {message}"
            );
        }
        other => panic!("expected Corruption, got {other:?}"),
    }
}

#[test]
fn append_at_u64_max_fails_with_revision_overflow_before_any_write() {
    let root = TempRoot::new("overflow");
    let sid = session_id("overflow");
    initialize_schema(root.path());
    set_head(root.path(), sid.as_str(), u32::MAX, u32::MAX);
    let journal = open_journal(root.path());
    std::thread::sleep(std::time::Duration::from_millis(20));
    let before = support::snapshot_tree(root.path());
    assert_eq!(
        journal
            .append(
                &sid,
                Revision::new(u64::MAX),
                vec![support::cancelled_event(&sid)]
            )
            .unwrap_err(),
        JournalError::RevisionOverflow
    );
    assert_eq!(
        support::snapshot_tree(root.path()),
        before,
        "the overflow is detected before any write"
    );
    let connection = support::direct_connection(root.path());
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM journal_entries", [], |row| row.get(0))
        .expect("rows count");
    assert_eq!(rows, 0);
}
