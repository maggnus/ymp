mod support;
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use ymp_domain::{Digest, Id, Result, task::*, workspace::*};
use ymp_kernel::{
    journal::{ContentStore, Journal, ParameterSchemas},
    ports::execution::WorkspaceProvider,
    workspace_guard::WorkspaceGuard,
};
use ymp_runtime::{
    application::{Application, IntakeRequest},
    workspace::direct::Direct,
};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
fn setup(
    provider: &dyn WorkspaceProvider,
    database: &support::Directory,
) -> (Arc<SqliteJournal>, Id) {
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let session = id("workspace-session");
    Application::new(journal.clone())
        .open(
            session.clone(),
            1,
            IntakeRequest {
                task: Task {
                    id: id("task"),
                    goal: Goal {
                        request: "Retain exact workspace content".into(),
                        assumptions: vec![],
                        clarifications: vec![],
                    },
                    contract: id("contract"),
                    constraints: Constraints {
                        budget: Real::new(100.0).unwrap(),
                        verification_reserve: Real::new(10.0).unwrap(),
                        deadline: None,
                        pins: Pins::unrestricted(),
                        allowed: BTreeSet::new(),
                        parallel_limit: 2,
                        attempt_limit: 2,
                        max_members: 2,
                    },
                },
                criteria: vec![Criterion {
                    id: id("criterion"),
                    text: "Preserve before and after bytes".into(),
                    kind: CriterionKind::Preserve,
                    weight: Real::new(1.0).unwrap(),
                    required: true,
                    origin: CriterionOrigin::User,
                    needs_class: BTreeSet::new(),
                }],
            },
            vec![provider.selection().clone()],
        )
        .unwrap();
    (journal, session)
}
#[test]
fn direct_snapshots_retain_exact_bytes_permissions_and_empty_directories_after_restart() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::create_dir(root.0.join("empty")).unwrap();
    fs::write(root.0.join("run.sh"), b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(root.0.join("run.sh"), fs::Permissions::from_mode(0o751)).unwrap();
    fs::write(root.0.join("binary"), [0, 255, 10]).unwrap();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let (journal, session) = setup(&provider, &database);
    let guard = WorkspaceGuard::new(journal.clone(), Arc::new(journal.content_store()));
    let workspace = id("workspace");
    guard
        .open(&session, 2, 2, workspace.clone(), &provider)
        .unwrap();
    guard
        .snapshot(&session, 3, 3, &workspace, id("before"), &provider)
        .unwrap();
    let before = guard.retained(&session, &id("before")).unwrap();
    assert_eq!(
        before.tree.files[&WorkspacePath::new("run.sh").unwrap()].mode,
        0o751
    );
    assert!(
        before
            .tree
            .directories
            .contains_key(&WorkspacePath::new("empty").unwrap())
    );
    fs::write(root.0.join("binary"), [3, 2, 1, 0]).unwrap();
    fs::remove_file(root.0.join("run.sh")).unwrap();
    guard
        .snapshot(&session, 5, 4, &workspace, id("after"), &provider)
        .unwrap();
    let after = guard.retained(&session, &id("after")).unwrap();
    let recorded = guard.view(&session).unwrap();
    drop(guard);
    drop(journal);
    drop(provider);
    fs::remove_file(root.0.join("binary")).unwrap();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let guard = WorkspaceGuard::new(journal.clone(), Arc::new(journal.content_store()));
    assert_eq!(guard.view(&session).unwrap(), recorded);
    assert_eq!(guard.retained(&session, &id("before")).unwrap(), before);
    assert_eq!(guard.retained(&session, &id("after")).unwrap(), after);
    assert_eq!(
        guard
            .read_artifact(
                &session,
                &id("before"),
                &WorkspacePath::new("binary").unwrap()
            )
            .unwrap(),
        [0, 255, 10]
    );
    assert_eq!(
        guard
            .read_artifact(
                &session,
                &id("after"),
                &WorkspacePath::new("binary").unwrap()
            )
            .unwrap(),
        [3, 2, 1, 0]
    );
    assert!(
        guard
            .read_artifact(
                &session,
                &id("after"),
                &WorkspacePath::new("run.sh").unwrap()
            )
            .is_err()
    );
}
#[test]
fn links_escapes_missing_content_and_changed_roots_cannot_commit_a_snapshot() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let outside = support::Directory::new();
    fs::write(outside.0.join("secret"), b"outside").unwrap();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let (journal, session) = setup(&provider, &database);
    let guard = WorkspaceGuard::new(journal.clone(), Arc::new(journal.content_store()));
    guard
        .open(&session, 2, 2, id("workspace"), &provider)
        .unwrap();

    symlink(&outside.0, root.0.join("escape")).unwrap();
    assert!(
        provider
            .validate_paths(&[WorkspacePath::new("escape/secret").unwrap()])
            .is_err()
    );
    assert_eq!(
        guard
            .snapshot(&session, 3, 3, &id("workspace"), id("symlink"), &provider)
            .unwrap_err()
            .code,
        "workspace_type"
    );
    fs::remove_file(root.0.join("escape")).unwrap();
    fs::hard_link(outside.0.join("secret"), root.0.join("alias")).unwrap();
    assert!(
        provider
            .validate_paths(&[WorkspacePath::new("alias").unwrap()])
            .is_err()
    );
    assert_eq!(
        guard
            .snapshot(&session, 5, 3, &id("workspace"), id("hardlink"), &provider)
            .unwrap_err()
            .code,
        "workspace_alias"
    );
    fs::remove_file(root.0.join("alias")).unwrap();
    let view = guard.view(&session).unwrap();
    assert!(view.snapshots().is_empty());
    assert!(
        view.capture_reads()
            .values()
            .all(|c| c.ended.is_some() && c.failure.is_some())
    );
    let before = journal.read(&session).unwrap();
    for invalid in [
        "../secret",
        "/secret",
        "a/../b",
        "a//b",
        "a/./b",
        "a\\b",
        "",
    ] {
        assert!(WorkspacePath::new(invalid).is_err(), "{invalid}");
        assert!(serde_json::from_value::<WorkspacePath>(serde_json::json!(invalid)).is_err());
    }
    fs::rename(&root.0, outside.0.join("moved")).unwrap();
    fs::create_dir(&root.0).unwrap();
    assert!(
        guard
            .snapshot(
                &session,
                7,
                3,
                &id("workspace"),
                id("replacement"),
                &provider
            )
            .is_err()
    );
    assert_eq!(journal.read(&session).unwrap(), before);
}
struct MutatingStore {
    inner: SqliteContent,
    path: std::path::PathBuf,
    changed: AtomicBool,
}
impl ContentStore for MutatingStore {
    fn put(&self, bytes: &[u8]) -> Result<Digest> {
        if !self.changed.swap(true, Ordering::SeqCst) {
            fs::write(&self.path, b"changed during capture").unwrap();
        }
        self.inner.put(bytes)
    }
    fn get(&self, digest: &Digest, limit: usize) -> Result<Vec<u8>> {
        self.inner.get(digest, limit)
    }
}
#[test]
fn changes_during_capture_are_aborted_without_publishing_a_snapshot() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("file"), b"original").unwrap();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let (journal, session) = setup(&provider, &database);
    let guard = WorkspaceGuard::new(
        journal.clone(),
        Arc::new(MutatingStore {
            inner: journal.content_store(),
            path: root.0.join("file"),
            changed: AtomicBool::new(false),
        }),
    );
    guard
        .open(&session, 2, 2, id("workspace"), &provider)
        .unwrap();

    assert_eq!(
        guard
            .snapshot(&session, 3, 3, &id("workspace"), id("unstable"), &provider)
            .unwrap_err()
            .code,
        "workspace_changed"
    );
    let view = guard.view(&session).unwrap();
    assert!(view.snapshots().is_empty());
    assert!(view.capture_reads()[&id("unstable")].ended.is_some());
    assert!(
        view.capture_reads()[&id("unstable")]
            .failure
            .as_ref()
            .unwrap()
            .contains("workspace_changed")
    );
    let tiny = Direct::open(
        &root.0,
        CaptureLimits {
            max_file_bytes: 2,
            max_total_bytes: 2,
            ..CaptureLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        tiny.capture(&journal.content_store()).unwrap_err().code,
        "capture_limit"
    );
}
#[test]
fn stored_content_corruption_is_detected_by_snapshot_reopening() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("file"), b"original").unwrap();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let (journal, session) = setup(&provider, &database);
    let guard = WorkspaceGuard::new(journal.clone(), Arc::new(journal.content_store()));
    guard
        .open(&session, 2, 2, id("workspace"), &provider)
        .unwrap();
    guard
        .snapshot(&session, 3, 3, &id("workspace"), id("snapshot"), &provider)
        .unwrap();
    let digest = Digest::of(b"original");
    let connection = rusqlite::Connection::open(database.database()).unwrap();
    connection
        .execute(
            "UPDATE content_values SET bytes=?1 WHERE digest=?2",
            rusqlite::params![b"modified".as_slice(), digest.as_str()],
        )
        .unwrap();
    assert!(guard.retained(&session, &id("snapshot")).is_err());
}

