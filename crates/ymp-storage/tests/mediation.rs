//! Actual file operations through the kernel capability, without native execution.
mod support;
use ymp_kernel as kernel;
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use std::{
    collections::BTreeSet,
    fs,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
use ymp_domain::{
    Id, Result,
    identity::ExecutionProfile,
    journal::{Capability, PolicySelection},
    workspace::*,
};
use ymp_kernel::{
    journal::{ContentStore, Journal, ParameterSchemas},
    ports::execution::WorkspaceProvider,
    workspace_guard::{FileAccess, LockRequest, MediatedAccess, WorkspaceGuard},
    workspace_locks::Cessation,
};
use ymp_runtime::workspace::{binding::MARKER, direct::Direct, read_only::ReadOnly};
use ymp_storage::{content::SqliteContent, journal::SqliteJournal};
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn path(s: &str) -> WorkspacePath {
    WorkspacePath::new(s).unwrap()
}
type Guard = WorkspaceGuard<SqliteJournal, SqliteContent>;
struct Setup {
    guard: Arc<Guard>,
    journal: Arc<SqliteJournal>,
    provider: Arc<dyn WorkspaceProvider>,
    profile: ExecutionProfile,
    session: Id,
}
impl Setup {
    fn new(
        database: &support::Directory,
        provider: Arc<dyn WorkspaceProvider>,
        name: &str,
    ) -> Self {
        Self::capabilities(
            database,
            provider,
            name,
            BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]),
        )
    }
    fn capabilities(
        database: &support::Directory,
        provider: Arc<dyn WorkspaceProvider>,
        name: &str,
        capabilities: BTreeSet<Capability>,
    ) -> Self {
        let journal = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let session = id(name);
        let (guard, profile) =
            if capabilities == BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]) {
                fixture::open(
                    journal.clone(),
                    Arc::new(journal.content_store()),
                    &session,
                    provider.as_ref(),
                )
            } else {
                fixture::open_with_capabilities(
                    journal.clone(),
                    Arc::new(journal.content_store()),
                    &session,
                    provider.as_ref(),
                    capabilities,
                )
            };
        guard
            .bind_workspace(&session, 4, 4, &id("workspace"), provider.as_ref())
            .unwrap();
        Self {
            guard: Arc::new(guard),
            journal,
            provider,
            profile,
            session,
        }
    }
    fn revision(&self) -> u64 {
        self.guard.view(&self.session).unwrap().revision()
    }
    fn mediate(
        &self,
        assignment: &str,
        paths: &[(&str, LockMode)],
    ) -> Result<MediatedAccess<SqliteJournal>> {
        self.guard.mediate(
            &self.session,
            self.revision(),
            10,
            LockRequest {
                assignment: id(assignment),
                workspace: id("workspace"),
                profile: self.profile.clone(),
                paths: paths.iter().map(|(p, m)| (path(p), *m)).collect(),
            },
            self.provider.clone(),
        )
    }
    fn authorize(&self, assignment: &str) {
        self.guard
            .authorize_access(
                &self.session,
                self.revision(),
                11,
                id(assignment),
                id(&format!("invocation-{assignment}")),
                self.provider.as_ref(),
            )
            .unwrap();
    }
    fn release(&self, access: &MediatedAccess<SqliteJournal>) {
        let evidence = self.guard.withdraw_mediated(access).unwrap();
        self.guard
            .release(&self.session, self.revision(), 20, &evidence)
            .unwrap();
    }
}
fn direct(root: &support::Directory) -> Arc<dyn WorkspaceProvider> {
    Arc::new(Direct::open(&root.0, CaptureLimits::default()).unwrap())
}
#[test]
fn only_authorized_modes_and_paths_reach_real_files() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("input"), b"original").unwrap();
    let s = Setup::new(&database, direct(&root), "session");
    let a = s
        .mediate(
            "a",
            &[("input", LockMode::Read), ("output", LockMode::Write)],
        )
        .unwrap();
    assert!(a.read(&path("input"), 100, 12).is_err());
    assert!(a.write(&path("output"), b"too early", 12).is_err());
    assert!(!root.0.join("output").exists());
    let stale = s.guard.never_authorized(&s.session, &id("a")).unwrap();
    s.authorize("a");
    assert!(
        s.guard
            .release(&s.session, s.revision(), 12, &stale)
            .is_err()
    );
    assert_eq!(a.read(&path("input"), 100, 12).unwrap(), b"original");
    a.write(&path("output"), b"created", 12).unwrap();
    a.write(&path("output"), b"short", 12).unwrap();
    assert_eq!(fs::read(root.0.join("output")).unwrap(), b"short");
    assert_eq!(
        a.read(&path("output"), 100, 12).unwrap_err().code,
        "access_scope"
    );
    assert!(a.write(&path("input"), b"forbidden", 12).is_err());
    assert!(a.write(&path("elsewhere"), b"forbidden", 12).is_err());
    assert!(a.read(&path("input"), 2, 12).is_err());
    assert_eq!(fs::read(root.0.join("input")).unwrap(), b"original");
    s.release(&a);
    assert!(a.write(&path("output"), b"after release", 12).is_err());
    let state = s.guard.view(&s.session).unwrap();
    let basis = state.path_locks()[&id("a")].released.clone().unwrap();
    assert_eq!(basis.kind, Cessation::AccessWithdrawn);
    assert_eq!(basis.invocation, Some(id("invocation-a")));
    let reopened = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let replay = reopened
        .read(&s.session)
        .unwrap()
        .view_with_schemas(&s.session, None, reopened.schemas())
        .unwrap();
    assert_eq!(
        replay.path_locks()[&id("a")].released.as_ref(),
        Some(&basis)
    );
    assert!(replay.treasury().is_none());
}
#[test]
fn metadata_links_and_oversized_writes_are_refused_before_truncation() {
    let root = support::Directory::new();
    let outside = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("kept"), b"kept").unwrap();
    fs::write(outside.0.join("target"), b"outside").unwrap();
    let limits = CaptureLimits {
        max_file_bytes: 16,
        ..CaptureLimits::default()
    };
    let s = Setup::new(
        &database,
        Arc::new(Direct::open(&root.0, limits).unwrap()),
        "session",
    );
    let a = s
        .mediate("a", &[(".", LockMode::Read), (".", LockMode::Write)])
        .unwrap();
    s.authorize("a");
    let marker = fs::read(root.0.join(MARKER)).unwrap();
    for forbidden in [
        MARKER.to_owned(),
        MARKER.to_uppercase(),
        format!("{MARKER}.tmp-guest"),
    ] {
        assert!(a.write(&path(&forbidden), b"bad", 12).is_err());
        assert!(a.read(&path(&forbidden), 4096, 12).is_err());
    }
    assert_eq!(fs::read(root.0.join(MARKER)).unwrap(), marker);
    assert!(!root.0.join(format!("{MARKER}.tmp-guest")).exists());
    std::os::unix::fs::symlink(outside.0.join("target"), root.0.join("link")).unwrap();
    std::os::unix::fs::symlink(&outside.0, root.0.join("directory-link")).unwrap();
    fs::hard_link(outside.0.join("target"), root.0.join("hard")).unwrap();
    for forbidden in ["link", "hard", "directory-link/target"] {
        assert!(a.write(&path(forbidden), b"bad", 12).is_err());
        assert!(a.read(&path(forbidden), 4096, 12).is_err());
    }
    assert_eq!(fs::read(outside.0.join("target")).unwrap(), b"outside");
    assert!(a.write(&path("kept"), &[0; 17], 12).is_err());
    assert!(a.write(&path("new"), &[0; 17], 12).is_err());
    assert_eq!(fs::read(root.0.join("kept")).unwrap(), b"kept");
    assert!(!root.0.join("new").exists());
    assert!(WorkspacePath::new("../target").is_err());
    assert!(WorkspacePath::new("/target").is_err());
    s.release(&a);
}
#[test]
fn revocation_retains_conflicts_while_disjoint_operations_continue() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("shared"), b"read").unwrap();
    let first = Setup::new(&database, direct(&root), "first");
    let second = Setup::new(&database, direct(&root), "second");
    let r1 = first.mediate("r1", &[("shared", LockMode::Read)]).unwrap();
    let r2 = second.mediate("r2", &[("shared", LockMode::Read)]).unwrap();
    first.authorize("r1");
    second.authorize("r2");
    thread::scope(|scope| {
        let one = scope.spawn(|| r1.read(&path("shared"), 100, 12).unwrap());
        let two = scope.spawn(|| r2.read(&path("shared"), 100, 12).unwrap());
        assert_eq!(one.join().unwrap(), two.join().unwrap());
    });
    let a = first.mediate("a", &[("left", LockMode::Write)]).unwrap();
    let b = second.mediate("b", &[("right", LockMode::Write)]).unwrap();
    first.authorize("a");
    second.authorize("b");
    thread::scope(|scope| {
        let one = scope.spawn(|| a.write(&path("left"), b"left", 12));
        let two = scope.spawn(|| b.write(&path("right"), b"right", 12));
        one.join().unwrap().unwrap();
        two.join().unwrap().unwrap();
    });
    first
        .guard
        .revoke_access(
            &first.session,
            first.revision(),
            12,
            id("a"),
            "Stop requested".into(),
        )
        .unwrap();
    assert!(a.write(&path("left"), b"revoked", 12).is_err());
    let conflict = second
        .mediate("conflict", &[("left", LockMode::Write)])
        .err()
        .unwrap();
    assert!(!conflict.refs.is_empty());
    b.write(&path("right"), b"still usable", 12).unwrap();
    first.release(&a);
    let successor = second
        .mediate("successor", &[("left", LockMode::Write)])
        .unwrap();
    second.authorize("successor");
    successor.write(&path("left"), b"successor", 12).unwrap();
    first.release(&r1);
    second.release(&r2);
    second.release(&b);
    second.release(&successor);
    assert_eq!(fs::read(root.0.join("left")).unwrap(), b"successor");
}
#[test]
fn read_only_provider_rejects_write_admission_and_preserves_its_policy() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("input"), b"read only").unwrap();
    let s = Setup::new(
        &database,
        Arc::new(ReadOnly::open(&root.0, CaptureLimits::default()).unwrap()),
        "session",
    );
    let before = s.revision();
    assert!(s.mediate("writer", &[("input", LockMode::Write)]).is_err());
    assert_eq!(s.revision(), before);
    let a = s.mediate("reader", &[("input", LockMode::Read)]).unwrap();
    s.authorize("reader");
    assert_eq!(a.read(&path("input"), 100, 12).unwrap(), b"read only");
    assert!(a.write(&path("input"), b"bad", 12).is_err());
    s.guard
        .snapshot(
            &s.session,
            s.revision(),
            12,
            &id("workspace"),
            id("snapshot"),
            s.provider.as_ref(),
        )
        .unwrap();
    assert_eq!(
        s.guard
            .read_artifact(&s.session, &id("snapshot"), &path("input"))
            .unwrap(),
        b"read only"
    );
    let view = s.guard.view(&s.session).unwrap();
    assert_eq!(
        &view.workspaces()[&id("workspace")].provider,
        s.provider.selection()
    );
    assert_eq!(s.provider.selection().policy.implementation, "ReadOnly");
    s.release(&a);
}
#[test]
fn process_capability_cannot_be_narrowed_by_a_declared_file_scope() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let s = Setup::capabilities(
        &database,
        direct(&root),
        "session",
        BTreeSet::from([
            Capability::ReadFiles,
            Capability::WriteFiles,
            Capability::RunProcess,
        ]),
    );
    let before = s.revision();
    let denial = s
        .mediate("native-like", &[("narrow", LockMode::Write)])
        .err()
        .unwrap();
    assert_eq!(denial.code, "mediation_boundary");
    assert_eq!(s.revision(), before);
}
#[test]
fn unstarted_and_disconnected_handles_do_not_invent_cessation() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let s = Setup::new(&database, direct(&root), "session");
    let a = s
        .mediate("unstarted", &[("file", LockMode::Write)])
        .unwrap();
    s.release(&a);
    assert_eq!(
        s.guard.view(&s.session).unwrap().path_locks()[&id("unstarted")]
            .released
            .as_ref()
            .unwrap()
            .kind,
        Cessation::NeverAuthorized
    );
    let b = s
        .mediate("disconnected", &[("file", LockMode::Write)])
        .unwrap();
    s.authorize("disconnected");
    drop(b);
    let other_guard = WorkspaceGuard::new(s.journal.clone(), Arc::new(s.journal.content_store()));
    assert!(
        other_guard
            .never_authorized(&s.session, &id("disconnected"))
            .is_err()
    );
    assert!(s.mediate("conflict", &[("file", LockMode::Write)]).is_err());
    assert!(
        s.mediate("unrelated", &[("elsewhere", LockMode::Write)])
            .is_ok()
    );
}

