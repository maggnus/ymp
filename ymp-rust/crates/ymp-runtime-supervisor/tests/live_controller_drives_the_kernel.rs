//! What a live managed run answers about resumption and termination, next to what the kernel
//! answers about the same slice.
//!
//! The kernel admits a resumption only for a committed fact a yield named, in the order
//! `admission_order` states, only while the wake is funded, and only while the attempt has wakes
//! left. A live run that decided the same question from bookkeeping of its own could answer
//! differently, and a run whose live answer and modelled answer differ is a run whose generated
//! schedules prove nothing about it. What is measured here is that the live run has no separate
//! answer: every resumption and every terminal it reaches is a committed kernel transition.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ymp_agent_api::{AgentToolCall, AgentToolHandler, SubmitArguments, YieldArguments};
use ymp_agent_rpc::SocketToolHandler;
use ymp_application::Application;
use ymp_domain::commitment::{
    CommitmentEvent, CommitmentLedger, InvocationClosure, InvocationState, ObligationState,
    OpenAuthority, RootTerminal, Verdict, WakeCondition,
};
use ymp_domain::{Budget, RunStatus, digest_bytes};
use ymp_runtime_api::{
    InvocationRequest, ProbeReport, RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeEventKind,
    RuntimeKind, RuntimeSession, Usage,
};
use ymp_runtime_fake::{FakeRuntime, ScriptStep};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, ManagedRunHandle,
    start_unattested_managed_candidate,
};

/// How the scripted runtime behaves at the two points a supervised run can end badly at: the
/// interruption a cancellation reaches it through, and the resumption a wake drives.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Disposition {
    /// The runtime answers an interruption with an interruption, which is what the fixture runtime
    /// does on its own.
    Orderly,
    /// The tool call the runtime had in flight fails because the interruption landed in the middle
    /// of it, and the runtime reports that failure instead of an interruption. This is how a
    /// cancellation reaches a runtime that is working rather than waiting.
    FailsWhenInterrupted,
    /// The wall-time limit of this runtime had already run out when the interruption reached it,
    /// so what it reports on its way out is the limit it exceeded and not the interruption. This
    /// is how an expired limit and an operator's cancellation land in the same window.
    ExceedsItsLimitWhenInterrupted,
    /// Supervision of this run dies of a panic while it drives the runtime.
    PanicsWhenResumed,
}

/// A runtime that yields through the coordination tool the way a supervised participant does. It
/// yields once for each step of its script and submits its candidate when it has no yields left.
struct YieldingRuntime {
    yields: u32,
    /// When it is set, the session holds its teardown open until this is opened. The worker winds
    /// the runtime down after it has recorded the terminal of the slice and before it reports
    /// itself finished, so holding it there is what puts a controller inside that window.
    teardown: Option<Arc<AtomicBool>>,
    disposition: Disposition,
}

impl RuntimeDriver for YieldingRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Fake
    }

    fn executable(&self) -> &Path {
        Path::new("ymp-internal-fake")
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        FakeRuntime::default().probe()
    }

    fn start(&self, request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        let binding = request.mcp.as_ref().ok_or_else(|| {
            RuntimeError::InvalidProfile("test runtime requires MCP binding".to_owned())
        })?;
        let mut controller = SocketToolHandler::for_invocation(
            &binding.socket_path,
            &binding.token,
            &request.attempt_id,
            &request.invocation_id,
        );
        let mut script: Vec<ScriptStep> = (0..self.yields)
            .map(|index| ScriptStep::Yield(format!("cursor-{index}")))
            .collect();
        script.push(ScriptStep::Complete(Usage::default()));
        let taken = AtomicU32::new(1);
        controller
            .call(AgentToolCall::Yield(YieldArguments {
                command_id: "agent.yield.0".to_owned(),
            }))
            .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
        let invocation_id = request.invocation_id.clone();
        Ok(Box::new(YieldingSession {
            inner: FakeRuntime::with_script(script).start(request)?,
            controller,
            taken,
            yields: self.yields,
            teardown: self.teardown.clone(),
            disposition: self.disposition,
            interrupted: false,
            invocation_id,
            last_sequence: 0,
            limit_reported: false,
        }))
    }
}

struct YieldingSession {
    inner: Box<dyn RuntimeSession>,
    controller: SocketToolHandler,
    taken: AtomicU32,
    yields: u32,
    teardown: Option<Arc<AtomicBool>>,
    disposition: Disposition,
    interrupted: bool,
    /// The identity and the ordering of the events this session has passed on, so that an event it
    /// reports on its own continues the same numbered progression the controller validates.
    invocation_id: String,
    last_sequence: u64,
    limit_reported: bool,
}

