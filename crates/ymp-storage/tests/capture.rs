mod support;
use ymp_kernel as kernel;
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use std::{
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};
use ymp_domain::{
    Denial, Id, Result,
    journal::{Actor, Envelope, PolicySelection},
    workspace::*,
};
use ymp_kernel::{
    events::Event,
    journal::{
        AppendResolution, ContentStore, Journal, JournalRead, ParameterSchemas, WorkspaceInventory,
        resolve_append, validate_append,
    },
    ports::execution::WorkspaceProvider,
    workspace_guard::WorkspaceGuard,
    workspace_locks::{LockChange, attribution},
};
use ymp_runtime::workspace::direct::Direct;
use ymp_storage::journal::SqliteJournal;
fn id<T>(value: &str) -> Id<T> {
    Id::new(value).unwrap()
}
struct CaptureProvider {
    direct: Direct,
    calls: AtomicUsize,
    fail: bool,
    entered: Option<mpsc::Sender<()>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}
impl CaptureProvider {
    fn new(path: &std::path::Path, fail: bool) -> Self {
        Self {
            direct: Direct::open(path, CaptureLimits::default()).unwrap(),
            calls: AtomicUsize::new(0),
            fail,
            entered: None,
            release: Mutex::new(None),
        }
    }
}
impl WorkspaceProvider for CaptureProvider {
    fn selection(&self) -> &PolicySelection {
        self.direct.selection()
    }
    fn location(&self) -> Result<WorkspaceLocation> {
        self.direct.location()
    }
    fn validate_paths(&self, paths: &[WorkspacePath]) -> Result<()> {
        self.direct.validate_paths(paths)
    }
    fn observe_paths(&self, paths: &[WorkspacePath]) -> Result<Vec<PathObservation>> {
        self.direct.observe_paths(paths)
    }
    fn capture(&self, store: &dyn ContentStore) -> Result<SnapshotTree> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(entered) = &self.entered {
            entered.send(()).unwrap();
        }
        if let Some(release) = self.release.lock().unwrap().take() {
            release
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
        }
        if self.fail {
            return Err(Denial::new(
                "fixture_capture",
                "Synchronous capture ended unsuccessfully",
            ));
        }
        self.direct.capture(store)
    }
}
#[test]
fn a_capture_holds_read_access_until_completion_and_blocks_a_different_session_writer() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    fs::write(root.0.join("file"), b"stable").unwrap();
    let (entered, ready) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let mut provider = CaptureProvider::new(&root.0, false);
    provider.entered = Some(entered);
    *provider.release.lock().unwrap() = Some(wait);
    let provider = Arc::new(provider);
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let (guard, _) = fixture::open(
        journal.clone(),
        Arc::new(journal.content_store()),
        &id("capture"),
        provider.as_ref(),
    );
    fixture::open(
        journal.clone(),
        Arc::new(journal.content_store()),
        &id("writer"),
        provider.as_ref(),
    );
    let guard = Arc::new(guard);
    let worker_guard = guard.clone();
    let worker_provider = provider.clone();
    let worker = thread::spawn(move || {
        worker_guard.snapshot(
            &id("capture"),
            4,
            5,
            &id("workspace"),
            id("snapshot"),
            worker_provider.as_ref(),
        )
    });
    ready
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let writer = fixture::acquire(
        journal.as_ref(),
        &id("writer"),
        provider.as_ref(),
        "file",
        LockMode::Write,
    );
    assert_eq!(
        journal
            .append(&id("writer"), 4, std::slice::from_ref(&writer))
            .unwrap_err()
            .code,
        "path_conflict"
    );
    assert_eq!(
        guard
            .abandon_capture(&id("capture"), 5, 6, &id("snapshot"), "Too early".into())
            .unwrap_err()
            .code,
        "capture_active"
    );
    release.send(()).unwrap();
    assert!(worker.join().unwrap().is_ok());
    journal.append(&id("writer"), 4, &[writer]).unwrap();
    let before = provider.calls.load(Ordering::SeqCst);
    assert_eq!(
        guard
            .snapshot(
                &id("capture"),
                6,
                7,
                &id("workspace"),
                id("blocked"),
                provider.as_ref()
            )
            .unwrap_err()
            .code,
        "path_conflict"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), before);
    assert!(
        guard.view(&id("capture")).unwrap().capture_reads()[&id("snapshot")]
            .ended
            .is_some()
    );
}
struct CompletionFault {
    inner: Arc<SqliteJournal>,
    armed: AtomicBool,
    after_commit: bool,
}
impl Journal for CompletionFault {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        self.inner.read(session)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        let completion = events.iter().any(|e| {
            matches!(
                e.payload,
                Event::SnapshotTaken { version: 2, .. }
                    | Event::LockChanged {
                        change: LockChange::CaptureAborted { .. },
                        ..
                    }
            )
        });
        if completion && self.armed.swap(false, Ordering::SeqCst) {
            if self.after_commit {
                self.inner.append(session, expected, events)?;
                return Err(Denial::new(
                    "storage_indeterminate",
                    "Fixture lost completion acknowledgement",
                ));
            }
            // Advance the same session with a compatible read hold, then expose a real CAS failure.
            let view =
                self.inner
                    .read(session)?
                    .view_with_schemas(session, None, self.schemas())?;
            let source = view
                .capture_reads()
                .values()
                .find(|c| c.ended.is_none())
                .unwrap();
            let change = LockChange::CaptureStarted {
                owner: ymp_domain::Digest::of(b"a different synthetic capture owner"),
                snapshot: id("concurrent-read"),
                workspace: source.workspace.clone(),
                observation: source.observation.clone(),
            };
            let other = Envelope {
                seq: expected + 1,
                session: session.clone(),
                at: events[0].at,
                actor: Actor::Runtime,
                policy: None,
                input: None,
                refs: attribution(&view, &change)?,
                payload: Event::LockChanged { version: 1, change },
            };
            self.inner.append(session, expected, &[other])?;
        }
        self.inner.append(session, expected, events)
    }
}
#[test]
fn completed_io_survives_cas_failures_and_lost_acknowledgements_without_recapture() {
    for (fail_capture, after_commit) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let root = support::Directory::new();
        let database = support::Directory::new();
        fs::write(root.0.join("file"), b"retained").unwrap();
        let provider = CaptureProvider::new(&root.0, fail_capture);
        let sqlite = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let journal = Arc::new(CompletionFault {
            inner: sqlite.clone(),
            armed: AtomicBool::new(true),
            after_commit,
        });
        let (guard, _) = fixture::open(
            journal.clone(),
            Arc::new(sqlite.content_store()),
            &id("session"),
            &provider,
        );
        assert!(
            guard
                .snapshot(
                    &id("session"),
                    4,
                    5,
                    &id("workspace"),
                    id("snapshot"),
                    &provider
                )
                .is_err()
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        let view = guard.view(&id("session")).unwrap();
        assert_eq!(
            view.capture_reads()[&id("snapshot")].ended.is_some(),
            after_commit
        );
        if !after_commit {
            let fresh = WorkspaceGuard::new(journal.clone(), Arc::new(sqlite.content_store()));
            assert_eq!(
                fresh
                    .resolve_capture(&id("session"), view.revision(), 7, &id("snapshot"))
                    .unwrap_err()
                    .code,
                "capture_evidence"
            );
        }
        let revision = guard
            .resolve_capture(&id("session"), view.revision(), 8, &id("snapshot"))
            .unwrap();
        assert_eq!(
            guard
                .resolve_capture(&id("session"), revision, 9, &id("snapshot"))
                .unwrap(),
            revision
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        let view = guard.view(&id("session")).unwrap();
        assert!(view.capture_reads()[&id("snapshot")].ended.is_some());
        assert_eq!(
            view.snapshots().contains_key(&id("snapshot")),
            !fail_capture
        );
        if !fail_capture {
            assert_eq!(
                guard
                    .read_artifact(
                        &id("session"),
                        &id("snapshot"),
                        &WorkspacePath::new("file").unwrap()
                    )
                    .unwrap(),
                b"retained"
            );
        }
        assert!(
            guard
                .snapshot(
                    &id("session"),
                    view.revision(),
                    10,
                    &id("workspace"),
                    id("snapshot"),
                    &provider
                )
                .is_err()
        );
    }
}
#[test]
fn completed_but_unpublished_capture_can_be_abandoned_and_its_hold_survives_restart() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = CaptureProvider::new(&root.0, false);
    let sqlite =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let journal = Arc::new(CompletionFault {
        inner: sqlite.clone(),
        armed: AtomicBool::new(true),
        after_commit: false,
    });
    let (guard, _) = fixture::open(
        journal.clone(),
        Arc::new(sqlite.content_store()),
        &id("session"),
        &provider,
    );
    assert!(
        guard
            .snapshot(
                &id("session"),
                4,
                5,
                &id("workspace"),
                id("snapshot"),
                &provider
            )
            .is_err()
    );
    let before = guard.view(&id("session")).unwrap();
    let reopened =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let fresh = WorkspaceGuard::new(reopened.clone(), Arc::new(reopened.content_store()));
    assert_eq!(fresh.view(&id("session")).unwrap(), before);
    assert_eq!(
        fresh
            .abandon_capture(
                &id("session"),
                before.revision(),
                8,
                &id("snapshot"),
                "No evidence".into()
            )
            .unwrap_err()
            .code,
        "capture_evidence"
    );
    guard
        .abandon_capture(
            &id("session"),
            before.revision(),
            8,
            &id("snapshot"),
            "Publication intentionally abandoned".into(),
        )
        .unwrap();
    let after = guard.view(&id("session")).unwrap();
    assert!(!after.snapshots().contains_key(&id("snapshot")));
    assert!(
        after.capture_reads()[&id("snapshot")]
            .failure
            .as_ref()
            .unwrap()
            .contains("capture_abandoned")
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn historical_v1_snapshots_remain_replayable_and_resolvable_but_cannot_be_new_appends() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = CaptureProvider::new(&root.0, false);
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let (guard, _) = fixture::open(
        journal.clone(),
        Arc::new(journal.content_store()),
        &id("session"),
        &provider,
    );
    let prefix = journal.read(&id("session")).unwrap();
    guard
        .snapshot(
            &id("session"),
            4,
            5,
            &id("workspace"),
            id("snapshot"),
            &provider,
        )
        .unwrap();
    let view = guard.view(&id("session")).unwrap();
    let snapshot = view.snapshots()[&id("snapshot")].clone();
    let workspace = &view.workspaces()[&id("workspace")];
    let old = Envelope {
        seq: 5,
        session: id("session"),
        at: 5,
        actor: Actor::Runtime,
        policy: Some(workspace.provider.policy.clone()),
        input: None,
        refs: vec![workspace.reference().unwrap()],
        payload: Event::SnapshotTaken {
            version: 1,
            snapshot: Box::new(snapshot.clone()),
        },
    };
    let mut historical = prefix.clone();
    historical.events.push(old.clone());
    historical.revision += 1;
    let replay = historical.view(&id("session"), None).unwrap();
    assert_eq!(replay.snapshots()[&id("snapshot")], snapshot);
    assert!(replay.capture_reads().is_empty());
    assert_eq!(
        resolve_append(
            &historical,
            &id("session"),
            4,
            std::slice::from_ref(&old),
            journal.schemas()
        )
        .unwrap(),
        AppendResolution::Committed(5)
    );
    assert_eq!(
        validate_append(
            &prefix,
            &id("session"),
            4,
            std::slice::from_ref(&old),
            journal.schemas()
        )
        .unwrap_err()
        .code,
        "snapshot_version"
    );
    let mut missing_hold = old;
    if let Event::SnapshotTaken { version, .. } = &mut missing_hold.payload {
        *version = 2;
    }
    assert_eq!(
        validate_append(
            &prefix,
            &id("session"),
            4,
            &[missing_hold],
            journal.schemas()
        )
        .unwrap_err()
        .code,
        "capture_missing"
    );
}

#[test]
fn exact_commit_resolution_cannot_begin_inside_an_incomplete_intake_refinement() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = CaptureProvider::new(&root.0, false);
    let journal =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let (guard, _) = fixture::open(
        journal.clone(),
        Arc::new(journal.content_store()),
        &id("session"),
        &provider,
    );
    let view = guard.view(&id("session")).unwrap();
    let previous = view.contract().unwrap().reference();
    let clarification = ymp_domain::task::Clarification {
        question: "Which result?".into(),
        answer: "The explicit one".into(),
        at: 5,
    };
    let mut task = view.task().unwrap().clone();
    task.goal.clarifications.push(clarification.clone());
    let criteria = view.criteria().to_vec();
    let contract = ymp_domain::task::AcceptanceContract::new(&task, &criteria, vec![]).unwrap();
    let first = Envelope {
        seq: 5,
        session: id("session"),
        at: 5,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: vec![previous.clone()],
        payload: Event::ClarificationRecorded {
            version: 1,
            clarification,
        },
    };
    let second = Envelope {
        seq: 6,
        session: id("session"),
        at: 5,
        actor: Actor::Runtime,
        policy: None,
        input: None,
        refs: vec![previous.clone()],
        payload: Event::CriteriaCommitted {
            version: 1,
            data: Box::new(kernel::events::CriteriaCommitted {
                task,
                contract,
                criteria,
                previous: Some(previous),
                reason: "Explicit clarification".into(),
            }),
        },
    };
    journal
        .append(&id("session"), 4, &[first.clone(), second.clone()])
        .unwrap();
    let current = journal.read(&id("session")).unwrap();
    assert!(current.view(&id("session"), Some(5)).is_err());
    assert!(
        resolve_append(
            &current,
            &id("session"),
            5,
            std::slice::from_ref(&second),
            journal.schemas()
        )
        .is_err()
    );
    assert_eq!(
        resolve_append(
            &current,
            &id("session"),
            4,
            &[first, second],
            journal.schemas()
        )
        .unwrap(),
        AppendResolution::Committed(6)
    );
}

