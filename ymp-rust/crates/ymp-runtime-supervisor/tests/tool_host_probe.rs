#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;
use ymp_runtime_api::{
    CancellationToken, InvocationRequest, ModelSpend, ProbeReport, ProbeTransportIdentity,
    Readiness, RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeEventKind, RuntimeKind,
    RuntimeSession, TOOL_HOST_PROBE_ENVIRONMENT, TOOL_HOST_PROBE_INTERNAL_ARGUMENTS,
    TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND, TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION,
    TOOL_HOST_PROBE_SCHEMA_VERSION, TOOL_HOST_PROBE_SERVER_VERSION,
    TOOL_HOST_PROBE_WORKSPACE_SERVER, ToolHostProbeCostAvailability, ToolHostProbeEffect,
    ToolHostProbeError, ToolHostProbeInvocation, ToolHostProbeRequest, ToolHostProbeResourceVector,
    ToolHostProbeRuntimeIdentity, ToolHostProbeTool, ToolHostProbeTrust, Usage, evidence_digest,
    probe_transport_digest, tool_host_probe_tool_schema_digest,
};
use ymp_runtime_codex::{PINNED_CODEX_PROMPT_POLICY, codex_compatibility_contract_digest};
use ymp_runtime_supervisor::execute_tool_host_probe;

const DEADLINE_MS: u64 = 1_000;
const NONCE: &str = "opaque-caller-nonce-7c1e";

fn reservation() -> ToolHostProbeResourceVector {
    ToolHostProbeResourceVector {
        model_calls: 1,
        max_input_tokens: 64,
        max_cached_input_tokens: 32,
        max_output_tokens: 32,
        max_reasoning_output_tokens: 16,
        max_cost_microusd: Some(100),
        max_wall_time_ms: DEADLINE_MS,
        workspace_reads: 1,
        workspace_writes: 1,
        invocation_starts: 1,
        protected_queries: 0,
        external_actions: 0,
        participant_starts: 0,
        attempt_starts: 0,
        offer_creations: 0,
        obligation_creations: 0,
        board_actions: 0,
        task_actions: 0,
        recruitment_actions: 0,
        candidate_actions: 0,
        communication_actions: 0,
    }
}

fn identity(executable: &Path) -> ToolHostProbeRuntimeIdentity {
    let probe_transport = ProbeTransportIdentity {
        mcp_protocol_version: TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION.to_owned(),
        server_name: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
        server_version: TOOL_HOST_PROBE_SERVER_VERSION.to_owned(),
        tool_schema_digest: tool_host_probe_tool_schema_digest(),
        ordered_tools: [
            ToolHostProbeTool::WorkspaceWrite,
            ToolHostProbeTool::WorkspaceRead,
        ],
        server_executable_digest: "1".repeat(64),
        launcher_executable_digest: "1".repeat(64),
        internal_subcommand: TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND.to_owned(),
        arguments: TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        inherited_environment: TOOL_HOST_PROBE_ENVIRONMENT
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        canonical_workspace_root_digest: "2".repeat(64),
    };
    ToolHostProbeRuntimeIdentity {
        runtime_kind: RuntimeKind::Fake,
        route: "fixture/no-network".to_owned(),
        profile: "workspace-read-write-only".to_owned(),
        cli: executable.display().to_string(),
        cli_version: "fake-cli 1.0.0".to_owned(),
        compatibility_contract_digest: evidence_digest(b"fixture compatibility contract v1"),
        executable_digest: evidence_digest(&fs::read(executable).expect("runtime bytes")),
        driver: "fake-process-driver".to_owned(),
        driver_version: "fake-process-driver 1.0.0".to_owned(),
        tool_schema_digest: tool_host_probe_tool_schema_digest(),
        probe_transport_digest: probe_transport_digest(&probe_transport),
        probe_transport,
    }
}

