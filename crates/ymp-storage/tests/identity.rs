mod support;
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};
use ymp_domain::{Digest, Id};
use ymp_kernel::journal::{ContentStore, Journal, ParameterSchemas};
use ymp_storage::journal::SqliteJournal;
#[test]
fn upgrading_v1_preserves_history_content_and_chooses_one_identity_concurrently() {
    let directory = support::Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap());
    support::populate(journal.clone());
    let before = journal.read(&Id::new("methods").unwrap()).unwrap();
    let content = journal.content_store().put(b"retained bytes").unwrap();
    drop(journal);
    let connection = rusqlite::Connection::open(directory.database()).unwrap();
    connection
        .execute_batch("DROP TABLE store_identity; PRAGMA user_version=1;")
        .unwrap();
    let rows: Vec<(String, Vec<u8>)> = connection
        .prepare("SELECT digest,bytes FROM content_values ORDER BY digest")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    drop(connection);
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let path = directory.database();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                SqliteJournal::open(path, ParameterSchemas::default())
                    .unwrap()
                    .binding_identity()
                    .unwrap()
            })
        })
        .collect();
    let ids: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(ids[0], ids[1]);
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    assert_eq!(journal.read(&Id::new("methods").unwrap()).unwrap(), before);
    assert_eq!(
        journal.content_store().get(&content, 100).unwrap(),
        b"retained bytes"
    );
    let connection = rusqlite::Connection::open(directory.database()).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    let after: Vec<(String, Vec<u8>)> = connection
        .prepare("SELECT digest,bytes FROM content_values ORDER BY digest")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows, after);
}
#[test]
fn changed_identity_replacement_and_hardlinks_are_not_adopted_by_existing_handles() {
    let directory = support::Directory::new();
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    let original = journal.binding_identity().unwrap();
    let connection = rusqlite::Connection::open(directory.database()).unwrap();
    connection
        .execute(
            "UPDATE store_identity SET identity=?1",
            [Digest::of(b"changed identity").as_str()],
        )
        .unwrap();
    assert!(journal.binding_identity().is_err());
    assert!(journal.read(&Id::new("session").unwrap()).is_err());
    connection
        .execute(
            "UPDATE store_identity SET identity=?1",
            [original.id.as_str()],
        )
        .unwrap();
    drop(connection);
    assert_eq!(journal.binding_identity().unwrap(), original);
    let alias = directory.0.join("alias.sqlite");
    fs::hard_link(directory.database(), &alias).unwrap();
    assert!(journal.binding_identity().is_err());
    assert!(SqliteJournal::open(&alias, ParameterSchemas::default()).is_err());
    fs::remove_file(alias).unwrap();
    fs::rename(directory.database(), directory.0.join("old.sqlite")).unwrap();
    let replacement =
        SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    assert_ne!(replacement.binding_identity().unwrap(), original);
    assert!(journal.binding_identity().is_err());
}
#[test]
fn malformed_or_missing_identity_is_refused_without_repair() {
    for missing in [false, true] {
        let directory = support::Directory::new();
        let journal =
            SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
        let connection = rusqlite::Connection::open(directory.database()).unwrap();
        if missing {
            connection
                .execute("DELETE FROM store_identity", [])
                .unwrap();
        } else {
            connection
                .execute("UPDATE store_identity SET identity=?1", ["Z".repeat(64)])
                .unwrap();
        }
        assert!(journal.binding_identity().is_err());
        assert!(SqliteJournal::open(directory.database(), ParameterSchemas::default()).is_err());
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM store_identity", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, if missing { 0 } else { 1 });
    }
}
