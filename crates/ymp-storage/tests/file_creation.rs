//! Faults at the durable creation boundary; no native inference or real user data.
mod support;
use ymp_kernel as kernel;
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use ymp_domain::{Denial, Id, Result, journal::Envelope, workspace::*};
use ymp_kernel::{
    events::Event,
    journal::{Journal, JournalRead, ParameterSchemas, WorkspaceInventory},
    ports::execution::WorkspaceProvider,
    workspace_guard::{FileAccess, LockRequest},
    workspace_locks::LockChange,
};
use ymp_runtime::workspace::direct::Direct;
use ymp_storage::journal::SqliteJournal;
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn path(s: &str) -> WorkspacePath {
    WorkspacePath::new(s).unwrap()
}
struct FaultJournal {
    inner: Arc<SqliteJournal>,
    fault: AtomicUsize,
}
impl Journal for FaultJournal {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn binding_identity(&self) -> Result<JournalIdentity> {
        self.inner.binding_identity()
    }
    fn workspace_binding(&self, root: &WorkspaceLocation) -> Result<Option<WorkspaceBinding>> {
        self.inner.workspace_binding(root)
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        self.inner.read(session)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        let fault = self.fault.load(Ordering::SeqCst);
        let matches = events.iter().any(|e| match e.payload {
            Event::LockChanged {
                change: LockChange::FileCreationStarted { .. },
                ..
            } => fault == 1 || fault == 2,
            Event::LockChanged {
                change: LockChange::FileCreated { .. },
                ..
            } => fault == 3 || fault == 4,
            _ => false,
        });
        if matches && self.fault.swap(0, Ordering::SeqCst) != 0 {
            if fault == 2 || fault == 4 {
                self.inner.append(session, expected, events)?;
            }
            return Err(Denial::new(
                "fixture_append",
                "Injected failure before commit or after acknowledgement loss",
            ));
        }
        self.inner.append(session, expected, events)
    }
}
#[test]
fn lost_creation_commits_resolve_without_recreating_or_writing_data() {
    for fault in 1..=4 {
        let root = support::Directory::new();
        let database = support::Directory::new();
        let sqlite = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let journal = Arc::new(FaultJournal {
            inner: sqlite.clone(),
            fault: AtomicUsize::new(0),
        });
        let provider = Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap());
        let session = id("creator");
        let (guard, profile) = fixture::open(
            journal.clone(),
            Arc::new(sqlite.content_store()),
            &session,
            provider.as_ref(),
        );
        guard
            .bind_workspace(&session, 4, 4, &id("workspace"), provider.as_ref())
            .unwrap();
        let handle = guard
            .mediate(
                &session,
                5,
                5,
                LockRequest {
                    assignment: id("assignment"),
                    workspace: id("workspace"),
                    profile,
                    paths: vec![(path("file"), LockMode::Write)],
                },
                provider.clone(),
            )
            .unwrap();
        guard
            .authorize_access(
                &session,
                6,
                6,
                id("assignment"),
                id("invocation"),
                provider.as_ref(),
            )
            .unwrap();
        journal.fault.store(fault, Ordering::SeqCst);
        assert!(
            handle
                .write(&path("file"), b"must not be replayed", 12)
                .is_err()
        );
        assert_eq!(root.0.join("file").exists(), fault >= 3);
        if fault >= 3 {
            assert!(fs::read(root.0.join("file")).unwrap().is_empty());
        }
        let before = guard.view(&session).unwrap();
        assert_eq!(
            before.path_locks()[&id("assignment")].creation.is_some(),
            fault == 2 || fault == 3
        );
        let inode = if fault >= 3 {
            use std::os::unix::fs::MetadataExt;
            Some(fs::metadata(root.0.join("file")).unwrap().ino())
        } else {
            None
        };
        handle.resolve_creation(12).unwrap();
        assert!(
            guard.view(&session).unwrap().path_locks()[&id("assignment")]
                .creation
                .is_none()
        );
        if fault >= 3 {
            assert!(fs::read(root.0.join("file")).unwrap().is_empty());
        }
        handle
            .write(&path("file"), b"explicit subsequent write", 12)
            .unwrap();
        if let Some(inode) = inode {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(fs::metadata(root.0.join("file")).unwrap().ino(), inode);
        }
        assert_eq!(
            fs::read(root.0.join("file")).unwrap(),
            b"explicit subsequent write"
        );
        for event in &sqlite.read(&session).unwrap().events {
            if matches!(
                event.payload,
                Event::LockChanged {
                    change: LockChange::FileCreationStarted { .. }
                        | LockChange::FileCreated { .. }
                        | LockChange::FileCreationAborted { .. },
                    ..
                }
            ) {
                assert_eq!(event.at, 12);
            }
        }
        let evidence = guard.withdraw_mediated(&handle).unwrap();
        guard
            .release(
                &session,
                guard.view(&session).unwrap().revision(),
                20,
                &evidence,
            )
            .unwrap();
    }
}
#[test]
fn a_restart_cannot_clear_unpublished_creation_or_recover_it_from_its_old_name() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let sqlite =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let journal = Arc::new(FaultJournal {
        inner: sqlite.clone(),
        fault: AtomicUsize::new(0),
    });
    let provider = Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap());
    let session = id("creator");
    let (guard, profile) = fixture::open(
        journal.clone(),
        Arc::new(sqlite.content_store()),
        &session,
        provider.as_ref(),
    );
    guard
        .bind_workspace(&session, 4, 4, &id("workspace"), provider.as_ref())
        .unwrap();
    let handle = guard
        .mediate(
            &session,
            5,
            5,
            LockRequest {
                assignment: id("assignment"),
                workspace: id("workspace"),
                profile,
                paths: vec![(path("file"), LockMode::Write)],
            },
            provider.clone(),
        )
        .unwrap();
    guard
        .authorize_access(
            &session,
            6,
            6,
            id("assignment"),
            id("invocation"),
            provider.as_ref(),
        )
        .unwrap();
    journal.fault.store(3, Ordering::SeqCst);
    assert!(handle.write(&path("file"), b"unwritten", 12).is_err());
    fs::rename(root.0.join("file"), root.0.join("moved")).unwrap();
    drop(handle);
    drop(guard);
    drop(journal);
    drop(sqlite);
    drop(provider);
    let reopened =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let provider = Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap());
    let (other, profile) = fixture::open(
        reopened.clone(),
        Arc::new(reopened.content_store()),
        &id("other"),
        provider.as_ref(),
    );
    other
        .bind_workspace(&id("other"), 4, 4, &id("workspace"), provider.as_ref())
        .unwrap();
    let access = other
        .mediate(
            &id("other"),
            5,
            5,
            LockRequest {
                assignment: id("other"),
                workspace: id("workspace"),
                profile,
                paths: vec![(path("unrelated"), LockMode::Write)],
            },
            provider.clone(),
        )
        .unwrap();
    other
        .authorize_access(
            &id("other"),
            6,
            6,
            id("other"),
            id("other-invocation"),
            provider.as_ref(),
        )
        .unwrap();
    assert_eq!(
        access
            .write(&path("unrelated"), b"blocked", 12)
            .unwrap_err()
            .code,
        "file_creation_pending"
    );
    assert!(!root.0.join("file").exists());
    assert!(fs::read(root.0.join("moved")).unwrap().is_empty());
    assert!(other.never_authorized(&session, &id("assignment")).is_err());
    let replay = other.view(&session).unwrap();
    assert!(replay.path_locks()[&id("assignment")].creation.is_some());
}