fn request(executable: &Path) -> ToolHostProbeRequest {
    ToolHostProbeRequest {
        schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
        probe_id: "probe-fixture-1".to_owned(),
        invocation_id: "invocation-fixture-1".to_owned(),
        nonce: NONCE.to_owned(),
        workspace_path: PathBuf::from("probe/nonce.txt"),
        deadline_ms: DEADLINE_MS,
        resource_reservation: reservation(),
        expected_runtime: identity(executable),
        cancellation: CancellationToken::default(),
    }
}

fn complete_usage() -> Usage {
    Usage {
        input_tokens: 11,
        cached_input_tokens: 3,
        output_tokens: 5,
        reasoning_output_tokens: 2,
        cost_microusd: None,
        cost_by_model: Vec::new(),
        wall_time_ms: 7,
        protected_queries: 0,
        in_flight_excess: Default::default(),
    }
}

fn event(invocation: &str, sequence: u64, event: RuntimeEventKind) -> RuntimeEvent {
    RuntimeEvent {
        sequence,
        event_id: format!("{invocation}.event-{sequence}"),
        invocation_id: invocation.to_owned(),
        event,
    }
}

fn write_event(request: &ToolHostProbeRequest, sequence: u64) -> RuntimeEvent {
    event(
        &request.invocation_id,
        sequence,
        RuntimeEventKind::McpToolCall {
            server: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
            tool: ToolHostProbeTool::WorkspaceWrite.to_string(),
            status: "completed".to_owned(),
            arguments: json!({
                "path": request.workspace_path,
                "content": request.nonce,
            }),
            result: Some(json!({"bytes_written": request.nonce.len()})),
            error: None,
        },
    )
}

fn read_event(request: &ToolHostProbeRequest, sequence: u64) -> RuntimeEvent {
    event(
        &request.invocation_id,
        sequence,
        RuntimeEventKind::McpToolCall {
            server: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
            tool: ToolHostProbeTool::WorkspaceRead.to_string(),
            status: "completed".to_owned(),
            arguments: json!({"path": request.workspace_path}),
            result: Some(json!({"content": request.nonce})),
            error: None,
        },
    )
}

fn successful_events(request: &ToolHostProbeRequest, usage: &Usage) -> VecDeque<RuntimeEvent> {
    [
        event(
            &request.invocation_id,
            1,
            RuntimeEventKind::Started {
                opaque_session_id: "fake-session".to_owned(),
            },
        ),
        write_event(request, 2),
        read_event(request, 3),
        event(
            &request.invocation_id,
            4,
            RuntimeEventKind::Completed {
                usage: usage.clone(),
            },
        ),
    ]
    .into()
}

#[derive(Clone, Copy)]
enum Scenario {
    Success,
    MissingRead,
    IncompleteUsage,
    AmbiguousTerminal,
    TimedOut,
    WrongDeadline,
    ExtraTool,
    ReorderedTools,
    ExtraOutput,
    WrongReadback,
    ReportedCost,
    Forbidden(ToolHostProbeEffect),
}

struct ScriptedDriver {
    executable: PathBuf,
    identity: ToolHostProbeRuntimeIdentity,
    scenario: Scenario,
    starts: Arc<AtomicUsize>,
}

