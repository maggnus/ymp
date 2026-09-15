mod support;
use rusqlite::params;
use ymp_domain::{Digest, Id};
use ymp_kernel::journal::{ContentStore, Journal, ParameterSchemas};
use ymp_storage::{MAX_CONTENT_BYTES, journal::SqliteJournal};

#[test]
fn corrupt_or_missing_content_and_links_are_refused_without_repair() {
    for case in 0..7 {
        let directory = support::Directory::new();
        let journal =
            SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
        let bad = Id::new("bad").unwrap();
        let healthy = Id::new("healthy").unwrap();
        journal.append(&bad, 0, &[support::opening(&bad)]).unwrap();
        journal
            .append(&healthy, 0, &[support::opening(&healthy)])
            .unwrap();
        let connection = rusqlite::Connection::open(directory.database()).unwrap();
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .unwrap();
        let payload: String = connection
            .query_row(
                "SELECT payload FROM journal_events WHERE session='bad'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        match case {
            0 => {
                connection
                    .execute(
                        "UPDATE content_values SET bytes=?1 WHERE digest=?2",
                        params![b"altered valid bytes".as_slice(), payload],
                    )
                    .unwrap();
            }
            1 => {
                connection
                    .execute("DELETE FROM content_values WHERE digest=?1", [payload])
                    .unwrap();
            }
            2 => {
                connection
                    .execute("DELETE FROM event_content WHERE session='bad'", [])
                    .unwrap();
            }
            3 => {
                connection
                    .execute(
                        "INSERT INTO event_content(session,seq,digest) VALUES('bad',?1,?2)",
                        params![
                            99_u64.to_be_bytes().as_slice(),
                            support::policy("SoloWithVerifier").policy.params.as_str()
                        ],
                    )
                    .unwrap();
            }
            4 => {
                connection
                    .execute(
                        "UPDATE journal_heads SET chain=?1 WHERE session='bad'",
                        ["0".repeat(64)],
                    )
                    .unwrap();
            }
            5 => {
                connection
                    .execute("DELETE FROM journal_heads WHERE session='bad'", [])
                    .unwrap();
            }
            6 => {
                connection
                    .execute(
                        "UPDATE journal_events SET seq=?1 WHERE session='bad'",
                        [2_u64.to_be_bytes().as_slice()],
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(journal.read(&bad).is_err(), "case {case}");
        assert!(journal.append(&bad, 1, &[support::opening(&bad)]).is_err());
        assert_eq!(journal.read(&healthy).unwrap().revision, 1, "case {case}");
        drop(connection);
        let reopened =
            SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
        assert!(reopened.read(&bad).is_err());
        assert_eq!(reopened.read(&healthy).unwrap().revision, 1);
    }
}

#[test]
fn immutable_put_rechecks_existing_bytes_and_bounded_get_checks_size_first() {
    let directory = support::Directory::new();
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    let content = journal.content_store();
    let expected = b"original immutable content";
    let digest = content.put(expected).unwrap();
    assert_eq!(content.put(expected).unwrap(), digest);
    let connection = rusqlite::Connection::open(directory.database()).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .unwrap();
    connection
        .execute(
            "UPDATE content_values SET bytes=?1 WHERE digest=?2",
            params![b"corrupted".as_slice(), digest.as_str()],
        )
        .unwrap();
    assert_eq!(content.put(expected).unwrap_err().code, "content_digest");
    assert_eq!(
        content.get(&digest, 100).unwrap_err().code,
        "content_digest"
    );
    let large = Digest::of(b"large object address");
    connection
        .execute(
            "INSERT INTO content_values(digest,bytes) VALUES(?1,zeroblob(?2))",
            params![large.as_str(), (MAX_CONTENT_BYTES + 1) as i64],
        )
        .unwrap();
    assert_eq!(
        content.get(&large, usize::MAX).unwrap_err().code,
        "content_limit"
    );
}

#[test]
fn foreign_unknown_and_malformed_formats_are_not_mutated_on_open() {
    for case in 0..4 {
        let directory = support::Directory::new();
        let path = directory.database();
        if case == 0 {
            std::fs::write(&path, b"not a SQLite database").unwrap();
        } else {
            if case == 2 {
                SqliteJournal::open(&path, ParameterSchemas::default()).unwrap();
            }
            let connection = rusqlite::Connection::open(&path).unwrap();
            match case {
                1=>connection.execute_batch("CREATE TABLE foreign_data(value TEXT);").unwrap(),
                2=>connection.pragma_update(None,"user_version",999).unwrap(),
                3=>connection.execute_batch("PRAGMA application_id=1498239054; PRAGMA user_version=1; CREATE TABLE content_values(x); CREATE TABLE event_content(x); CREATE TABLE journal_events(x); CREATE TABLE journal_heads(x);").unwrap(),
                _=>unreachable!(),
            }
        }
        let before = std::fs::read(&path).unwrap();
        assert!(
            SqliteJournal::open(&path, ParameterSchemas::default()).is_err(),
            "case {case}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before, "case {case}");
    }
}

#[test]
fn aggregate_read_bound_is_checked_before_loading_or_decoding_payloads() {
    let directory = support::Directory::new();
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    let mut connection = rusqlite::Connection::open(directory.database()).unwrap();
    let transaction = connection.transaction().unwrap();
    let bogus = "0".repeat(64);
    transaction
        .execute(
            "INSERT INTO content_values(digest,bytes) VALUES(?1,zeroblob(?2))",
            params![bogus, MAX_CONTENT_BYTES as i64],
        )
        .unwrap();
    transaction
        .execute(
            "INSERT INTO journal_heads(session,last_seq,chain) VALUES('bounded',?1,?2)",
            params![6_u64.to_be_bytes().as_slice(), bogus],
        )
        .unwrap();
    for seq in 1_u64..=6 {
        transaction.execute("INSERT INTO journal_events(session,seq,payload,prior,chain) VALUES('bounded',?1,?2,?2,?2)",params![seq.to_be_bytes().as_slice(),bogus]).unwrap();
    }
    transaction.commit().unwrap();
    assert_eq!(
        journal.read(&Id::new("bounded").unwrap()).unwrap_err().code,
        "journal_limit"
    );
}