/// Winding the runtime down is where the worker spends the time between recording the terminal of
/// its slice and reporting itself finished. A session that waits here holds the worker in that
/// window for as long as the check needs it.
impl Drop for YieldingSession {
    fn drop(&mut self) {
        let Some(teardown) = &self.teardown else {
            return;
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !teardown.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl RuntimeSession for YieldingSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        // The work the interruption landed in the middle of is what fails, so the failure is what
        // the runtime reports next.
        if self.interrupted && self.disposition == Disposition::FailsWhenInterrupted {
            return Err(RuntimeError::RuntimeReportedFailure(
                "the tool call in flight failed when the interruption reached it".to_owned(),
            ));
        }
        // The wall-time limit ran out while the runtime worked, and the interruption reached it
        // afterwards. What it has to report is therefore the limit and not the interruption.
        if self.interrupted && self.disposition == Disposition::ExceedsItsLimitWhenInterrupted {
            if self.limit_reported {
                return Ok(None);
            }
            self.limit_reported = true;
            self.last_sequence += 1;
            return Ok(Some(RuntimeEvent {
                sequence: self.last_sequence,
                event_id: format!("{}.event-timed-out", self.invocation_id),
                invocation_id: self.invocation_id.clone(),
                event: RuntimeEventKind::TimedOut {
                    limit_ms: 1,
                    usage: Usage::default(),
                },
            }));
        }
        let event = self.inner.next_event()?;
        if let Some(event) = &event {
            self.last_sequence = event.sequence;
        }
        Ok(event)
    }

    /// A resumed participant either asks to be paused again or finishes its work. Both are
    /// recorded through the coordination tool, which is what the controller classifies the slice
    /// by.
    fn resume(&mut self, input: String) -> Result<(), RuntimeError> {
        assert!(
            self.disposition != Disposition::PanicsWhenResumed,
            "the supervision of this run died while it drove the runtime"
        );
        self.inner.resume(input)?;
        let taken = self.taken.fetch_add(1, Ordering::Relaxed);
        let call = if taken < self.yields {
            AgentToolCall::Yield(YieldArguments {
                command_id: format!("agent.yield.{taken}"),
            })
        } else {
            AgentToolCall::Submit(SubmitArguments {
                command_id: "agent.submit".to_owned(),
            })
        };
        self.controller
            .call(call)
            .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
        Ok(())
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        self.interrupted = true;
        self.inner.interrupt()
    }

    fn usage(&self) -> Usage {
        self.inner.usage()
    }
}

struct Fixture {
    handle: ManagedRunHandle,
    application: Arc<Mutex<Application>>,
    _temporary: tempfile::TempDir,
}

fn start(yields: u32) -> Fixture {
    start_with(yields, None, Disposition::Orderly)
}

fn start_with(yields: u32, teardown: Option<Arc<AtomicBool>>, disposition: Disposition) -> Fixture {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
    let application = Arc::new(Mutex::new(
        Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_unattested_managed_candidate(
        Arc::clone(&application),
        Box::new(YieldingRuntime {
            yields,
            teardown,
            disposition,
        }),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "contract-yield".to_owned(),
                contract_digest: "d".repeat(64),
                source,
                prompt: "yield then finish".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: std::env::current_exe().expect("current executable"),
        },
    )
    .expect("start managed candidate");
    Fixture {
        handle,
        application,
        _temporary: temporary,
    }
}

/// Wait until the kernel record of the slice says it has yielded. The record is what a resumption
/// is decided against, so it is what a controller waits for.
fn wait_until_yielded(handle: &ManagedRunHandle) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        while handle.try_next().is_some() {}
        if handle
            .kernel()
            .invocation_state()
            .expect("the slice record")
            == InvocationState::Yielded
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the managed runtime did not yield");
}

/// Wait until the run announces the candidate it committed, and answer with its digest.
fn wait_for_candidate(handle: &ManagedRunHandle) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::CandidateAvailable {
                candidate_digest, ..
            } = event
            {
                return candidate_digest;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the managed run announced no candidate");
}

/// The terminal the run's own journal reports, read so that a journal whose lock a panic poisoned
/// can still be measured. What is being checked is what the record says, and a check that could not
/// read a poisoned record could not tell a recorded terminal from a missing one.
fn journal_status(fixture: &Fixture) -> RunStatus {
    fixture
        .application
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state()
        .status
}

fn drain_until_finished(handle: &ManagedRunHandle) {
    let _ = failures_until_finished(handle);
}

/// Drain the run to its end and answer with what it reported as failing. It is what the run says
/// about itself, next to what the two records of its terminal say.
fn failures_until_finished(handle: &ManagedRunHandle) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut failures = Vec::new();
    let take = |handle: &ManagedRunHandle, failures: &mut Vec<String>| {
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Failed { detail } = event {
                failures.push(detail);
            }
        }
    };
    while Instant::now() < deadline && !handle.is_finished() {
        take(handle, &mut failures);
        std::thread::sleep(Duration::from_millis(5));
    }
    take(handle, &mut failures);
    failures
}