impl ScriptedDriver {
    fn new(executable: &Path, scenario: Scenario) -> Self {
        Self {
            executable: executable.to_owned(),
            identity: identity(executable),
            scenario,
            starts: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl RuntimeDriver for ScriptedDriver {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Fake
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        Ok(ProbeReport {
            kind: RuntimeKind::Fake,
            executable: self.executable.display().to_string(),
            version: Some(self.identity.cli_version.clone()),
            readiness: Readiness::Ready,
            detail: "scripted no-network fixture".to_owned(),
        })
    }

    fn start(&self, _request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        Err(RuntimeError::Unsupported("ordinary scripted invocation"))
    }

    fn tool_host_probe_identity(&self) -> Result<ToolHostProbeRuntimeIdentity, RuntimeError> {
        Ok(self.identity.clone())
    }

    fn start_tool_host_probe(
        &self,
        invocation: ToolHostProbeInvocation,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            invocation.allowed_tools,
            [
                ToolHostProbeTool::WorkspaceWrite,
                ToolHostProbeTool::WorkspaceRead,
            ]
        );
        let request = invocation.request;
        let usage = match self.scenario {
            Scenario::IncompleteUsage => Usage::default(),
            Scenario::ReportedCost => Usage {
                cost_microusd: Some(7),
                cost_by_model: vec![ModelSpend {
                    model: "fixture-model".to_owned(),
                    cost_microusd: 7,
                }],
                ..complete_usage()
            },
            _ => complete_usage(),
        };
        let mut events = successful_events(&request, &usage);
        match self.scenario {
            Scenario::Success | Scenario::IncompleteUsage | Scenario::ReportedCost => {}
            Scenario::MissingRead => {
                events.remove(2);
                let completed = events.get_mut(2).expect("completion");
                completed.sequence = 3;
                completed.event_id = format!("{}.event-3", request.invocation_id);
            }
            Scenario::AmbiguousTerminal => {
                events.pop_back();
            }
            Scenario::TimedOut => {
                events.pop_back();
                events.push_back(event(
                    &request.invocation_id,
                    4,
                    RuntimeEventKind::TimedOut {
                        limit_ms: DEADLINE_MS,
                        usage: usage.clone(),
                    },
                ));
            }
            Scenario::WrongDeadline => {
                events.pop_back();
                events.push_back(event(
                    &request.invocation_id,
                    4,
                    RuntimeEventKind::TimedOut {
                        limit_ms: DEADLINE_MS - 1,
                        usage: usage.clone(),
                    },
                ));
            }
            Scenario::ExtraTool => {
                events.pop_back();
                events.push_back(read_event(&request, 4));
                events.push_back(event(
                    &request.invocation_id,
                    5,
                    RuntimeEventKind::Completed {
                        usage: usage.clone(),
                    },
                ));
            }
            Scenario::ReorderedTools => {
                events.clear();
                events.push_back(event(
                    &request.invocation_id,
                    1,
                    RuntimeEventKind::Started {
                        opaque_session_id: "fake-session".to_owned(),
                    },
                ));
                events.push_back(read_event(&request, 2));
                events.push_back(write_event(&request, 3));
                events.push_back(event(
                    &request.invocation_id,
                    4,
                    RuntimeEventKind::Completed {
                        usage: usage.clone(),
                    },
                ));
            }
            Scenario::ExtraOutput => {
                events.pop_back();
                events.push_back(event(
                    &request.invocation_id,
                    4,
                    RuntimeEventKind::Output {
                        text: "task output is forbidden".to_owned(),
                    },
                ));
                events.push_back(event(
                    &request.invocation_id,
                    5,
                    RuntimeEventKind::Completed {
                        usage: usage.clone(),
                    },
                ));
            }
            Scenario::WrongReadback => {
                let read = events.get_mut(2).expect("read event");
                let RuntimeEventKind::McpToolCall { result, .. } = &mut read.event else {
                    panic!("read tool event");
                };
                *result = Some(json!({"content": "wrong-nonce"}));
            }
            Scenario::Forbidden(effect) => {
                let (server, tool) = match effect {
                    ToolHostProbeEffect::Board => ("ymp.board", "board_publish"),
                    ToolHostProbeEffect::Task => ("ymp.tasks", "task_create"),
                    ToolHostProbeEffect::Recruitment => ("ymp.control", "recruit_participant"),
                    ToolHostProbeEffect::Candidate => ("ymp.control", "candidate_submit"),
                };
                events.clear();
                events.push_back(event(
                    &request.invocation_id,
                    1,
                    RuntimeEventKind::Started {
                        opaque_session_id: "fake-session".to_owned(),
                    },
                ));
                events.push_back(event(
                    &request.invocation_id,
                    2,
                    RuntimeEventKind::McpToolCall {
                        server: server.to_owned(),
                        tool: tool.to_owned(),
                        status: "completed".to_owned(),
                        arguments: Value::Object(Default::default()),
                        result: Some(Value::Null),
                        error: None,
                    },
                ));
            }
        }
        Ok(Box::new(ScriptedSession { events, usage }))
    }
}

struct ScriptedSession {
    events: VecDeque<RuntimeEvent>,
    usage: Usage,
}

struct CompatibleCodexDriver {
    executable: PathBuf,
    version: String,
    starts: Arc<AtomicUsize>,
}

impl RuntimeDriver for CompatibleCodexDriver {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Codex
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        Ok(ProbeReport {
            kind: RuntimeKind::Codex,
            executable: self.executable.display().to_string(),
            version: Some(self.version.clone()),
            readiness: Readiness::Ready,
            detail: "behaviorally compatible fake Codex fixture".to_owned(),
        })
    }