struct ForgedViewJournal {
    inner: Arc<SqliteJournal>,
    forged: ymp_kernel::view::SessionView,
}
impl Journal for ForgedViewJournal {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn read(&self, session: &Id) -> Result<ymp_kernel::journal::JournalRead> {
        self.inner.read(session)
    }
    fn append(
        &self,
        session: &Id,
        expected: u64,
        events: &[ymp_domain::journal::Envelope<ymp_kernel::events::Event>],
    ) -> Result<u64> {
        self.inner.append(session, expected, events)
    }
    fn view(&self, _: &Id, _: Option<u64>) -> Result<ymp_kernel::view::SessionView> {
        Ok(self.forged.clone())
    }
}
#[test]
fn an_adapter_view_cannot_attribute_another_directorys_content_to_the_recorded_workspace() {
    let root = support::Directory::new();
    let other_root = support::Directory::new();
    let database = support::Directory::new();
    let other_database = support::Directory::new();
    fs::write(root.0.join("file"), b"real").unwrap();
    fs::write(other_root.0.join("file"), b"impostor").unwrap();
    let provider = Direct::open(&root.0, CaptureLimits::default()).unwrap();
    let other = Direct::open(&other_root.0, CaptureLimits::default()).unwrap();
    let (journal, session) = setup(&provider, &database);
    let (other_journal, _) = setup(&other, &other_database);
    let guard = WorkspaceGuard::new(journal.clone(), Arc::new(journal.content_store()));
    guard
        .open(&session, 2, 2, id("workspace"), &provider)
        .unwrap();
    let other_guard = WorkspaceGuard::new(
        other_journal.clone(),
        Arc::new(other_journal.content_store()),
    );
    other_guard
        .open(&session, 2, 2, id("workspace"), &other)
        .unwrap();
    let fake = Arc::new(ForgedViewJournal {
        inner: journal.clone(),
        forged: other_guard.view(&session).unwrap(),
    });
    let guard = WorkspaceGuard::new(fake, Arc::new(journal.content_store()));
    let before = journal.read(&session).unwrap();
    assert_eq!(
        guard
            .snapshot(
                &session,
                3,
                3,
                &id("workspace"),
                id("false-capture"),
                &other
            )
            .unwrap_err()
            .code,
        "workspace_provider"
    );
    assert_eq!(journal.read(&session).unwrap(), before);
}
