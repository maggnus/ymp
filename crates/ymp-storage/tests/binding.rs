mod support;
use ymp_kernel as kernel;
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};
use ymp_domain::{Id, workspace::*};
use ymp_kernel::{
    journal::{Journal, ParameterSchemas},
    ports::execution::WorkspaceProvider,
};
use ymp_runtime::workspace::{
    binding::{MARKER, RootBinding},
    direct::Direct,
};
use ymp_storage::journal::SqliteJournal;
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
#[test]
fn guard_records_an_immutable_binding_and_capture_excludes_only_its_control_marker() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("source.txt"), b"source").unwrap();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let (guard, _) = fixture::open(
        journal.clone(),
        Arc::new(journal.content_store()),
        &id("session"),
        &provider,
    );
    guard
        .bind_workspace(&id("session"), 4, 4, &id("workspace"), &provider)
        .unwrap();
    let binding =
        guard.view(&id("session")).unwrap().workspace_bindings()[&id("workspace")].clone();
    assert_eq!(binding.journal, journal.binding_identity().unwrap());
    let marker = fs::read(root.0.join(MARKER)).unwrap();
    guard
        .snapshot(
            &id("session"),
            5,
            5,
            &id("workspace"),
            id("bound"),
            &provider,
        )
        .unwrap();
    let snapshot = guard.retained(&id("session"), &id("bound")).unwrap();
    assert_eq!(snapshot.tree.files.len(), 1);
    assert!(
        snapshot
            .tree
            .files
            .contains_key(&WorkspacePath::new("source.txt").unwrap())
    );
    assert_eq!(fs::read(root.0.join(MARKER)).unwrap(), marker);
    let fresh = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    assert!(
        fresh
            .capture(&journal.binding_identity(), &journal.content_store())
            .is_err()
    );
    let before = guard.view(&id("session")).unwrap();
    assert_eq!(
        guard
            .bind_workspace(
                &id("session"),
                before.revision(),
                6,
                &id("workspace"),
                &fresh
            )
            .unwrap(),
        before.revision()
    );
    assert_eq!(guard.view(&id("session")).unwrap(), before);
    fs::remove_file(root.0.join(MARKER)).unwrap();
    assert!(RootBinding::open(&root.0, journal.as_ref(), &CaptureLimits::default()).is_err());
    assert!(!root.0.join(MARKER).exists());
}
#[test]
fn another_store_or_a_database_copy_cannot_adopt_the_same_root() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let other = support::Directory::new();
    let journal = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let bound = RootBinding::open(&root.0, &journal, &CaptureLimits::default()).unwrap();
    let marker = fs::read(root.0.join(MARKER)).unwrap();
    let other_journal = SqliteJournal::open(other.database(), ParameterSchemas::default()).unwrap();
    assert!(RootBinding::open(&root.0, &other_journal, &CaptureLimits::default()).is_err());
    let connection = rusqlite::Connection::open(database.database()).unwrap();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(connection);
    let copy = other.0.join("copy.sqlite");
    fs::copy(database.database(), &copy).unwrap();
    let copied = SqliteJournal::open(copy, ParameterSchemas::default()).unwrap();
    assert_eq!(
        copied.binding_identity().unwrap().id,
        journal.binding_identity().unwrap().id
    );
    assert_ne!(
        copied.binding_identity().unwrap().file,
        journal.binding_identity().unwrap().file
    );
    assert!(RootBinding::open(&root.0, &copied, &CaptureLimits::default()).is_err());
    bound.verify(&journal).unwrap();
    assert_eq!(fs::read(root.0.join(MARKER)).unwrap(), marker);
}
#[test]
fn nested_bindings_are_refused_in_both_orders_and_under_concurrency() {
    for child_first in [false, true] {
        let root = support::Directory::new();
        fs::create_dir(root.0.join("child")).unwrap();
        let database = support::Directory::new();
        let journal =
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        let (first, second) = if child_first {
            (root.0.join("child"), root.0.clone())
        } else {
            (root.0.clone(), root.0.join("child"))
        };
        RootBinding::open(&first, &journal, &CaptureLimits::default()).unwrap();
        assert!(RootBinding::open(&second, &journal, &CaptureLimits::default()).is_err());
    }
    let root = support::Directory::new();
    fs::create_dir(root.0.join("child")).unwrap();
    let databases = [support::Directory::new(), support::Directory::new()];
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = [root.0.clone(), root.0.join("child")]
        .into_iter()
        .zip(databases.iter())
        .map(|(path, database)| {
            let journal =
                SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                RootBinding::open(&path, &journal, &CaptureLimits::default()).map(|_| ())
            })
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
}
#[test]
fn foreign_markers_and_contained_storage_are_not_modified() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join(MARKER), b"user-owned file").unwrap();
    let journal = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    assert!(RootBinding::open(&root.0, &journal, &CaptureLimits::default()).is_err());
    assert_eq!(fs::read(root.0.join(MARKER)).unwrap(), b"user-owned file");
    let contained = support::Directory::new();
    let inside = SqliteJournal::open(contained.database(), ParameterSchemas::default()).unwrap();
    assert!(RootBinding::open(&contained.0, &inside, &CaptureLimits::default()).is_err());
    assert!(!contained.0.join(MARKER).exists());
}

