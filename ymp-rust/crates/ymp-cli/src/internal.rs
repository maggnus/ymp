//! The internal namespace: the product's own machinery, not an operator capability.
//!
//! Nothing here mirrors an action of the terminal interface. These children exist so the
//! workspace can drive a runtime, a verifier and the agent bridge from a test or a script; they
//! are deliberately kept out of the public surface, which
//! [`crate::surface`](crate::surface) holds and which the action inventory compares against the
//! interface in both directions.

use anyhow::{Context, bail};
use clap::{Subcommand, ValueEnum};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;
use ymp_application::{
    Application, ControllerToolHostProbeRequest, PreparedContract, VerificationOutcome,
    freeze_under,
};
use ymp_domain::Command as DomainCommand;
use ymp_runtime_api::{
    CancellationToken, InvocationRequest, McpBinding, Readiness, RuntimeDriver,
    ToolHostProbeResourceVector, unestablished_terminations,
};
use ymp_runtime_claude::ClaudeRuntime;
use ymp_runtime_codex::CodexRuntime;
use ymp_runtime_registry::{Engine, RegistryAddress};
use ymp_runtime_supervisor::{
    admit_runtime_start, admit_workspace_program, execute_tool_host_probe, initialize_private_git,
};

const ADMISSION_TOOL_HOST_PROBE_DEADLINE_MS: u64 = 120_000;

#[derive(Debug, Subcommand)]
pub enum InternalCommand {
    AgentMcp,
    RuntimeSmoke {
        #[arg(long)]
        runtime: RuntimeChoice,
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        prompt: String,
    },
    ManagedRuntimeSmoke {
        #[arg(long)]
        runtime: RuntimeChoice,
        #[arg(long)]
        workspace: PathBuf,
    },
    ManagedCandidateSmoke {
        #[arg(long)]
        runtime: RuntimeChoice,
    },
    ToolHostProbe {
        #[arg(long)]
        admission_manifest_digest: String,
    },
    Verifier {
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long)]
        expected_sha256: String,
    },
    VerifyManagedCandidate {
        #[arg(long)]
        program: PathBuf,
        #[arg(long = "arg")]
        arguments: Vec<String>,
        #[arg(long)]
        negative_control: PathBuf,
        #[arg(long)]
        contract_digest: String,
        #[arg(long)]
        oracle_digest: String,
        #[arg(long, default_value_t = 60_000)]
        wall_time_ms: u64,
        #[arg(long, default_value_t = 1_048_576)]
        output_limit_bytes: usize,
    },
}

/// The runtimes this executable can start. The in-process fixture runtime is absent: it attests
/// nothing about the programs a launch enters, so a run started with it — and any candidate that
/// run assembled — would carry no evidence of the program that wrote it. It is reachable only from
/// the checks, which link the fixture crate directly.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RuntimeChoice {
    Codex,
    Claude,
}

/// How this invocation addresses the engine registry.
///
/// A root the operator named is the registry and is taken as stated: `--root <dir>` records and
/// reads under `<dir>`, whatever root stands above it. An invocation that named one exact store
/// carries the store, and the root it stands under is resolved from it, so a decision recorded
/// under that root governs it too.
fn registry_address(root: Option<PathBuf>, store: &std::path::Path) -> RegistryAddress {
    match root {
        Some(root) => RegistryAddress::Root(root),
        None => RegistryAddress::Store(store.to_path_buf()),
    }
}

impl RuntimeChoice {
    /// The registry entity this choice starts, so the enabled flag is reachable from here too.
    const fn engine(self) -> Engine {
        match self {
            Self::Codex => Engine::Codex,
            Self::Claude => Engine::ClaudeCode,
        }
    }
}