    fn start(&self, _request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        Err(RuntimeError::Unsupported(
            "ordinary Codex fixture invocation",
        ))
    }

    fn tool_host_probe_identity(&self) -> Result<ToolHostProbeRuntimeIdentity, RuntimeError> {
        panic!("Codex transport identity must be reconstructed by the supervisor")
    }

    fn start_tool_host_probe(
        &self,
        invocation: ToolHostProbeInvocation,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        let request = invocation.request;
        let usage = complete_usage();
        Ok(Box::new(ScriptedSession {
            events: successful_events(&request, &usage),
            usage,
        }))
    }
}

fn codex_identity(executable: &Path, workspace: &Path) -> ToolHostProbeRuntimeIdentity {
    let current_exe = std::env::current_exe()
        .expect("current executable")
        .canonicalize()
        .expect("canonical current executable");
    let current_digest = evidence_digest(&fs::read(current_exe).expect("current executable bytes"));
    let workspace = workspace.canonicalize().expect("canonical workspace");
    let probe_transport = ProbeTransportIdentity {
        mcp_protocol_version: TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION.to_owned(),
        server_name: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
        server_version: TOOL_HOST_PROBE_SERVER_VERSION.to_owned(),
        tool_schema_digest: tool_host_probe_tool_schema_digest(),
        ordered_tools: [
            ToolHostProbeTool::WorkspaceWrite,
            ToolHostProbeTool::WorkspaceRead,
        ],
        server_executable_digest: current_digest.clone(),
        launcher_executable_digest: current_digest,
        internal_subcommand: TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND.to_owned(),
        arguments: TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        inherited_environment: TOOL_HOST_PROBE_ENVIRONMENT
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        canonical_workspace_root_digest: evidence_digest(workspace.as_os_str().as_encoded_bytes()),
    };
    ToolHostProbeRuntimeIdentity {
        runtime_kind: RuntimeKind::Codex,
        route: "openai_responses_chatgpt".to_owned(),
        profile: PINNED_CODEX_PROMPT_POLICY.to_owned(),
        cli: executable.display().to_string(),
        cli_version: "codex-cli 0.151.0".to_owned(),
        compatibility_contract_digest: codex_compatibility_contract_digest(),
        executable_digest: evidence_digest(&fs::read(executable).expect("Codex fixture bytes")),
        driver: "ymp-runtime-codex".to_owned(),
        driver_version: env!("CARGO_PKG_VERSION").to_owned(),
        tool_schema_digest: tool_host_probe_tool_schema_digest(),
        probe_transport_digest: probe_transport_digest(&probe_transport),
        probe_transport,
    }
}

fn codex_request(executable: &Path, workspace: &Path) -> ToolHostProbeRequest {
    let mut request = request(executable);
    request.expected_runtime = codex_identity(executable, workspace);
    request
}

impl RuntimeSession for ScriptedSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        Ok(self.events.pop_front())
    }

    fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("probe resume"))
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        self.events.clear();
        Ok(())
    }

    fn usage(&self) -> Usage {
        self.usage.clone()
    }
}