/// One live resumption, read out of the committed facts rather than out of the controller.
#[test]
fn a_live_run_resumes_through_the_kernel_transitions() {
    let fixture = start(1);
    let handle = &fixture.handle;
    wait_until_yielded(handle);

    // The slice registered what would be worth resuming for, and nothing has answered it yet: the
    // queue of admissible slices is empty, because there is no committed fact its condition names.
    let yielded = handle.kernel().snapshot().expect("the ledger");
    let record = yielded
        .invocations()
        .get(handle.invocation_id())
        .expect("the slice record");
    let wake = record.wake.as_ref().expect("the wake registration");
    assert!(
        matches!(
            wake.conditions.as_slice(),
            [WakeCondition::BidRecorded { .. }]
        ),
        "unexpected wake registration: {:?}",
        wake.conditions
    );
    assert!(
        handle
            .kernel()
            .admission_order()
            .expect("the admission order")
            .is_empty(),
        "a slice nothing answered is admissible"
    );

    handle.wake("wake-1", "continue once").expect("the wake");

    // The resumption is a committed fact, and it names the sequence of the fact that authorized it.
    let resumed = handle.kernel().snapshot().expect("the ledger");
    let (matched_sequence, wakes_used) = resumed
        .facts()
        .iter()
        .find_map(|fact| match fact {
            CommitmentEvent::InvocationResumed {
                invocation_id,
                matched_sequence,
                wakes_used,
                ..
            } if invocation_id == handle.invocation_id() => Some((*matched_sequence, *wakes_used)),
            _ => None,
        })
        .expect("the run records the resumption");
    assert_eq!(wakes_used, 1);
    let authorizing = resumed
        .facts()
        .get(usize::try_from(matched_sequence).expect("a sequence") - 1)
        .expect("the fact the resumption named");
    assert!(
        matches!(authorizing, CommitmentEvent::BidRecorded { proposal_digest, .. }
            if proposal_digest.as_deref() == Some(digest_bytes(b"continue once").as_str())),
        "the resumption was authorized by {authorizing:?}"
    );
    assert_eq!(
        resumed
            .invocations()
            .get(handle.invocation_id())
            .expect("the slice record")
            .state,
        InvocationState::Running
    );

    // A repeated delivery of the same wake is the recorded one, and a reused identifier carrying
    // different content is refused rather than served twice.
    handle
        .wake("wake-1", "continue once")
        .expect("the repeated wake");
    assert!(handle.wake("wake-1", "different input").is_err());
    let after_repeat = handle.kernel().snapshot().expect("the ledger");
    assert_eq!(
        after_repeat
            .facts()
            .iter()
            .filter(|fact| matches!(fact, CommitmentEvent::InvocationResumed { .. }))
            .count(),
        1,
        "a repeated wake resumed the slice twice"
    );

    drain_until_finished(handle);

    // The slice closed, the work carries the candidate a protected query would be spent on, and the
    // run is not quiescent: nothing has verified that candidate, so the kernel names the work as
    // what can still advance the run, and the journal agrees that the run is still open.
    let ended = handle.kernel().snapshot().expect("the ledger");
    assert_eq!(
        ended
            .invocations()
            .get(handle.invocation_id())
            .expect("the slice record")
            .state,
        InvocationState::Closed
    );
    assert!(
        ended
            .contracts()
            .get(handle.kernel().contract_id())
            .expect("the work record")
            .candidate_digest
            .is_some(),
        "the committed candidate did not reach the work it was produced under"
    );
    assert!(matches!(
        handle
            .kernel()
            .open_authority()
            .expect("the open authority"),
        Some(OpenAuthority::Obligation { .. })
    ));
    assert_eq!(handle.kernel().root_terminal().expect("the terminal"), None);
    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::Running
    );
}

