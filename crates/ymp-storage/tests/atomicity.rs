mod support;
use std::{
    process::Command,
    sync::{Arc, Barrier},
    thread,
};
use ymp_domain::Id;
use ymp_kernel::journal::{Journal, ParameterSchemas};
use ymp_storage::journal::SqliteJournal;

#[test]
fn concurrent_open_and_compare_and_append_have_one_winner() {
    let directory = support::Directory::new();
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let path = directory.database();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                let journal = SqliteJournal::open(path, ParameterSchemas::default()).unwrap();
                let session = Id::new("race").unwrap();
                journal.append(&session, 0, &[support::opening(&session)])
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results.iter().find_map(|r| r.as_ref().err()).unwrap().code,
        "stale_revision"
    );
}

#[test]
fn invalid_batch_tail_rolls_back_the_whole_write() {
    let directory = support::Directory::new();
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    let session = Id::new("atomic").unwrap();
    let first = support::opening(&session);
    let mut invalid = first.clone();
    invalid.seq = 2;
    assert!(journal.append(&session, 0, &[first, invalid]).is_err());
    assert_eq!(journal.read(&session).unwrap().revision, 0);
    let connection = rusqlite::Connection::open(directory.database()).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM content_values", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn process_writer() {
    let Some(path) = std::env::var_os("YMP_STORAGE_WRITER") else {
        return;
    };
    let journal = SqliteJournal::open(path, ParameterSchemas::default()).unwrap();
    let session = Id::new("process-race").unwrap();
    match journal.append(&session, 0, &[support::opening(&session)]) {
        Ok(_) => {}
        Err(error) if error.code == "stale_revision" => std::process::exit(71),
        Err(error) => panic!("{error}"),
    }
}
#[test]
fn two_processes_cannot_commit_the_same_revision_twice() {
    let directory = support::Directory::new();
    SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    let mut children: Vec<_> = (0..2)
        .map(|_| {
            Command::new(std::env::current_exe().unwrap())
                .args(["process_writer", "--exact", "--nocapture"])
                .env("YMP_STORAGE_WRITER", directory.database())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    let codes: Vec<_> = children
        .iter_mut()
        .map(|child| child.wait().unwrap().code().unwrap())
        .collect();
    assert_eq!(codes.iter().filter(|&&code| code == 0).count(), 1);
    assert_eq!(codes.iter().filter(|&&code| code == 71).count(), 1);
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    assert_eq!(
        journal
            .read(&Id::new("process-race").unwrap())
            .unwrap()
            .revision,
        1
    );
}