/// Pauses one real provider call after kernel admission. No detached I/O survives return.
struct Gated {
    direct: Direct,
    operation_target: Option<Direct>,
    entered: mpsc::Sender<()>,
    continue_io: Arc<Mutex<mpsc::Receiver<()>>>,
}
impl WorkspaceProvider for Gated {
    fn selection(&self) -> &PolicySelection {
        self.direct.selection()
    }
    fn location(&self) -> Result<WorkspaceLocation> {
        self.direct.location()
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
        store: &dyn ContentStore,
    ) -> Result<SnapshotTree> {
        self.direct.capture(journal, store)
    }
    fn file_modes(&self) -> Vec<LockMode> {
        self.direct.file_modes()
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
        let file = self
            .operation_target
            .as_ref()
            .unwrap_or(&self.direct)
            .prepare_file(access)?;
        Ok(Box::new(GatedFile {
            file,
            entered: self.entered.clone(),
            continue_io: self.continue_io.clone(),
        }))
    }
}
struct GatedFile {
    file: Box<dyn ymp_kernel::ports::execution::WorkspaceFile>,
    entered: mpsc::Sender<()>,
    continue_io: Arc<Mutex<mpsc::Receiver<()>>>,
}
impl ymp_kernel::ports::execution::WorkspaceFile for GatedFile {
    fn identity(&self) -> FileIdentity {
        self.file.identity()
    }
    fn read(&mut self) -> Result<Vec<u8>> {
        self.file.read()
    }
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.entered.send(()).unwrap();
        self.continue_io
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        self.file.write(bytes)
    }
}
#[test]
fn withdrawal_drains_active_io_and_denies_queued_operations_before_release() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let provider = Arc::new(Gated {
        operation_target: None,
        direct: Direct::open(&root.0, CaptureLimits::default()).unwrap(),
        entered: entered_tx,
        continue_io: Arc::new(Mutex::new(continue_rx)),
    });
    let s = Setup::new(&database, provider, "session");
    let a = Arc::new(s.mediate("writer", &[("file", LockMode::Write)]).unwrap());
    s.authorize("writer");
    let active = {
        let a = a.clone();
        thread::spawn(move || a.write(&path("file"), b"admitted", 12))
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (queued_tx, queued_rx) = mpsc::channel();
    let queued = {
        let a = a.clone();
        thread::spawn(move || {
            queued_tx.send(()).unwrap();
            a.write(&path("file"), b"queued", 12)
        })
    };
    queued_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    a.request_withdrawal();
    let (withdrawn_tx, withdrawn_rx) = mpsc::channel();
    let withdrawn = {
        let a = a.clone();
        let guard = s.guard.clone();
        thread::spawn(move || {
            let proof = guard.withdraw_mediated(&a);
            withdrawn_tx.send(()).unwrap();
            proof
        })
    };
    assert!(
        withdrawn_rx
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
    assert!(s.mediate("conflict", &[("file", LockMode::Write)]).is_err());
    continue_tx.send(()).unwrap();
    active.join().unwrap().unwrap();
    assert!(queued.join().unwrap().is_err());
    let proof = withdrawn.join().unwrap().unwrap();
    assert_eq!(fs::read(root.0.join("file")).unwrap(), b"admitted");
    s.guard
        .release(&s.session, s.revision(), 20, &proof)
        .unwrap();
    assert!(s.mediate("next", &[("file", LockMode::Write)]).is_ok());
}

#[test]
fn replacing_a_scoped_ancestor_or_file_cannot_redirect_an_authorized_operation() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::create_dir(root.0.join("scope")).unwrap();
    fs::write(root.0.join("scope/file"), b"original").unwrap();
    fs::write(root.0.join("leaf"), b"leaf").unwrap();
    let s = Setup::new(&database, direct(&root), "session");
    let a = s
        .mediate(
            "directory",
            &[("scope", LockMode::Read), ("scope", LockMode::Write)],
        )
        .unwrap();
    let b = s
        .mediate(
            "leaf",
            &[("leaf", LockMode::Read), ("leaf", LockMode::Write)],
        )
        .unwrap();
    s.authorize("directory");
    s.authorize("leaf");
    fs::rename(root.0.join("scope"), root.0.join("moved")).unwrap();
    fs::create_dir(root.0.join("scope")).unwrap();
    fs::write(root.0.join("scope/file"), b"replacement").unwrap();
    assert!(a.write(&path("scope/file"), b"wrong", 12).is_err());
    assert!(a.read(&path("scope/file"), 100, 12).is_err());
    assert_eq!(fs::read(root.0.join("scope/file")).unwrap(), b"replacement");
    assert_eq!(fs::read(root.0.join("moved/file")).unwrap(), b"original");
    fs::rename(root.0.join("leaf"), root.0.join("moved-leaf")).unwrap();
    assert!(b.write(&path("leaf"), b"recreate", 12).is_err());
    assert!(!root.0.join("leaf").exists());
    fs::write(root.0.join("leaf"), b"new leaf").unwrap();
    assert!(b.write(&path("leaf"), b"wrong", 12).is_err());
    assert!(b.read(&path("leaf"), 100, 12).is_err());
    assert_eq!(fs::read(root.0.join("leaf")).unwrap(), b"new leaf");
    s.release(&a);
    s.release(&b);
}