/// Drive one live run to the point where its candidate is committed and nothing has judged it, then
/// record the verdict of a protected query against that exact candidate.
///
/// The state in between is the one this whole build used to end in: the slice is closed, the work
/// carries a candidate, and the run reports no terminal at all. What moves it off that state is the
/// verdict entering the kernel as a fact.
fn verified_run(verdict: Verdict) -> (Fixture, CommitmentLedger) {
    let (fixture, candidate) = finished_run();
    fixture
        .handle
        .verified(&candidate, verdict)
        .expect("the recorded verdict");
    let ledger = fixture.handle.kernel().snapshot().expect("the ledger");
    (fixture, ledger)
}

/// Drive one live run to the state every verdict and every late cancellation is issued against: the
/// runtime has finished, the candidate it produced is committed, and nothing has judged it, so the
/// run holds no terminal in either accounting.
fn finished_run() -> (Fixture, String) {
    let fixture = start(1);
    {
        let handle = &fixture.handle;
        wait_until_yielded(handle);
        handle.wake("wake-1", "continue once").expect("the wake");
        drain_until_finished(handle);
        assert_eq!(
            handle.kernel().root_terminal().expect("the terminal"),
            None,
            "the run reached a terminal before anything judged its candidate"
        );
    }
    let candidate = fixture
        .handle
        .kernel()
        .snapshot()
        .expect("the ledger")
        .contracts()[fixture.handle.kernel().contract_id()]
    .candidate_digest
    .clone()
    .expect("the committed candidate");
    (fixture, candidate)
}

/// The work obligation of a run whose verdict has been recorded, wherever that verdict points.
fn work_obligation(ledger: &CommitmentLedger, contract_id: &str) -> ObligationState {
    let obligation_id = &ledger.contracts()[contract_id].obligation_id;
    ledger.obligations()[obligation_id].state
}

/// A live run whose candidate passed the protected query closes its obligation and reaches the one
/// terminal state acceptance can be claimed from.
///
/// The verdict is what does it. Every earlier step of the same run — the slice completing, the
/// candidate being committed, the wake offers settling — leaves the work open on purpose, because
/// the controller performs no query and a run that closed its own accounting on quiet would be
/// claiming an answer nobody produced.
#[test]
fn a_live_run_whose_candidate_passed_reaches_acceptance_with_a_closed_obligation() {
    let (fixture, ledger) = verified_run(Verdict::Passed);
    let contract_id = fixture.handle.kernel().contract_id();

    let verification = ledger
        .verifications()
        .iter()
        .find(|record| record.contract_id == contract_id)
        .expect("the run records the verdict it was given");
    assert_eq!(verification.verdict, Verdict::Passed);
    assert!(
        verification.root_scope,
        "the work of a managed run does not hang directly under the obligation it is accountable for"
    );
    assert_eq!(
        work_obligation(&ledger, contract_id),
        ObligationState::Terminal,
        "a judged run left its work obligation open"
    );
    assert_eq!(
        fixture
            .handle
            .kernel()
            .open_authority()
            .expect("the open authority"),
        None
    );
    assert_eq!(
        fixture
            .handle
            .kernel()
            .root_terminal()
            .expect("the terminal"),
        Some(RootTerminal::Accepted)
    );
}

/// The same run with the same candidate, rejected. Its obligation closes exactly as it does on a
/// pass, and the run reaches a terminal state that is not acceptance: what separates the two is the
/// verdict and nothing else about how the run went.
#[test]
fn a_live_run_whose_candidate_was_rejected_closes_without_acceptance() {
    let (fixture, ledger) = verified_run(Verdict::Failed);
    let contract_id = fixture.handle.kernel().contract_id();

    // Stated rather than assumed: closing the work and settling the offer reach this same terminal
    // on their own, so without this the test would pass over a run nothing had judged at all.
    let verification = ledger
        .verifications()
        .iter()
        .find(|record| record.contract_id == contract_id)
        .expect("the run records the verdict it was given");
    assert_eq!(verification.verdict, Verdict::Failed);
    assert_eq!(
        work_obligation(&ledger, contract_id),
        ObligationState::Terminal
    );
    assert_eq!(
        fixture
            .handle
            .kernel()
            .root_terminal()
            .expect("the terminal"),
        Some(RootTerminal::Exhausted)
    );
}