#[test]
fn malformed_paths_and_cancelled_requests_never_start_an_invocation() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = Path::new("/bin/sh");

    for path in [PathBuf::from("/absolute"), PathBuf::from("../traversal")] {
        let driver = ScriptedDriver::new(executable, Scenario::Success);
        let mut request = request(executable);
        request.workspace_path = path;
        assert_eq!(
            execute_tool_host_probe(&driver, &workspace, request),
            Err(ToolHostProbeError::InvalidWorkspacePath)
        );
        assert_eq!(driver.starts.load(Ordering::SeqCst), 0);
    }

    let driver = ScriptedDriver::new(executable, Scenario::Success);
    let cancelled = request(executable);
    cancelled.cancellation.cancel();
    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, cancelled),
        Err(ToolHostProbeError::Cancelled)
    );
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);
}

#[test]
fn compatible_codex_is_reconstructed_without_version_equality_or_attestation_authority() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = Path::new("/bin/sh");
    let driver = CompatibleCodexDriver {
        executable: executable.to_owned(),
        version: "codex-cli 9.7.3".to_owned(),
        starts: Arc::new(AtomicUsize::new(0)),
    };
    let request = codex_request(executable, &workspace);
    assert_ne!(
        request.expected_runtime.cli_version, driver.version,
        "version strings are evidence, not compatibility authority"
    );

    let trace = execute_tool_host_probe(&driver, &workspace, request)
        .expect("behaviorally compatible Codex trace");
    assert_eq!(trace.runtime.cli_version, "codex-cli 9.7.3");
    assert_eq!(trace.runtime.runtime_kind, RuntimeKind::Codex);
    assert_eq!(trace.trust, ToolHostProbeTrust::UntrustedRuntimeTrace);
    assert_eq!(driver.starts.load(Ordering::SeqCst), 1);
}

#[test]
fn incompatible_codex_contract_executable_and_transport_fail_before_start() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = Path::new("/bin/sh");

    for (field, mutate) in [
        ("compatibility_contract_digest", 0_u8),
        ("executable_digest", 1_u8),
        ("server_executable_digest", 2_u8),
    ] {
        let driver = CompatibleCodexDriver {
            executable: executable.to_owned(),
            version: "codex-cli 0.151.0".to_owned(),
            starts: Arc::new(AtomicUsize::new(0)),
        };
        let mut request = codex_request(executable, &workspace);
        match mutate {
            0 => request.expected_runtime.compatibility_contract_digest = "f".repeat(64),
            1 => request.expected_runtime.executable_digest = "e".repeat(64),
            2 => {
                request
                    .expected_runtime
                    .probe_transport
                    .server_executable_digest = "d".repeat(64);
                request.expected_runtime.probe_transport_digest =
                    probe_transport_digest(&request.expected_runtime.probe_transport);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            execute_tool_host_probe(&driver, &workspace, request),
            Err(ToolHostProbeError::RuntimeIdentityMismatch { field })
        );
        assert_eq!(driver.starts.load(Ordering::SeqCst), 0);
    }
}

#[cfg(unix)]
#[test]
fn an_existing_symlink_cannot_move_the_probe_write_outside_the_workspace() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    let outside = root.path().join("outside");
    fs::create_dir(&workspace).expect("workspace");
    fs::create_dir(&outside).expect("outside");
    symlink(&outside, workspace.join("escape")).expect("escape symlink");
    let executable = Path::new("/bin/sh");
    let driver = ScriptedDriver::new(executable, Scenario::Success);
    let mut request = request(executable);
    request.workspace_path = PathBuf::from("escape/nonce.txt");

    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, request),
        Err(ToolHostProbeError::InvalidWorkspacePath)
    );
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);
    assert!(!outside.join("nonce.txt").exists());
}

#[cfg(unix)]
#[test]
fn codex_workspace_root_substitution_after_measurement_fails_before_start() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    let substitute = root.path().join("substitute");
    fs::create_dir(&workspace).expect("workspace");
    fs::create_dir(&substitute).expect("substitute");
    let executable = Path::new("/bin/sh");
    let driver = CompatibleCodexDriver {
        executable: executable.to_owned(),
        version: "codex-cli 0.151.0".to_owned(),
        starts: Arc::new(AtomicUsize::new(0)),
    };
    let request = codex_request(executable, &workspace);
    fs::remove_dir(&workspace).expect("remove measured workspace");
    symlink(&substitute, &workspace).expect("substitute workspace root");

    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, request),
        Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "canonical_workspace_root_digest"
        })
    );
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);
    assert!(!substitute.join("probe/nonce.txt").exists());
}

