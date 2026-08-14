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
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ymp_agent_api::{AgentToolCall, AgentToolHandler, SubmitArguments, YieldArguments};
use ymp_agent_rpc::SocketToolHandler;
use ymp_application::Application;
use ymp_domain::commitment::{
    CommitmentEvent, CommitmentLedger, InvocationState, ObligationState, OpenAuthority,
    RootTerminal, Verdict, WakeCondition,
};
use ymp_domain::{Budget, RunStatus, digest_bytes};
use ymp_runtime_api::{
    InvocationRequest, ProbeReport, RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeKind,
    RuntimeSession, Usage,
};
use ymp_runtime_fake::{FakeRuntime, ScriptStep};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunHandle, start_unattested_managed_candidate,
};

/// A runtime that yields through the coordination tool the way a supervised participant does. It
/// yields once for each step of its script and submits its candidate when it has no yields left.
struct YieldingRuntime {
    yields: u32,
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
        Ok(Box::new(YieldingSession {
            inner: FakeRuntime::with_script(script).start(request)?,
            controller,
            taken,
            yields: self.yields,
        }))
    }
}

struct YieldingSession {
    inner: Box<dyn RuntimeSession>,
    controller: SocketToolHandler,
    taken: AtomicU32,
    yields: u32,
}

impl RuntimeSession for YieldingSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        self.inner.next_event()
    }

    /// A resumed participant either asks to be paused again or finishes its work. Both are
    /// recorded through the coordination tool, which is what the controller classifies the slice
    /// by.
    fn resume(&mut self, input: String) -> Result<(), RuntimeError> {
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
        Box::new(YieldingRuntime { yields }),
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

fn drain_until_finished(handle: &ManagedRunHandle) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !handle.is_finished() {
        while handle.try_next().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }
    while handle.try_next().is_some() {}
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
    let fixture = start(1);
    let handle = &fixture.handle;
    wait_until_yielded(handle);
    handle.wake("wake-1", "continue once").expect("the wake");
    drain_until_finished(handle);

    assert_eq!(
        handle.kernel().root_terminal().expect("the terminal"),
        None,
        "the run reached a terminal before anything judged its candidate"
    );
    let candidate = handle.kernel().snapshot().expect("the ledger").contracts()
        [handle.kernel().contract_id()]
    .candidate_digest
    .clone()
    .expect("the committed candidate");
    handle
        .verified(&candidate, verdict)
        .expect("the recorded verdict");
    let ledger = handle.kernel().snapshot().expect("the ledger");
    (fixture, ledger)
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