#[test]
fn an_unbound_ancestor_preview_cannot_read_a_bound_descendant() {
    let parent = support::Directory::new();
    fs::create_dir(parent.0.join("child")).unwrap();
    fs::write(parent.0.join("child/data"), b"owned source").unwrap();
    let owner = support::Directory::new();
    let preview = support::Directory::new();
    let owner = SqliteJournal::open(owner.database(), ParameterSchemas::default()).unwrap();
    let preview = SqliteJournal::open(preview.database(), ParameterSchemas::default()).unwrap();
    RootBinding::open(&parent.0.join("child"), &owner, &CaptureLimits::default()).unwrap();
    let provider = Direct::open(&parent.0, CaptureLimits::default()).unwrap();
    assert_eq!(
        provider
            .capture(&preview.binding_identity(), &preview.content_store())
            .unwrap_err()
            .code,
        "binding_nested"
    );
}
struct PausedContent {
    inner: ymp_storage::content::SqliteContent,
    entered: std::sync::mpsc::Sender<()>,
    resume: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
}
impl ymp_kernel::journal::ContentStore for PausedContent {
    fn put(&self, bytes: &[u8]) -> ymp_domain::Result<ymp_domain::Digest> {
        if let Some(resume) = self.resume.lock().unwrap().take() {
            self.entered.send(()).unwrap();
            resume
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
        }
        self.inner.put(bytes)
    }
    fn get(&self, digest: &ymp_domain::Digest, limit: usize) -> ymp_domain::Result<Vec<u8>> {
        self.inner.get(digest, limit)
    }
}
#[test]
fn a_descendant_binding_cannot_start_while_an_unbound_ancestor_is_being_captured() {
    let parent = support::Directory::new();
    fs::create_dir(parent.0.join("child")).unwrap();
    fs::write(parent.0.join("data"), b"source").unwrap();
    let database = support::Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let provider = Direct::open(&parent.0, CaptureLimits::default()).unwrap();
    let (entered, ready) = std::sync::mpsc::channel();
    let (resume, wait) = std::sync::mpsc::channel();
    let content = PausedContent {
        inner: journal.content_store(),
        entered,
        resume: std::sync::Mutex::new(Some(wait)),
    };
    let identity = journal.binding_identity();
    let worker = thread::spawn(move || provider.capture(&identity, &content));
    ready
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    assert_eq!(
        RootBinding::open(
            &parent.0.join("child"),
            journal.as_ref(),
            &CaptureLimits::default()
        )
        .err()
        .unwrap()
        .code,
        "binding_busy"
    );
    assert!(!parent.0.join("child").join(MARKER).exists());
    resume.send(()).unwrap();
    worker.join().unwrap().unwrap();
    RootBinding::open(
        &parent.0.join("child"),
        journal.as_ref(),
        &CaptureLimits::default(),
    )
    .unwrap();
}

#[test]
fn process_binding() {
    let Ok(root) = std::env::var("YMP_BIND_TEST_ROOT") else {
        return;
    };
    let database = std::env::var("YMP_BIND_TEST_DB").unwrap();
    let output = std::env::var("YMP_BIND_TEST_OUTPUT").unwrap();
    let journal = SqliteJournal::open(database, ParameterSchemas::default()).unwrap();
    let result = RootBinding::open(
        std::path::Path::new(&root),
        &journal,
        &CaptureLimits::default(),
    );
    fs::write(
        output,
        match result {
            Ok(_) => "bound".to_owned(),
            Err(error) => error.code,
        },
    )
    .unwrap();
}
#[test]
fn independent_processes_bind_at_most_one_overlapping_root_and_allow_siblings() {
    for relation in ["same", "nested", "siblings"] {
        let root = support::Directory::new();
        let first = support::Directory::new();
        let second = support::Directory::new();
        fs::create_dir(root.0.join("left")).unwrap();
        fs::create_dir(root.0.join("right")).unwrap();
        let paths = match relation {
            "same" => [root.0.clone(), root.0.clone()],
            "nested" => [root.0.clone(), root.0.join("left")],
            _ => [root.0.join("left"), root.0.join("right")],
        };
        let mut children = vec![];
        for (path, database) in paths.iter().zip([&first, &second]) {
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
            children.push(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "process_binding", "--nocapture"])
                    .env("YMP_BIND_TEST_ROOT", path)
                    .env("YMP_BIND_TEST_DB", database.database())
                    .env("YMP_BIND_TEST_OUTPUT", database.0.join("result"))
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            );
        }
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
        let results = [
            fs::read_to_string(first.0.join("result")).unwrap(),
            fs::read_to_string(second.0.join("result")).unwrap(),
        ];
        assert_eq!(
            results.iter().filter(|r| r.as_str() == "bound").count(),
            if relation == "siblings" { 2 } else { 1 },
            "{relation}: {results:?}"
        );
    }
}

#[test]
fn a_bound_provider_cannot_be_reused_by_a_foreign_guards_unbound_preview() {
    let root = support::Directory::new();
    let first = support::Directory::new();
    let second = support::Directory::new();
    fs::write(root.0.join("source"), b"owned").unwrap();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let first =
        Arc::new(SqliteJournal::open(first.database(), ParameterSchemas::default()).unwrap());
    let second =
        Arc::new(SqliteJournal::open(second.database(), ParameterSchemas::default()).unwrap());
    let (guard, _) = fixture::open(
        first.clone(),
        Arc::new(first.content_store()),
        &id("owner"),
        &provider,
    );
    guard
        .bind_workspace(&id("owner"), 4, 4, &id("workspace"), &provider)
        .unwrap();
    let (foreign, _) = fixture::open(
        second.clone(),
        Arc::new(second.content_store()),
        &id("foreign"),
        &provider,
    );
    assert_eq!(
        foreign
            .snapshot(
                &id("foreign"),
                4,
                5,
                &id("workspace"),
                id("forbidden"),
                &provider
            )
            .unwrap_err()
            .code,
        "binding_conflict"
    );
    assert!(foreign.view(&id("foreign")).unwrap().snapshots().is_empty());
}