/// Every child of the internal namespace, dispatched over the contract packages the command line
/// named.
pub fn run(
    data_root: PathBuf,
    root: Option<PathBuf>,
    contracts: &[PathBuf],
    command: InternalCommand,
) -> anyhow::Result<()> {
    let registry = registry_address(root, &data_root);
    match command {
        InternalCommand::AgentMcp => run_agent_mcp(),
        InternalCommand::RuntimeSmoke {
            runtime,
            workspace,
            prompt,
        } => run_runtime_smoke(registry, runtime, workspace, prompt),
        InternalCommand::ManagedRuntimeSmoke { runtime, workspace } => run_managed_runtime_smoke(
            data_root,
            registry,
            runtime,
            workspace,
            crate::one_contract(contracts)?,
        ),
        InternalCommand::ManagedCandidateSmoke { runtime } => run_managed_candidate_smoke(
            data_root,
            registry,
            runtime,
            crate::one_contract(contracts)?,
        ),
        InternalCommand::ToolHostProbe {
            admission_manifest_digest,
        } => run_tool_host_probe(data_root, registry, admission_manifest_digest),
        InternalCommand::Verifier {
            candidate,
            expected_sha256,
        } => {
            let outcome = ymp_verifier::check_exact_digest(candidate, expected_sha256)?;
            println!("{}", serde_json::to_string_pretty(&outcome)?);
            if !outcome.matched {
                bail!("candidate digest does not match");
            }
            Ok(())
        }
        InternalCommand::VerifyManagedCandidate {
            program,
            arguments,
            negative_control,
            contract_digest,
            oracle_digest,
            wall_time_ms,
            output_limit_bytes,
        } => run_managed_verification(ManagedVerificationRequest {
            data_root,
            program,
            arguments,
            negative_control,
            contract_digest,
            oracle_digest,
            wall_time_ms,
            output_limit_bytes,
        }),
    }
}

/// The only place in this namespace where a runtime driver is built, and therefore the only place
/// these commands can start a runtime from.
///
/// Two admissions stand in front of every driver. The registry answers whether this host admits
/// the engine at all and refuses a disabled one with the reason the operator recorded, so the
/// product's own machinery cannot reach an engine the operator's surfaces will not. The
/// controller's gate then answers whether the driver attests the programs its launch enters and
/// whether the utilities the run ends its processes with are admitted.
fn runtime_driver(
    registry: &RegistryAddress,
    runtime: RuntimeChoice,
) -> anyhow::Result<Box<dyn RuntimeDriver>> {
    registry.registry().admit(runtime.engine())?;
    let driver: Box<dyn RuntimeDriver> = match runtime {
        RuntimeChoice::Codex => Box::new(CodexRuntime::default()),
        RuntimeChoice::Claude => Box::new(ClaudeRuntime::default()),
    };
    admit_runtime_start(driver.kind())?;
    Ok(driver)
}

/// Reads what the runtime session could not establish about the processes it started. The places
/// that end a process tree while they are already reporting something else keep it, because they
/// have no caller to tell; a command that printed its events and exited zero without reading it
/// would report a clean end it never measured.
fn require_established_termination() -> anyhow::Result<()> {
    let unestablished = unestablished_terminations();
    if !unestablished.is_empty() {
        bail!(
            "the processes this run started could not be ended: {}",
            unestablished.join("; ")
        );
    }
    Ok(())
}

fn run_runtime_smoke(
    registry: RegistryAddress,
    runtime: RuntimeChoice,
    workspace: PathBuf,
    prompt: String,
) -> anyhow::Result<()> {
    let driver = runtime_driver(&registry, runtime)?;
    let probe = driver.probe()?;
    println!("{}", serde_json::to_string(&probe)?);
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    let mut session = driver.start(InvocationRequest {
        invocation_id: format!("smoke-{}", Uuid::new_v4()),
        attempt_id: format!("attempt-{}", Uuid::new_v4()),
        workspace,
        mcp: None,
        prompt,
        cancellation: CancellationToken::default(),
    })?;
    while let Some(event) = session.next_event()? {
        println!("{}", serde_json::to_string(&event)?);
    }
    drop(session);
    require_established_termination()?;
    Ok(())
}