struct BeginRace {
    inner: Arc<SqliteJournal>,
    barrier: std::sync::Barrier,
    resolve_existing: bool,
    starts: AtomicUsize,
    committed: (Mutex<bool>, std::sync::Condvar),
}
impl Journal for BeginRace {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        self.inner.read(session)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        if events.iter().any(|e|matches!(&e.payload,Event::LockChanged {change:LockChange::CaptureStarted {snapshot,..},..} if snapshot==&id("race"))) {
            let ordinal=self.starts.fetch_add(1,Ordering::SeqCst);
            self.barrier.wait();
            if self.resolve_existing {
                if ordinal==0 {
                    let result=self.inner.append(session,expected,events);
                    *self.committed.0.lock().unwrap()=true;self.committed.1.notify_all();
                    return result;
                }
                let committed=self.committed.0.lock().unwrap();
                let (committed,_)=self.committed.1.wait_timeout_while(committed,std::time::Duration::from_secs(10),|v|!*v).unwrap();
                assert!(*committed);
                return match self.inner.resolve_append(session,expected,events)? {
                    AppendResolution::Committed(revision)=>Ok(revision),
                    _=>Err(Denial::new("stale_revision","A different begin committed after an uncertain local attempt")),
                };
            }
        }
        self.inner.append(session, expected, events)
    }
}
#[test]
fn identical_requests_from_two_guards_cannot_abort_the_winners_live_capture() {
    for resolve_existing in [false, true] {
        race_identical_capture_requests(resolve_existing);
    }
}
fn race_identical_capture_requests(resolve_existing: bool) {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let (entered, ready) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let mut provider = CaptureProvider::new(&root.0, false);
    provider.entered = Some(entered);
    *provider.release.lock().unwrap() = Some(wait);
    let provider = Arc::new(provider);
    let sqlite =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    fixture::open(
        sqlite.clone(),
        Arc::new(sqlite.content_store()),
        &id("session"),
        provider.as_ref(),
    );
    fixture::open(
        sqlite.clone(),
        Arc::new(sqlite.content_store()),
        &id("writer"),
        provider.as_ref(),
    );
    let journal = Arc::new(BeginRace {
        inner: sqlite.clone(),
        barrier: std::sync::Barrier::new(2),
        resolve_existing,
        starts: AtomicUsize::new(0),
        committed: (Mutex::new(false), std::sync::Condvar::new()),
    });
    let guards: Vec<_> = (0..2)
        .map(|_| {
            Arc::new(WorkspaceGuard::new(
                journal.clone(),
                Arc::new(sqlite.content_store()),
            ))
        })
        .collect();
    let (finished, result) = mpsc::channel();
    let workers: Vec<_> = guards
        .iter()
        .enumerate()
        .map(|(index, guard)| {
            let guard = guard.clone();
            let provider = provider.clone();
            let finished = finished.clone();
            thread::spawn(move || {
                let value = guard.snapshot(
                    &id("session"),
                    4,
                    5,
                    &id("workspace"),
                    id("race"),
                    provider.as_ref(),
                );
                finished.send((index, value)).unwrap();
            })
        })
        .collect();
    ready
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let (loser, error) = result
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    assert_eq!(error.unwrap_err().code, "stale_revision");
    let view = guards[loser].view(&id("session")).unwrap();
    assert!(view.capture_reads()[&id("race")].ended.is_none());
    assert_eq!(
        guards[loser]
            .resolve_capture(&id("session"), view.revision(), 6, &id("race"))
            .unwrap_err()
            .code,
        "capture_evidence"
    );
    let writer = fixture::acquire(
        sqlite.as_ref(),
        &id("writer"),
        provider.as_ref(),
        "file",
        LockMode::Write,
    );
    assert_eq!(
        sqlite.append(&id("writer"), 4, &[writer]).unwrap_err().code,
        "path_conflict"
    );
    release.send(()).unwrap();
    for worker in workers {
        worker.join().unwrap();
    }
    assert!(
        result
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
            .1
            .is_ok()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}
struct BeginLostAck {
    inner: Arc<SqliteJournal>,
    armed: AtomicBool,
}
impl Journal for BeginLostAck {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        self.inner.read(session)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        let revision = self.inner.append(session, expected, events)?;
        if events.iter().any(|e| {
            matches!(
                e.payload,
                Event::LockChanged {
                    change: LockChange::CaptureStarted { .. },
                    ..
                }
            )
        }) && self.armed.swap(false, Ordering::SeqCst)
        {
            return Err(Denial::new(
                "storage_indeterminate",
                "Fixture lost begin acknowledgement",
            ));
        }
        Ok(revision)
    }
}
#[test]
fn an_unacknowledged_begin_can_only_abort_its_own_nonce_bound_unstarted_capture() {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = CaptureProvider::new(&root.0, false);
    let sqlite =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let journal = Arc::new(BeginLostAck {
        inner: sqlite.clone(),
        armed: AtomicBool::new(true),
    });
    let (guard, _) = fixture::open(
        journal,
        Arc::new(sqlite.content_store()),
        &id("session"),
        &provider,
    );
    assert_eq!(
        guard
            .snapshot(
                &id("session"),
                4,
                5,
                &id("workspace"),
                id("unknown"),
                &provider
            )
            .unwrap_err()
            .code,
        "storage_indeterminate"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let view = guard.view(&id("session")).unwrap();
    assert!(view.capture_reads()[&id("unknown")].ended.is_some());
    assert!(
        view.capture_reads()[&id("unknown")]
            .failure
            .as_ref()
            .unwrap()
            .contains("storage_indeterminate")
    );
    assert!(view.snapshots().is_empty());
    assert_eq!(
        guard
            .abandon_capture(
                &id("session"),
                view.revision(),
                6,
                &id("unknown"),
                "No local I/O".into()
            )
            .unwrap_err()
            .code,
        "capture_state"
    );
}

struct ForeignBegins {
    inner: Arc<SqliteJournal>,
    remaining: AtomicUsize,
    unavailable: AtomicBool,
    delay_read: bool,
}
impl Journal for ForeignBegins {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn read(&self, session: &Id) -> Result<JournalRead> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(Denial::new(
                "fixture_read",
                "Read temporarily unavailable after failed begin",
            ));
        }
        self.inner.read(session)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64> {
        if events.len() == 1
            && matches!(
                events[0].payload,
                Event::LockChanged {
                    change: LockChange::CaptureStarted { .. },
                    ..
                }
            )
            && self
                .remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_sub(1))
                .is_ok()
        {
            let mut foreign = events[0].clone();
            if let Event::LockChanged {
                change:
                    LockChange::CaptureStarted {
                        owner, snapshot, ..
                    },
                ..
            } = &mut foreign.payload
            {
                *owner = ymp_domain::Digest::of(snapshot.as_str().as_bytes());
            }
            self.inner.append(session, expected, &[foreign])?;
            if self.delay_read {
                self.unavailable.store(true, Ordering::SeqCst);
            }
            return Err(Denial::new(
                "stale_revision",
                "A foreign capture attempt committed",
            ));
        }
        self.inner.append(session, expected, events)
    }
}
#[test]
fn losing_foreign_begins_does_not_exhaust_local_completion_capacity_or_abort_their_holds() {
    for delay_read in [false, true] {
        losing_foreign_begin_cleanup(delay_read);
    }
}
fn losing_foreign_begin_cleanup(delay_read: bool) {
    let root = support::Directory::new();
    let database = support::Directory::new();
    let provider = CaptureProvider::new(&root.0, false);
    let sqlite =
        Arc::new(SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap());
    let journal = Arc::new(ForeignBegins {
        inner: sqlite.clone(),
        remaining: AtomicUsize::new(64),
        unavailable: AtomicBool::new(false),
        delay_read,
    });
    let (guard, _) = fixture::open(
        journal.clone(),
        Arc::new(sqlite.content_store()),
        &id("session"),
        &provider,
    );
    for index in 0..64 {
        let snapshot = id(&format!("foreign-{index}"));
        assert_eq!(
            guard
                .snapshot(
                    &id("session"),
                    guard.view(&id("session")).unwrap().revision(),
                    5,
                    &id("workspace"),
                    snapshot.clone(),
                    &provider
                )
                .unwrap_err()
                .code,
            "stale_revision"
        );
        if delay_read {
            journal.unavailable.store(false, Ordering::SeqCst);
            assert_eq!(
                guard
                    .resolve_capture(
                        &id("session"),
                        guard.view(&id("session")).unwrap().revision(),
                        6,
                        &snapshot
                    )
                    .unwrap_err()
                    .code,
                "capture_evidence"
            );
        }
        // The losing Guard must leave the foreign hold intact. Its independent
        // owner then ends it before the next attempt, avoiding an unrelated
        // stress test of dozens of simultaneously active root readers.
        let view = guard.view(&id("session")).unwrap();
        let foreign = &view.capture_reads()[&snapshot];
        assert!(foreign.ended.is_none());
        let change = LockChange::CaptureAborted {
            snapshot,
            started: foreign.started.clone(),
            reason: "Foreign fixture owner completed its own capture".into(),
        };
        let completed = Envelope {
            seq: view.revision() + 1,
            session: id("session"),
            at: 6,
            actor: Actor::Runtime,
            policy: None,
            input: None,
            refs: attribution(&view, &change).unwrap(),
            payload: Event::LockChanged { version: 1, change },
        };
        sqlite
            .append(&id("session"), view.revision(), &[completed])
            .unwrap();
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let view = guard.view(&id("session")).unwrap();
    assert_eq!(view.capture_reads().len(), 64);
    assert!(view.capture_reads().values().all(|c| c.ended.is_some()));
    guard
        .snapshot(
            &id("session"),
            view.revision(),
            6,
            &id("workspace"),
            id("owned"),
            &provider,
        )
        .unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}