#[test]
fn malformed_terminal_tool_usage_and_output_return_no_trace() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = Path::new("/bin/sh");
    let cases = [
        (
            Scenario::MissingRead,
            ToolHostProbeError::MissingToolEvent {
                tool: ToolHostProbeTool::WorkspaceRead,
            },
        ),
        (
            Scenario::IncompleteUsage,
            ToolHostProbeError::IncompleteUsage { field: "tokens" },
        ),
        (
            Scenario::AmbiguousTerminal,
            ToolHostProbeError::AmbiguousTerminal,
        ),
        (
            Scenario::TimedOut,
            ToolHostProbeError::TimedOut {
                limit_ms: DEADLINE_MS,
            },
        ),
        (
            Scenario::WrongDeadline,
            ToolHostProbeError::InvalidEventStream {
                detail: "runtime timeout does not match the requested deadline".to_owned(),
            },
        ),
        (
            Scenario::ExtraTool,
            ToolHostProbeError::UnexpectedToolUse {
                server: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
                tool: ToolHostProbeTool::WorkspaceRead.to_string(),
            },
        ),
        (
            Scenario::ReorderedTools,
            ToolHostProbeError::UnexpectedToolUse {
                server: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
                tool: ToolHostProbeTool::WorkspaceRead.to_string(),
            },
        ),
        (Scenario::ExtraOutput, ToolHostProbeError::UnexpectedOutput),
        (
            Scenario::WrongReadback,
            ToolHostProbeError::InvalidToolEvent {
                tool: ToolHostProbeTool::WorkspaceRead,
                field: "result",
            },
        ),
    ];
    for (scenario, expected) in cases {
        let driver = ScriptedDriver::new(executable, scenario);
        let result = execute_tool_host_probe(&driver, &workspace, request(executable));
        assert_eq!(result, Err(expected));
    }
}

#[test]
fn board_task_recruitment_and_candidate_effects_are_typed_refusals() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = Path::new("/bin/sh");
    for effect in [
        ToolHostProbeEffect::Board,
        ToolHostProbeEffect::Task,
        ToolHostProbeEffect::Recruitment,
        ToolHostProbeEffect::Candidate,
    ] {
        let driver = ScriptedDriver::new(executable, Scenario::Forbidden(effect));
        assert!(matches!(
            execute_tool_host_probe(&driver, &workspace, request(executable)),
            Err(ToolHostProbeError::ForbiddenEffect {
                effect: found,
                ..
            }) if found == effect
        ));
    }
}

#[test]
fn separate_reservation_and_runtime_tuple_fail_before_success() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = Path::new("/bin/sh");

    let driver = ScriptedDriver::new(executable, Scenario::Success);
    let mut arm_funded = request(executable);
    arm_funded.resource_reservation.candidate_actions = 1;
    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, arm_funded),
        Err(ToolHostProbeError::InvalidReservation {
            field: "candidate_actions"
        })
    );
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);

    let driver = ScriptedDriver::new(executable, Scenario::Success);
    let mut wrong_contract = request(executable);
    wrong_contract
        .expected_runtime
        .compatibility_contract_digest = "f".repeat(64);
    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, wrong_contract),
        Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "compatibility_contract_digest"
        })
    );
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);

    let driver = ScriptedDriver::new(executable, Scenario::Success);
    let mut wrong_route = request(executable);
    wrong_route.expected_runtime.route = "different-route".to_owned();
    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, wrong_route),
        Err(ToolHostProbeError::RuntimeIdentityMismatch { field: "route" })
    );
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);

    let driver = ScriptedDriver::new(executable, Scenario::ReportedCost);
    let mut unreserved_cost = request(executable);
    unreserved_cost.resource_reservation.max_cost_microusd = None;
    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, unreserved_cost),
        Err(ToolHostProbeError::ReservationExceeded {
            field: "cost_microusd"
        })
    );
}