/// A verdict aimed at a bundle this run never committed changes nothing: the kernel compares the
/// digest, so a candidate cannot be judged by a query that was spent on something else.
#[test]
fn a_verdict_naming_another_bundle_is_refused_and_leaves_the_run_open() {
    let fixture = start(1);
    let handle = &fixture.handle;
    wait_until_yielded(handle);
    handle.wake("wake-1", "continue once").expect("the wake");
    drain_until_finished(handle);

    let refusal = handle
        .verified(&digest_bytes(b"another bundle"), Verdict::Passed)
        .expect_err("the kernel accepted a verdict on a bundle it never recorded");
    assert!(
        refusal.to_string().contains("is not the candidate"),
        "unexpected refusal: {refusal}"
    );
    assert_eq!(handle.kernel().root_terminal().expect("the terminal"), None);
    assert!(
        handle
            .kernel()
            .snapshot()
            .expect("the ledger")
            .verifications()
            .is_empty(),
        "a refused verdict was recorded anyway"
    );
}

/// A cancellation issued after the runtime has finished reaches the kernel, so the terminal the
/// journal records is the terminal the kernel holds.
///
/// The state this is issued against is the one a completed run rests in: the slice is closed, the
/// candidate is committed and nothing has judged it. A cancellation that moved only the journal
/// would leave the kernel open on that candidate, and the verdict of a query issued afterwards
/// would drive the very run the journal calls cancelled to acceptance.
#[test]
fn cancelling_a_finished_run_reaches_the_kernel_and_refuses_a_later_verdict() {
    let (fixture, candidate) = finished_run();
    let handle = &fixture.handle;

    handle
        .cancel("stopped after the runtime finished")
        .expect("cancel");

    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::Cancelled
    );
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled),
        "the journal records a cancelled run the kernel has not stopped"
    );

    // The run is over, and a verdict produced against its candidate no longer moves it: the kernel
    // refuses the fact rather than recording one that would name a second terminal.
    let refusal = handle
        .verified(&candidate, Verdict::Passed)
        .expect_err("the kernel accepted a verdict for a cancelled run");
    assert!(
        refusal.to_string().contains("the run was stopped"),
        "unexpected refusal: {refusal}"
    );
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled),
        "a verdict drove a cancelled run to another terminal"
    );
    assert!(
        handle
            .kernel()
            .snapshot()
            .expect("the ledger")
            .verifications()
            .is_empty(),
        "a refused verdict was recorded anyway"
    );
}

/// The same cancellation, issued in the window where the worker has recorded the terminal of its
/// slice but has not yet reported itself finished.
///
/// The window is real: the runtime is wound down between those two points, and ending a process
/// tree takes as long as it takes. What decides whether the cancellation reaches the kernel is
/// therefore the committed state of the slice and not the flag the worker sets on its way out — a
/// controller that read the flag would leave exactly this run's two records naming different
/// terminals.
#[test]
fn a_cancellation_inside_the_teardown_window_reaches_the_kernel() {
    let teardown = Arc::new(AtomicBool::new(false));
    let fixture = start_with(1, Some(Arc::clone(&teardown)), Disposition::Orderly);
    let handle = &fixture.handle;
    wait_until_yielded(handle);
    handle.wake("wake-1", "continue once").expect("the wake");

    // The candidate being announced places the worker past the terminal of its slice and past the
    // submission, and the held teardown keeps it there.
    let candidate = wait_for_candidate(handle);
    assert!(
        !handle.is_finished(),
        "the worker reported itself finished while its teardown was held"
    );
    assert_eq!(
        handle
            .kernel()
            .invocation_state()
            .expect("the slice record"),
        InvocationState::Closed
    );

    handle
        .cancel("stopped while the runtime was being wound down")
        .expect("cancel");
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled),
        "a cancellation issued before the worker finished never reached the kernel"
    );

    teardown.store(true, Ordering::Release);
    drain_until_finished(handle);

    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::Cancelled
    );
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled)
    );
    assert!(
        handle.verified(&candidate, Verdict::Passed).is_err(),
        "the kernel accepted a verdict for a cancelled run"
    );
}

/// The same two commands in the other order. A verdict is recorded first, and the cancellation that
/// follows is refused rather than renaming the terminal the kernel already holds.
///
/// Which of the two wins is decided by which was committed first, and the loser changes neither
/// record. The journal of a judged run carries no terminal of its own here — the decision of a
/// protected query is entered there by the caller that performed it — so what this measures is that
/// the cancellation writes no terminal the kernel does not hold.
#[test]
fn a_cancellation_after_a_recorded_verdict_is_refused_and_renames_no_terminal() {
    let (fixture, _) = verified_run(Verdict::Passed);
    let handle = &fixture.handle;

    let refusal = handle
        .cancel("stopped after the query answered")
        .expect_err("a run the kernel had already ended was cancelled");
    assert!(
        refusal.to_string().contains("Accepted"),
        "unexpected refusal: {refusal}"
    );
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Accepted),
        "the cancellation renamed the terminal the verdict had committed"
    );
    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::Running,
        "the journal recorded a terminal the kernel does not hold"
    );
}

