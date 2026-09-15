//! Tests of the Codex CLI adapter over the `ExecutionBackend` port.
//!
//! Every unattended test below runs against a local scripted fixture: a
//! deterministic shell script that emulates the `codex exec --json` output
//! contract. No unattended test spawns the real `codex` executable, contacts
//! a provider, reads credentials, opens the network or touches user data —
//! the structural guarantee behind criterion 8 of the bounded execution
//! contract.
//!
//! The one real-invocation test is `#[ignore]`d: it spends real provider
//! quota and requires the owner's explicit go. Run it manually with
//!
//! ```text
//! cargo test -p ymp-runtime codex_ -- --ignored --nocapture
//! ```
//!
//! (`--ignored` is a libtest flag and belongs after `--`; the filter selects
//! exactly `codex_real_invocation_completes_with_output`.)

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ymp_runtime::{
    AcceptanceContract, Allowance, AssignmentRequest, BackendInvocation, CodexBackend, CodexProbe,
    CodexRegistry, CodexStreamStats, CodexTokenUsage, Constraints, Criterion, CriterionId,
    EmptyPoolReason, ErrorClass, ExecutionBackend, ExecutionError, ExecutionObservation,
    ExecutionScenario, Goal, InvocationId, InvocationLimits, Journal, JournalEntry, JournalError,
    ManualClock, MemoryJournal, ModelOffering, OfferingId, PoolEligibility, Registry,
    ReservationPurpose, ResourceAmount, Revision, Role, ScriptedProvider, ScriptedRegistry,
    SessionEvent, SessionId, Settings, StartOutcome, Task, TaskId, Termination, WorkspaceAccess,
    WorkspaceOperation, WorkspaceScope,
};

// ---------------------------------------------------------------- fixtures

/// One deterministic temp directory per test; removed on drop.
struct FixtureDir {
    path: PathBuf,
}