fn run_managed_runtime_smoke(
    data_root: PathBuf,
    registry: RegistryAddress,
    runtime: RuntimeChoice,
    workspace: PathBuf,
    contract: PreparedContract,
) -> anyhow::Result<()> {
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let prompt = contract.document.prompt.clone();
    // What the run may create participants from is fixed before it exists, out of the pools of the
    // root that governs this store. A root that offers the run nothing states so and starts
    // nothing, exactly as the interface does.
    let frozen = freeze_under(&registry.root(), None)?;
    let (mut application, _) = Application::create_with_contract(&data_root, &contract, &frozen)?;
    application.execute(
        format!("{attempt_id}.start"),
        DomainCommand::StartAttempt {
            attempt_id: attempt_id.clone(),
        },
    )?;
    let application = Arc::new(Mutex::new(application));
    let token = Uuid::new_v4().simple().to_string();
    let socket_path = data_root.join("runtime").join("agent.sock");
    let _rpc_server = ymp_agent_rpc::AgentRpcServer::start(
        &socket_path,
        &token,
        &attempt_id,
        Arc::clone(&application),
    )?;
    let executable = std::env::current_exe()
        .context("resolve current ymp executable")?
        .canonicalize()
        .context("canonicalize current ymp executable")?;
    let driver = runtime_driver(&registry, runtime)?;
    let probe = driver.probe()?;
    println!("{}", serde_json::to_string(&probe)?);
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    let mut session = driver.start(InvocationRequest {
        invocation_id: format!("invocation-{}", Uuid::new_v4()),
        attempt_id,
        workspace,
        mcp: Some(McpBinding {
            executable,
            socket_path,
            token,
        }),
        prompt,
        cancellation: CancellationToken::default(),
    })?;
    while let Some(event) = session.next_event()? {
        println!("{}", serde_json::to_string(&event)?);
    }
    drop(session);
    require_established_termination()?;
    let application = application
        .lock()
        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "managed_smoke_state",
            "state": application.state()
        }))?
    );
    Ok(())
}

fn run_managed_candidate_smoke(
    data_root: PathBuf,
    registry: RegistryAddress,
    runtime: RuntimeChoice,
    contract: PreparedContract,
) -> anyhow::Result<()> {
    // Source, prompt and capture exclusions come from the approved contract; the command adds
    // nothing the contract does not state.
    let source = contract.document.source.clone();
    let prompt = contract.document.prompt.clone();
    let exclusions: Vec<&str> = contract
        .document
        .capture_exclusions
        .iter()
        .map(String::as_str)
        .collect();
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let frozen = freeze_under(&registry.root(), None)?;
    let (mut application, _) = Application::create_with_contract(&data_root, &contract, &frozen)?;
    let artifacts = application.artifact_store();
    let base = artifacts.capture_source(&source)?;
    let workspace = data_root.join("workspaces").join(&attempt_id);
    artifacts.materialize(&base.manifest_digest, &workspace)?;
    initialize_private_git(&workspace, &admit_workspace_program()?)?;
    application.execute(
        format!("{attempt_id}.start"),
        DomainCommand::StartAttempt {
            attempt_id: attempt_id.clone(),
        },
    )?;

    let application = Arc::new(Mutex::new(application));
    let token = Uuid::new_v4().simple().to_string();
    let socket_path = data_root.join("runtime").join("agent.sock");
    let _rpc_server = ymp_agent_rpc::AgentRpcServer::start(
        &socket_path,
        &token,
        &attempt_id,
        Arc::clone(&application),
    )?;
    let executable = std::env::current_exe()
        .context("resolve current ymp executable")?
        .canonicalize()
        .context("canonicalize current ymp executable")?;
    let driver = runtime_driver(&registry, runtime)?;
    let probe = driver.probe()?;
    println!("{}", serde_json::to_string(&probe)?);
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    let mut session = driver.start(InvocationRequest {
        invocation_id: format!("invocation-{}", Uuid::new_v4()),
        attempt_id: attempt_id.clone(),
        workspace: workspace.clone(),
        mcp: Some(McpBinding {
            executable,
            socket_path,
            token,
        }),
        prompt,
        cancellation: CancellationToken::default(),
    })?;
    while let Some(event) = session.next_event()? {
        println!("{}", serde_json::to_string(&event)?);
    }
    drop(session);
    require_established_termination()?;

    let mut application = application
        .lock()
        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
    let submitted = application.submit_workspace_candidate_excluding(
        format!("{attempt_id}.submit-workspace"),
        &attempt_id,
        &base.manifest_digest,
        &workspace,
        &exclusions,
    )?;
    let candidate_directory = data_root
        .join("candidates")
        .join(&submitted.candidate.snapshot_digest);
    application
        .artifact_store()
        .materialize(&submitted.candidate.snapshot_digest, &candidate_directory)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "managed_candidate_smoke_result",
            "base": base,
            "submission": submitted.submission,
            "candidate": submitted.candidate,
            "candidate_directory": candidate_directory,
            "state": application.state()
        }))?
    );
    Ok(())
}