/// A verdict issued while the runtime is still working is refused on either side of a cancellation,
/// so the cancellation is what ends such a run in both records.
///
/// A verdict names the exact bundle the work committed, and a run that is still working has
/// committed none: the kernel has nothing to attach the decision to. After the cancellation the
/// same verdict is refused again — for the candidate it never recorded, or for the run being
/// stopped, depending on where the worker had got to — and the run ends cancelled in both records.
#[test]
fn a_verdict_while_the_runtime_works_is_refused_and_the_cancellation_ends_the_run() {
    let fixture = start(1);
    let handle = &fixture.handle;
    wait_until_yielded(handle);

    let before = handle
        .verified(
            &digest_bytes(b"a bundle nothing committed"),
            Verdict::Passed,
        )
        .expect_err("the kernel judged a run that had submitted nothing");
    assert!(
        before
            .to_string()
            .contains("carries no candidate to verify"),
        "unexpected refusal: {before}"
    );

    handle
        .cancel("stopped while the runtime was working")
        .expect("cancel");
    assert!(
        handle
            .verified(
                &digest_bytes(b"a bundle nothing committed"),
                Verdict::Passed
            )
            .is_err(),
        "a cancelled run was judged"
    );
    drain_until_finished(handle);

    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled)
    );
    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::Cancelled
    );
    assert!(
        handle
            .kernel()
            .snapshot()
            .expect("the ledger")
            .verifications()
            .is_empty(),
        "a refused verdict was recorded anyway"
    );
}

/// The wakes of one attempt are finite, and the live controller runs out of them exactly where the
/// kernel says it does rather than resuming a ninth time on its own authority.
#[test]
fn the_live_wakes_of_one_attempt_are_finite() {
    let fixture = start(12);
    let handle = &fixture.handle;
    for wake in 0..8 {
        wait_until_yielded(handle);
        handle
            .wake(format!("wake-{wake}"), format!("continue {wake}"))
            .unwrap_or_else(|error| panic!("wake {wake} was refused: {error}"));
    }
    wait_until_yielded(handle);
    let refusal = handle
        .wake("wake-8", "continue 8")
        .expect_err("the kernel has no wake left to fund");
    assert!(
        refusal.to_string().contains("used all 8 of its wakes"),
        "unexpected refusal: {refusal}"
    );
    let ledger = handle.kernel().snapshot().expect("the ledger");
    assert_eq!(
        ledger
            .facts()
            .iter()
            .filter(|fact| matches!(fact, CommitmentEvent::InvocationResumed { .. }))
            .count(),
        8,
        "the live run resumed more slices than the kernel funded"
    );
    assert_eq!(
        ledger
            .invocations()
            .get(handle.invocation_id())
            .expect("the slice record")
            .state,
        InvocationState::Yielded,
        "a refused wake left the slice running"
    );
    handle.cancel("test finished").expect("cancel");
    drain_until_finished(handle);
}

/// A stopped live run winds its accounting down and reaches a kernel terminal, and it is the
/// terminal the run's own journal reports.
#[test]
fn a_cancelled_live_run_reaches_the_same_terminal_in_both_records() {
    let fixture = start(1);
    let handle = &fixture.handle;
    wait_until_yielded(handle);
    handle.cancel("stopped by the operator").expect("cancel");
    drain_until_finished(handle);

    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled)
    );
    // A slice that ran nothing out closed of the cancellation and of nothing else. Stated here so
    // that the closure a stopped run records cannot drift into the vocabulary reserved for a run
    // that exceeded something.
    assert_eq!(
        handle
            .kernel()
            .snapshot()
            .expect("the ledger")
            .invocations()
            .get(handle.invocation_id())
            .expect("the recorded slice")
            .closure,
        Some(InvocationClosure::Cancelled)
    );
    assert_eq!(
        handle
            .kernel()
            .open_authority()
            .expect("the open authority"),
        None,
        "a stopped run is still held open"
    );
    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::Cancelled
    );
}