#[test]
fn an_operation_cannot_be_forwarded_to_another_root_in_the_same_journal() {
    let root = support::Directory::new();
    let other_root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(other_root.0.join("file"), b"other root").unwrap();
    fs::write(root.0.join("file"), b"first root").unwrap();
    let journal = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let other = Direct::open(&other_root.0, CaptureLimits::default()).unwrap();
    other.bind(&journal).unwrap();
    let (entered_tx, _entered_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let provider = Arc::new(Gated {
        direct: Direct::open(&root.0, CaptureLimits::default()).unwrap(),
        operation_target: Some(other),
        entered: entered_tx,
        continue_io: Arc::new(Mutex::new(continue_rx)),
    });
    let s = Setup::new(&database, provider, "session");
    let a = s
        .mediate(
            "writer",
            &[("file", LockMode::Read), ("file", LockMode::Write)],
        )
        .unwrap();
    s.authorize("writer");
    continue_tx.send(()).unwrap();
    assert_eq!(
        a.write(&path("file"), b"wrong root", 12).unwrap_err().code,
        "binding_conflict"
    );
    assert_eq!(
        a.read(&path("file"), 100, 12).unwrap_err().code,
        "binding_conflict"
    );
    assert_eq!(fs::read(other_root.0.join("file")).unwrap(), b"other root");
    assert_eq!(fs::read(root.0.join("file")).unwrap(), b"first root");
    s.release(&a);
}
#[test]
fn interrupted_provider_io_retains_ownership_and_cannot_certify_withdrawal() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let provider = Arc::new(Gated {
        direct: Direct::open(&root.0, CaptureLimits::default()).unwrap(),
        operation_target: None,
        entered: entered_tx,
        continue_io: Arc::new(Mutex::new(continue_rx)),
    });
    let s = Setup::new(&database, provider, "session");
    let a = Arc::new(s.mediate("writer", &[("file", LockMode::Write)]).unwrap());
    s.authorize("writer");
    let active = {
        let a = a.clone();
        thread::spawn(move || a.write(&path("file"), b"interrupted", 12))
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    // A closed fixture channel panics inside the admitted provider operation.
    drop(continue_tx);
    assert!(active.join().is_err());
    assert!(s.guard.withdraw_mediated(&a).is_err());
    assert!(s.mediate("conflict", &[("file", LockMode::Write)]).is_err());
    assert!(
        s.mediate("unrelated", &[("other", LockMode::Write)])
            .is_ok()
    );
}

#[test]
fn originally_missing_scopes_cannot_adopt_foreign_files_or_directories() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::create_dir(root.0.join("old")).unwrap();
    fs::write(root.0.join("old/file"), b"held elsewhere").unwrap();
    let s = Setup::new(&database, direct(&root), "session");
    let a = s
        .mediate("missing-directory", &[("new/file", LockMode::Write)])
        .unwrap();
    let b = s
        .mediate("original-owner", &[("old/file", LockMode::Write)])
        .unwrap();
    let c = s
        .mediate(
            "missing-file",
            &[("leaf", LockMode::Read), ("leaf", LockMode::Write)],
        )
        .unwrap();
    s.authorize("missing-directory");
    s.authorize("original-owner");
    s.authorize("missing-file");
    fs::rename(root.0.join("old"), root.0.join("new")).unwrap();
    assert!(a.write(&path("new/file"), b"stolen", 12).is_err());
    assert_eq!(
        fs::read(root.0.join("new/file")).unwrap(),
        b"held elsewhere"
    );
    fs::rename(root.0.join("new/file"), root.0.join("leaf")).unwrap();
    assert!(c.write(&path("leaf"), b"stolen", 12).is_err());
    assert!(c.read(&path("leaf"), 100, 12).is_err());
    assert_eq!(fs::read(root.0.join("leaf")).unwrap(), b"held elsewhere");
    fs::rename(root.0.join("leaf"), root.0.join("foreign")).unwrap();
    c.write(&path("leaf"), b"own first write", 12).unwrap();
    c.write(&path("leaf"), b"own second write", 12).unwrap();
    assert_eq!(c.read(&path("leaf"), 100, 12).unwrap(), b"own second write");
    fs::rename(root.0.join("leaf"), root.0.join("own-moved")).unwrap();
    fs::rename(root.0.join("foreign"), root.0.join("leaf")).unwrap();
    assert!(c.write(&path("leaf"), b"stolen after create", 12).is_err());
    assert!(c.read(&path("leaf"), 100, 12).is_err());
    assert_eq!(fs::read(root.0.join("leaf")).unwrap(), b"held elsewhere");
    s.release(&a);
    s.release(&b);
    s.release(&c);
}