struct PreparationProvider {
    direct: Direct,
    entered: std::sync::mpsc::Sender<()>,
    proceed: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    fail_after_open: bool,
    partial_data: bool,
}
impl WorkspaceProvider for PreparationProvider {
    fn selection(&self) -> &ymp_domain::journal::PolicySelection {
        self.direct.selection()
    }
    fn location(&self) -> Result<WorkspaceLocation> {
        self.direct.location()
    }
    fn file_modes(&self) -> Vec<LockMode> {
        self.direct.file_modes()
    }
    fn bind(&self, journal: &dyn Journal) -> Result<WorkspaceBinding> {
        self.direct.bind(journal)
    }
    fn verify_binding(&self, binding: &WorkspaceBinding, journal: &dyn Journal) -> Result<()> {
        self.direct.verify_binding(binding, journal)
    }
    fn validate_paths(&self, paths: &[WorkspacePath]) -> Result<()> {
        self.direct.validate_paths(paths)
    }
    fn observe_paths(&self, paths: &[WorkspacePath]) -> Result<Vec<PathObservation>> {
        self.direct.observe_paths(paths)
    }
    fn capture(
        &self,
        journal: &Result<JournalIdentity>,
        store: &dyn ymp_kernel::journal::ContentStore,
    ) -> Result<SnapshotTree> {
        self.direct.capture(journal, store)
    }
    fn coordinate(
        &self,
        binding: &WorkspaceBinding,
    ) -> Result<Box<dyn ymp_kernel::ports::execution::WorkspaceCoordination + '_>> {
        self.direct.coordinate(binding)
    }
    fn validate_file_request(&self, access: &FileAccess) -> Result<()> {
        self.direct.validate_file_request(access)
    }
    fn prepare_file(
        &self,
        access: &FileAccess,
    ) -> Result<Box<dyn ymp_kernel::ports::execution::WorkspaceFile>> {
        let file = self.direct.prepare_file(access)?;
        self.entered.send(()).unwrap();
        self.proceed
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        if self.fail_after_open {
            return Err(Denial::new(
                "fixture_prepare",
                "Preparation failed after exclusive creation",
            ));
        }
        Ok(Box::new(DataFile {
            file,
            partial: self.partial_data,
        }))
    }
}
struct DataFile {
    file: Box<dyn ymp_kernel::ports::execution::WorkspaceFile>,
    partial: bool,
}
impl ymp_kernel::ports::execution::WorkspaceFile for DataFile {
    fn identity(&self) -> FileIdentity {
        self.file.identity()
    }
    fn read(&mut self) -> Result<Vec<u8>> {
        self.file.read()
    }
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if self.partial {
            self.file.write(&bytes[..bytes.len().min(2)])?;
            return Err(Denial::new(
                "fixture_data",
                "Data I/O failed after changing bytes",
            ));
        }
        self.file.write(bytes)
    }
}
#[test]
fn preparation_and_publication_exclude_other_resolution_but_not_later_disjoint_data_io() {
    for fail_after_open in [false, true] {
        let root = support::Directory::new();
        let database = support::Directory::new();
        fs::write(root.0.join("other"), b"before").unwrap();
        let journal = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (proceed_tx, proceed_rx) = std::sync::mpsc::channel();
        let provider = Arc::new(PreparationProvider {
            direct: Direct::open(&root.0, CaptureLimits::default()).unwrap(),
            entered: entered_tx,
            proceed: std::sync::Mutex::new(proceed_rx),
            fail_after_open,
            partial_data: false,
        });
        let (guard, profile) = fixture::open(
            journal.clone(),
            Arc::new(journal.content_store()),
            &id("creator"),
            provider.as_ref(),
        );
        guard
            .bind_workspace(&id("creator"), 4, 4, &id("workspace"), provider.as_ref())
            .unwrap();
        let handle = Arc::new(
            guard
                .mediate(
                    &id("creator"),
                    5,
                    5,
                    LockRequest {
                        assignment: id("creator"),
                        workspace: id("workspace"),
                        profile,
                        paths: vec![(path("file"), LockMode::Write)],
                    },
                    provider.clone(),
                )
                .unwrap(),
        );
        guard
            .authorize_access(
                &id("creator"),
                6,
                6,
                id("creator"),
                id("invocation"),
                provider.as_ref(),
            )
            .unwrap();
        let direct = Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap());
        let (other, profile) = fixture::open(
            journal.clone(),
            Arc::new(journal.content_store()),
            &id("other"),
            direct.as_ref(),
        );
        other
            .bind_workspace(&id("other"), 4, 4, &id("workspace"), direct.as_ref())
            .unwrap();
        let observer = other
            .mediate(
                &id("other"),
                5,
                5,
                LockRequest {
                    assignment: id("observer"),
                    workspace: id("workspace"),
                    profile,
                    paths: vec![(path("other"), LockMode::Write)],
                },
                direct.clone(),
            )
            .unwrap();
        other
            .authorize_access(
                &id("other"),
                6,
                6,
                id("observer"),
                id("observer-invocation"),
                direct.as_ref(),
            )
            .unwrap();
        let creator = {
            let handle = handle.clone();
            std::thread::spawn(move || handle.write(&path("file"), b"created data", 12))
        };
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(fs::read(root.0.join("file")).unwrap().is_empty());
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = observer.write(&path("other"), b"other data", 12);
            finished_tx.send(()).unwrap();
            (observer, result)
        });
        assert!(
            finished_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        assert_eq!(fs::read(root.0.join("other")).unwrap(), b"before");
        proceed_tx.send(()).unwrap();
        assert_eq!(creator.join().unwrap().is_err(), fail_after_open);
        let (observer, result) = worker.join().unwrap();
        assert_eq!(result.is_err(), fail_after_open);
        if fail_after_open {
            assert_eq!(result.unwrap_err().code, "file_creation_pending");
            assert!(handle.resolve_creation(12).is_err());
            let proof = guard.withdraw_mediated(&handle).unwrap();
            guard
                .release(
                    &id("creator"),
                    guard.view(&id("creator")).unwrap().revision(),
                    20,
                    &proof,
                )
                .unwrap();
            observer
                .write(&path("other"), b"after cessation", 12)
                .unwrap();
        } else {
            assert_eq!(fs::read(root.0.join("file")).unwrap(), b"created data");
            assert_eq!(fs::read(root.0.join("other")).unwrap(), b"other data");
        }
    }
}
#[test]
fn partial_data_failure_keeps_published_ownership_without_an_unresolved_creation() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let (entered_tx, _entered_rx) = std::sync::mpsc::channel();
    let (proceed_tx, proceed_rx) = std::sync::mpsc::channel();
    let provider = Arc::new(PreparationProvider {
        direct: Direct::open(&root.0, CaptureLimits::default()).unwrap(),
        entered: entered_tx,
        proceed: std::sync::Mutex::new(proceed_rx),
        fail_after_open: false,
        partial_data: true,
    });
    let (guard, profile) = fixture::open(
        journal.clone(),
        Arc::new(journal.content_store()),
        &id("creator"),
        provider.as_ref(),
    );
    guard
        .bind_workspace(&id("creator"), 4, 4, &id("workspace"), provider.as_ref())
        .unwrap();
    let handle = guard
        .mediate(
            &id("creator"),
            5,
            5,
            LockRequest {
                assignment: id("creator"),
                workspace: id("workspace"),
                profile,
                paths: vec![(path("file"), LockMode::Write)],
            },
            provider.clone(),
        )
        .unwrap();
    guard
        .authorize_access(
            &id("creator"),
            6,
            6,
            id("creator"),
            id("invocation"),
            provider.as_ref(),
        )
        .unwrap();
    proceed_tx.send(()).unwrap();
    assert_eq!(
        handle
            .write(&path("file"), b"partial", 12)
            .unwrap_err()
            .code,
        "fixture_data"
    );
    assert_eq!(fs::read(root.0.join("file")).unwrap(), b"pa");
    let view = guard.view(&id("creator")).unwrap();
    let owner = &view.path_locks()[&id("creator")];
    assert!(owner.creation.is_none());
    assert_eq!(owner.file_holds.len(), 1);
    assert!(owner.released.is_none());
    let proof = guard.withdraw_mediated(&handle).unwrap();
    guard
        .release(&id("creator"), view.revision(), 20, &proof)
        .unwrap();
}