/// A cancellation that makes the runtime fail ends the run as cancelled in both records.
///
/// A cancellation reaches a working runtime by interrupting it, and the tool call the interruption
/// lands in the middle of fails. Classified by that failure, the slice would close as a runtime
/// failure, which the kernel accounts as an infrastructure error, while the journal the same
/// cancellation moved reports the run as cancelled: two records naming different terminals for one
/// run, and the more serious of the two naming a fault where an operator merely stopped the work.
/// What the run ended of is the cancellation, so the cancellation is what both records state.
#[test]
fn a_cancel_that_makes_the_runtime_fail_ends_the_run_as_cancelled() {
    let fixture = start_with(1, None, Disposition::FailsWhenInterrupted);
    let handle = &fixture.handle;
    wait_until_yielded(handle);
    handle.cancel("stopped by the operator").expect("cancel");
    let failures = failures_until_finished(handle);

    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled),
        "the kernel named the failure the cancellation caused as the ending of the run"
    );
    assert!(
        !failures
            .iter()
            .any(|detail| detail.contains("managed_runtime_supervision_failed")),
        "a cancelled run reported the supervision of itself as having failed: {failures:?}"
    );
    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::Cancelled,
        "the journal records a terminal the kernel does not hold"
    );
    assert_eq!(
        handle
            .kernel()
            .open_authority()
            .expect("the open authority"),
        None,
        "a stopped run is still held open"
    );
}

/// A worker that dies of a panic still drives the run to a kernel terminal.
///
/// A panic used to take the ending of the run with it: the thread unwound, the slice stayed open,
/// the run reached no terminal at all, and a cancellation issued afterwards moved the journal alone
/// and reported success for stopping a run the kernel still held running. Supervision that dies is
/// an infrastructure failure, and it is recorded as one before the worker leaves.
#[test]
fn a_worker_that_dies_of_a_panic_still_reaches_a_kernel_terminal() {
    let fixture = start_with(1, None, Disposition::PanicsWhenResumed);
    let handle = &fixture.handle;
    wait_until_yielded(handle);
    handle.wake("wake-1", "continue once").expect("the wake");
    let failures = failures_until_finished(handle);

    assert!(
        handle.is_finished(),
        "the worker died without reporting the run finished"
    );
    assert!(
        failures
            .iter()
            .any(|detail| detail.contains("managed_runtime_worker_panicked")),
        "the run reported nothing about the supervision that died: {failures:?}"
    );
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::InfrastructureError),
        "the run whose supervision died holds no kernel terminal"
    );
    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::InfrastructureError,
        "the journal reports a run the kernel ended as still open"
    );

    // The run is over, and neither of the two commands that end one moves it any further.
    assert!(
        handle
            .verified(
                &digest_bytes(b"a bundle nothing committed"),
                Verdict::Passed
            )
            .is_err(),
        "the kernel judged a run whose supervision died"
    );
    handle
        .cancel("stopped after the supervision died")
        .expect("cancel");
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::InfrastructureError),
        "a later cancellation renamed the terminal the kernel already held"
    );
    assert_eq!(
        fixture
            .application
            .lock()
            .expect("application")
            .state()
            .status,
        RunStatus::InfrastructureError,
        "a later cancellation renamed the terminal the journal already held"
    );
}

/// A journal whose lock a panic poisoned still receives the terminal of the run.
///
/// A panic under the held journal lock poisons it, and the record of the failure used to be
/// abandoned there: the kernel closed the slice on the infrastructure error while the journal was
/// left saying the run was still running. The two records then disagreed about whether the run had
/// ended at all, and the journal — the record an operator reads — was the one claiming work that
/// nothing was performing. The guard is taken poisoned for the same reason the terminal guard is:
/// every fact of the run is committed whole before it is applied, so there is no half-written datum
/// behind the poison, and refusing to write leaves a worse record than writing does.
#[test]
fn a_poisoned_journal_still_records_the_terminal_of_the_run() {
    let fixture = start(1);
    let handle = &fixture.handle;
    wait_until_yielded(handle);

    let application = Arc::clone(&fixture.application);
    let poisoning = std::thread::spawn(move || {
        let _journal = application.lock().expect("the journal");
        panic!("a controller died while it held the journal");
    });
    assert!(
        poisoning.join().is_err(),
        "the thread that was to poison the journal did not die"
    );
    assert!(
        fixture.application.is_poisoned(),
        "the journal lock outlived the panic unpoisoned"
    );

    handle.wake("wake-1", "continue once").expect("the wake");
    let failures = failures_until_finished(handle);

    assert!(
        handle.is_finished(),
        "the run did not report itself finished"
    );
    assert!(
        !failures.is_empty(),
        "the run reported nothing about supervision it could not complete"
    );
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::InfrastructureError),
        "the run holds no kernel terminal"
    );
    assert_eq!(
        journal_status(&fixture),
        RunStatus::InfrastructureError,
        "the journal reports as running a run the kernel has ended"
    );
}

