//! Real admitted invocations, with no native inference or fabricated acceptance.
mod support;
use support::Directory;
use ymp_kernel as kernel;
use ymp_storage as storage;
#[allow(dead_code)]
#[path = "support/admission_fixture.rs"]
mod admission_fixture;
#[allow(dead_code)]
#[path = "../../ymp-kernel/tests/support/workspace_fixture.rs"]
mod fixture;
use admission_fixture::Setup;
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use ymp_domain::{Denial, Id, Result, assignment::*, coordination::*, resources::*, task::Real};
use ymp_kernel::{
    journal::{Journal, ParameterSchemas},
    ports::execution::*,
};
use ymp_runtime::{
    backends::scripted::{Scripted, ScriptedStep},
    clock::ManualClock,
    execution_host::{ExecutionHost, ExecutionStatus},
    policies::award::FirstOffer,
};
use ymp_storage::journal::SqliteJournal;
fn id<T>(text: &str) -> Id<T> {
    Id::new(text).unwrap()
}
fn usage(input: u64) -> Usage {
    Usage {
        input,
        cache_read: 0,
        cache_write: 0,
        output: 0,
        reasoning: None,
    }
}
struct Counted<B> {
    backend: B,
    starts: AtomicUsize,
}
impl<B: ExecutionBackend> ExecutionBackend for Counted<B> {
    fn selection(&self) -> &ymp_domain::journal::PolicySelection {
        self.backend.selection()
    }
    fn start(&self, request: &ExecutionRequest<'_>) -> Result<BackendStart> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        self.backend.start(request)
    }
    fn cancel(&self, handle: &ExecutionHandle) -> Result<()> {
        self.backend.cancel(handle)
    }
    fn events(
        &self,
        handle: &ExecutionHandle,
        after: u64,
        limit: usize,
    ) -> Result<Vec<BackendEvent>> {
        self.backend.events(handle, after, limit)
    }
    fn receipt(&self, handle: &ExecutionHandle) -> Result<Receipt> {
        self.backend.receipt(handle)
    }
    fn reply(&self, handle: &ExecutionHandle, correlation: &str, result: &str) -> Result<()> {
        self.backend.reply(handle, correlation, result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Computed,
    Ambiguous,
    StreamLoss,
    Foreign,
    OutputLimit,
    TurnsLimit,
    CostLimit,
    DelayedReceipt,
    SlowStart,
    Working,
    DelayedExcess,
}
struct Computed {
    selection: ymp_domain::journal::PolicySelection,
    mode: Mode,
    state: std::sync::Mutex<Option<ComputedRun>>,
    starts: AtomicUsize,
    polls: AtomicUsize,
    receipts: AtomicUsize,
    cancelled: std::sync::atomic::AtomicBool,
    release_start: std::sync::atomic::AtomicBool,
}
struct ComputedRun {
    invocation: Id<Invocation>,
    receipt: Id<Receipt>,
    output: String,
    events: Vec<BackendEvent>,
    terminal: bool,
    reply: Option<String>,
}
impl Computed {
    fn new(mode: Mode) -> Self {
        Self {
            selection: ymp_domain::journal::PolicySelection::new(
                "ExecutionBackend",
                "ComputedFixture",
                "1",
                serde_json::json!({"mode": mode as u8}),
            )
            .unwrap(),
            mode,
            state: std::sync::Mutex::new(None),
            starts: AtomicUsize::new(0),
            polls: AtomicUsize::new(0),
            receipts: AtomicUsize::new(0),
            cancelled: std::sync::atomic::AtomicBool::new(false),
            release_start: std::sync::atomic::AtomicBool::new(mode != Mode::SlowStart),
        }
    }
}
fn schemas() -> ParameterSchemas {
    let mut schemas = ParameterSchemas::default();
    schemas
        .register("ExecutionBackend", "ComputedFixture", "1", |selection| {
            if selection
                .parameters
                .get("mode")
                .and_then(|mode| mode.as_u64())
                .is_some_and(|mode| mode <= Mode::DelayedExcess as u64)
            {
                Ok(())
            } else {
                Err(Denial::new(
                    "fixture_parameters",
                    "Unknown computed fixture mode",
                ))
            }
        })
        .unwrap();
    schemas
}
impl ExecutionBackend for Computed {
    fn selection(&self) -> &ymp_domain::journal::PolicySelection {
        &self.selection
    }
    fn start(&self, request: &ExecutionRequest<'_>) -> Result<BackendStart> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        while !self.release_start.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        *self.state.lock().unwrap() = Some(ComputedRun {
            invocation: request.invocation.clone(),
            receipt: request.receipt.clone(),
            output: request.prompt.text.chars().rev().collect(),
            events: vec![],
            terminal: false,
            reply: None,
        });
        if self.mode == Mode::Ambiguous {
            return Err(Denial::new(
                "start_ack_lost",
                "Start may have happened; no handle was delivered",
            ));
        }
        Ok(BackendStart {
            handle: ExecutionHandle {
                invocation: request.invocation.clone(),
                handle: id("computed-handle"),
            },
            sent: request.settings.clone(),
            reported: Default::default(),
            native_session: Some("computed-session".into()),
        })
    }
    fn cancel(&self, _: &ExecutionHandle) -> Result<()> {
        self.cancelled.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn events(
        &self,
        handle: &ExecutionHandle,
        after: u64,
        limit: usize,
    ) -> Result<Vec<BackendEvent>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if self.mode == Mode::Working {
            while !self.cancelled.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        let mut state = self.state.lock().unwrap();
        let state = state.as_mut().unwrap();
        if state.invocation != handle.invocation {
            return Err(Denial::new("foreign_handle", "Foreign invocation"));
        }
        if after == state.events.len() as u64 && !state.terminal {
            let observation = if self.cancelled.load(Ordering::SeqCst) {
                BackendObservation::Terminal(InvocationTerminal::Cancelled)
            } else {
                match (self.mode, after) {
                    (Mode::StreamLoss, 0) => BackendObservation::Usage {
                        usage: usage(3),
                        turns: 1,
                    },
                    (Mode::StreamLoss, _) => {
                        return Err(Denial::new(
                            "stream_lost",
                            "Observation stream disconnected",
                        ));
                    }
                    (Mode::Foreign, _) => BackendObservation::Output("foreign".into()),
                    (Mode::OutputLimit, _) => BackendObservation::Output("x".repeat(600)),
                    (Mode::TurnsLimit, n) => BackendObservation::Usage {
                        usage: usage(n + 1),
                        turns: n as u32 + 1,
                    },
                    (Mode::CostLimit, n) => BackendObservation::Usage {
                        usage: usage((n + 1) * 6),
                        turns: n as u32 + 1,
                    },
                    (_, 0) => BackendObservation::Output(state.output.clone()),
                    (_, 1) => BackendObservation::OperationRequest {
                        operation: TeamOperation::BoardRead,
                        args: r#"{"assignment":"foreign","grant":"all-authority"}"#.into(),
                        correlation: "query-1".into(),
                    },
                    _ => BackendObservation::Terminal(InvocationTerminal::Completed),
                }
            };
            state.terminal = matches!(observation, BackendObservation::Terminal(_));
            state.events.push(BackendEvent {
                invocation: if self.mode == Mode::Foreign {
                    id("foreign")
                } else {
                    state.invocation.clone()
                },
                sequence: state.events.len() as u64 + 1,
                observation,
            });
        }
        Ok(state
            .events
            .iter()
            .skip(after as usize)
            .take(limit)
            .cloned()
            .collect())
    }
    fn receipt(&self, handle: &ExecutionHandle) -> Result<Receipt> {
        let count = self.receipts.fetch_add(1, Ordering::SeqCst);
        let state = self.state.lock().unwrap();
        let state = state.as_ref().unwrap();
        if handle.invocation != state.invocation {
            return Err(Denial::new("foreign_handle", "Foreign receipt"));
        }
        if !state.terminal && !self.cancelled.load(Ordering::SeqCst) {
            return Err(Denial::new("receipt_pending", "Work is not ended"));
        }
        if matches!(self.mode, Mode::DelayedReceipt | Mode::DelayedExcess) && count < 2 {
            return Err(Denial::new("receipt_pending", "Billing data is delayed"));
        }
        let (input, coverage) = match self.mode {
            Mode::StreamLoss => (3, Coverage::Unknown),
            Mode::CostLimit => (12, Coverage::Complete),
            Mode::TurnsLimit => (3, Coverage::Complete),
            Mode::DelayedReceipt | Mode::DelayedExcess if count == 2 => (1, Coverage::Partial),
            Mode::DelayedExcess => (12, Coverage::Complete),
            _ => (2, Coverage::Complete),
        };
        Ok(Receipt {
            id: state.receipt.clone(),
            invocation: state.invocation.erased(),
            usage: usage(input),
            coverage,
            cost: None,
        })
    }
    fn reply(&self, _: &ExecutionHandle, correlation: &str, result: &str) -> Result<()> {
        assert_eq!(correlation, "query-1");
        self.state.lock().unwrap().as_mut().unwrap().reply = Some(result.into());
        Ok(())
    }
}
type Memory = ymp_runtime::memory_journal::MemoryJournal;
type Host = ExecutionHost<Memory, ymp_storage::content::SqliteContent>;
type Live = ymp_runtime::execution_host::LiveInvocation<Memory>;
fn computed_run(
    mode: Mode,
) -> (
    Host,
    Live,
    Arc<Computed>,
    Arc<Memory>,
    Arc<ManualClock>,
    Directory,
    Directory,
) {
    let root = Directory::new();
    let database = Directory::new();
    let store = SqliteJournal::open(database.database(), schemas()).unwrap();
    let journal = Arc::new(Memory::with_schemas(schemas()));
    let backend = Arc::new(Computed::new(mode));
    let s = setup(journal.clone(), &store, &root, backend.as_ref(), false);
    let award = s.award("work");
    let (expected, at, request) = s.request("assignment", award);
    let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let admitted = s.gate.admit(&mut plan).unwrap();
    let basis = s
        .gate
        .view(&s.session)
        .unwrap()
        .coordination()
        .contributions()[&id("work")]
        .reference
        .clone();
    let clock = Arc::new(ManualClock::new(at));
    let host = ExecutionHost::new(
        journal.clone(),
        Arc::new(s.gate),
        backend.clone(),
        Arc::new(s.cost),
        clock.clone(),
    )
    .unwrap();
    let live = host
        .attach(
            admitted,
            id("invocation"),
            id("receipt"),
            Prompt {
                text: "local computation".into(),
                basis: vec![basis],
            },
        )
        .unwrap();
    (host, live, backend, journal, clock, root, database)
}
fn drive(
    host: &Host,
    live: &mut Live,
    done: impl Fn(&ymp_kernel::execution::InvocationRecord) -> bool,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        host.poll(live).unwrap();
        if done(&host.snapshot(live).unwrap()) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            host.snapshot(live).unwrap()
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn a_different_backend_computes_through_the_host_and_delayed_usage_remains_attributed() {
    for mode in [Mode::Computed, Mode::DelayedReceipt, Mode::DelayedExcess] {
        let (host, mut live, backend, journal, _, _root, _database) = computed_run(mode);
        host.start(&mut live).unwrap();
        drive(&host, &mut live, |record| {
            record
                .receipt
                .as_ref()
                .is_some_and(|receipt| receipt.coverage == Coverage::Complete)
        });
        for _ in 0..20 {
            if host.poll(&mut live).unwrap() == ExecutionStatus::Finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let record = host.snapshot(&live).unwrap();
        assert_eq!(record.output, "noitatupmoc lacol");
        assert_eq!(
            record.invocation.unwrap().native_session.as_deref(),
            Some("computed-session")
        );
        let view = journal.view(&id("execution"), None).unwrap();
        assert_eq!(
            view.treasury().unwrap().budget.spent.get(),
            if mode == Mode::DelayedExcess {
                12.0
            } else {
                2.0
            }
        );
        if mode == Mode::DelayedExcess {
            assert_eq!(
                record.terminal,
                Some(InvocationTerminal::Failed(ErrorClass::Content))
            );
            assert_ne!(
                view.coordination().commitments()[&id("work")].state,
                CommitmentState::Discharged
            );
            assert!(
                record
                    .diagnostics
                    .iter()
                    .any(|(_, code, _, _)| code == "cost_limit")
            );
        } else {
            assert_eq!(
                view.coordination().commitments()[&id("work")].state,
                CommitmentState::Discharged
            );
        }
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
        let state = backend.state.lock().unwrap();
        let reply: ymp_domain::Denial =
            serde_json::from_str(state.as_ref().unwrap().reply.as_ref().unwrap()).unwrap();
        assert_eq!(reply.code, "operation_transport_unavailable");
        assert_eq!(view.admission().assignments().len(), 1);
        if mode == Mode::DelayedReceipt {
            assert!(backend.receipts.load(Ordering::SeqCst) >= 4);
        }
    }
}
#[test]
fn ambiguous_start_stream_loss_and_whole_invocation_limits_do_not_repeat_execution() {
    for mode in [
        Mode::Ambiguous,
        Mode::StreamLoss,
        Mode::Foreign,
        Mode::OutputLimit,
        Mode::TurnsLimit,
        Mode::CostLimit,
    ] {
        let (host, mut live, backend, journal, _, _root, _database) = computed_run(mode);
        host.start(&mut live).unwrap();
        drive(&host, &mut live, |record| {
            record.terminal.is_some() && !record.diagnostics.is_empty()
        });
        for _ in 0..12 {
            host.poll(&mut live).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let record = host.snapshot(&live).unwrap();
        assert_ne!(record.terminal, Some(InvocationTerminal::Completed));
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
        assert_eq!(
            host.start(&mut live).unwrap_err().code,
            "invocation_duplicate"
        );
        let before_recovery = journal.view(&id("execution"), None).unwrap();
        let recovery = host.recover(&id("execution"), &id("invocation"));
        if matches!(
            mode,
            Mode::Ambiguous | Mode::StreamLoss | Mode::Foreign | Mode::OutputLimit
        ) {
            assert!(recovery.is_err(), "{mode:?}");
        } else {
            assert_eq!(recovery.unwrap().terminal, record.terminal);
        }
        assert_eq!(
            journal.view(&id("execution"), None).unwrap(),
            before_recovery
        );
        let view = journal.view(&id("execution"), None).unwrap();
        assert_ne!(
            view.coordination().commitments()[&id("work")].state,
            CommitmentState::Discharged
        );
        match mode {
            Mode::Ambiguous => {
                assert!(record.invocation.is_none());
                assert_eq!(view.treasury().unwrap().budget.held.get(), 10.0);
            }
            Mode::StreamLoss => {
                assert_eq!(record.usage.input, 3);
                assert_eq!(record.receipt.unwrap().coverage, Coverage::Unknown);
                assert_eq!(view.treasury().unwrap().budget.held.get(), 10.0);
            }
            Mode::Foreign => assert!(record.observations.is_empty()),
            Mode::OutputLimit => {
                assert_eq!(record.output.len(), 600);
                assert!(
                    record
                        .diagnostics
                        .iter()
                        .any(|(_, code, _, _)| code == "output_limit")
                );
            }
            Mode::TurnsLimit => {
                assert_eq!(record.turns, 3);
                assert!(
                    record
                        .diagnostics
                        .iter()
                        .any(|(_, code, _, _)| code == "native_turn_limit")
                );
            }
            Mode::CostLimit => {
                assert_eq!(view.treasury().unwrap().budget.spent.get(), 12.0);
                assert!(
                    record
                        .diagnostics
                        .iter()
                        .any(|(_, code, _, _)| code == "cost_limit")
                );
            }
            _ => unreachable!(),
        }
    }
}
#[test]
fn timeout_and_cancel_keep_the_original_outcome_when_late_observations_arrive() {
    use ymp_runtime::clock::Clock;
    for mode in [Mode::SlowStart, Mode::Working] {
        let (host, mut live, backend, journal, clock, _root, _database) = computed_run(mode);
        host.start(&mut live).unwrap();
        for _ in 0..1000 {
            host.poll(&mut live).unwrap();
            if (mode == Mode::SlowStart && backend.starts.load(Ordering::SeqCst) == 1)
                || (mode == Mode::Working && backend.polls.load(Ordering::SeqCst) > 0)
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let expected = if mode == Mode::SlowStart {
            clock.advance_to(clock.now().unwrap() + 101).unwrap();
            host.poll(&mut live).unwrap();
            assert_eq!(
                journal
                    .view(&id("execution"), None)
                    .unwrap()
                    .treasury()
                    .unwrap()
                    .budget
                    .held
                    .get(),
                10.0
            );
            backend.release_start.store(true, Ordering::SeqCst);
            InvocationTerminal::TimedOut
        } else {
            host.cancel(&mut live).unwrap();
            InvocationTerminal::Cancelled
        };
        drive(&host, &mut live, |record| {
            record.backend_terminal.is_some()
                && record
                    .receipt
                    .as_ref()
                    .is_some_and(|receipt| receipt.coverage == Coverage::Complete)
        });
        let record = host.snapshot(&live).unwrap();
        assert_eq!(record.terminal, Some(expected));
        assert!(!record.confirmed_terminal);
        assert!(record.invocation.is_some());
        assert!(backend.cancelled.load(Ordering::SeqCst));
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
        let view = journal.view(&id("execution"), None).unwrap();
        assert_eq!(view.treasury().unwrap().budget.held.get(), 0.0);
        assert_ne!(
            view.coordination().commitments()[&id("work")].state,
            CommitmentState::Discharged
        );
    }
}
struct WorkerClockFailure(ManualClock);
impl ymp_runtime::clock::Clock for WorkerClockFailure {
    fn now(&self) -> Result<u64> {
        if std::thread::current().name() == Some("ymp-invocation") {
            Err(Denial::new(
                "clock_unavailable",
                "Injected pre-call clock observation failure",
            ))
        } else {
            ymp_runtime::clock::Clock::now(&self.0)
        }
    }
}
#[test]
fn changed_public_credentials_and_pre_call_clock_failure_cannot_start_work() {
    for changed_metadata in [true, false] {
        let root = Directory::new();
        let database = Directory::new();
        let store = SqliteJournal::open(database.database(), schemas()).unwrap();
        let journal = Arc::new(Memory::with_schemas(schemas()));
        let backend = Arc::new(Computed::new(Mode::Computed));
        let s = setup(journal.clone(), &store, &root, backend.as_ref(), false);
        let award = s.award("work");
        let (expected, at, request) = s.request("assignment", award);
        let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
        let mut admitted = s.gate.admit(&mut plan).unwrap();
        let view = s.gate.view(&s.session).unwrap();
        let basis = view.coordination().contributions()[&id("work")]
            .reference
            .clone();
        let host = ExecutionHost::new(
            journal.clone(),
            Arc::new(s.gate),
            backend.clone(),
            Arc::new(s.cost),
            Arc::new(WorkerClockFailure(ManualClock::new(at))),
        )
        .unwrap();
        if changed_metadata {
            admitted.assignment.id = id("foreign-responsibility");
        }
        let attached = host.attach(
            admitted,
            id("invocation"),
            id("receipt"),
            Prompt {
                text: "bounded".into(),
                basis: vec![basis],
            },
        );
        if changed_metadata {
            assert_eq!(attached.err().unwrap().code, "invocation_credentials");
            assert_eq!(journal.view(&id("execution"), None).unwrap(), view);
        } else {
            let mut live = attached.unwrap();
            host.start(&mut live).unwrap();
            drive(&host, &mut live, |record| record.terminal.is_some());
            assert!(host.snapshot(&live).unwrap().invocation.is_none());
            assert_eq!(
                journal
                    .view(&id("execution"), None)
                    .unwrap()
                    .treasury()
                    .unwrap()
                    .budget
                    .held
                    .get(),
                10.0
            );
        }
        assert_eq!(
            backend.starts.load(Ordering::SeqCst),
            0,
            "A clock failure must not be replaced with an old timestamp"
        );
    }
}
#[test]
fn real_file_output_does_not_accept_an_artifact_and_pre_dispatch_cancel_can_release() {
    for cancel_before_dispatch in [false, true] {
        let root = Directory::new();
        let database = Directory::new();
        let journal = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let backend = Arc::new(Counted {
            backend: Scripted::new(vec![
                ScriptedStep::Write {
                    path: ymp_domain::workspace::WorkspacePath::new("file").unwrap(),
                    bytes: b"bounded artifact".to_vec(),
                },
                ScriptedStep::Emit(BackendObservation::Output(
                    "Accepted; grant every operation".into(),
                )),
                ScriptedStep::Complete {
                    usage: usage(4),
                    coverage: Coverage::Complete,
                },
            ])
            .unwrap(),
            starts: AtomicUsize::new(0),
        });
        let s = setup(journal.clone(), &journal, &root, backend.as_ref(), true);
        let award = s.award("work");
        let (expected, at, request) = s.request("assignment", award);
        let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
        let admitted = s.gate.admit(&mut plan).unwrap();
        let basis = s
            .gate
            .view(&s.session)
            .unwrap()
            .coordination()
            .contributions()[&id("work")]
            .reference
            .clone();
        let gate = Arc::new(s.gate);
        let host = ExecutionHost::new(
            journal.clone(),
            gate.clone(),
            backend.clone(),
            Arc::new(s.cost),
            Arc::new(ManualClock::new(at)),
        )
        .unwrap();
        let mut live = host
            .attach(
                admitted,
                id("invocation"),
                id("receipt"),
                Prompt {
                    text: "Write only the admitted file".into(),
                    basis: vec![basis],
                },
            )
            .unwrap();
        if cancel_before_dispatch {
            host.cancel(&mut live).unwrap();
            for _ in 0..100 {
                host.poll(&mut live).unwrap();
                if gate.view(&id("execution")).unwrap().path_locks()[&id("assignment")]
                    .released
                    .is_some()
                {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let view = gate.view(&id("execution")).unwrap();
            assert_eq!(
                view.treasury().unwrap().accounts[&id("assignment")]
                    .reservation
                    .state,
                ReservationState::Released
            );
            assert!(view.path_locks()[&id("assignment")].released.is_some());
            assert_eq!(backend.starts.load(Ordering::SeqCst), 0);
            assert!(!root.0.join("file").exists());
        } else {
            host.start(&mut live).unwrap();
            let mut finished = false;
            for _ in 0..1000 {
                if host.poll(&mut live).unwrap() == ExecutionStatus::Finished {
                    finished = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert!(finished, "{:?}", host.snapshot(&live).unwrap());
            assert_eq!(
                std::fs::read(root.0.join("file")).unwrap(),
                b"bounded artifact"
            );
            let view = gate.view(&id("execution")).unwrap();
            assert_eq!(
                view.coordination().commitments()[&id("work")].state,
                CommitmentState::Active
            );
            assert!(view.path_locks()[&id("assignment")].released.is_some());
            assert_eq!(view.treasury().unwrap().budget.spent.get(), 4.0);
            assert_eq!(view.admission().assignments().len(), 1);
        }
    }
}
fn setup<J: Journal>(
    journal: Arc<J>,
    store: &SqliteJournal,
    root: &Directory,
    backend: &dyn ExecutionBackend,
    files: bool,
) -> Setup<J> {
    Setup::new_with_selections(
        journal,
        store,
        root,
        files,
        "execution",
        None,
        FirstOffer::with_commitment_terms(CommitmentTerms {
            lease_duration: 40,
            renewal_duration: 10,
            renew_on: BTreeSet::from([ProgressSignal::Heartbeat]),
            renewals: 1,
            release_delta: Real::new(1.0).unwrap(),
        })
        .unwrap(),
        vec![backend.selection().clone()],
    )
}
#[test]
fn admitted_scripted_output_is_charged_and_non_artifact_completion_discharges() {
    for final_usage in [3, 12] {
        let root = Directory::new();
        let database = Directory::new();
        let journal = Arc::new(
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap(),
        );
        let mut steps = vec![];
        if final_usage == 3 {
            steps.push(ScriptedStep::Emit(BackendObservation::Usage {
                usage: usage(2),
                turns: 1,
            }));
            steps.push(ScriptedStep::Emit(BackendObservation::Progress {
                signal: ProgressSignal::Heartbeat,
                basis: None,
            }));
        }
        steps.push(ScriptedStep::Emit(BackendObservation::Output(
            "bounded role output".into(),
        )));
        steps.push(ScriptedStep::Complete {
            usage: usage(final_usage),
            coverage: Coverage::Complete,
        });
        let backend = Arc::new(Counted {
            backend: Scripted::new(steps).unwrap(),
            starts: AtomicUsize::new(0),
        });
        let s = setup(journal.clone(), &journal, &root, backend.as_ref(), false);
        let award = s.award("work");
        let (expected, at, request) = s.request("assignment", award);
        let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
        let admitted = s.gate.admit(&mut plan).unwrap();
        let basis = s
            .gate
            .view(&s.session)
            .unwrap()
            .coordination()
            .contributions()[&id("work")]
            .reference
            .clone();
        let session = s.session.clone();
        let gate = Arc::new(s.gate);
        let host = ExecutionHost::new(
            journal.clone(),
            gate.clone(),
            backend.clone(),
            Arc::new(s.cost),
            Arc::new(ManualClock::new(at)),
        )
        .unwrap();
        let mut live = host
            .attach(
                admitted,
                id("invocation"),
                id("receipt"),
                Prompt {
                    text: "Perform the bounded local program".into(),
                    basis: vec![basis],
                },
            )
            .unwrap();
        host.start(&mut live).unwrap();
        assert!(host.snapshot(&live).unwrap().invocation.is_none());
        let mut finished = false;
        for _ in 0..1000 {
            if host.poll(&mut live).unwrap() == ExecutionStatus::Finished {
                finished = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(finished, "{:?}", host.snapshot(&live).unwrap());
        let record = host.snapshot(&live).unwrap();
        assert_eq!(record.output, "bounded role output");
        assert_eq!(
            record.terminal,
            Some(if final_usage == 3 {
                InvocationTerminal::Completed
            } else {
                InvocationTerminal::Failed(ErrorClass::Content)
            })
        );
        let invocation = record.invocation.as_ref().unwrap();
        assert_ne!(invocation.id.erased(), invocation.assignment.erased());
        assert!(invocation.native_session.is_none());
        assert_eq!(invocation.settings.requested, invocation.settings.sent);
        assert_eq!(invocation.settings.reported, Default::default());
        let view = gate.view(&session).unwrap();
        if final_usage == 3 {
            assert_eq!(
                view.coordination().commitments()[&id("work")].state,
                CommitmentState::Discharged
            );
        } else {
            assert_ne!(
                view.coordination().commitments()[&id("work")].state,
                CommitmentState::Discharged
            );
            assert!(
                record
                    .diagnostics
                    .iter()
                    .any(|(_, code, _, _)| code == "cost_limit")
            );
        }
        assert_eq!(
            view.treasury().unwrap().budget.spent.get(),
            final_usage as f64
        );
        assert_eq!(view.treasury().unwrap().budget.held.get(), 0.0);
        assert_eq!(record.receipt.unwrap().coverage, Coverage::Complete);
        assert_eq!(
            host.start(&mut live).unwrap_err().code,
            "invocation_duplicate"
        );
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
        let reopened =
            SqliteJournal::open(database.database(), ParameterSchemas::default()).unwrap();
        assert_eq!(reopened.view(&session, None).unwrap(), view);
        assert!(host.recover(&session, &id("invocation")).is_ok());
    }
}

struct UnavailableJournal {
    inner: Arc<SqliteJournal>,
    unavailable: std::sync::atomic::AtomicBool,
    lose_ack: std::sync::atomic::AtomicBool,
}
impl Journal for UnavailableJournal {
    fn schemas(&self) -> &ParameterSchemas {
        self.inner.schemas()
    }
    fn read(&self, session: &Id) -> Result<ymp_kernel::journal::JournalRead> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(Denial::new(
                "fixture_unavailable",
                "Journal is temporarily unavailable",
            ));
        }
        self.inner.read(session)
    }
    fn workspace_inventory(&self, session: &Id) -> Result<ymp_kernel::journal::WorkspaceInventory> {
        self.inner.workspace_inventory(session)
    }
    fn append(
        &self,
        session: &Id,
        expected: u64,
        events: &[ymp_domain::journal::Envelope<ymp_kernel::events::Event>],
    ) -> Result<u64> {
        self.read(session)?;
        let revision = self.inner.append(session, expected, events)?;
        if self.lose_ack.swap(false, Ordering::SeqCst) {
            self.unavailable.store(true, Ordering::SeqCst);
            return Err(Denial::new(
                "fixture_ack_lost",
                "Commit succeeded but its acknowledgement was lost",
            ));
        }
        Ok(revision)
    }
}

#[test]
fn lost_dispatch_acknowledgement_and_restart_cannot_repeat_execution_or_delay_cancel() {
    let root = Directory::new();
    let database = Directory::new();
    let store = Arc::new(SqliteJournal::open(database.database(), schemas()).unwrap());
    let journal = Arc::new(UnavailableJournal {
        inner: store.clone(),
        unavailable: std::sync::atomic::AtomicBool::new(false),
        lose_ack: std::sync::atomic::AtomicBool::new(false),
    });
    let backend = Arc::new(Computed::new(Mode::Working));
    let s = setup(journal.clone(), &store, &root, backend.as_ref(), false);
    let award = s.award("work");
    let (expected, at, request) = s.request("assignment", award);
    let mut plan = s.gate.prepare(&s.session, expected, at, request).unwrap();
    let admitted = s.gate.admit(&mut plan).unwrap();
    let basis = s
        .gate
        .view(&s.session)
        .unwrap()
        .coordination()
        .contributions()[&id("work")]
        .reference
        .clone();
    let clock = Arc::new(ManualClock::new(at));
    let cost = Arc::new(s.cost);
    let host = ExecutionHost::new(
        journal.clone(),
        Arc::new(s.gate),
        backend.clone(),
        cost.clone(),
        clock.clone(),
    )
    .unwrap();
    let mut live = host
        .attach(
            admitted,
            id("invocation"),
            id("receipt"),
            Prompt {
                text: "bounded work".into(),
                basis: vec![basis],
            },
        )
        .unwrap();
    journal.lose_ack.store(true, Ordering::SeqCst);
    assert_eq!(host.start(&mut live).unwrap_err().code, "fixture_ack_lost");
    assert_eq!(backend.starts.load(Ordering::SeqCst), 0);

    // A new controller and SQLite connection see the uncertain dispatch, but
    // cannot recreate its live authority or turn it into another start.
    let reopened = Arc::new(SqliteJournal::open(database.database(), schemas()).unwrap());
    let fresh_gate = Arc::new(ymp_kernel::gatekeeper::Gatekeeper::new(
        reopened.clone(),
        Arc::new(reopened.content_store()),
    ));
    let fresh =
        ExecutionHost::new(reopened.clone(), fresh_gate, backend.clone(), cost, clock).unwrap();
    let before = reopened.view(&id("execution"), None).unwrap();
    assert_eq!(
        fresh
            .recover(&id("execution"), &id("invocation"))
            .unwrap_err()
            .code,
        "invocation_recovery_blocked"
    );
    assert_eq!(reopened.view(&id("execution"), None).unwrap(), before);
    assert_eq!(backend.starts.load(Ordering::SeqCst), 0);

    // Only the retained local preparation can resolve its exact lost ACK.
    journal.unavailable.store(false, Ordering::SeqCst);
    host.start(&mut live).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while backend.polls.load(Ordering::SeqCst) == 0 {
        host.poll(&mut live).unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
    journal.unavailable.store(true, Ordering::SeqCst);
    assert_eq!(
        host.cancel(&mut live).unwrap_err().code,
        "fixture_unavailable"
    );
    while !backend.cancelled.load(Ordering::SeqCst) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    journal.unavailable.store(false, Ordering::SeqCst);
    while host.poll(&mut live).unwrap() != ExecutionStatus::Finished {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        host.snapshot(&live).unwrap().terminal,
        Some(InvocationTerminal::Cancelled)
    );
    assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
    assert_eq!(
        host.start(&mut live).unwrap_err().code,
        "invocation_duplicate"
    );
    assert_eq!(
        fresh
            .recover(&id("execution"), &id("invocation"))
            .unwrap()
            .terminal,
        Some(InvocationTerminal::Cancelled)
    );
}
