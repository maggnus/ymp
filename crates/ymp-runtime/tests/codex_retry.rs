//! Offline protocol tests for the Codex App Server adapter.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use ymp_runtime::{
    AgentId, BackendInvocation, CodexBackend, CodexProbe, CodexRegistry, CodexStreamStats,
    ErrorClass, ExecutionBackend, ExecutionObservation, InvocationId, InvocationLimits,
    ManualClock, ModelOffering, OfferingId, PoolEligibility, Receipt, Registry, SettingKey,
    SettingValue, Settings, Termination, WorkspaceAccess, WorkspaceOperation, WorkspaceScope,
};

struct Fixture {
    root: PathBuf,
    executable: PathBuf,
}

impl Fixture {
    fn new(tag: &str, scenario: Value) -> Self {
        let unique = format!(
            "ymp-app-server-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock follows the epoch")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).expect("fixture directory is created");
        std::fs::write(root.join("scenario.json"), scenario.to_string())
            .expect("scenario is written");
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex_app_server.py");
        let executable = root.join("codex");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\nYMP_CODEX_FIXTURE={} exec python3 {} \"$@\"\n",
                shell_quote(&root),
                shell_quote(&fixture)
            ),
        )
        .expect("wrapper is written");
        make_executable(&executable);
        Self { root, executable }
    }

    fn requests(&self) -> Vec<Value> {
        std::fs::read_to_string(self.root.join("requests.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).expect("logged request is JSON"))
            .collect()
    }

    fn methods(&self) -> Vec<String> {
        self.requests()
            .iter()
            .filter_map(|request| request.get("method").and_then(Value::as_str))
            .map(str::to_owned)
            .collect()
    }

    fn pid(&self, name: &str) -> u32 {
        wait_for_file(&self.root.join(name), Duration::from_secs(5))
            .trim()
            .parse()
            .expect("fixture PID is numeric")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("fixture is executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {
    panic!("Codex process tests require a Unix host");
}

fn base_scenario() -> Value {
    json!({"notifications": [message_completed("pong"), turn_completed("completed")]})
}

fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

fn message_delta(text: &str) -> Value {
    notification(
        "item/agentMessage/delta",
        json!({"threadId": "thread-fixture", "turnId": "turn-fixture", "delta": text}),
    )
}

fn message_completed(text: &str) -> Value {
    notification(
        "item/completed",
        json!({
            "threadId": "thread-fixture",
            "turnId": "turn-fixture",
            "item": {"type": "agentMessage", "text": text}
        }),
    )
}

fn turn_completed(status: &str) -> Value {
    notification(
        "turn/completed",
        json!({
            "threadId": "thread-fixture",
            "turn": {"id": "turn-fixture", "status": status}
        }),
    )
}

fn usage(thread: &str, turn: &str, input: u64, output: u64, last_input: u64) -> Value {
    notification(
        "thread/tokenUsage/updated",
        json!({
            "threadId": thread,
            "turnId": turn,
            "tokenUsage": {
                "total": {
                    "inputTokens": input,
                    "outputTokens": output,
                    "cachedInputTokens": 3,
                    "cacheWriteInputTokens": 2,
                    "reasoningOutputTokens": 1
                },
                "last": {"inputTokens": last_input, "outputTokens": 2}
            }
        }),
    )
}

fn retry(will_retry: Value, code: Value) -> Value {
    notification(
        "error",
        json!({
            "threadId": "thread-fixture",
            "turnId": "turn-fixture",
            "willRetry": will_retry,
            "error": {"codexErrorInfo": code, "message": "sensitive fixture detail"}
        }),
    )
}

fn agent() -> AgentId {
    AgentId::new("codex").expect("valid agent ID")
}

fn invocation_id() -> InvocationId {
    InvocationId::new("invocation-codex-1").expect("valid invocation ID")
}

fn offering() -> ModelOffering {
    ModelOffering::new(
        OfferingId::new("codex-cli-local").expect("valid offering ID"),
        Vec::new(),
    )
    .expect("valid offering")
}

fn settings(pairs: &[(&str, &str)]) -> Settings {
    Settings::from_pairs(pairs.iter().map(|(key, value)| {
        (
            SettingKey::new(*key).expect("valid setting key"),
            SettingValue::new(*value).expect("valid setting value"),
        )
    }))
    .expect("unique settings")
}

fn invocation(fixture: &Fixture, sent: Settings) -> BackendInvocation {
    let scope = WorkspaceScope::new(fixture.root.to_string_lossy().into_owned())
        .expect("valid workspace scope");
    BackendInvocation::new(
        invocation_id(),
        agent(),
        sent,
        vec![
            WorkspaceAccess::new(scope, [WorkspaceOperation::Write])
                .expect("valid workspace access"),
        ],
        InvocationLimits::new(8, 100_000, Duration::from_secs(30)).expect("valid limits"),
    )
}

fn backend(fixture: &Fixture) -> CodexBackend {
    CodexBackend::new(
        fixture.executable.to_string_lossy().into_owned(),
        Arc::new(ManualClock::new()),
    )
    .with_prompt("Reply with pong")
    .with_cancel_grace(Duration::from_millis(150))
}

fn drain(backend: &mut CodexBackend) -> Vec<ExecutionObservation> {
    let mut observations = Vec::new();
    while let Some(observation) = backend.next_event(&invocation_id()) {
        observations.push(observation);
    }
    backend.commit_scan(&invocation_id());
    observations
}

fn wait_for_receipt(
    backend: &mut CodexBackend,
    timeout: Duration,
) -> (Vec<ExecutionObservation>, Receipt) {
    let deadline = Instant::now() + timeout;
    let mut observations = Vec::new();
    loop {
        observations.extend(drain(backend));
        if let Some(receipt) = backend.receipt(&invocation_id()) {
            return (observations, receipt);
        }
        assert!(Instant::now() < deadline, "receipt did not arrive");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_file(path: &Path, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(contents) = std::fs::read_to_string(path) {
            return contents;
        }
        assert!(
            Instant::now() < deadline,
            "file {} did not appear",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn process_exists(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn wait_for_process_exit(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while process_exists(pid) {
        assert!(
            Instant::now() < deadline,
            "process {pid} survived group cleanup"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn registry_is_unscanned_before_explicit_scan() {
    let registry = CodexRegistry::new(agent(), offering(), "codex");
    assert!(registry.pool().entries().is_empty());
    assert!(registry.probe().is_none());
}

#[test]
fn registry_paginates_native_models_and_reports_controls() {
    let scenario = json!({
        "model_pages": [
            [{"model":"model-a","isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"low"},{"reasoningEffort":"high"}]}],
            [{"model":"model-b","isDefault":false,"supportedReasoningEfforts":[{"reasoningEffort":"high"},{"reasoningEffort":"max"}]}]
        ]
    });
    let fixture = Fixture::new("registry-pages", scenario);
    let mut registry = CodexRegistry::new(
        agent(),
        offering(),
        fixture.executable.to_string_lossy().as_ref(),
    );

    let pool = registry.scan().expect("scan completes");
    let PoolEligibility::Eligible { offering } = pool.eligibility(&agent()) else {
        panic!("fixture should be eligible");
    };
    let controls = offering.controls().collect::<Vec<_>>();
    assert_eq!(
        controls
            .iter()
            .find(|(key, _)| key.as_str() == "model")
            .unwrap()
            .1,
        [
            SettingValue::new("model-a").unwrap(),
            SettingValue::new("model-b").unwrap()
        ]
    );
    assert!(offering.supports(
        &SettingKey::new("effort").unwrap(),
        &SettingValue::new("high").unwrap()
    ));
    assert!(!offering.supports(
        &SettingKey::new("effort").unwrap(),
        &SettingValue::new("max").unwrap()
    ));
    assert_eq!(
        registry.probe(),
        Some(&CodexProbe::Ready {
            version: "codex-fixture 1.0".to_owned()
        })
    );
    assert_eq!(
        fixture.methods(),
        ["initialize", "initialized", "model/list", "model/list"]
    );
}

#[test]
fn registry_reports_missing_executable_as_not_ready() {
    let mut registry = CodexRegistry::new(agent(), offering(), "missing-codex-fixture-8821");
    let pool = registry.scan().expect("scan returns a typed pool");
    assert!(matches!(
        pool.eligibility(&agent()),
        PoolEligibility::Excluded(_)
    ));
    assert!(
        registry
            .probe()
            .unwrap()
            .not_ready_detail()
            .unwrap()
            .contains("cannot spawn")
    );
}

#[test]
fn registry_bounds_a_stalled_initialize() {
    let fixture = Fixture::new("registry-timeout", json!({"hang_initialize": true}));
    let mut registry = CodexRegistry::new(
        agent(),
        offering(),
        fixture.executable.to_string_lossy().as_ref(),
    )
    .with_probe_timeout(Duration::from_millis(100));
    let started = Instant::now();
    let pool = registry.scan().expect("scan returns a typed pool");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches!(
        pool.eligibility(&agent()),
        PoolEligibility::Excluded(_)
    ));
}

#[test]
fn registry_rejects_a_repeated_pagination_cursor() {
    let fixture = Fixture::new("registry-cursor", json!({"repeat_cursor": true}));
    let mut registry = CodexRegistry::new(
        agent(),
        offering(),
        fixture.executable.to_string_lossy().as_ref(),
    );
    let pool = registry.scan().expect("scan returns a typed pool");
    assert!(matches!(
        pool.eligibility(&agent()),
        PoolEligibility::Excluded(_)
    ));
    assert!(
        registry
            .probe()
            .unwrap()
            .not_ready_detail()
            .unwrap()
            .contains("repeated")
    );
}

#[test]
fn json_rpc_error_code_cannot_surface_arbitrary_content() {
    let fixture = Fixture::new(
        "rpc-error-code",
        json!({
            "initialize_error": {
                "code": {"secret": "must-not-surface"},
                "message": "must-not-surface"
            }
        }),
    );
    let mut registry = CodexRegistry::new(
        agent(),
        offering(),
        fixture.executable.to_string_lossy().as_ref(),
    );
    registry.scan().unwrap();
    let detail = registry.probe().unwrap().not_ready_detail().unwrap();
    assert!(detail.contains("unreported"));
    assert!(!detail.contains("must-not-surface"));
}

#[test]
fn backend_completes_handshake_and_reports_session_settings_output_and_usage() {
    let scenario = json!({"notifications": [
        usage("thread-fixture", "turn-fixture", 12, 4, 12),
        message_completed("pong"),
        turn_completed("completed")
    ]});
    let fixture = Fixture::new("complete", scenario);
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .expect("handshake succeeds");
    let (observations, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));

    assert_eq!(receipt.termination(), &Termination::Completed);
    assert_eq!(receipt.session(), Some("thread-fixture"));
    assert_eq!(receipt.reported_settings(), Some(&Settings::new()));
    assert_eq!(receipt.usage().input_tokens(), Some(12));
    assert_eq!(receipt.usage().output_tokens(), Some(4));
    assert_eq!(receipt.usage().output_chars(), Some(4));
    assert!(
        observations.contains(&ExecutionObservation::SettingsReported {
            settings: Settings::new()
        })
    );
    assert!(observations.contains(&ExecutionObservation::WritesEnded));
    assert_eq!(
        fixture.methods(),
        [
            "initialize",
            "initialized",
            "model/list",
            "thread/start",
            "turn/start"
        ]
    );
}

#[test]
fn backend_sends_model_and_effort_from_the_sent_settings_table() {
    let fixture = Fixture::new("settings", base_scenario());
    let sent = settings(&[("model", "model-a"), ("effort", "high")]);
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, sent.clone()))
        .expect("settings are advertised");
    let (observations, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    let requests = fixture.requests();
    let thread = requests
        .iter()
        .find(|request| request["method"] == "thread/start")
        .unwrap();
    let turn = requests
        .iter()
        .find(|request| request["method"] == "turn/start")
        .unwrap();
    assert_eq!(thread["params"]["model"], "model-a");
    assert_eq!(thread["params"]["config"]["model_reasoning_effort"], "high");
    assert_eq!(turn["params"]["model"], "model-a");
    assert_eq!(turn["params"]["effort"], "high");
    assert_eq!(receipt.reported_settings(), Some(&sent));
    assert!(observations.contains(&ExecutionObservation::SettingsReported { settings: sent }));
}

#[test]
fn backend_rejects_unadvertised_effort_before_turn_start() {
    let fixture = Fixture::new("invalid-effort", base_scenario());
    let mut backend = backend(&fixture);
    let failure = backend
        .start(&invocation(
            &fixture,
            settings(&[("model", "model-a"), ("effort", "max")]),
        ))
        .expect_err("unadvertised effort is rejected");
    assert!(failure.confirmed_never_started());
    assert!(
        !fixture
            .methods()
            .iter()
            .any(|method| method == "turn/start")
    );
}

#[test]
fn backend_rejects_negotiated_model_drift_before_turn_start() {
    let fixture = Fixture::new(
        "model-drift",
        json!({"thread_response": {"model": "other-model"}}),
    );
    let mut backend = backend(&fixture);
    let failure = backend
        .start(&invocation(&fixture, settings(&[("model", "model-a")])))
        .expect_err("model drift is rejected");
    assert!(!failure.confirmed_never_started());
    assert!(
        !fixture
            .methods()
            .iter()
            .any(|method| method == "turn/start")
    );
}

#[test]
fn backend_resumes_the_requested_session_and_returns_the_native_session() {
    let fixture = Fixture::new("resume", base_scenario());
    let mut backend = backend(&fixture);
    let resumed = invocation(&fixture, Settings::new()).with_session("thread-existing");
    backend.start(&resumed).expect("resume handshake succeeds");
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    let request = fixture
        .requests()
        .into_iter()
        .find(|request| request["method"] == "thread/resume")
        .expect("resume request is sent");
    assert_eq!(request["params"]["threadId"], "thread-existing");
    assert_eq!(receipt.session(), Some("thread-fixture"));
    assert_eq!(
        backend.thread_id(&invocation_id()).as_deref(),
        Some("thread-fixture")
    );
}

#[test]
fn backend_filters_other_threads_and_uses_restored_resume_baseline() {
    let scenario = json!({"notifications": [
        usage("other-thread", "other-turn", 5000, 500, 100),
        usage("thread-fixture", "restored-turn", 1000, 100, 100),
        usage("thread-fixture", "turn-fixture", 1100, 110, 100),
        turn_completed("completed")
    ]});
    let fixture = Fixture::new("filter", scenario);
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()).with_session("thread-existing"))
        .expect("resume starts");
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(receipt.usage().input_tokens(), Some(100));
    assert_eq!(receipt.usage().output_tokens(), Some(10));
    assert!(!receipt.usage().is_partial());
}

#[test]
fn will_retry_is_nonterminal_and_repeated_usage_does_not_double_count() {
    let scenario = json!({
        "before_turn_response": true,
        "notifications": [
            usage("thread-fixture", "turn-fixture", 10, 2, 10),
            retry(json!(true), json!("serverOverloaded")),
            usage("thread-fixture", "turn-fixture", 10, 2, 10),
            usage("thread-fixture", "turn-fixture", 15, 5, 15),
            message_completed("done"),
            turn_completed("completed")
        ]
    });
    let fixture = Fixture::new("retry", scenario);
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .expect("retry notification before response is preserved");
    let (observations, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(receipt.termination(), &Termination::Completed);
    assert_eq!(receipt.usage().input_tokens(), Some(15));
    assert_eq!(receipt.usage().output_tokens(), Some(5));
    assert!(observations.contains(&ExecutionObservation::RetryObserved {
        session: "thread-fixture".to_owned(),
        turn: "turn-fixture".to_owned(),
        error_code: Some("serverOverloaded".to_owned())
    }));
}

#[test]
fn will_retry_requires_a_literal_boolean_true() {
    for value in [json!(false), Value::Null, json!("true")] {
        let fixture = Fixture::new(
            "retry-terminal",
            json!({"notifications": [retry(value, json!("serverOverloaded"))]}),
        );
        let mut backend = backend(&fixture);
        backend
            .start(&invocation(&fixture, Settings::new()))
            .unwrap();
        let (observations, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
        assert!(
            matches!(receipt.termination(), Termination::Failed { class }
            if class == &ErrorClass::new("codex-error-event").unwrap())
        );
        assert!(
            !observations
                .iter()
                .any(|item| matches!(item, ExecutionObservation::RetryObserved { .. }))
        );
    }
}

#[test]
fn retry_preserves_only_allowlisted_structured_error_codes() {
    let fixture = Fixture::new(
        "retry-code",
        json!({"notifications": [
            retry(json!(true), json!({"responseStreamDisconnected": {"status": 503}})),
            retry(json!(true), json!("sensitive-unknown-code")),
            turn_completed("completed")
        ]}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (observations, _) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    let codes = observations
        .iter()
        .filter_map(|item| match item {
            ExecutionObservation::RetryObserved { error_code, .. } => Some(error_code.as_deref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(codes, [Some("responseStreamDisconnected"), None]);
    assert!(!format!("{observations:?}").contains("sensitive"));
}

#[test]
fn terminal_error_retains_classification_without_free_form_text() {
    let fixture = Fixture::new(
        "terminal-error",
        json!({"notifications": [retry(json!(false), json!("serverOverloaded"))]}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert!(
        matches!(receipt.termination(), Termination::Failed { class }
        if class == &ErrorClass::new("codex-error-event").unwrap())
    );
    assert_eq!(
        backend.failure_code(&invocation_id()).as_deref(),
        Some("serverOverloaded")
    );
    assert_eq!(
        backend
            .stream_stats(&invocation_id())
            .unwrap()
            .dropped_failure_details,
        1
    );
}

#[test]
fn failed_turn_is_a_typed_termination() {
    let fixture = Fixture::new(
        "failed-turn",
        json!({"notifications": [turn_completed("failed")]}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert!(
        matches!(receipt.termination(), Termination::Failed { class }
        if class == &ErrorClass::new("codex-turn-failed").unwrap())
    );
}

#[test]
fn completed_turn_without_a_final_message_is_not_success() {
    let fixture = Fixture::new(
        "empty-response",
        json!({"notifications": [turn_completed("completed")]}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert!(
        matches!(receipt.termination(), Termination::Failed { class }
        if class == &ErrorClass::new("codex-empty-response").unwrap())
    );
}

#[test]
fn output_deltas_are_not_double_counted_by_the_completed_item() {
    let fixture = Fixture::new(
        "delta",
        json!({"notifications": [
            message_delta("po"),
            message_delta("ng"),
            message_completed("pong"),
            turn_completed("completed")
        ]}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(receipt.usage().output_chars(), Some(4));
}

#[test]
fn invalid_json_is_a_protocol_failure() {
    let fixture = Fixture::new("malformed", json!({"malformed_after_turn": true}));
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert!(
        matches!(receipt.termination(), Termination::Failed { class }
        if class == &ErrorClass::new("codex-protocol-failure").unwrap())
    );
    assert_eq!(
        backend
            .stream_stats(&invocation_id())
            .unwrap()
            .malformed_lines,
        1
    );
}

#[test]
fn oversized_protocol_line_is_rejected() {
    let fixture = Fixture::new("oversized", json!({"oversized_after_turn": true}));
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(10));
    assert!(
        matches!(receipt.termination(), Termination::Failed { class }
        if class == &ErrorClass::new("codex-protocol-failure").unwrap())
    );
}

#[test]
fn duplicate_successful_start_returns_the_first_outcome() {
    let fixture = Fixture::new("duplicate-success", base_scenario());
    let mut backend = backend(&fixture);
    let request = invocation(&fixture, Settings::new());
    backend.start(&request).expect("first start succeeds");
    backend.start(&request).expect("duplicate returns success");
    let (_, _) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("effects.log")).unwrap(),
        "turn-started\n"
    );
}

#[test]
fn duplicate_failed_start_returns_the_first_unknown_outcome() {
    let fixture = Fixture::new(
        "duplicate-failure",
        json!({"thread_response": {"thread": {}}}),
    );
    let mut backend = backend(&fixture);
    let request = invocation(&fixture, Settings::new());
    let first = backend.start(&request).expect_err("first handshake fails");
    let second = backend
        .start(&request)
        .expect_err("duplicate returns first failure");
    assert_eq!(first, second);
    assert!(!second.confirmed_never_started());
    assert_eq!(
        fixture
            .methods()
            .iter()
            .filter(|method| *method == "initialize")
            .count(),
        1
    );
}

#[test]
fn arbitrary_cli_arguments_are_rejected_before_spawn() {
    let fixture = Fixture::new("bad-args", base_scenario());
    let mut backend = backend(&fixture).with_extra_args(["--dangerous-unknown".to_owned()]);
    let failure = backend
        .start(&invocation(&fixture, Settings::new()))
        .expect_err("unknown argument is rejected");
    assert!(failure.confirmed_never_started());
    assert_eq!(
        failure.class(),
        &ErrorClass::new("unsupported-codex-argument").unwrap()
    );
    assert!(!fixture.root.join("process.pid").exists());
}

#[test]
fn allowlisted_cli_arguments_map_to_thread_parameters() {
    let fixture = Fixture::new("allowed-args", base_scenario());
    let mut backend = backend(&fixture).with_extra_args(
        [
            "--ephemeral",
            "-c",
            "approval_policy=\"never\"",
            "--sandbox",
            "workspace-write",
            "-c",
            "sandbox_workspace_write.writable_roots=[]",
            "-c",
            "sandbox_workspace_write.exclude_slash_tmp=true",
            "-c",
            "sandbox_workspace_write.exclude_tmpdir_env_var=true",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let _ = wait_for_receipt(&mut backend, Duration::from_secs(5));
    let requests = fixture.requests();
    let request = requests
        .iter()
        .find(|request| request["method"] == "thread/start")
        .unwrap();
    assert_eq!(request["params"]["ephemeral"], true);
    assert_eq!(request["params"]["approvalPolicy"], "never");
    assert_eq!(request["params"]["sandbox"], "workspace-write");
    assert_eq!(
        request["params"]["config"]["sandbox_workspace_write"]["writable_roots"],
        json!([])
    );
    assert_eq!(
        serde_json::from_str::<Value>(
            &std::fs::read_to_string(fixture.root.join("argv.json")).unwrap()
        )
        .unwrap(),
        json!(["app-server", "--stdio"])
    );
}

#[test]
fn server_request_is_answered_while_turn_start_waits() {
    let fixture = Fixture::new(
        "server-push",
        json!({"server_request": true, "notifications": [turn_completed("completed")]}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let _ = wait_for_receipt(&mut backend, Duration::from_secs(5));
    let response: Value = serde_json::from_str(&wait_for_file(
        &fixture.root.join("server-response.json"),
        Duration::from_secs(5),
    ))
    .unwrap();
    assert_eq!(response["id"], 900);
    assert_eq!(response["result"]["decision"], "accept");
}

#[test]
fn observations_are_delivered_one_at_a_time_and_can_be_unread() {
    let fixture = Fixture::new("scan", base_scenario());
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let first = loop {
        if let Some(observation) = backend.next_event(&invocation_id()) {
            break observation;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    backend.unread_last(&invocation_id());
    assert_eq!(backend.next_event(&invocation_id()), Some(first));
    backend.reset_scan(&invocation_id());
    assert!(backend.next_event(&invocation_id()).is_some());
}

#[test]
fn cancellation_kills_the_owned_process_group() {
    let fixture = Fixture::new(
        "cancel-group",
        json!({"hang": true, "ignore_term": true, "spawn_descendant": true}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let parent = fixture.pid("process.pid");
    let descendant = fixture.pid("descendant.pid");
    backend
        .cancel(&invocation_id())
        .expect("cancellation is requested");
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(receipt.termination(), &Termination::Cancelled);
    wait_for_process_exit(parent);
    wait_for_process_exit(descendant);
}

#[test]
fn supervisor_observes_exit_without_waiting_for_stdout_eof_first() {
    let fixture = Fixture::new("eof-before-wait", json!({"close_stdout_then_hang": true}));
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    backend
        .cancel(&invocation_id())
        .expect("cancellation remains available after EOF");
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(receipt.termination(), &Termination::Cancelled);
}

#[test]
fn cancellation_does_not_replace_an_observed_turn_completion() {
    let fixture = Fixture::new(
        "completion-race",
        json!({
            "notifications": [message_completed("done"), turn_completed("completed")],
            "hang": true
        }),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    wait_for_file(
        &fixture.root.join("notifications.done"),
        Duration::from_secs(5),
    );
    std::thread::sleep(Duration::from_millis(100));
    backend
        .cancel(&invocation_id())
        .expect("late cancellation is an idempotent no-op");
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(receipt.termination(), &Termination::Completed);
}

#[test]
fn dropping_backend_kills_the_owned_process_group() {
    let fixture = Fixture::new(
        "drop-group",
        json!({"hang": true, "ignore_term": true, "spawn_descendant": true}),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let parent = fixture.pid("process.pid");
    let descendant = fixture.pid("descendant.pid");
    drop(backend);
    wait_for_process_exit(parent);
    wait_for_process_exit(descendant);
}

#[test]
fn stderr_is_drained_without_becoming_adapter_output() {
    let fixture = Fixture::new(
        "stderr",
        json!({
            "stderr_bytes": 200_000,
            "notifications": [message_completed("done"), turn_completed("completed")]
        }),
    );
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert_eq!(receipt.termination(), &Termination::Completed);
    assert_eq!(
        backend.stream_stats(&invocation_id()),
        Some(CodexStreamStats::default())
    );
}

#[test]
fn process_exit_without_turn_completion_is_not_success() {
    let fixture = Fixture::new("early-exit", json!({"exit_code": 0}));
    let mut backend = backend(&fixture);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .unwrap();
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(5));
    assert!(
        matches!(receipt.termination(), Termination::Failed { class }
        if class == &ErrorClass::new("codex-protocol-ended").unwrap())
    );
}

#[test]
fn unsupported_sent_setting_is_rejected_before_spawn() {
    let fixture = Fixture::new("bad-setting", base_scenario());
    let mut backend = backend(&fixture);
    let failure = backend
        .start(&invocation(&fixture, settings(&[("unknown", "value")])))
        .expect_err("unknown setting is rejected");
    assert!(failure.confirmed_never_started());
    assert_eq!(
        failure.class(),
        &ErrorClass::new("unsupported-sent-setting").unwrap()
    );
}

#[test]
#[ignore = "spends real provider quota; requires the owner's explicit go"]
fn real_app_server_invocation_is_manual_only() {
    let fixture = Fixture::new("real-workspace", base_scenario());
    let mut backend = CodexBackend::new("codex", Arc::new(ymp_runtime::SystemClock::new()))
        .with_prompt("Reply with the single word: pong")
        .with_extra_args(["--ephemeral".to_owned()]);
    backend
        .start(&invocation(&fixture, Settings::new()))
        .expect("real App Server invocation starts");
    let (_, receipt) = wait_for_receipt(&mut backend, Duration::from_secs(240));
    assert_eq!(receipt.termination(), &Termination::Completed);
}