#[test]
fn version_difference_alone_does_not_reject_the_runtime_projection() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = Path::new("/bin/sh");
    let mut driver = ScriptedDriver::new(executable, Scenario::Success);
    driver.identity.cli_version = "fake-cli 9.7.3".to_owned();
    let request = request(executable);
    assert_ne!(
        request.expected_runtime.cli_version,
        driver.identity.cli_version
    );

    let trace = execute_tool_host_probe(&driver, &workspace, request)
        .expect("compatible behavior with a different observed version");
    assert_eq!(trace.runtime.cli_version, "fake-cli 9.7.3");
}

#[test]
fn executable_replacement_after_identity_measurement_is_refused_before_start() {
    let root = TempDir::new().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let executable = root.path().join("runtime");
    fs::write(&executable, b"measured runtime bytes").expect("measured runtime");
    let driver = ScriptedDriver::new(&executable, Scenario::Success);
    let request = request(&executable);
    fs::write(&executable, b"substituted runtime bytes").expect("substituted runtime");

    assert_eq!(
        execute_tool_host_probe(&driver, &workspace, request),
        Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "executable_digest"
        })
    );
    assert_eq!(driver.starts.load(Ordering::SeqCst), 0);
}

struct ProcessDriver {
    executable: PathBuf,
    identity: ToolHostProbeRuntimeIdentity,
    project: PathBuf,
    home: PathBuf,
    ymp_home: PathBuf,
    temporary: PathBuf,
    build: PathBuf,
    export: PathBuf,
}

impl RuntimeDriver for ProcessDriver {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Fake
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        Ok(ProbeReport {
            kind: RuntimeKind::Fake,
            executable: self.executable.display().to_string(),
            version: Some(self.identity.cli_version.clone()),
            readiness: Readiness::Ready,
            detail: "fake executable ready".to_owned(),
        })
    }

    fn start(&self, _request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        Err(RuntimeError::Unsupported("ordinary process invocation"))
    }

    fn tool_host_probe_identity(&self) -> Result<ToolHostProbeRuntimeIdentity, RuntimeError> {
        Ok(self.identity.clone())
    }

    fn start_tool_host_probe(
        &self,
        invocation: ToolHostProbeInvocation,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        let resources = serde_json::to_string(&invocation.request.resource_reservation)
            .map_err(|error| RuntimeError::MalformedEvent(error.to_string()))?;
        let tools = serde_json::to_string(&invocation.allowed_tools)
            .map_err(|error| RuntimeError::MalformedEvent(error.to_string()))?;
        let mut command = Command::new(&self.executable);
        command
            .args([
                invocation.workspace.as_os_str(),
                invocation.request.nonce.as_ref(),
                invocation.request.workspace_path.as_os_str(),
                invocation.request.deadline_ms.to_string().as_ref(),
                resources.as_ref(),
                tools.as_ref(),
            ])
            .current_dir(&self.project)
            .env_clear()
            .env("HOME", &self.home)
            .env("YMP_HOME", &self.ymp_home)
            .env("TMPDIR", &self.temporary)
            .env("CARGO_TARGET_DIR", &self.build)
            .env("YMP_EXPORT", &self.export)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(mutation) = std::env::var_os("YMP_TOOL_HOST_PROBE_MUTATION") {
            command.env("YMP_TOOL_HOST_PROBE_MUTATION", mutation);
        }
        let child = command.spawn()?;
        Ok(Box::new(ProcessSession {
            child: Some(child),
            request: invocation.request,
            expected_resources: resources,
            expected_tools: tools,
            events: VecDeque::new(),
            usage: complete_usage(),
        }))
    }
}

struct ProcessSession {
    child: Option<Child>,
    request: ToolHostProbeRequest,
    expected_resources: String,
    expected_tools: String,
    events: VecDeque<RuntimeEvent>,
    usage: Usage,
}