impl FixtureDir {
    fn new(tag: &str) -> Self {
        let unique = format!(
            "ymp-codex-fixture-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock is after the epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).expect("fixture directory is created");
        Self { path }
    }

    /// Writes an executable POSIX shell script and returns its path.
    fn executable(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, body).expect("fixture script is written");
        make_executable(&path);
        path
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A journal that rejects the first `InvocationStarted` append after
/// retaining every earlier event.
#[derive(Clone)]
struct FailStartedOnceJournal {
    inner: MemoryJournal,
    fired: Arc<AtomicBool>,
}

impl FailStartedOnceJournal {
    fn new() -> Self {
        Self {
            inner: MemoryJournal::new(),
            fired: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Journal for FailStartedOnceJournal {
    fn read(&self, session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError> {
        self.inner.read(session_id)
    }

    fn append(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        let contains_started = events
            .iter()
            .any(|event| matches!(event, SessionEvent::InvocationStarted { .. }));
        if contains_started && !self.fired.swap(true, Ordering::SeqCst) {
            return Err(JournalError::AdapterFailure {
                message: "injected InvocationStarted append failure".to_owned(),
            });
        }
        self.inner.append(session_id, expected_revision, events)
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("fixture permissions are set");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {
    panic!("the codex adapter tests require a Unix host");
}

/// The happy-path `codex exec --json` stream, matching the event shapes
/// verified from the official documentation of this Codex generation.
const HAPPY_STREAM: &str = r#"{"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a53"}
{"type":"turn.started"}
{"type":"item.started","item":{"id":"item_0","type":"agent_message","status":"in_progress"}}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"pong"}}
{"type":"turn.completed","usage":{"input_tokens":42,"cached_input_tokens":40,"output_tokens":7,"reasoning_output_tokens":0}}"#;

fn happy_codex(dir: &FixtureDir) -> PathBuf {
    dir.executable(
        "codex",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \\\n{}\nexit 0\n",
            quote_stream(HAPPY_STREAM)
        ),
    )
}

/// Turns the stream into one quoted shell argument list for `printf '%s\n'`.
fn quote_stream(stream: &str) -> String {
    stream
        .lines()
        .map(|line| format!("'{line}'"))
        .collect::<Vec<_>>()
        .join(" \\\n  ")
}

// ---------------------------------------------------------------- helpers

fn agent_id() -> ymp_runtime::AgentId {
    ymp_runtime::AgentId::new("codex").expect("valid agent ID")
}

fn offering() -> ModelOffering {
    ModelOffering::new(
        OfferingId::new("codex-cli-local").expect("valid offering ID"),
        Vec::new(),
    )
    .expect("valid offering")
}

fn invocation() -> InvocationId {
    InvocationId::new("invocation-codex-1").expect("valid invocation ID")
}

fn limits() -> InvocationLimits {
    InvocationLimits::new(8, 100_000, Duration::from_secs(60)).expect("valid limits")
}

fn drain_events(
    backend: &mut CodexBackend,
    invocation: &InvocationId,
) -> Vec<ExecutionObservation> {
    let mut drained = Vec::new();
    while let Some(observation) = backend.next_event(invocation) {
        drained.push(observation);
    }
    backend.commit_scan(invocation);
    drained
}

fn workspace_accesses(scope: WorkspaceScope) -> Vec<WorkspaceAccess> {
    vec![WorkspaceAccess::new(scope, [WorkspaceOperation::Write]).expect("valid workspace access")]
}

fn backend_invocation(dir: &FixtureDir, sent_settings: Settings) -> BackendInvocation {
    let workspace = WorkspaceScope::new(dir.path().to_string_lossy().into_owned())
        .expect("valid workspace scope");
    BackendInvocation::new(
        invocation(),
        agent_id(),
        sent_settings,
        workspace_accesses(workspace),
        limits(),
    )
}

fn execution_task() -> Task {
    Task::new(
        TaskId::new("task-codex-start-retry").expect("valid task ID"),
        Goal::new("Verify an idempotent Codex start retry").expect("valid goal"),
        AcceptanceContract::new(vec![
            Criterion::new(
                CriterionId::new("idempotent-start").expect("valid criterion ID"),
                "The retry returns the first start outcome.",
            )
            .expect("valid criterion"),
        ])
        .expect("valid acceptance contract"),
        Constraints::new(Vec::new()).expect("valid constraints"),
    )
}

fn backend_for(executable: &Path) -> CodexBackend {
    CodexBackend::new(
        executable.to_string_lossy().into_owned(),
        Arc::new(ManualClock::new()),
    )
    .with_prompt("Reply with the single word: pong")
    .with_cancel_grace(Duration::from_millis(750))
}

/// Pumps the port until a receipt exists, collecting every drained
/// observation; returns `None` on timeout.
fn wait_for_observations(
    backend: &mut CodexBackend,
    timeout: Duration,
) -> (Vec<ExecutionObservation>, Option<ymp_runtime::Receipt>) {
    let deadline = Instant::now() + timeout;
    let mut drained = Vec::new();
    loop {
        drained.extend(drain_events(backend, &invocation()));
        if let Some(receipt) = backend.receipt(&invocation()) {
            return (drained, Some(receipt));
        }
        if Instant::now() >= deadline {
            return (drained, None);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

// ---------------------------------------------------------------- registry
// ---------------------------------------------------------------- registry

#[test]
fn codex_registry_pool_before_scan_is_unscanned() {
    let registry = CodexRegistry::new(agent_id(), offering(), "codex");
    let pool = registry.pool();
    assert_eq!(pool.entries(), []);
    assert_eq!(pool.empty_reason(), Some(EmptyPoolReason::NotScanned));
    assert!(registry.probe().is_none());
}

#[test]
fn codex_registry_scan_reports_ready_agent_with_configured_offering() {
    let dir = FixtureDir::new("registry-ready");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\nprintf 'codex-cli 0.154.0\\n'\nexit 0\n",
    );
    let mut registry = CodexRegistry::new(agent_id(), offering(), codex.to_string_lossy().as_ref());

    let pool = registry.scan().expect("the scan runs");

    assert_eq!(pool.entries().len(), 1);
    let entry = &pool.entries()[0];
    assert_eq!(entry.agent(), &agent_id());
    assert!(entry.exclusion().is_none());
    assert_eq!(
        pool.eligibility(&agent_id()),
        PoolEligibility::Eligible {
            offering: offering(),
        }
    );
    assert_eq!(
        registry.probe(),
        Some(&CodexProbe::Ready {
            version: "codex-cli 0.154.0".to_owned(),
        })
    );
    // The pool snapshot persists after the scan without re-probing.
    assert_eq!(registry.pool(), pool);
}

#[test]
fn codex_registry_scan_reports_not_ready_when_executable_absent() {
    let mut registry =
        CodexRegistry::new(agent_id(), offering(), "ymp-codex-absent-from-path-0f1e2d");

    let pool = registry.scan().expect("the scan runs");

    // Exclusion is a fact about serving, not a denial of identity.
    assert_eq!(pool.entries().len(), 1);
    match pool.eligibility(&agent_id()) {
        PoolEligibility::Excluded(ymp_runtime::ExclusionReason::NotReady { detail }) => {
            assert!(
                detail.contains("cannot spawn"),
                "typed not-ready detail mentions the spawn failure: {detail}"
            );
        }
        other => panic!("expected a typed not-ready exclusion, got {other:?}"),
    }
}

#[test]
fn codex_registry_scan_reports_not_ready_when_version_probe_times_out() {
    let dir = FixtureDir::new("registry-timeout");
    let codex = dir.executable("codex", "#!/bin/sh\nsleep 30\nexit 0\n");
    let mut registry = CodexRegistry::new(agent_id(), offering(), codex.to_string_lossy().as_ref())
        .with_probe_timeout(Duration::from_millis(200));

    let pool = registry.scan().expect("the scan runs");

    match pool.eligibility(&agent_id()) {
        PoolEligibility::Excluded(ymp_runtime::ExclusionReason::NotReady { detail }) => {
            assert!(
                detail.contains("did not answer"),
                "typed not-ready detail mentions the timeout: {detail}"
            );
        }
        other => panic!("expected a typed not-ready exclusion, got {other:?}"),
    }
}

#[test]
fn codex_registry_scan_reports_not_ready_on_probe_failure() {
    let dir = FixtureDir::new("registry-failing");
    let codex = dir.executable("codex", "#!/bin/sh\nexit 7\n");
    let mut registry = CodexRegistry::new(agent_id(), offering(), codex.to_string_lossy().as_ref());

    let pool = registry.scan().expect("the scan runs");

    match pool.eligibility(&agent_id()) {
        PoolEligibility::Excluded(ymp_runtime::ExclusionReason::NotReady { detail }) => {
            assert!(
                detail.contains("exited with status"),
                "typed not-ready detail mentions the failing exit: {detail}"
            );
        }
        other => panic!("expected a typed not-ready exclusion, got {other:?}"),
    }
}

// ---------------------------------------------------------------- backend

#[test]
fn codex_backend_streams_and_completes() {
    let dir = FixtureDir::new("backend-happy");
    let codex = happy_codex(&dir);
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    let (observations, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the receipt arrives after exit");

    // Stream mapping: turn count, agent-message output, completed
    // termination, writes ended — in observation order.
    assert!(observations.contains(&ExecutionObservation::UsageObserved {
        usage: ymp_runtime::ObservedUsage::unknown().with_turns(1),
    }));
    assert!(observations.contains(&ExecutionObservation::OutputObserved { chars: 4 }));
    assert!(observations.contains(&ExecutionObservation::Terminated {
        termination: Termination::Completed,
    }));
    assert!(observations.contains(&ExecutionObservation::WritesEnded));
    assert_eq!(observations.len(), 4);

    // Receipt: observed usage, no invented settings report.
    assert_eq!(receipt.termination(), &Termination::Completed);
    assert_eq!(receipt.reported_settings(), None);
    let usage = receipt.usage();
    assert_eq!(usage.turns(), Some(1));
    assert_eq!(usage.output_chars(), Some(4));
    assert!(usage.wall_clock().is_some());

    // Adapter-side evidence: native identifiers and token totals preserved.
    assert_eq!(
        backend.thread_id(&invocation()).as_deref(),
        Some("0199a213-81c0-7800-8aa1-bbab2a035a53")
    );
    assert_eq!(
        backend.token_totals(&invocation()),
        Some(CodexTokenUsage {
            input_tokens: Some(42),
            cached_input_tokens: Some(40),
            output_tokens: Some(7),
            reasoning_output_tokens: Some(0),
        })
    );
    assert_eq!(
        backend.stream_stats(&invocation()),
        Some(CodexStreamStats::default())
    );
}

#[test]
fn codex_backend_maps_error_event_to_failed_class() {
    let dir = FixtureDir::new("backend-error");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         printf '%s\\n' '{\"type\":\"error\",\"message\":\"usage limit reached\"}'\n\
         exit 1\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    let (observations, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the receipt arrives after exit");

    let failed = Termination::Failed {
        class: ErrorClass::new("codex-error-event").expect("valid error class"),
    };
    assert!(observations.contains(&ExecutionObservation::Terminated {
        termination: failed.clone(),
    }));
    assert_eq!(receipt.termination(), &failed);
    // The class states the class. The free-form `message` is never kept:
    // without an allowlisted protocol code the adapter-side code is the
    // stable generic marker, and the dropped text is only counted.
    assert_eq!(
        backend.failure_code(&invocation()).as_deref(),
        Some("codex-failure-unspecified")
    );
    assert_eq!(
        backend.stream_stats(&invocation()),
        Some(CodexStreamStats {
            unmapped_lines: 0,
            malformed_lines: 0,
            dropped_failure_details: 1,
        })
    );
}

#[test]
fn codex_backend_maps_turn_failed_event_to_failed_class() {
    let dir = FixtureDir::new("backend-turn-failed");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\nprintf '%s\\n' '{\"type\":\"turn.failed\"}'\nexit 1\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    let (_, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the receipt arrives after exit");

    assert_eq!(
        receipt.termination(),
        &Termination::Failed {
            class: ErrorClass::new("codex-turn-failed").expect("valid error class"),
        }
    );
    // No allowlisted code and no free-form text in this bare event: the
    // generic marker, and nothing dropped to count.
    assert_eq!(
        backend.failure_code(&invocation()).as_deref(),
        Some("codex-failure-unspecified")
    );
    assert_eq!(
        backend.stream_stats(&invocation()),
        Some(CodexStreamStats {
            unmapped_lines: 0,
            malformed_lines: 0,
            dropped_failure_details: 0,
        })
    );
}

#[test]
fn codex_backend_error_event_with_allowlisted_code_never_persists_raw_text() {
    let dir = FixtureDir::new("backend-leak");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         printf '%s\\n' '{\"type\":\"error\",\"message\":\"request failed for key sk-LEAK-0000000000000000\",\"codexErrorInfo\":\"usageLimitExceeded\"}'\n\
         printf 'stderr diagnostic sk-LEAK-STDERR\\n' >&2\n\
         exit 1\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    let (observations, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the receipt arrives after exit");

    // The allowlisted protocol code survives; the class states the class.
    assert_eq!(
        receipt.termination(),
        &Termination::Failed {
            class: ErrorClass::new("codex-error-event").expect("valid error class"),
        }
    );
    assert_eq!(
        backend.failure_code(&invocation()).as_deref(),
        Some("usageLimitExceeded")
    );
    // The free-form message was seen and dropped: counted, never stored.
    assert_eq!(
        backend.stream_stats(&invocation()),
        Some(CodexStreamStats {
            unmapped_lines: 0,
            malformed_lines: 0,
            dropped_failure_details: 1,
        })
    );
    // The fake secret marker appears nowhere: not in the receipt, the
    // termination, any observation or any adapter-side evidence field.
    let mut surfaced = format!("{receipt:?}{observations:?}");
    surfaced.push_str(&backend.failure_code(&invocation()).unwrap_or_default());
    if let Some(thread) = backend.thread_id(&invocation()) {
        surfaced.push_str(&thread);
    }
    if let Some(stats) = backend.stream_stats(&invocation()) {
        surfaced.push_str(&format!("{stats:?}"));
    }
    assert!(
        !surfaced.contains("sk-LEAK"),
        "raw provider text must never reach a receipt, termination or diagnostic: {surfaced}"
    );
}

#[test]
fn codex_backend_object_form_error_code_maps_to_allowlisted_key() {
    let dir = FixtureDir::new("backend-object-code");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         printf '%s\\n' '{\"type\":\"error\",\"message\":\"stream broke after sk-LEAK-MESSAGE\",\"codexErrorInfo\":{\"responseStreamDisconnected\":{\"httpStatusCode\":503,\"diagnostic\":\"sk-LEAK-PAYLOAD\"}}}'\n\
         exit 1\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    let (observations, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the receipt arrives after exit");

    // An object-form `codexErrorInfo` keeps only its allowlisted key; the
    // nested payload and the message are dropped.
    assert_eq!(
        receipt.termination(),
        &Termination::Failed {
            class: ErrorClass::new("codex-error-event").expect("valid error class"),
        }
    );
    assert_eq!(
        backend.failure_code(&invocation()).as_deref(),
        Some("responseStreamDisconnected")
    );
    let mut surfaced = format!("{receipt:?}{observations:?}");
    surfaced.push_str(&backend.failure_code(&invocation()).unwrap_or_default());
    assert!(
        !surfaced.contains("sk-LEAK"),
        "neither the message nor the nested payload may be surfaced: {surfaced}"
    );
}

#[test]
fn codex_backend_maps_nonzero_exit_without_error_event() {
    let dir = FixtureDir::new("backend-nonzero");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\nprintf 'fatal sk-LEAK-STDERR diagnostic\\n' >&2\nexit 3\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    let (_, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the receipt arrives after exit");

    assert_eq!(
        receipt.termination(),
        &Termination::Failed {
            class: ErrorClass::new("codex-nonzero-exit").expect("valid error class"),
        }
    );
    // No failure event was seen, so there is no failure code; and stderr is
    // drained to a sink, never retained, so the child's stderr text must
    // not exist anywhere on the backend.
    assert_eq!(backend.failure_code(&invocation()), None);
    let mut surfaced = format!("{receipt:?}");
    if let Some(stats) = backend.stream_stats(&invocation()) {
        surfaced.push_str(&format!("{stats:?}"));
    }
    assert!(
        !surfaced.contains("sk-LEAK-STDERR"),
        "no stderr text may appear in any receipt or statistic: {surfaced}"
    );
}

#[test]
fn codex_backend_counts_unmapped_and_malformed_lines() {
    let dir = FixtureDir::new("backend-noise");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         printf '%s\\n' '{\"type\":\"noise\"}' 'not json at all' \\\n\
         '{\"type\":\"item.completed\",\"item\":{\"id\":\"i\",\"type\":\"plan_update\"}}'\n\
         exit 0\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    let (observations, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the receipt arrives after exit");

    // Unverified shapes map to no observation: nothing is guessed.
    assert!(
        observations
            .iter()
            .all(|observation| !matches!(observation, ExecutionObservation::OutputObserved { .. }))
    );
    assert_eq!(receipt.termination(), &Termination::Completed);
    assert_eq!(
        backend.stream_stats(&invocation()),
        Some(CodexStreamStats {
            unmapped_lines: 2,
            malformed_lines: 1,
            dropped_failure_details: 0,
        })
    );
}

#[test]
fn codex_backend_passes_admitted_model_setting() {
    let dir = FixtureDir::new("backend-model");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         seen_model=0\n\
         prev=\"\"\n\
         for arg in \"$@\"; do\n\
           if [ \"$prev\" = \"-m\" ] && [ \"$arg\" = \"gpt-5.2\" ]; then seen_model=1; fi\n\
           prev=\"$arg\"\n\
         done\n\
         if [ \"$seen_model\" -ne 1 ]; then printf 'missing -m gpt-5.2\\n' >&2; exit 9; fi\n\
         printf '%s\\n' '{\"type\":\"turn.started\"}' \\\n\
           '{\"type\":\"item.completed\",\"item\":{\"id\":\"i\",\"type\":\"agent_message\",\"text\":\"pong\"}}' \\\n\
           '{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":1,\"cached_input_tokens\":1,\"output_tokens\":1,\"reasoning_output_tokens\":0}}'\n\
         exit 0\n",
    );
    let mut backend = backend_for(&codex);
    let mut settings = Settings::new();
    settings.insert(
        ymp_runtime::SettingKey::new("model").expect("valid setting key"),
        ymp_runtime::SettingValue::new("gpt-5.2").expect("valid setting value"),
    );

    backend
        .start(&backend_invocation(&dir, settings))
        .expect("the invocation starts");
    let (_, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));

    let receipt = receipt.expect("the receipt arrives after exit");
    assert_eq!(receipt.termination(), &Termination::Completed);
    assert_eq!(receipt.usage().output_chars(), Some(4));
}

#[test]
fn codex_backend_refuses_unsupported_sent_settings() {
    let dir = FixtureDir::new("backend-unsupported");
    let codex = happy_codex(&dir);
    let mut backend = backend_for(&codex);
    let mut settings = Settings::new();
    settings.insert(
        ymp_runtime::SettingKey::new("effort").expect("valid setting key"),
        ymp_runtime::SettingValue::new("high").expect("valid setting value"),
    );

    let failure = backend
        .start(&backend_invocation(&dir, settings))
        .expect_err("unsupported settings are refused");

    // The refusal happens before any spawn, so it confirms the invocation
    // never started.
    assert!(failure.confirmed_never_started());
    assert_eq!(
        failure.class(),
        &ErrorClass::new("unsupported-sent-setting").expect("valid error class")
    );
    assert!(
        failure.detail().contains("effort"),
        "the refusal names the unsupported key: {}",
        failure.detail()
    );
    assert!(backend.next_event(&invocation()).is_none());
    assert!(backend.receipt(&invocation()).is_none());
}

#[test]
fn codex_backend_start_fails_typed_when_executable_missing() {
    let dir = FixtureDir::new("backend-spawn-missing");
    let mut backend = CodexBackend::new(
        "ymp-codex-absent-from-path-0f1e2d",
        Arc::new(ManualClock::new()),
    )
    .with_prompt("Reply with the single word: pong");

    let failure = backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect_err("a missing executable is a typed start failure");

    assert!(failure.confirmed_never_started());
    assert_eq!(
        failure.class(),
        &ErrorClass::new("codex-spawn-failed").expect("valid error class")
    );
}

#[test]
fn codex_backend_start_fails_typed_when_workspace_missing() {
    let dir = FixtureDir::new("backend-workspace-missing");
    let codex = happy_codex(&dir);
    let mut backend = backend_for(&codex);
    let missing = WorkspaceScope::new(
        dir.path()
            .join("does-not-exist")
            .to_string_lossy()
            .into_owned(),
    )
    .expect("valid workspace scope");
    let invocation = BackendInvocation::new(
        invocation(),
        agent_id(),
        Settings::new(),
        workspace_accesses(missing.clone()),
        limits(),
    );

    let failure = backend
        .start(&invocation)
        .expect_err("a missing workspace directory is a typed start failure");

    assert!(failure.confirmed_never_started());
    assert_eq!(
        failure.class(),
        &ErrorClass::new("workspace-dir-missing").expect("valid error class")
    );

    std::fs::create_dir_all(missing.as_str()).expect("the missing workspace is created");
    let retry = backend
        .start(&invocation)
        .expect_err("the retry returns the first start failure");
    assert_eq!(retry, failure);
    assert!(backend.receipt(invocation.invocation()).is_none());
}

#[test]
fn codex_backend_start_fails_typed_without_prompt() {
    let dir = FixtureDir::new("backend-prompt-missing");
    let codex = happy_codex(&dir);
    let mut backend = CodexBackend::new(
        codex.to_string_lossy().into_owned(),
        Arc::new(ManualClock::new()),
    );

    let failure = backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect_err("a missing prompt is a typed start failure");

    assert!(failure.confirmed_never_started());
    assert_eq!(
        failure.class(),
        &ErrorClass::new("missing-prompt").expect("valid error class")
    );
}

#[test]
fn codex_backend_replays_successful_start_for_duplicate_invocation() {
    let dir = FixtureDir::new("backend-duplicate");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\ntrap 'exit 0' TERM\nwhile :; do sleep 0.1; done\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the first start succeeds");
    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("a second start returns the first successful outcome");

    backend.cancel(&invocation()).expect("cleanup cancels");
    let _ = wait_for_observations(&mut backend, Duration::from_secs(5));
}

#[test]
fn codex_scenario_retries_after_invocation_started_append_failure() {
    let dir = FixtureDir::new("scenario-started-append-retry");
    let codex = happy_codex(&dir);
    let clock = ManualClock::new();
    let backend = backend_for(&codex);
    let workspace = WorkspaceScope::new(dir.path().to_string_lossy().into_owned())
        .expect("valid workspace scope");
    let access = WorkspaceAccess::new(workspace.clone(), [WorkspaceOperation::Write])
        .expect("valid workspace access");
    let provider = ScriptedProvider::new(agent_id(), offering())
        .with_effective_workspace_accesses([access.clone()]);
    let journal = FailStartedOnceJournal::new();
    let scenario = ExecutionScenario::over(
        journal.clone(),
        backend,
        [access.clone()],
        ScriptedRegistry::new([provider]),
        ResourceAmount::new(1),
        Arc::new(clock),
    );
    let session_id = SessionId::new("codex-started-append-retry").expect("valid session ID");
    scenario
        .open_session(session_id.clone(), execution_task())
        .expect("session opens");
    scenario.scan().expect("registry scan succeeds");
    let request = AssignmentRequest::new(
        agent_id(),
        Role::new("implementer").expect("valid role"),
        Settings::new(),
        Allowance::new(
            ResourceAmount::new(1),
            ReservationPurpose::Production,
            limits(),
        )
        .expect("valid allowance"),
        vec![access],
    )
    .expect("valid assignment request");
    let assignment = scenario
        .admit(&session_id, request, Revision::new(1))
        .expect("admission commits");

    assert!(matches!(
        scenario
            .invoke(&session_id, assignment.invocation(), Revision::new(2))
            .expect_err("the first InvocationStarted append fails"),
        ExecutionError::Journal(JournalError::AdapterFailure { .. })
    ));
    assert!(matches!(
        journal
            .read(&session_id)
            .expect("history reads")
            .last()
            .expect("the start attempt exists")
            .event(),
        SessionEvent::InvocationStartAttempted { .. }
    ));

    assert_eq!(
        scenario
            .invoke(&session_id, assignment.invocation(), Revision::new(3))
            .expect("the retry returns the original successful start"),
        StartOutcome::Started
    );
    assert_eq!(
        journal
            .read(&session_id)
            .expect("history reads")
            .iter()
            .filter(|entry| matches!(entry.event(), SessionEvent::InvocationStarted { .. }))
            .count(),
        1
    );
}

// ---------------------------------------------------------------- cancel

#[test]
fn codex_cancel_observes_cancelled_termination() {
    let dir = FixtureDir::new("cancel-graceful");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         trap 'exit 0' TERM\n\
         printf '%s\\n' '{\"type\":\"thread.started\",\"thread_id\":\"thread-cancel\"}'\n\
         while :; do sleep 0.1 2>/dev/null || sleep 1; done\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");
    backend
        .cancel(&invocation())
        .expect("the backend acknowledges the cancellation request");

    let (observations, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the exit is observed");

    // The termination is claimed only because the child's exit was observed.
    assert!(observations.contains(&ExecutionObservation::Terminated {
        termination: Termination::Cancelled,
    }));
    assert!(observations.contains(&ExecutionObservation::WritesEnded));
    assert_eq!(receipt.termination(), &Termination::Cancelled);
}

#[test]
fn codex_cancel_escalates_to_sigkill_when_term_is_ignored() {
    let dir = FixtureDir::new("cancel-escalate");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         trap '' TERM\n\
         printf '%s\\n' '{\"type\":\"turn.started\"}'\n\
         while :; do sleep 0.1 2>/dev/null || sleep 1; done\n",
    );
    let mut backend = backend_for(&codex); // grace 750 ms

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");
    backend
        .cancel(&invocation())
        .expect("the backend acknowledges the cancellation request");

    let (_, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("SIGKILL makes the exit observable");

    assert_eq!(receipt.termination(), &Termination::Cancelled);
}

#[test]
fn codex_cancel_refuses_unknown_invocation() {
    let dir = FixtureDir::new("cancel-unknown");
    let codex = happy_codex(&dir);
    let mut backend = backend_for(&codex);

    let refused = backend
        .cancel(&invocation())
        .expect_err("cancelling an unknown invocation is refused");
    assert!(
        refused.detail().contains("does not know"),
        "the refusal states its cause: {}",
        refused.detail()
    );
}

#[test]
fn codex_backend_drains_large_stderr_without_deadlock() {
    let dir = FixtureDir::new("backend-stderr");
    let codex = dir.executable(
        "codex",
        "#!/bin/sh\n\
         i=0\n\
         while [ $i -lt 2048 ]; do printf '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef' >&2; i=$((i+1)); done\n\
         printf '%s\\n' '{\"type\":\"turn.started\"}' \\\n\
           '{\"type\":\"item.completed\",\"item\":{\"id\":\"i\",\"type\":\"agent_message\",\"text\":\"pong\"}}'\n\
         exit 0\n",
    );
    let mut backend = backend_for(&codex);

    backend
        .start(&backend_invocation(&dir, Settings::new()))
        .expect("the invocation starts");

    // ~108 KiB of stderr exceeds the pipe buffer; without the drainer
    // thread the child would block forever and never complete.
    let (_, receipt) = wait_for_observations(&mut backend, Duration::from_secs(5));
    let receipt = receipt.expect("the child completes without blocking");
    assert_eq!(receipt.termination(), &Termination::Completed);
}

// ---------------------------------------------------- real provider (cost)

/// REAL PROVIDER INVOCATION — SPENDS REAL QUOTA.
///
/// This test is `#[ignore]`d and never runs in unattended gates (criterion
/// 8: real provider execution must never run in unattended tests). It runs
/// one tiny real `codex exec --json` invocation ("Reply with the single
/// word: pong") in a temp directory against the owner's natively
/// authenticated Codex CLI, using the provider's own default model and
/// sandbox. It requires the owner's explicit go:
///
/// ```text
/// cargo test -p ymp-runtime codex_ -- --ignored --nocapture
/// ```
#[test]
#[ignore = "spends real provider quota; requires the owner's explicit go"]
fn codex_real_invocation_completes_with_output() {
    let dir = FixtureDir::new("real");
    let mut registry = CodexRegistry::new(agent_id(), offering(), "codex");
    let pool = registry.scan().expect("the scan runs");
    assert_eq!(
        pool.eligibility(&agent_id()),
        PoolEligibility::Eligible {
            offering: offering()
        },
        "the real codex CLI must be present and ready; probe: {:?}",
        registry.probe()
    );
    println!("codex probe: {:?}", registry.probe());

    let mut backend = CodexBackend::new("codex", Arc::new(ymp_runtime::SystemClock::new()))
        .with_prompt("Reply with the single word: pong");
    let real_invocation = BackendInvocation::new(
        invocation(),
        agent_id(),
        Settings::new(),
        workspace_accesses(
            WorkspaceScope::new(dir.path().to_string_lossy().into_owned())
                .expect("valid workspace scope"),
        ),
        InvocationLimits::new(4, 4_000, Duration::from_secs(240)).expect("valid limits"),
    );

    backend
        .start(&real_invocation)
        .expect("the real invocation starts");

    let deadline = Instant::now() + Duration::from_secs(240);
    let mut observations = Vec::new();
    let receipt = loop {
        observations.extend(drain_events(&mut backend, real_invocation.invocation()));
        if let Some(receipt) = backend.receipt(real_invocation.invocation()) {
            break receipt;
        }
        assert!(
            Instant::now() < deadline,
            "the real invocation did not report a receipt in time; observations so far: \
             {observations:?}; thread {:?}; tokens {:?}; failure code {:?}",
            backend.thread_id(real_invocation.invocation()),
            backend.token_totals(real_invocation.invocation()),
            backend.failure_code(real_invocation.invocation()),
        );
        std::thread::sleep(Duration::from_millis(100));
    };

    for observation in &observations {
        println!("observation: {observation:?}");
    }
    println!("receipt: {receipt:?}");
    println!(
        "token totals: {:?}; thread: {:?}",
        backend.token_totals(real_invocation.invocation()),
        backend.thread_id(real_invocation.invocation()),
    );

    assert_eq!(receipt.termination(), &Termination::Completed);
    let output: u64 = observations
        .iter()
        .map(|observation| match observation {
            ExecutionObservation::OutputObserved { chars } => *chars,
            _ => 0,
        })
        .sum();
    assert!(
        output >= 4,
        "some agent output is observed (at least the word 'pong')"
    );
}