fn run_tool_host_probe(
    data_root: PathBuf,
    registry: RegistryAddress,
    admission_manifest_digest: String,
) -> anyhow::Result<()> {
    let driver = runtime_driver(&registry, RuntimeChoice::Codex)?;
    let expected_runtime = driver
        .tool_host_probe_identity()
        .context("load the controller-pinned tool-host probe runtime identity")?;
    run_controller_tool_host_probe(
        data_root,
        driver,
        ControllerToolHostProbeRequest::new(
            admission_manifest_digest,
            expected_runtime,
            ADMISSION_TOOL_HOST_PROBE_DEADLINE_MS,
            admission_tool_host_probe_reservation(),
        ),
    )
}

fn run_controller_tool_host_probe(
    data_root: PathBuf,
    driver: Box<dyn RuntimeDriver>,
    request: ControllerToolHostProbeRequest,
) -> anyhow::Result<()> {
    let mut application = Application::open(&data_root)?;
    let export = application.attested_tool_host_probe_handle_export_path();
    match std::fs::symlink_metadata(&export) {
        Ok(_) => bail!(
            "tool-host probe handle export already exists: {}",
            export.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let handle = application.controller_tool_host_probe(request, |workspace, request| {
        execute_tool_host_probe(driver.as_ref(), workspace, request)
    })?;
    let export = application.export_attested_tool_host_probe_handle(&handle)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "tool_host_probe_handle",
            "handle": handle,
            "export": export,
        }))?
    );
    Ok(())
}

fn admission_tool_host_probe_reservation() -> ToolHostProbeResourceVector {
    ToolHostProbeResourceVector {
        model_calls: 1,
        max_input_tokens: 32_768,
        max_cached_input_tokens: 32_768,
        max_output_tokens: 1_024,
        max_reasoning_output_tokens: 1_024,
        max_cost_microusd: None,
        max_wall_time_ms: ADMISSION_TOOL_HOST_PROBE_DEADLINE_MS,
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

pub struct ManagedVerificationRequest {
    pub data_root: PathBuf,
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub negative_control: PathBuf,
    pub contract_digest: String,
    pub oracle_digest: String,
    pub wall_time_ms: u64,
    pub output_limit_bytes: usize,
}

pub fn run_managed_verification(request: ManagedVerificationRequest) -> anyhow::Result<()> {
    let ManagedVerificationRequest {
        data_root,
        program,
        arguments,
        negative_control,
        contract_digest,
        oracle_digest,
        wall_time_ms,
        output_limit_bytes,
    } = request;
    let program = program
        .canonicalize()
        .with_context(|| format!("canonicalize verifier program {}", program.display()))?;
    let negative_control = negative_control.canonicalize().with_context(|| {
        format!(
            "canonicalize negative control {}",
            negative_control.display()
        )
    })?;
    let mut application = Application::open(&data_root)?;
    let candidate_digest = application
        .state()
        .candidate_digest
        .clone()
        .context("run has no submitted candidate")?;
    let verifier = ymp_verifier::CommandVerifier::new(
        contract_digest,
        oracle_digest,
        program,
        arguments,
        Duration::from_millis(wall_time_ms),
        output_limit_bytes,
    )?;
    let environment_digest = verifier.environment_digest()?;
    if let Some(outcome) = application.stored_verification(
        &candidate_digest,
        verifier.contract_digest(),
        verifier.oracle_digest(),
        &environment_digest,
    ) {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "managed_verification_result",
                "outcome": outcome,
                "state": application.state()
            }))?
        );
        return Ok(());
    }
    let (_, candidate) = materialize_controller_candidate(&application, &data_root)?;
    let command_id = format!("managed.verify.{}", Uuid::new_v4());
    // What a verdict and a verifier that did not decide each become in the journal is decided in
    // one place, which the terminal interface reaches through the same call: a verifier that
    // failed is an infrastructure condition there as it is here, and never a rejected candidate.
    let (evidence, error, decided) =
        match verifier.verify_candidate(&candidate, &negative_control, &candidate_digest) {
            Ok(evidence) => (
                Some(serde_json::to_value(&evidence)?),
                None,
                VerificationOutcome::Judged(Box::new(evidence)),
            ),
            Err(failure) => (
                None,
                Some(failure.to_string()),
                VerificationOutcome::Undecided {
                    reason: format!("the verifier failed: {failure}"),
                },
            ),
        };
    let outcome = application.record_verification_outcome(command_id, decided)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "managed_verification_result",
            "evidence": evidence,
            "error": error,
            "outcome": outcome,
            "state": application.state()
        }))?
    );
    Ok(())
}

