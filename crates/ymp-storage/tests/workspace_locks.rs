mod support;
use ymp_kernel as kernel;
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use std::{
    fs,
    process::Command,
    sync::{Arc, Barrier},
    thread,
};
use ymp_domain::{
    Id,
    journal::{Actor, Envelope},
    workspace::*,
};
use ymp_kernel::{
    events::Event,
    journal::{Journal, ParameterSchemas},
    ports::execution::WorkspaceProvider,
    workspace_locks::{LockAcquisition, LockChange, attribution},
};
use ymp_runtime::{memory_journal::MemoryJournal, workspace::direct::Direct};
use ymp_storage::journal::SqliteJournal;
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
/// Explicit synthetic boundary data for the aggregate adapter/replay contract.
/// Consumer tests separately exercise the opaque evidence capability.
fn acquire<J: Journal>(
    journal: &J,
    session: &Id,
    provider: &Direct,
    path: &str,
    mode: LockMode,
) -> Envelope<Event> {
    let view = journal
        .read(session)
        .unwrap()
        .view_with_schemas(session, None, journal.schemas())
        .unwrap();
    let workspace = &view.workspaces()[&id("workspace")];
    let profile = view.registry().unwrap().decisions[0].profile.clone();
    let path = WorkspacePath::new(path).unwrap();
    let lock = PathLock {
        path: path.clone(),
        mode,
        holder: id("assignment"),
    };
    let change = LockChange::Acquired(Box::new(LockAcquisition {
        workspace: id("workspace"),
        assignment: id("assignment"),
        profile,
        requested: vec![lock.clone()],
        effective: vec![ObservedPathLock {
            lock,
            observation: provider.observe_paths(&[path]).unwrap().remove(0),
        }],
        basis: vec![workspace.reference().unwrap()],
    }));
    Envelope {
        seq: view.revision() + 1,
        session: session.clone(),
        at: 5,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: attribution(&view, &change).unwrap(),
        payload: Event::LockChanged { version: 1, change },
    }
}
#[test]
fn incompatible_cross_session_appends_have_one_winner_in_both_adapters() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let sqlite =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let memory = Arc::new(MemoryJournal::new());
    for session in [id("a"), id("b")] {
        fixture::open(
            sqlite.clone(),
            Arc::new(sqlite.content_store()),
            &session,
            &provider,
        );
        fixture::open(
            memory.clone(),
            Arc::new(sqlite.content_store()),
            &session,
            &provider,
        );
    }
    race(sqlite, &provider);
    race(memory, &provider);
}
fn race<J: Journal + 'static>(journal: Arc<J>, provider: &Direct) {
    let events = [
        acquire(
            journal.as_ref(),
            &id("a"),
            provider,
            "shared",
            LockMode::Write,
        ),
        acquire(
            journal.as_ref(),
            &id("b"),
            provider,
            "SHARED/child",
            LockMode::Read,
        ),
    ];
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = events
        .into_iter()
        .map(|event| {
            let journal = journal.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                journal.append(&event.session, event.seq - 1, std::slice::from_ref(&event))
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let error = results.iter().find_map(|r| r.as_ref().err()).unwrap();
    assert_eq!(error.code, "path_conflict");
    assert!(!error.refs.is_empty());
}
#[test]
fn missing_owner_heads_cannot_hide_holds_from_the_inventory() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    for session in [id("owner"), id("other")] {
        fixture::open(
            journal.clone(),
            Arc::new(journal.content_store()),
            &session,
            &provider,
        );
    }
    let held = acquire(
        journal.as_ref(),
        &id("owner"),
        &provider,
        ".",
        LockMode::Write,
    );
    journal.append(&id("owner"), 4, &[held]).unwrap();
    let request = acquire(
        journal.as_ref(),
        &id("other"),
        &provider,
        "file",
        LockMode::Write,
    );
    let before = journal.read(&id("other")).unwrap();
    let connection = rusqlite::Connection::open(database.database()).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "OFF")
        .unwrap();
    connection
        .execute("DELETE FROM journal_heads WHERE session='owner'", [])
        .unwrap();
    assert_eq!(
        journal
            .workspace_inventory(&id("other"))
            .err()
            .unwrap()
            .code,
        "storage_corrupt"
    );
    assert_eq!(
        journal
            .append(&id("other"), 4, &[request])
            .unwrap_err()
            .code,
        "storage_corrupt"
    );
    assert_eq!(journal.read(&id("other")).unwrap(), before);
}
#[test]
fn path_observations_catch_parent_roots_and_keep_unrelated_scopes_disjoint() {
    let root = support::Directory::new();
    fs::create_dir(root.0.join("child")).unwrap();
    let parent = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let child = Direct::open(&root.0.join("child"), CaptureLimits::default()).unwrap();
    let ancestor = parent
        .observe_paths(&[WorkspacePath::new("child").unwrap()])
        .unwrap()
        .remove(0);
    let descendant = child
        .observe_paths(&[WorkspacePath::new("file").unwrap()])
        .unwrap()
        .remove(0);
    assert!(ancestor.overlaps(&descendant));
    assert!(descendant.overlaps(&ancestor));
    let unrelated = parent
        .observe_paths(&[WorkspacePath::new("elsewhere").unwrap()])
        .unwrap()
        .remove(0);
    assert!(!unrelated.overlaps(&descendant));
}
#[test]
fn process_writer() {
    let Ok(database) = std::env::var("YMP_LOCK_TEST_DATABASE") else {
        return;
    };
    let root = std::env::var("YMP_LOCK_TEST_ROOT").unwrap();
    let name = std::env::var("YMP_LOCK_TEST_SESSION").unwrap();
    let output = std::env::var("YMP_LOCK_TEST_OUTPUT").unwrap();
    let journal = SqliteJournal::open(database, ParameterSchemas::default()).unwrap();
    let provider = Direct::open(std::path::Path::new(&root), CaptureLimits::default()).unwrap();
    let event = acquire(&journal, &id(&name), &provider, "shared", LockMode::Write);
    let result = journal.append(&id(&name), event.seq - 1, &[event]);
    fs::write(
        output,
        match result {
            Ok(_) => "committed".to_owned(),
            Err(error) => error.code,
        },
    )
    .unwrap();
}
#[test]
fn independent_processes_cannot_acquire_the_same_path_in_different_sessions() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    for session in [id("one"), id("two")] {
        fixture::open(
            journal.clone(),
            Arc::new(journal.content_store()),
            &session,
            &provider,
        );
    }
    let mut children = vec![];
    for name in ["one", "two"] {
        children.push(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "process_writer", "--nocapture"])
                .env("YMP_LOCK_TEST_DATABASE", database.database())
                .env("YMP_LOCK_TEST_ROOT", &root.0)
                .env("YMP_LOCK_TEST_SESSION", name)
                .env("YMP_LOCK_TEST_OUTPUT", database.0.join(name))
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let mut results: Vec<_> = ["one", "two"]
        .iter()
        .map(|name| fs::read_to_string(database.0.join(name)).unwrap())
        .collect();
    results.sort();
    assert_eq!(results, ["committed", "path_conflict"]);
    let inventory = journal.workspace_inventory(&id("one")).unwrap();
    let current = inventory.current.view(&id("one"), None).unwrap();
    let ownership = ymp_kernel::workspace_locks::WorkspaceOwnership::from_view(&current);
    ymp_kernel::workspace_locks::validate_ownership(
        inventory.other.iter().chain(std::iter::once(&ownership)),
    )
    .unwrap();
}

#[test]
fn an_oversized_session_key_is_refused_before_loading_the_inventory() {
    let database = support::Directory::new();
    let journal = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let connection = rusqlite::Connection::open(database.database()).unwrap();
    connection
        .execute(
            "INSERT INTO journal_heads(session,last_seq,chain) VALUES(?1,?2,?3)",
            rusqlite::params![
                "x".repeat(4096),
                1u64.to_be_bytes().as_slice(),
                ymp_domain::Digest::of(b"invalid head").as_str()
            ],
        )
        .unwrap();
    let error = journal.workspace_inventory(&id("current")).err().unwrap();
    assert_eq!(error.code, "storage_corrupt");
    assert!(error.message.contains("session key"));
}