#[test]
fn moving_another_owners_file_beneath_a_directory_scope_preserves_its_hold() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::create_dir(root.0.join("a")).unwrap();
    fs::create_dir(root.0.join("b")).unwrap();
    fs::write(root.0.join("b/file"), b"other owner").unwrap();
    let first = Setup::new(&database, direct(&root), "first");
    let second = Setup::new(&database, direct(&root), "second");
    let a = first
        .mediate(
            "directory",
            &[("a", LockMode::Write), ("a", LockMode::Read)],
        )
        .unwrap();
    let b = second
        .mediate("file", &[("b/file", LockMode::Write)])
        .unwrap();
    first.authorize("directory");
    second.authorize("file");
    fs::rename(root.0.join("b"), root.0.join("a/moved")).unwrap();
    let denied = a.write(&path("a/moved/file"), b"stolen", 12).unwrap_err();
    assert_eq!(denied.code, "path_conflict");
    assert!(!denied.refs.is_empty());
    assert!(a.read(&path("a/moved/file"), 100, 12).is_err());
    assert_eq!(
        fs::read(root.0.join("a/moved/file")).unwrap(),
        b"other owner"
    );
    second.release(&b);
    a.write(&path("a/moved/file"), b"after release", 12)
        .unwrap();
    first.release(&a);
}