pub fn materialize_controller_candidate(
    application: &Application,
    data_root: &std::path::Path,
) -> anyhow::Result<(String, PathBuf)> {
    let candidate_digest = application
        .state()
        .candidate_digest
        .clone()
        .context("run has no submitted candidate")?;
    let candidate = data_root.join("verification-inputs").join(format!(
        "{}-{}",
        candidate_digest,
        Uuid::new_v4()
    ));
    application
        .artifact_store()
        .materialize(&candidate_digest, &candidate)
        .context("materialize the controller-bound candidate for verification")?;
    Ok((candidate_digest, candidate))
}

fn run_agent_mcp() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let handler = ymp_agent_rpc::SocketToolHandler::from_env()
        .context("load controller-bound agent RPC capability")?;
    let mut server = ymp_agent_mcp::McpServer::new(handler);
    for line in stdin.lock().lines() {
        let line = line?;
        if let Some(response) = server.handle_line(&line) {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ManagedVerificationRequest, admission_tool_host_probe_reservation,
        materialize_controller_candidate, run_controller_tool_host_probe,
    };
    use crate::{Cli, Command as CliCommand};
    use clap::Parser;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use tempfile::tempdir;
    use ymp_application::{
        Application, AttestedToolHostProbeHandle, ControllerToolHostProbeRequest,
        TOOL_HOST_PROBE_HANDLE_EXPORT,
    };
    use ymp_domain::{Budget, Command, RunStatus};
    use ymp_runtime_api::{
        InvocationRequest, ProbeReport, Readiness, RuntimeDriver, RuntimeError, RuntimeEvent,
        RuntimeEventKind, RuntimeKind, RuntimeSession, TOOL_HOST_PROBE_WORKSPACE_SERVER,
        ToolHostProbeInvocation, ToolHostProbeRuntimeIdentity, ToolHostProbeTool, Usage,
        tool_host_probe_tool_schema_digest,
    };

    struct FakeProbeDriver {
        executable: PathBuf,
        identity: ToolHostProbeRuntimeIdentity,
    }

    impl FakeProbeDriver {
        fn new() -> Self {
            let executable = PathBuf::from("ymp-internal-fake");
            Self {
                executable: executable.clone(),
                identity: ToolHostProbeRuntimeIdentity {
                    runtime_kind: RuntimeKind::Fake,
                    route: "fixture/no-network".to_owned(),
                    profile: "workspace-read-write-only".to_owned(),
                    cli: executable.display().to_string(),
                    cli_version: "fake-cli 1.0.0".to_owned(),
                    driver: "fake-process-driver".to_owned(),
                    driver_version: "fake-process-driver 1.0.0".to_owned(),
                    tool_schema_digest: tool_host_probe_tool_schema_digest(),
                },
            }
        }
    }

    impl RuntimeDriver for FakeProbeDriver {
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
                detail: "isolated deterministic probe fixture".to_owned(),
            })
        }

        fn start(
            &self,
            _request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            Err(RuntimeError::Unsupported("ordinary fake invocation"))
        }

        fn tool_host_probe_identity(&self) -> Result<ToolHostProbeRuntimeIdentity, RuntimeError> {
            Ok(self.identity.clone())
        }

        fn start_tool_host_probe(
            &self,
            invocation: ToolHostProbeInvocation,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            assert_eq!(
                invocation.allowed_tools,
                [
                    ToolHostProbeTool::WorkspaceWrite,
                    ToolHostProbeTool::WorkspaceRead,
                ]
            );
            let request = invocation.request;
            fs::write(
                invocation.workspace.join(&request.workspace_path),
                request.nonce.as_bytes(),
            )
            .expect("fake workspace write");
            let usage = Usage {
                input_tokens: 11,
                cached_input_tokens: 3,
                output_tokens: 5,
                reasoning_output_tokens: 2,
                cost_microusd: None,
                cost_by_model: Vec::new(),
                wall_time_ms: 7,
                protected_queries: 0,
                in_flight_excess: Default::default(),
            };
            let invocation_id = request.invocation_id.clone();
            let events = [
                RuntimeEventKind::Started {
                    opaque_session_id: "fake-session".to_owned(),
                },
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
                RuntimeEventKind::McpToolCall {
                    server: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
                    tool: ToolHostProbeTool::WorkspaceRead.to_string(),
                    status: "completed".to_owned(),
                    arguments: json!({"path": request.workspace_path}),
                    result: Some(json!({"content": request.nonce})),
                    error: None,
                },
                RuntimeEventKind::Completed {
                    usage: usage.clone(),
                },
            ]
            .into_iter()
            .enumerate()
            .map(|(index, event)| {
                let sequence = u64::try_from(index + 1).expect("bounded event sequence");
                RuntimeEvent {
                    sequence,
                    event_id: format!("{invocation_id}.event-{sequence}"),
                    invocation_id: invocation_id.clone(),
                    event,
                }
            })
            .collect();
            Ok(Box::new(FakeProbeSession { events, usage }))
        }
    }

    struct FakeProbeSession {
        events: VecDeque<RuntimeEvent>,
        usage: Usage,
    }

    impl RuntimeSession for FakeProbeSession {
        fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
            Ok(self.events.pop_front())
        }

        fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
            Err(RuntimeError::NotYielded)
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
    fn internal_probe_command_accepts_no_evidence_or_destination_material() {
        let cli = Cli::try_parse_from([
            "ymp",
            "internal",
            "tool-host-probe",
            "--admission-manifest-digest",
            &"a".repeat(64),
        ])
        .expect("minimal internal probe command");
        assert!(matches!(
            cli.command,
            Some(CliCommand::Internal {
                command: super::InternalCommand::ToolHostProbe { .. }
            })
        ));

        for forbidden in [
            "--nonce",
            "--workspace",
            "--object-digest",
            "--attestation",
            "--export",
            "--cli-version",
        ] {
            assert!(
                Cli::try_parse_from([
                    "ymp",
                    "internal",
                    "tool-host-probe",
                    "--admission-manifest-digest",
                    &"a".repeat(64),
                    forbidden,
                    "caller-controlled",
                ])
                .is_err(),
                "internal probe accepted forbidden argument {forbidden}"
            );
        }
    }

    #[test]
    fn managed_verification_materializes_the_controller_bound_candidate() {
        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        fs::create_dir_all(&source).expect("source directory");
        fs::write(source.join("result.txt"), b"before\n").expect("base file");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        fs::write(workspace.join("result.txt"), b"after\n").expect("candidate edit");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        let submitted = application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission");

        let (digest, materialized) =
            materialize_controller_candidate(&application, &data_root).expect("materialization");
        assert_eq!(digest, submitted.candidate.snapshot_digest);
        assert_eq!(
            fs::read(materialized.join("result.txt")).expect("materialized file"),
            b"after\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn passing_managed_negative_control_records_infrastructure_error() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        let negative = root.path().join("negative");
        let program = root.path().join("oracle");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&negative).expect("negative directory");
        fs::write(source.join("result.txt"), b"candidate\n").expect("source file");
        fs::write(&program, b"#!/bin/sh\nexit 0\n").expect("oracle program");
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission");
        drop(application);

        let result = super::run_managed_verification(ManagedVerificationRequest {
            data_root: data_root.clone(),
            program,
            arguments: Vec::new(),
            negative_control: negative,
            contract_digest: "1".repeat(64),
            oracle_digest: "2".repeat(64),
            wall_time_ms: Duration::from_secs(5).as_millis() as u64,
            output_limit_bytes: 1024,
        });
        assert!(result.is_ok(), "infrastructure result must be committed");

        let recovered = Application::open(&data_root).expect("reopen application");
        assert_eq!(recovered.state().status, RunStatus::InfrastructureError);
        assert!(recovered.state().candidate_digest.is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unavailable_managed_launcher_records_infrastructure_error() {
        use std::os::unix::fs::PermissionsExt;

        let launcher_available = std::process::Command::new("/usr/bin/unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--pid",
                "--net",
                "--fork",
                "/bin/true",
            ])
            .status()
            .is_ok_and(|status| status.success());
        if launcher_available {
            return;
        }

        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        let negative = root.path().join("negative");
        let program = root.path().join("oracle");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&negative).expect("negative directory");
        fs::write(source.join("result.txt"), b"candidate\n").expect("source file");
        fs::write(&program, b"#!/bin/sh\nexit 1\n").expect("oracle program");
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        let candidate_digest = application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission")
            .candidate
            .snapshot_digest;
        drop(application);

        super::run_managed_verification(ManagedVerificationRequest {
            data_root: data_root.clone(),
            program,
            arguments: Vec::new(),
            negative_control: negative,
            contract_digest: "1".repeat(64),
            oracle_digest: "2".repeat(64),
            wall_time_ms: Duration::from_secs(5).as_millis() as u64,
            output_limit_bytes: 1024,
        })
        .expect("infrastructure result must be committed");

        let recovered = Application::open(&data_root).expect("reopen application");
        assert_eq!(recovered.state().status, RunStatus::InfrastructureError);
        assert_eq!(
            recovered.state().candidate_digest.as_deref(),
            Some(candidate_digest.as_str())
        );
    }

    #[cfg(unix)]
    #[test]
    fn repeated_managed_verification_returns_the_stored_result() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        let negative = root.path().join("negative");
        let program = root.path().join("oracle");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&negative).expect("negative directory");
        fs::write(source.join("pass"), b"candidate\n").expect("candidate marker");
        fs::write(
            &program,
            b"#!/bin/sh\nif [ -f \"$1/pass\" ]; then exit 0; else exit 1; fi\n",
        )
        .expect("oracle program");
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission");
        drop(application);

        for _ in 0..2 {
            super::run_managed_verification(ManagedVerificationRequest {
                data_root: data_root.clone(),
                program: program.clone(),
                arguments: Vec::new(),
                negative_control: negative.clone(),
                contract_digest: "1".repeat(64),
                oracle_digest: "2".repeat(64),
                wall_time_ms: Duration::from_secs(5).as_millis() as u64,
                output_limit_bytes: 1024,
            })
            .expect("managed verification");
        }

        let recovered = Application::open(&data_root).expect("reopen application");
        assert_eq!(recovered.state().status, RunStatus::Accepted);
        assert_eq!(recovered.state().budget.verification_queries_remaining, 0);
        assert_eq!(recovered.events_after(0).expect("events").len(), 4);
    }

    #[test]
    fn internal_probe_path_uses_the_fake_seam_and_only_the_isolated_store() {
        let root = tempdir().expect("short isolated root");
        let project = root.path().join("project");
        let home = root.path().join("home");
        let ymp_home = root.path().join("ymp-home");
        let temporary = root.path().join("tmp");
        let build = root.path().join("build");
        let external_export = root.path().join("export");
        for directory in [
            &project,
            &home,
            &ymp_home,
            &temporary,
            &build,
            &external_export,
        ] {
            fs::create_dir(directory).expect("isolated directory");
        }
        let data_root = ymp_home.join("projects/p/runs/0001");
        Application::create(&data_root, "run-probe", Budget::new(1, 1))
            .expect("isolated application");

        let driver = FakeProbeDriver::new();
        let expected_runtime = driver
            .tool_host_probe_identity()
            .expect("fake probe identity");
        run_controller_tool_host_probe(
            data_root.clone(),
            Box::new(driver),
            ControllerToolHostProbeRequest::new(
                "a".repeat(64),
                expected_runtime,
                super::ADMISSION_TOOL_HOST_PROBE_DEADLINE_MS,
                admission_tool_host_probe_reservation(),
            ),
        )
        .expect("internal fake probe");

        let export = data_root.join(TOOL_HOST_PROBE_HANDLE_EXPORT);
        let handle: AttestedToolHostProbeHandle =
            serde_json::from_slice(&fs::read(&export).expect("handle export"))
                .expect("strict handle");
        let application = Application::open(&data_root).expect("reopen isolated application");
        let attestation = application
            .attested_tool_host_probe(&handle)
            .expect("verified attestation");
        assert_eq!(attestation.admission_manifest_digest(), "a".repeat(64));
        assert_eq!(attestation.runtime().runtime_kind, RuntimeKind::Fake);
        assert!(data_root.join("runtime-evidence/tool-host-probes").is_dir());
        for untouched in [project, home, temporary, build, external_export] {
            assert_eq!(
                fs::read_dir(&untouched)
                    .expect("untouched isolated directory")
                    .count(),
                0,
                "probe wrote outside isolated YMP_HOME: {}",
                untouched.display()
            );
        }
    }
}