/// A limit that ran out is accounted for even when a cancellation lands in the same window.
///
/// A cancellation is what a run ends of whenever it arrives, and both records name it: that is what
/// keeps an operator's stop from being reported as a fault. An exhausted limit is not something the
/// cancellation caused, though, and it is a fact about what the run consumed rather than a competing
/// account of who ended it. Classifying the ending by the cancellation alone dropped it: the slice
/// closed as cancelled and the exceeded limit appeared nowhere. The run is therefore stopped as
/// cancelled, in both records, while the slice closes on the limit it ran out of.
#[test]
fn a_limit_that_expired_survives_a_cancellation_in_the_same_window() {
    let fixture = start_with(1, None, Disposition::ExceedsItsLimitWhenInterrupted);
    let handle = &fixture.handle;
    wait_until_yielded(handle);
    handle.cancel("stopped by the operator").expect("cancel");
    drain_until_finished(handle);

    let closure = handle
        .kernel()
        .snapshot()
        .expect("the ledger")
        .invocations()
        .get(handle.invocation_id())
        .expect("the recorded slice")
        .closure;
    assert_eq!(
        closure,
        Some(InvocationClosure::LimitExceeded),
        "the limit the run exceeded is absent from the accounting of the slice"
    );
    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        Some(RootTerminal::Cancelled),
        "the run an operator stopped holds another terminal"
    );
    assert_eq!(
        journal_status(&fixture),
        RunStatus::Cancelled,
        "the journal records a terminal the kernel does not hold"
    );
    assert_eq!(
        handle
            .kernel()
            .open_authority()
            .expect("the open authority"),
        None,
        "a stopped run is still held open"
    );
}

/// The two commands that end a run from the control side decide its terminal one at a time.
///
/// A cancellation reads the kernel, then moves the journal, then takes the kernel transition that
/// reading admitted. A verdict recorded between the reading and the transition is a fact the
/// cancellation has already decided against: the run is stopped on the strength of a kernel that no
/// longer holds, and the terminal a committed query produced is renamed. The journal lock is what
/// holds a cancellation between those two points, so it is taken here to place a verdict exactly
/// there.
#[test]
fn a_verdict_racing_a_cancellation_renames_no_committed_terminal() {
    let (fixture, candidate) = finished_run();
    let handle = &fixture.handle;
    // The cancellation reads the kernel before it takes this, and moves neither record until it
    // has it.
    let journal = fixture.application.lock().expect("the journal");
    std::thread::scope(|scope| {
        let cancelling = scope.spawn(|| handle.cancel("stopped while the query was answering"));
        // Long enough for the cancellation to have read the kernel and to be waiting on the
        // journal. A cancellation that has not got that far leaves the two commands in their
        // sequential order, which is consistent whatever this measures.
        std::thread::sleep(Duration::from_millis(200));
        let judging = scope.spawn(|| handle.verified(&candidate, Verdict::Passed));
        // A verdict serialized against the cancellation cannot be recorded while the cancellation
        // is between its reading and its transition, so the journal is released after waiting for
        // one rather than requiring one.
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline && !judging.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(journal);
        let _ = cancelling.join().expect("the cancelling thread");
        let _ = judging.join().expect("the judging thread");
    });

    let terminal = handle.kernel().root_terminal().expect("the terminal");
    let status = fixture
        .application
        .lock()
        .expect("application")
        .state()
        .status;
    if handle
        .kernel()
        .snapshot()
        .expect("the ledger")
        .verifications()
        .is_empty()
    {
        assert_eq!(
            terminal,
            Some(RootTerminal::Cancelled),
            "the run the cancellation stopped holds another terminal"
        );
        assert_eq!(
            status,
            RunStatus::Cancelled,
            "the journal records a terminal the kernel does not hold"
        );
    } else {
        assert_eq!(
            terminal,
            Some(RootTerminal::Accepted),
            "a cancellation renamed the terminal a recorded verdict had committed"
        );
        assert_ne!(
            status,
            RunStatus::Cancelled,
            "the journal stopped a run the kernel had accepted"
        );
    }
}