#[test]
fn created_file_ownership_survives_restart_and_a_move_into_another_scope() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::create_dir(root.0.join("a")).unwrap();
    let first = Setup::new(&database, direct(&root), "first");
    let second = Setup::new(&database, direct(&root), "second");
    let a = first
        .mediate("directory", &[("a", LockMode::Write)])
        .unwrap();
    let b = second
        .mediate("creator", &[("created", LockMode::Write)])
        .unwrap();
    first.authorize("directory");
    second.authorize("creator");
    b.write(&path("created"), b"owned after create", 12)
        .unwrap();
    fs::rename(root.0.join("created"), root.0.join("a/moved")).unwrap();
    let reopened = SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
    let replay = reopened
        .read(&second.session)
        .unwrap()
        .view_with_schemas(&second.session, None, reopened.schemas())
        .unwrap();
    assert_eq!(replay.path_locks()[&id("creator")].file_holds.len(), 1);
    assert_eq!(
        a.write(&path("a/moved"), b"stolen", 12).unwrap_err().code,
        "path_conflict"
    );
    assert_eq!(
        fs::read(root.0.join("a/moved")).unwrap(),
        b"owned after create"
    );
    second.release(&b);
    a.write(&path("a/moved"), b"after release", 12).unwrap();
    first.release(&a);
}
#[test]
fn an_open_existing_file_keeps_physical_ownership_when_moved_out_of_its_directory() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::create_dir(root.0.join("a")).unwrap();
    fs::create_dir(root.0.join("b")).unwrap();
    fs::write(root.0.join("a/file"), b"before").unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let first = Setup::new(
        &database,
        Arc::new(Gated {
            direct: Direct::open(&root.0, CaptureLimits::default()).unwrap(),
            operation_target: None,
            entered: entered_tx,
            continue_io: Arc::new(Mutex::new(continue_rx)),
        }),
        "first",
    );
    let second = Setup::new(&database, direct(&root), "second");
    let a = Arc::new(
        first
            .mediate("directory", &[("a", LockMode::Write)])
            .unwrap(),
    );
    first.authorize("directory");
    let writer = {
        let a = a.clone();
        thread::spawn(move || a.write(&path("a/file"), b"pinned write", 12))
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    fs::rename(root.0.join("a/file"), root.0.join("b/file")).unwrap();
    let denied = second
        .mediate("new-owner", &[("b/file", LockMode::Write)])
        .err()
        .unwrap();
    assert_eq!(denied.code, "path_conflict");
    assert!(!denied.refs.is_empty());
    continue_tx.send(()).unwrap();
    writer.join().unwrap().unwrap();
    assert_eq!(fs::read(root.0.join("b/file")).unwrap(), b"pinned write");
    first.release(&a);
    assert!(
        second
            .mediate("new-owner", &[("b/file", LockMode::Write)])
            .is_ok()
    );
}