impl ProcessSession {
    fn load(&mut self) -> Result<(), RuntimeError> {
        let child = self.child.take().ok_or(RuntimeError::MalformedEvent(
            "process already read".to_owned(),
        ))?;
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(RuntimeError::UnsuccessfulExit {
                status: output.status.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        let parts: Vec<&[u8]> = output.stdout.split(|byte| *byte == 0).collect();
        if parts.len() != 4
            || parts[0] != self.request.nonce.as_bytes()
            || parts[1] != self.request.deadline_ms.to_string().as_bytes()
            || parts[2] != self.expected_resources.as_bytes()
            || parts[3] != self.expected_tools.as_bytes()
        {
            return Err(RuntimeError::MalformedEvent(
                "fake process did not receive the exact probe envelope".to_owned(),
            ));
        }
        self.events = successful_events(&self.request, &self.usage);
        Ok(())
    }
}

impl RuntimeSession for ProcessSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.child.is_some() {
            self.load()?;
        }
        Ok(self.events.pop_front())
    }

    fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("probe resume"))
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
        self.events.clear();
        Ok(())
    }

    fn usage(&self) -> Usage {
        self.usage.clone()
    }
}

impl Drop for ProcessSession {
    fn drop(&mut self) {
        let _ = self.interrupt();
    }
}

#[cfg(unix)]
fn write_fake_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(
        path,
        r#"#!/bin/sh
set -eu
workspace=$1
nonce=$2
relative=$3
deadline=$4
resources=$5
tools=$6
target="$workspace/$relative"
if [ "${YMP_TOOL_HOST_PROBE_MUTATION:-}" = "skip_workspace_write" ]; then
  readback=$nonce
else
  /bin/mkdir -p "${target%/*}"
  printf '%s' "$nonce" > "$target"
  readback=$(/bin/cat "$target")
fi
printf '%s\0%s\0%s\0%s' "$readback" "$deadline" "$resources" "$tools"
"#,
    )
    .expect("write fake executable");
    let mut permissions = fs::metadata(path).expect("script metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make fake executable runnable");
}

#[cfg(unix)]
#[test]
fn fake_process_walk_returns_only_an_untrusted_complete_trace() {
    let root = TempDir::new().expect("fresh evaluation root");
    let project = root.path().join("project");
    let home = root.path().join("home");
    let ymp_home = root.path().join("ymp-home");
    let temporary = root.path().join("tmp");
    let build = root.path().join("build");
    let export = root.path().join("export");
    let workspace = project.join("workspace");
    for directory in [
        &project, &home, &ymp_home, &temporary, &build, &export, &workspace,
    ] {
        fs::create_dir_all(directory).expect("isolated directory");
    }
    let executable = project.join("fake-tool-host");
    write_fake_executable(&executable);
    let driver = ProcessDriver {
        executable: executable.clone(),
        identity: identity(&executable),
        project,
        home,
        ymp_home,
        temporary,
        build,
        export,
    };

    let trace = execute_tool_host_probe(&driver, &workspace, request(&executable))
        .expect("fake process probe");
    assert_eq!(trace.runtime, identity(&executable));
    assert_eq!(trace.resource_reservation, reservation());
    assert_eq!(trace.model_calls, 1);
    assert_eq!(trace.runtime_reported_readback, NONCE);
    assert_eq!(trace.input_digest, evidence_digest(NONCE.as_bytes()));
    assert_eq!(trace.output_digest, evidence_digest(NONCE.as_bytes()));
    assert_eq!(trace.tool_event_digests.len(), 2);
    assert_eq!(
        trace.cost.availability,
        ToolHostProbeCostAvailability::Unavailable
    );
    assert_eq!(trace.trust, ToolHostProbeTrust::UntrustedRuntimeTrace);
    assert_eq!(
        fs::read_to_string(workspace.join("probe/nonce.txt")).expect("runtime workspace write"),
        NONCE
    );
}
