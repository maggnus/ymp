#![forbid(unsafe_code)]

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use uuid::Uuid;
use ymp_application::Application;
use ymp_application::WorkspaceSubmission;
use ymp_domain::{Command, EventKind, MAX_IDENTIFIER_CHARS, RunStatus, digest_bytes};
use ymp_runtime_api::{
    AdmittedProgram, CancellationToken, DiagnosticSummary, InvocationRequest, LaunchDescriptor,
    McpBinding, ProbeReport, ProgramIdentity, ProgramRequirement, ProgramRole, Readiness,
    RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeEventKind, RuntimeFailureKind, RuntimeKind,
    Usage, admit_lifecycle_programs,
};

const CONTRACT_SCHEMA_VERSION: u32 = 1;
const MAX_CONTRACT_BYTES: usize = 1024 * 1024;
const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_RUNTIME_EVIDENCE_BYTES: u64 = 4 * 1024 * 1024;
const RUNTIME_EVIDENCE_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractFile {
    schema_version: u32,
    contract_id: String,
    source: PathBuf,
    prompt: String,
    #[serde(default)]
    capture_exclusions: Vec<String>,
    verifier: Option<VerifierFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifierFile {
    program: PathBuf,
    #[serde(default)]
    arguments: Vec<String>,
    negative_control: PathBuf,
    oracle_digest: String,
    #[serde(default = "default_verifier_wall_time_ms")]
    wall_time_ms: u64,
    #[serde(default = "default_verifier_output_limit_bytes")]
    output_limit_bytes: usize,
}

const fn default_verifier_wall_time_ms() -> u64 {
    60_000
}

const fn default_verifier_output_limit_bytes() -> usize {
    1024 * 1024
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedVerifier {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub negative_control: PathBuf,
    pub oracle_digest: String,
    pub wall_time_ms: u64,
    pub output_limit_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedContract {
    pub contract_id: String,
    pub contract_digest: String,
    pub source: PathBuf,
    pub prompt: String,
    pub capture_exclusions: Vec<String>,
    pub verifier: Option<ManagedVerifier>,
}

impl ManagedContract {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let bytes =
            fs::read(path).with_context(|| format!("read managed contract {}", path.display()))?;
        if bytes.len() > MAX_CONTRACT_BYTES {
            bail!(
                "managed contract exceeds its {MAX_CONTRACT_BYTES}-byte limit: {}",
                path.display()
            );
        }
        let contract: ContractFile = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse managed contract {}", path.display()))?;
        if contract.schema_version != CONTRACT_SCHEMA_VERSION {
            bail!(
                "unsupported managed contract schema version {}",
                contract.schema_version
            );
        }
        if contract.contract_id.is_empty()
            || contract.contract_id.chars().count() > MAX_IDENTIFIER_CHARS
        {
            bail!("contract_id must contain between 1 and {MAX_IDENTIFIER_CHARS} characters");
        }
        if contract.prompt.is_empty() || contract.prompt.len() > MAX_PROMPT_BYTES {
            bail!("prompt must contain between 1 and {MAX_PROMPT_BYTES} bytes");
        }
        for exclusion in &contract.capture_exclusions {
            validate_relative_path(exclusion)?;
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let source = if contract.source.is_absolute() {
            contract.source
        } else {
            parent.join(contract.source)
        }
        .canonicalize()
        .with_context(|| format!("canonicalize managed source for {}", path.display()))?;
        if !source.is_dir() {
            bail!(
                "managed contract source is not a directory: {}",
                source.display()
            );
        }
        let verifier = contract
            .verifier
            .map(|verifier| load_verifier(verifier, parent))
            .transpose()?;
        Ok(Self {
            contract_id: contract.contract_id,
            contract_digest: digest_bytes(&bytes),
            source,
            prompt: contract.prompt,
            capture_exclusions: contract.capture_exclusions,
            verifier,
        })
    }
}

fn load_verifier(verifier: VerifierFile, parent: &Path) -> anyhow::Result<ManagedVerifier> {
    if verifier.arguments.len() > 32
        || verifier
            .arguments
            .iter()
            .any(|argument| argument.len() > 4096)
    {
        bail!("verifier arguments exceed the declared count or size limit");
    }
    if !is_canonical_digest(&verifier.oracle_digest) {
        bail!("verifier oracle_digest is not a canonical lowercase SHA-256 digest");
    }
    if verifier.wall_time_ms == 0 || verifier.wall_time_ms > 10 * 60 * 1000 {
        bail!("verifier wall_time_ms must be between 1 and 600000");
    }
    if verifier.output_limit_bytes == 0 || verifier.output_limit_bytes > 16 * 1024 * 1024 {
        bail!("verifier output_limit_bytes must be between 1 and 16777216");
    }
    let program = resolve_contract_path(parent, verifier.program, "verifier program")?;
    if !program.is_file() {
        bail!("verifier program is not a file: {}", program.display());
    }
    if !is_executable_file(&program)? {
        bail!("verifier program is not executable: {}", program.display());
    }
    let negative_control = resolve_contract_path(
        parent,
        verifier.negative_control,
        "verifier negative control",
    )?;
    if !negative_control.is_dir() {
        bail!(
            "verifier negative control is not a directory: {}",
            negative_control.display()
        );
    }
    Ok(ManagedVerifier {
        program,
        arguments: verifier.arguments,
        negative_control,
        oracle_digest: verifier.oracle_digest,
        wall_time_ms: verifier.wall_time_ms,
        output_limit_bytes: verifier.output_limit_bytes,
    })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> anyhow::Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    Ok(fs::metadata(path)?.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> anyhow::Result<bool> {
    Ok(path.is_file())
}

fn resolve_contract_path(parent: &Path, path: PathBuf, kind: &str) -> anyhow::Result<PathBuf> {
    let path = if path.is_absolute() {
        path
    } else {
        parent.join(path)
    };
    path.canonicalize()
        .with_context(|| format!("canonicalize {kind} {}", path.display()))
}

fn is_canonical_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_relative_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("capture exclusion is not a normalized relative path: {path}");
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ManagedCandidateRequest {
    pub contract: ManagedContract,
    pub bridge_executable: PathBuf,
}

#[derive(Clone, Debug)]
pub enum ManagedRunEvent {
    Runtime(RuntimeEvent),
    CandidateAvailable {
        candidate_digest: String,
        change_count: Option<usize>,
    },
    Finished,
    Failed {
        detail: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RuntimeEvidenceKind {
    Launch {
        descriptor: LaunchDescriptor,
        descriptor_digest: String,
    },
    Started {
        opaque_session_digest: String,
    },
    Output {
        text_digest: String,
        text_bytes: usize,
    },
    McpToolCall {
        server: String,
        tool: String,
        status: String,
        arguments_digest: String,
        result_digest: Option<String>,
        error_digest: Option<String>,
    },
    Yielded {
        cursor_digest: String,
    },
    Completed {
        usage: Usage,
    },
    Failed {
        kind: RuntimeFailureKind,
        usage: Usage,
        diagnostic: Option<DiagnosticSummary>,
    },
    TimedOut {
        limit_ms: u64,
        usage: Usage,
    },
    Cancelled {
        usage: Usage,
    },
    Interrupted,
}

#[derive(Serialize)]
struct RuntimeProfileEvidence<'a> {
    schema_version: u32,
    runtime_kind: RuntimeKind,
    invocation_id: &'a str,
    probe: &'a ProbeReport,
    runtime_executable_digest: Option<String>,
    launch_descriptor: Option<&'a LaunchDescriptor>,
    launch_descriptor_digest: Option<&'a str>,
    /// The program that builds the managed workspace baseline. It is not part of the launch
    /// descriptor because it runs before the runtime is prepared.
    workspace_program: &'a AdmittedProgram,
    /// The utilities the run executes to observe and terminate what it started. They are named here
    /// so that the record enumerates every program the run executes on its own behalf, not only the
    /// ones the launch enters.
    lifecycle_programs: &'a [AdmittedProgram],
    coordination: CoordinationEvidence,
    environment_policy: &'static str,
}

#[derive(Serialize)]
struct CoordinationEvidence {
    transport: &'static str,
    bridge_executable_digest: String,
    endpoint_path_digest: String,
    allowed_tools: [&'static str; 4],
    invocation_scoped: bool,
    credential_values_recorded: bool,
}

#[derive(Serialize)]
struct RuntimeEvidenceDigestInput<'a> {
    schema_version: u32,
    run_id: &'a str,
    attempt_id: &'a str,
    invocation_id: &'a str,
    runtime_kind: RuntimeKind,
    contract_id: &'a str,
    contract_digest: &'a str,
    profile_digest: &'a str,
    sequence: u64,
    event_id: &'a str,
    predecessor_digest: &'a Option<String>,
    event: &'a RuntimeEvidenceKind,
}

#[derive(Serialize)]
struct RuntimeEvidenceEnvelope<'a> {
    schema_version: u32,
    run_id: &'a str,
    attempt_id: &'a str,
    invocation_id: &'a str,
    runtime_kind: RuntimeKind,
    contract_id: &'a str,
    contract_digest: &'a str,
    profile_digest: &'a str,
    sequence: u64,
    event_id: &'a str,
    predecessor_digest: &'a Option<String>,
    event: &'a RuntimeEvidenceKind,
    digest: &'a str,
}

pub struct ManagedRunHandle {
    attempt_id: String,
    invocation_id: String,
    runtime_kind: RuntimeKind,
    cancellation: CancellationToken,
    application: Arc<Mutex<Application>>,
    receiver: Receiver<ManagedRunEvent>,
    control_sender: Sender<ManagedControl>,
    lifecycle: Arc<Mutex<ManagedLifecycle>>,
    finished: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

enum ManagedControl {
    Wake { input: String },
}

#[derive(Default)]
struct ManagedLifecycle {
    yielded: bool,
    terminal: bool,
    wake_commands: HashMap<String, String>,
}

impl ManagedRunHandle {
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    pub fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    pub const fn runtime_kind(&self) -> RuntimeKind {
        self.runtime_kind
    }

    pub fn try_next(&self) -> Option<ManagedRunEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    pub fn cancel(&self, reason: impl Into<String>) -> anyhow::Result<()> {
        let reason = reason.into();
        let mut application = self
            .application
            .lock()
            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
        if application.state().status == RunStatus::Running {
            application.execute(
                format!("{}.cancel", self.attempt_id),
                Command::Cancel { reason },
            )?;
        }
        self.cancellation.cancel();
        Ok(())
    }

    pub fn wake(
        &self,
        command_id: impl Into<String>,
        input: impl Into<String>,
    ) -> anyhow::Result<()> {
        let command_id = command_id.into();
        let input = input.into();
        let command_chars = command_id.chars().count();
        if !(1..=MAX_IDENTIFIER_CHARS).contains(&command_chars) {
            bail!(
                "wake command identifier must contain between 1 and {MAX_IDENTIFIER_CHARS} characters"
            );
        }
        if input.is_empty() || input.len() > MAX_PROMPT_BYTES {
            bail!("wake input must contain between 1 and {MAX_PROMPT_BYTES} bytes");
        }
        let input_digest = digest_bytes(input.as_bytes());
        let mut lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| anyhow::anyhow!("managed lifecycle lock was poisoned"))?;
        if let Some(recorded) = lifecycle.wake_commands.get(&command_id) {
            if recorded == &input_digest {
                return Ok(());
            }
            bail!("wake command identifier was reused with different input");
        }
        if lifecycle.terminal {
            bail!("managed runtime is terminal");
        }
        if !lifecycle.yielded {
            bail!("managed runtime is not yielded");
        }
        lifecycle
            .wake_commands
            .insert(command_id.clone(), input_digest);
        lifecycle.yielded = false;
        if self
            .control_sender
            .send(ManagedControl::Wake { input })
            .is_err()
        {
            lifecycle.wake_commands.remove(&command_id);
            lifecycle.yielded = true;
            bail!("managed runtime control channel is closed");
        }
        Ok(())
    }

    pub fn join(mut self) -> anyhow::Result<()> {
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("managed runtime worker panicked"))?;
        }
        Ok(())
    }
}

impl Drop for ManagedRunHandle {
    fn drop(&mut self) {
        if !self.is_finished() {
            let _ = self.cancel("managed runtime controller closed");
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Whether the launch of the runtime being started has to be attested.
#[derive(Clone, Copy, Eq, PartialEq)]
enum LaunchAttestation {
    Required,
    Waived,
}

/// The one gate every start of a runtime passes, wherever that start is written.
///
/// Two things are established here and nowhere else. The runtime must attest the programs its
/// launch enters, because attestation is what binds the bytes that execute to the bytes that were
/// admitted; and the utilities the run observes and ends its own processes with must be admitted,
/// because a run that cannot see what it starts cannot say what it left running. The admitted
/// utilities are returned, so the caller records what it will execute on its own behalf.
///
/// A start written outside this crate reaches the same gate by calling this function. That every
/// such start does is not left to reading: `ymp-cli/tests/unattested_runtime_is_unreachable.rs`
/// rejects a shipped module that builds a runtime driver and starts it without this call, and
/// drives the built product to establish that no command offers a runtime this gate would refuse.
pub fn admit_runtime_start(kind: RuntimeKind) -> anyhow::Result<Vec<AdmittedProgram>> {
    admit_start(kind, LaunchAttestation::Required)
}

fn admit_start(
    kind: RuntimeKind,
    attestation: LaunchAttestation,
) -> anyhow::Result<Vec<AdmittedProgram>> {
    if attestation == LaunchAttestation::Required && !requires_launch_attestation(kind) {
        bail!(
            "{kind:?} does not attest the programs its launch enters, so it cannot start a managed \
             run"
        );
    }
    // The run observes and ends its own processes with these utilities. One that cannot be admitted
    // stops the run here, naming the program and the reason, rather than leaving a run that reports
    // a clean termination it was never able to establish.
    admit_lifecycle_programs()
        .context("the utilities this run observes and ends its own processes with")
}

/// Starts a managed run that produces a candidate. The runtime passes [`admit_runtime_start`]
/// before anything of it is executed.
pub fn start_managed_candidate(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    request: ManagedCandidateRequest,
) -> anyhow::Result<ManagedRunHandle> {
    start_candidate(application, driver, request, LaunchAttestation::Required)
}

/// Starts a managed run with a runtime whose launch is not attested. This exists for the checks
/// that must drive the controller without a runtime executable; nothing the product ships names it.
#[doc(hidden)]
pub fn start_unattested_managed_candidate(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    request: ManagedCandidateRequest,
) -> anyhow::Result<ManagedRunHandle> {
    start_candidate(application, driver, request, LaunchAttestation::Waived)
}

fn start_candidate(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    request: ManagedCandidateRequest,
    attestation: LaunchAttestation,
) -> anyhow::Result<ManagedRunHandle> {
    let lifecycle_programs = admit_start(driver.kind(), attestation)?;
    let probe = driver.probe()?;
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    let bridge_executable = request.bridge_executable.canonicalize().with_context(|| {
        format!(
            "canonicalize current ymp executable {}",
            request.bridge_executable.display()
        )
    })?;
    let runtime_kind = driver.kind();
    let probed_runtime_executable_digest = digest_regular_file(driver.executable())?;
    let bridge_executable_digest = digest_regular_file(&bridge_executable)?.ok_or_else(|| {
        anyhow::anyhow!(
            "current ymp executable is not a regular file: {}",
            bridge_executable.display()
        )
    })?;
    let workspace_program = admit_workspace_program()?;
    let contract_id = request.contract.contract_id.clone();
    let contract_digest = request.contract.contract_digest.clone();
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let invocation_id = format!("invocation-{}", Uuid::new_v4());
    let evidence_invocation_id = invocation_id.clone();
    let (data_root, run_id, base_digest, workspace, controller_cursor) = {
        let mut application = application
            .lock()
            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
        if application.state().status != RunStatus::Running {
            bail!("run is already terminal");
        }
        if !application.state().active_attempts.is_empty() {
            bail!("the POC profile admits only one active attempt");
        }
        let data_root = application.data_root().to_path_buf();
        let base = application
            .artifact_store()
            .capture_source(&request.contract.source)?;
        let workspace = data_root.join("workspaces").join(&attempt_id);
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)?;
        initialize_private_git(&workspace, &workspace_program)?;
        application.execute(
            format!("{attempt_id}.start"),
            Command::StartAttempt {
                attempt_id: attempt_id.clone(),
            },
        )?;
        (
            data_root,
            application.state().run_id.clone(),
            base.manifest_digest,
            workspace,
            application.state().last_sequence,
        )
    };

    let token = Uuid::new_v4().simple().to_string();
    let socket_path = std::env::temp_dir()
        .join("ymp-runtime")
        .join(format!("{}.sock", Uuid::new_v4().simple()));
    let endpoint_path_digest = digest_bytes(socket_path.to_string_lossy().as_bytes());
    let exclusions = request.contract.capture_exclusions.clone();
    let rpc_server = match ymp_agent_rpc::AgentRpcServer::start_with_submission_for_invocation(
        &socket_path,
        &token,
        &attempt_id,
        &invocation_id,
        Arc::clone(&application),
        WorkspaceSubmission::new(&base_digest, &workspace, exclusions.clone()),
    ) {
        Ok(server) => server,
        Err(error) => {
            record_infrastructure_failure(&application, &attempt_id, &error.to_string());
            return Err(error.into());
        }
    };
    let invocation_control = rpc_server
        .invocation_control()
        .context("managed invocation control was not created")?;
    let cancellation = CancellationToken::default();
    let invocation_request = InvocationRequest {
        invocation_id: invocation_id.clone(),
        attempt_id: attempt_id.clone(),
        workspace: workspace.clone(),
        mcp: Some(McpBinding {
            executable: bridge_executable,
            socket_path,
            token,
        }),
        prompt: request.contract.prompt,
        cancellation: cancellation.clone(),
    };
    let launch_descriptor = match driver.prepare_launch(&invocation_request) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            record_infrastructure_failure(&application, &attempt_id, runtime_error_code(&error));
            return Err(error.into());
        }
    };
    if requires_launch_attestation(runtime_kind) && launch_descriptor.is_none() {
        record_infrastructure_failure(
            &application,
            &attempt_id,
            "runtime_launch_descriptor_missing",
        );
        bail!("{runtime_kind:?} did not provide a launch descriptor");
    }
    if let Some(descriptor) = &launch_descriptor {
        validate_launch_descriptor(descriptor, &attempt_id, &invocation_id, &workspace)?;
        // A driver that omitted the chain, or that bound one of its programs to a digest it chose
        // itself rather than to a location this account cannot write, would reach the same launch
        // through a program the account could have planted. Both are refused here, where the
        // runtime kind says the launch is one that must be attested.
        if cfg!(unix) && requires_launch_attestation(runtime_kind) {
            let admits_shell = descriptor
                .launch_chain
                .iter()
                .any(|program| program.role == ProgramRole::LaunchShell);
            let bound_by_location = descriptor
                .launch_chain
                .iter()
                .all(|program| program.identity == ProgramIdentity::SystemPath);
            if !admits_shell || !bound_by_location {
                record_infrastructure_failure(
                    &application,
                    &attempt_id,
                    "runtime_launch_chain_missing",
                );
                bail!(
                    "{runtime_kind:?} did not admit the programs its launch chain enters at a \
                     location this account cannot write"
                );
            }
        }
    }
    let launch_descriptor_digest = launch_descriptor
        .as_ref()
        .map(digest_serialized)
        .transpose()?;
    let runtime_executable_digest = launch_descriptor
        .as_ref()
        .map(|descriptor| descriptor.executable_digest.clone())
        .map(Some)
        .unwrap_or(probed_runtime_executable_digest);
    let bridge_executable_digest = launch_descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.coordination_executable_digest.clone())
        .unwrap_or(bridge_executable_digest);
    let evidence_directory = data_root.join("runtime-evidence").join(&attempt_id);
    if let Err(error) = fs::create_dir_all(&evidence_directory) {
        record_infrastructure_failure(&application, &attempt_id, &error.to_string());
        return Err(error.into());
    }
    let mut evidence_file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(evidence_directory.join("events.jsonl"))
    {
        Ok(file) => file,
        Err(error) => {
            record_infrastructure_failure(&application, &attempt_id, &error.to_string());
            return Err(error.into());
        }
    };
    let profile_digest = match write_runtime_profile_evidence(
        &evidence_directory,
        RuntimeProfileEvidence {
            schema_version: RUNTIME_EVIDENCE_SCHEMA_VERSION,
            runtime_kind,
            invocation_id: &evidence_invocation_id,
            probe: &probe,
            runtime_executable_digest,
            launch_descriptor: launch_descriptor.as_ref(),
            launch_descriptor_digest: launch_descriptor_digest.as_deref(),
            workspace_program: &workspace_program,
            lifecycle_programs: &lifecycle_programs,
            coordination: CoordinationEvidence {
                transport: "stdio_mcp_via_private_rpc",
                bridge_executable_digest,
                endpoint_path_digest,
                allowed_tools: ["read_control", "read_events", "yield", "submit"],
                invocation_scoped: true,
                credential_values_recorded: false,
            },
            environment_policy: match runtime_kind {
                RuntimeKind::Codex => "synthetic_allowlist_v1",
                // Claude Code resolves its subscription credential from the home directory, so a
                // generated home is authenticated only after the operator's credential is
                // delegated into it. `SECURITY.md` requires that weaker profile to be labelled
                // rather than presented as strict containment.
                RuntimeKind::ClaudeCode => "synthetic_allowlist_with_delegated_credential_v1",
                RuntimeKind::Fake => "deterministic_fixture",
            },
        },
    ) {
        Ok(digest) => digest,
        Err(error) => {
            record_infrastructure_failure(&application, &attempt_id, &error.to_string());
            return Err(error);
        }
    };
    let mut session = match driver.start_prepared(invocation_request, launch_descriptor.as_ref()) {
        Ok(session) => session,
        Err(error) => {
            record_infrastructure_failure(&application, &attempt_id, runtime_error_code(&error));
            return Err(error.into());
        }
    };
    let (sender, receiver) = channel();
    let (control_sender, control_receiver) = channel();
    let lifecycle = Arc::new(Mutex::new(ManagedLifecycle::default()));
    let worker_lifecycle = Arc::clone(&lifecycle);
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = Arc::clone(&finished);
    let worker_application = Arc::clone(&application);
    let worker_attempt = attempt_id.clone();
    let worker_invocation = invocation_id.clone();
    let worker_cancellation = cancellation.clone();
    let initial_launch_descriptor = launch_descriptor.clone();
    let worker = thread::Builder::new()
        .name(format!("ymp-runtime-{worker_attempt}"))
        .spawn(move || {
            let _rpc_server = rpc_server;
            let result = (|| -> anyhow::Result<()> {
                let mut completed = false;
                let mut predecessor_digest = None;
                let mut runtime_progress = RuntimeProgress::new(&worker_invocation);
                let mut expected_session = None;
                let mut saw_launch = !requires_launch_attestation(runtime_kind);
                let mut pending_error = None;
                let mut terminal_failure = None;
                let mut application_cursor = controller_cursor;
                let mut yield_cursor = 0;
                loop {
                    let mut event = match pending_error.take().map_or_else(
                        || session.next_event(),
                        Err::<Option<RuntimeEvent>, RuntimeError>,
                    ) {
                        Ok(Some(event)) => event,
                        Ok(None) => {
                            if worker_cancellation.is_cancelled() {
                                break;
                            }
                            bail!("runtime ended without a terminal event");
                        }
                        Err(error) => RuntimeEvent {
                            sequence: runtime_progress.next_sequence()?,
                            event_id: format!(
                                "{}.event-{}",
                                worker_invocation,
                                runtime_progress.next_sequence()?
                            ),
                            invocation_id: worker_invocation.clone(),
                            event: RuntimeEventKind::Failed {
                                kind: runtime_error_kind(&error),
                                usage: session.usage(),
                                diagnostic: runtime_error_diagnostic(&error),
                            },
                        },
                    };
                    if matches!(
                        &event.event,
                        RuntimeEventKind::Completed { .. } | RuntimeEventKind::Yielded { .. }
                    ) {
                        event.event = authoritative_lifecycle_event(
                            LifecycleAuthority {
                                application: &worker_application,
                                attempt_id: &worker_attempt,
                                invocation_id: &worker_invocation,
                                invocation_control: &invocation_control,
                                application_cursor: &mut application_cursor,
                                yield_cursor: &mut yield_cursor,
                            },
                            &event.event,
                            session.usage(),
                        )?;
                    }
                    if !saw_launch && !matches!(&event.event, RuntimeEventKind::Launch { .. }) {
                        bail!("{runtime_kind:?} emitted an event before launch attestation");
                    }
                    runtime_progress.validate(&event)?;
                    match &event.event {
                        RuntimeEventKind::Launch { descriptor } => {
                            validate_runtime_launch(
                                runtime_kind,
                                initial_launch_descriptor.as_ref(),
                                descriptor,
                                saw_launch,
                                expected_session.as_deref(),
                            )?;
                            saw_launch = true;
                        }
                        RuntimeEventKind::Started { opaque_session_id } => {
                            if let Some(expected) = &expected_session {
                                if expected != opaque_session_id {
                                    bail!("runtime session identifier changed across resume");
                                }
                            } else {
                                expected_session = Some(opaque_session_id.clone());
                            }
                        }
                        _ => {}
                    }
                    predecessor_digest = Some(append_runtime_evidence(
                        &mut evidence_file,
                        &run_id,
                        &worker_attempt,
                        runtime_kind,
                        &contract_id,
                        &contract_digest,
                        &profile_digest,
                        &predecessor_digest,
                        &event,
                    )?);
                    let yielded = matches!(&event.event, RuntimeEventKind::Yielded { .. });
                    let terminal = matches!(
                        &event.event,
                        RuntimeEventKind::Completed { .. }
                            | RuntimeEventKind::Failed { .. }
                            | RuntimeEventKind::TimedOut { .. }
                            | RuntimeEventKind::Cancelled { .. }
                            | RuntimeEventKind::Interrupted
                    );
                    completed = matches!(&event.event, RuntimeEventKind::Completed { .. });
                    terminal_failure = match &event.event {
                        RuntimeEventKind::Failed { .. } => Some("managed_runtime_failed"),
                        RuntimeEventKind::TimedOut { .. } => Some("managed_runtime_timed_out"),
                        _ => None,
                    };
                    if yielded {
                        worker_lifecycle
                            .lock()
                            .map_err(|_| anyhow::anyhow!("managed lifecycle lock was poisoned"))?
                            .yielded = true;
                    }
                    let _ = sender.send(ManagedRunEvent::Runtime(event));
                    if terminal {
                        break;
                    }
                    if yielded {
                        loop {
                            if worker_cancellation.is_cancelled() {
                                if let Err(error) = session.interrupt() {
                                    pending_error = Some(error);
                                }
                                break;
                            }
                            match control_receiver
                                .recv_timeout(std::time::Duration::from_millis(10))
                            {
                                Ok(ManagedControl::Wake { input }) => {
                                    if let Err(error) = session.resume(input) {
                                        pending_error = Some(error);
                                    }
                                    break;
                                }
                                Err(RecvTimeoutError::Timeout) => {}
                                Err(RecvTimeoutError::Disconnected) => {
                                    if let Err(error) = session.interrupt() {
                                        pending_error = Some(error);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                if let Some(failure) = terminal_failure {
                    bail!(failure);
                }
                if worker_cancellation.is_cancelled() {
                    return Ok(());
                }
                if !completed {
                    bail!("runtime ended without a completed event");
                }
                let candidate_digest = {
                    let application = worker_application
                        .lock()
                        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
                    if application.state().status != RunStatus::Running {
                        return Ok(());
                    }
                    application
                        .state()
                        .candidate_digest
                        .clone()
                        .context("completed runtime has no controller-committed candidate")?
                };
                let _ = sender.send(ManagedRunEvent::CandidateAvailable {
                    candidate_digest,
                    change_count: None,
                });
                Ok(())
            })();
            // The runtime session terminates its process tree when it is dropped, so it is dropped
            // here rather than at the end of the thread. Without this the controller could observe
            // a terminal outcome while the managed processes were still being signalled, and a
            // reading of the process table taken at that moment would be racing the supervisor
            // instead of measuring it.
            drop(session);
            if let Ok(mut lifecycle) = worker_lifecycle.lock() {
                lifecycle.yielded = false;
                lifecycle.terminal = true;
            }
            if let Err(error) = result {
                let _ = error;
                let detail = "managed_runtime_supervision_failed".to_owned();
                record_infrastructure_failure(&worker_application, &worker_attempt, &detail);
                let _ = sender.send(ManagedRunEvent::Failed { detail });
            }
            let _ = sender.send(ManagedRunEvent::Finished);
            worker_finished.store(true, Ordering::Release);
        })?;

    Ok(ManagedRunHandle {
        attempt_id,
        invocation_id,
        runtime_kind,
        cancellation,
        application,
        receiver,
        control_sender,
        lifecycle,
        finished,
        worker: Some(worker),
    })
}

struct LifecycleAuthority<'a> {
    application: &'a Arc<Mutex<Application>>,
    attempt_id: &'a str,
    invocation_id: &'a str,
    invocation_control: &'a ymp_agent_rpc::InvocationControl,
    application_cursor: &'a mut u64,
    yield_cursor: &'a mut u64,
}

fn authoritative_lifecycle_event(
    authority: LifecycleAuthority<'_>,
    runtime_event: &RuntimeEventKind,
    fallback_usage: Usage,
) -> anyhow::Result<RuntimeEventKind> {
    let completed_usage = match runtime_event {
        RuntimeEventKind::Completed { usage } => Some(usage.clone()),
        RuntimeEventKind::Yielded { .. } => None,
        _ => bail!("controller classification requires a lifecycle runtime event"),
    };
    let application = authority
        .application
        .lock()
        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
    let events = application.events_after(*authority.application_cursor)?;
    if let Some(last) = events.last() {
        *authority.application_cursor = last.sequence;
    }
    let submissions: Vec<_> = events
        .iter()
        .filter(|event| match &event.event {
            EventKind::CandidateSubmitted {
                attempt_id: submitted_attempt,
                ..
            } => submitted_attempt == authority.attempt_id,
            _ => false,
        })
        .collect();
    drop(application);
    let yields = authority
        .invocation_control
        .confirmations_after(*authority.yield_cursor)?;
    if let Some(last) = yields.last() {
        *authority.yield_cursor = last.sequence;
    }
    let action_count = submissions.len().saturating_add(yields.len());
    let classified = match action_count {
        1 if submissions.len() == 1 && completed_usage.is_some() => RuntimeEventKind::Completed {
            usage: completed_usage.expect("checked completed usage"),
        },
        1 if yields.len() == 1 => {
            let confirmation = &yields[0];
            if confirmation.attempt_id != authority.attempt_id
                || confirmation.invocation_id != authority.invocation_id
            {
                bail!("yield confirmation identity does not match managed invocation");
            }
            RuntimeEventKind::Yielded {
                cursor: confirmation.command_id.clone(),
            }
        }
        _ => RuntimeEventKind::Failed {
            kind: RuntimeFailureKind::Protocol,
            usage: completed_usage.unwrap_or(fallback_usage),
            diagnostic: Some(DiagnosticSummary::from_bytes(
                b"controller_action_count_invalid",
                false,
            )),
        },
    };
    Ok(classified)
}

#[allow(clippy::too_many_arguments)]
fn append_runtime_evidence(
    file: &mut File,
    run_id: &str,
    attempt_id: &str,
    runtime_kind: RuntimeKind,
    contract_id: &str,
    contract_digest: &str,
    profile_digest: &str,
    predecessor_digest: &Option<String>,
    runtime_event: &RuntimeEvent,
) -> anyhow::Result<String> {
    let event = runtime_evidence_kind(&runtime_event.event)?;
    let input = RuntimeEvidenceDigestInput {
        schema_version: RUNTIME_EVIDENCE_SCHEMA_VERSION,
        run_id,
        attempt_id,
        invocation_id: &runtime_event.invocation_id,
        runtime_kind,
        contract_id,
        contract_digest,
        profile_digest,
        sequence: runtime_event.sequence,
        event_id: &runtime_event.event_id,
        predecessor_digest,
        event: &event,
    };
    let digest = digest_bytes(&serde_json::to_vec(&input)?);
    let envelope = RuntimeEvidenceEnvelope {
        schema_version: RUNTIME_EVIDENCE_SCHEMA_VERSION,
        run_id,
        attempt_id,
        invocation_id: &runtime_event.invocation_id,
        runtime_kind,
        contract_id,
        contract_digest,
        profile_digest,
        sequence: runtime_event.sequence,
        event_id: &runtime_event.event_id,
        predecessor_digest,
        event: &event,
        digest: &digest,
    };
    let bytes = serde_json::to_vec(&envelope)?;
    let existing = file.metadata()?.len();
    if existing.saturating_add(bytes.len() as u64 + 1) > MAX_RUNTIME_EVIDENCE_BYTES {
        bail!("runtime evidence exceeded its {MAX_RUNTIME_EVIDENCE_BYTES}-byte transcript limit");
    }
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_data()?;
    Ok(digest)
}

fn write_runtime_profile_evidence(
    directory: &Path,
    profile: RuntimeProfileEvidence<'_>,
) -> anyhow::Result<String> {
    let profile = serde_json::to_value(profile)?;
    let digest = digest_bytes(&serde_json::to_vec(&profile)?);
    let envelope = serde_json::json!({
        "profile": profile,
        "digest": digest,
    });
    let bytes = serde_json::to_vec(&envelope)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("profile.json"))?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_data()?;
    Ok(digest)
}

fn digest_regular_file(path: &Path) -> anyhow::Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(digest_bytes(&fs::read(path)?)))
}

fn runtime_evidence_kind(event: &RuntimeEventKind) -> anyhow::Result<RuntimeEvidenceKind> {
    Ok(match event {
        RuntimeEventKind::Launch { descriptor } => RuntimeEvidenceKind::Launch {
            descriptor: descriptor.as_ref().clone(),
            descriptor_digest: digest_serialized(descriptor)?,
        },
        RuntimeEventKind::Started { opaque_session_id } => RuntimeEvidenceKind::Started {
            opaque_session_digest: digest_bytes(opaque_session_id.as_bytes()),
        },
        RuntimeEventKind::Output { text } => RuntimeEvidenceKind::Output {
            text_digest: digest_bytes(text.as_bytes()),
            text_bytes: text.len(),
        },
        RuntimeEventKind::McpToolCall {
            server,
            tool,
            status,
            arguments,
            result,
            error,
        } => RuntimeEvidenceKind::McpToolCall {
            server: server.clone(),
            tool: tool.clone(),
            status: status.clone(),
            arguments_digest: digest_json(arguments)?,
            result_digest: result.as_ref().map(digest_json).transpose()?,
            error_digest: error.as_ref().map(digest_json).transpose()?,
        },
        RuntimeEventKind::Yielded { cursor } => RuntimeEvidenceKind::Yielded {
            cursor_digest: digest_bytes(cursor.as_bytes()),
        },
        RuntimeEventKind::Completed { usage } => RuntimeEvidenceKind::Completed {
            usage: usage.clone(),
        },
        RuntimeEventKind::Failed {
            kind,
            usage,
            diagnostic,
        } => RuntimeEvidenceKind::Failed {
            kind: *kind,
            usage: usage.clone(),
            diagnostic: diagnostic.clone(),
        },
        RuntimeEventKind::TimedOut { limit_ms, usage } => RuntimeEvidenceKind::TimedOut {
            limit_ms: *limit_ms,
            usage: usage.clone(),
        },
        RuntimeEventKind::Cancelled { usage } => RuntimeEvidenceKind::Cancelled {
            usage: usage.clone(),
        },
        RuntimeEventKind::Interrupted => RuntimeEvidenceKind::Interrupted,
    })
}

fn digest_json(value: &Value) -> anyhow::Result<String> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
}

struct RuntimeProgress {
    invocation_id: String,
    last_sequence: u64,
    event_ids: HashSet<String>,
}

impl RuntimeProgress {
    fn new(invocation_id: &str) -> Self {
        Self {
            invocation_id: invocation_id.to_owned(),
            last_sequence: 0,
            event_ids: HashSet::new(),
        }
    }

    fn next_sequence(&self) -> anyhow::Result<u64> {
        self.last_sequence
            .checked_add(1)
            .context("runtime event sequence exhausted")
    }

    fn validate(&mut self, event: &RuntimeEvent) -> anyhow::Result<()> {
        if event.invocation_id != self.invocation_id {
            bail!("runtime event invocation_id does not match the launch descriptor");
        }
        let event_id_chars = event.event_id.chars().count();
        if !(1..=MAX_IDENTIFIER_CHARS).contains(&event_id_chars) {
            bail!("runtime event_id must contain between 1 and {MAX_IDENTIFIER_CHARS} characters");
        }
        if self.event_ids.contains(&event.event_id) {
            bail!("runtime repeated event_id {}", event.event_id);
        }
        let expected_sequence = self
            .last_sequence
            .checked_add(1)
            .context("runtime event sequence exhausted")?;
        if event.sequence != expected_sequence {
            bail!(
                "runtime event sequence {} does not match expected sequence {expected_sequence}",
                event.sequence
            );
        }
        self.event_ids.insert(event.event_id.clone());
        self.last_sequence = event.sequence;
        Ok(())
    }
}

fn digest_serialized(value: &impl Serialize) -> anyhow::Result<String> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
}

fn validate_launch_descriptor(
    descriptor: &LaunchDescriptor,
    attempt_id: &str,
    invocation_id: &str,
    workspace: &Path,
) -> anyhow::Result<()> {
    if descriptor.schema_version != 1 {
        bail!(
            "unsupported launch descriptor schema version {}",
            descriptor.schema_version
        );
    }
    if descriptor.attempt_id != attempt_id
        || descriptor.invocation_id != invocation_id
        || descriptor.working_directory != workspace
    {
        bail!("launch descriptor identity does not match the managed request");
    }
    let executable_digest = digest_regular_file(&descriptor.executable)?
        .context("launch descriptor executable is not a regular file")?;
    if executable_digest != descriptor.executable_digest {
        bail!("launch descriptor executable digest does not match its bytes");
    }
    // The controller re-establishes each program under the requirement it was admitted with, not
    // only its digest: a program recorded as standing where the account cannot write is checked
    // against that location again here. What the controller does not do is decide which programs
    // the chain should contain; that check is made where the runtime kind is known, because only
    // there is it known whether the launch is one that must be attested.
    for program in &descriptor.launch_chain {
        program
            .verify()
            .map_err(|error| anyhow::anyhow!("launch descriptor {error}"))?;
    }
    match (
        descriptor.coordination_executable.as_deref(),
        descriptor.coordination_executable_digest.as_deref(),
    ) {
        (Some(executable), Some(expected_digest)) => {
            let actual_digest = digest_regular_file(executable)?
                .context("launch descriptor MCP executable is not a regular file")?;
            if actual_digest != expected_digest {
                bail!("launch descriptor MCP executable digest does not match its bytes");
            }
        }
        (None, None) => {}
        _ => bail!("launch descriptor MCP executable and digest must be declared together"),
    }
    let mut names = HashSet::new();
    for variable in &descriptor.environment {
        if variable.name.is_empty() || !names.insert(variable.name.as_str()) {
            bail!("launch descriptor environment contains an empty or duplicate name");
        }
        if variable.confidential && variable.value.is_some() {
            bail!("launch descriptor records a confidential environment value");
        }
        if let Some(value) = &variable.value
            && digest_bytes(value.as_bytes()) != variable.value_digest
        {
            bail!("launch descriptor environment digest does not match its value");
        }
    }
    Ok(())
}

/// Runtimes whose managed process must be created from an immutable launch descriptor and must
/// attest that descriptor before any other event.
const fn requires_launch_attestation(kind: RuntimeKind) -> bool {
    matches!(kind, RuntimeKind::Codex | RuntimeKind::ClaudeCode)
}

/// The exact arguments a resumed managed process may use. Each runtime names its own resume
/// operand, so the rule is stated per runtime rather than inferred from the observed arguments.
fn expected_resume_arguments(
    kind: RuntimeKind,
    initial: &[String],
    session: &str,
) -> anyhow::Result<Vec<String>> {
    let mut expected = initial.to_vec();
    match kind {
        RuntimeKind::Codex => {
            let prompt_source = expected
                .pop()
                .context("initial runtime launch has no prompt-source argument")?;
            expected.extend(["resume".to_owned(), session.to_owned(), prompt_source]);
        }
        RuntimeKind::ClaudeCode => {
            expected.extend(["--resume".to_owned(), session.to_owned()]);
        }
        RuntimeKind::Fake => bail!("this runtime does not attest a resumed launch"),
    }
    Ok(expected)
}

fn validate_runtime_launch(
    kind: RuntimeKind,
    initial: Option<&LaunchDescriptor>,
    descriptor: &LaunchDescriptor,
    after_initial: bool,
    expected_session: Option<&str>,
) -> anyhow::Result<()> {
    let initial =
        initial.context("runtime emitted launch evidence without a prepared descriptor")?;
    validate_launch_descriptor(
        descriptor,
        &initial.attempt_id,
        &initial.invocation_id,
        &initial.working_directory,
    )?;
    if !after_initial {
        if descriptor != initial {
            bail!("runtime initial launch differs from the prepared descriptor");
        }
        return Ok(());
    }
    if descriptor.executable != initial.executable
        || descriptor.executable_digest != initial.executable_digest
        || descriptor.coordination_executable != initial.coordination_executable
        || descriptor.coordination_executable_digest != initial.coordination_executable_digest
        || descriptor.launch_chain != initial.launch_chain
        || descriptor.environment != initial.environment
        || descriptor.working_directory != initial.working_directory
    {
        bail!("runtime resume changed the executable, launch chain, environment, or workspace");
    }
    let expected_session = expected_session.context("runtime resume has no established session")?;
    let expected_arguments = expected_resume_arguments(kind, &initial.arguments, expected_session)?;
    if descriptor.arguments != expected_arguments {
        bail!("runtime resume changed its prepared arguments or session identifier");
    }
    Ok(())
}

fn runtime_error_code(error: &RuntimeError) -> &'static str {
    match error {
        RuntimeError::Process(_)
        | RuntimeError::UnsuccessfulExit { .. }
        | RuntimeError::SanitizedUnsuccessfulExit { .. } => "runtime_process_failed",
        RuntimeError::TimedOut { .. } => "runtime_timed_out",
        RuntimeError::OutputLimitExceeded { .. } => "runtime_output_limit_exceeded",
        RuntimeError::MalformedEvent(_) | RuntimeError::NonUtf8Output => "runtime_protocol_failed",
        RuntimeError::InvalidProfile(_) | RuntimeError::Unsupported(_) => "runtime_profile_invalid",
        RuntimeError::RuntimeReportedFailure(_) => "runtime_reported_failure",
        RuntimeError::NotYielded => "runtime_not_yielded",
    }
}

fn runtime_error_kind(error: &RuntimeError) -> RuntimeFailureKind {
    match error {
        RuntimeError::Process(_)
        | RuntimeError::UnsuccessfulExit { .. }
        | RuntimeError::SanitizedUnsuccessfulExit { .. } => RuntimeFailureKind::ProcessExit,
        RuntimeError::OutputLimitExceeded { .. } => RuntimeFailureKind::OutputLimit,
        RuntimeError::RuntimeReportedFailure(_) => RuntimeFailureKind::RuntimeReported,
        _ => RuntimeFailureKind::Protocol,
    }
}

fn runtime_error_diagnostic(error: &RuntimeError) -> Option<DiagnosticSummary> {
    match error {
        RuntimeError::SanitizedUnsuccessfulExit { diagnostic, .. } => Some(diagnostic.clone()),
        _ => Some(DiagnosticSummary::from_bytes(
            error.to_string().as_bytes(),
            false,
        )),
    }
}

fn record_infrastructure_failure(
    application: &Arc<Mutex<Application>>,
    attempt_id: &str,
    detail: &str,
) {
    let Ok(mut application) = application.lock() else {
        return;
    };
    if application.state().status != RunStatus::Running {
        return;
    }
    let reason = bounded_reason(detail);
    let _ = application.execute(
        format!("{attempt_id}.runtime-failed"),
        Command::FailInfrastructure { reason },
    );
}

fn bounded_reason(detail: &str) -> String {
    let mut end = detail.len().min(900);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        "managed runtime failed without diagnostic text".to_owned()
    } else {
        detail[..end].to_owned()
    }
}

/// Resolves the workspace program once and binds it to a location the account this run executes
/// under cannot write.
///
/// A name resolved through the search path is not an identity: the search path here is led by
/// several directories this account owns, so a program planted in one of them would answer to the
/// name and be admitted as if it were the program the run means. The first candidate the search
/// path yields is therefore refused unless it stands where only the superuser could have put it,
/// rather than skipped in favour of a later one, because a program answering to that name from a
/// writable directory is a condition the run must report and not step over.
pub fn admit_workspace_program() -> anyhow::Result<AdmittedProgram> {
    let path = std::env::var_os("PATH").context("PATH is not set, so git cannot be resolved")?;
    admit_workspace_program_from(&path)
}

/// The search path is a parameter so that the binding can be checked against a planted program
/// without changing the environment of the process running the check.
fn admit_workspace_program_from(search_path: &OsStr) -> anyhow::Result<AdmittedProgram> {
    const PROGRAM: &str = "git";
    let resolved = std::env::split_paths(search_path)
        .map(|directory| directory.join(PROGRAM))
        .find(|candidate| candidate.is_file() && is_executable_file(candidate).unwrap_or(false))
        .with_context(|| format!("{PROGRAM} was not found on PATH"))?;
    Ok(AdmittedProgram::admit(
        ProgramRole::Workspace,
        resolved,
        &ProgramRequirement::SystemPath,
    )?)
}

/// Builds the private baseline of a managed workspace with the admitted program, re-establishing
/// its identity and bytes before each execution. This is the only copy: a second one that named the
/// program instead would resolve it through the search path again on every run.
pub fn initialize_private_git(workspace: &Path, git: &AdmittedProgram) -> anyhow::Result<()> {
    for (action, arguments) in [
        ("initialize private Git repository", vec!["init", "--quiet"]),
        ("stage private workspace", vec!["add", "--all"]),
        (
            "commit private baseline",
            vec![
                "-c",
                "user.name=ymp",
                "-c",
                "user.email=ymp@invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "ymp private baseline",
            ],
        ),
    ] {
        git.verify()?;
        let status = ProcessCommand::new(&git.path)
            .args(arguments)
            .current_dir(workspace)
            .status()
            .with_context(|| format!("{action} in {}", workspace.display()))?;
        if !status.success() {
            bail!("{action} failed with {status}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ManagedCandidateRequest, ManagedContract, ManagedRunEvent, admit_workspace_program,
        admit_workspace_program_from, initialize_private_git, start_managed_candidate,
        start_unattested_managed_candidate, validate_launch_descriptor, validate_runtime_launch,
    };
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use ymp_agent_api::{AgentToolCall, AgentToolHandler, SubmitArguments, YieldArguments};
    use ymp_agent_rpc::SocketToolHandler;
    use ymp_application::Application;
    use ymp_domain::{Budget, RunStatus};
    use ymp_runtime_api::{
        AdmittedProgram, InvocationRequest, ProbeReport, ProgramIdentity, ProgramRequirement,
        ProgramRole, Readiness, RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeEventKind,
        RuntimeKind, RuntimeSession, Usage,
    };
    use ymp_runtime_fake::{FakeRuntime, ScriptStep};

    struct SubmittingRuntime {
        inner: FakeRuntime,
    }

    impl RuntimeDriver for SubmittingRuntime {
        fn kind(&self) -> RuntimeKind {
            self.inner.kind()
        }

        fn executable(&self) -> &Path {
            self.inner.executable()
        }

        fn probe(&self) -> Result<ProbeReport, RuntimeError> {
            self.inner.probe()
        }

        fn start(
            &self,
            request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            std::fs::write(request.workspace.join("input.txt"), b"agent candidate\n")?;
            let binding = request.mcp.as_ref().ok_or_else(|| {
                RuntimeError::InvalidProfile("test runtime requires MCP binding".to_owned())
            })?;
            let mut client = SocketToolHandler::for_invocation(
                &binding.socket_path,
                &binding.token,
                &request.attempt_id,
                &request.invocation_id,
            );
            client
                .call(AgentToolCall::Submit(SubmitArguments {
                    command_id: "agent.submit".to_owned(),
                }))
                .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
            self.inner.start(request)
        }
    }

    struct LifecycleRuntime {
        inner: FakeRuntime,
    }

    impl RuntimeDriver for LifecycleRuntime {
        fn kind(&self) -> RuntimeKind {
            self.inner.kind()
        }

        fn executable(&self) -> &Path {
            self.inner.executable()
        }

        fn probe(&self) -> Result<ProbeReport, RuntimeError> {
            self.inner.probe()
        }

        fn start(
            &self,
            request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            let binding = request.mcp.as_ref().ok_or_else(|| {
                RuntimeError::InvalidProfile("test runtime requires MCP binding".to_owned())
            })?;
            let mut controller = SocketToolHandler::for_invocation(
                &binding.socket_path,
                &binding.token,
                &request.attempt_id,
                &request.invocation_id,
            );
            controller
                .call(AgentToolCall::Yield(YieldArguments {
                    command_id: "agent.yield".to_owned(),
                }))
                .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
            Ok(Box::new(LifecycleSession {
                inner: self.inner.start(request)?,
                controller,
            }))
        }
    }

    struct LifecycleSession {
        inner: Box<dyn RuntimeSession>,
        controller: SocketToolHandler,
    }

    impl RuntimeSession for LifecycleSession {
        fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
            self.inner.next_event()
        }

        fn resume(&mut self, input: String) -> Result<(), RuntimeError> {
            self.inner.resume(input)?;
            self.controller
                .call(AgentToolCall::Submit(SubmitArguments {
                    command_id: "agent.submit.after-wake".to_owned(),
                }))
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

    #[derive(Clone, Copy, Debug)]
    enum ProgressFault {
        Gap,
        Reordered,
        RepeatedEventId,
        EmptyEventId,
        WrongInvocation,
    }

    struct FaultingRuntime {
        fault: ProgressFault,
    }

    impl RuntimeDriver for FaultingRuntime {
        fn kind(&self) -> RuntimeKind {
            RuntimeKind::Fake
        }

        fn executable(&self) -> &Path {
            Path::new("ymp-internal-faulting")
        }

        fn probe(&self) -> Result<ProbeReport, RuntimeError> {
            Ok(ProbeReport {
                kind: RuntimeKind::Fake,
                executable: self.executable().display().to_string(),
                version: Some("test".to_owned()),
                readiness: Readiness::Ready,
                detail: "faulting runtime for supervisor regression".to_owned(),
            })
        }

        fn start(
            &self,
            request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            let invocation_id = request.invocation_id;
            let event = |sequence, event_id: String, event| RuntimeEvent {
                sequence,
                event_id,
                invocation_id: invocation_id.clone(),
                event,
            };
            let first_id = format!("{invocation_id}.event-1");
            let mut events = vec![event(
                1,
                first_id.clone(),
                RuntimeEventKind::Started {
                    opaque_session_id: "opaque".to_owned(),
                },
            )];
            match self.fault {
                ProgressFault::Gap => events.push(event(
                    3,
                    format!("{invocation_id}.event-3"),
                    RuntimeEventKind::Output {
                        text: "gap".to_owned(),
                    },
                )),
                ProgressFault::Reordered => {
                    events.push(event(
                        2,
                        format!("{invocation_id}.event-2"),
                        RuntimeEventKind::Output {
                            text: "ordered".to_owned(),
                        },
                    ));
                    events.push(event(
                        1,
                        format!("{invocation_id}.event-reordered"),
                        RuntimeEventKind::Output {
                            text: "reordered".to_owned(),
                        },
                    ));
                }
                ProgressFault::RepeatedEventId => events.push(event(
                    2,
                    first_id,
                    RuntimeEventKind::Output {
                        text: "repeated identifier".to_owned(),
                    },
                )),
                ProgressFault::EmptyEventId => events.push(event(
                    2,
                    String::new(),
                    RuntimeEventKind::Output {
                        text: "empty identifier".to_owned(),
                    },
                )),
                ProgressFault::WrongInvocation => events.push(RuntimeEvent {
                    sequence: 2,
                    event_id: format!("{invocation_id}.event-2"),
                    invocation_id: "substituted-invocation".to_owned(),
                    event: RuntimeEventKind::Output {
                        text: "wrong invocation".to_owned(),
                    },
                }),
            }
            events.push(event(
                4,
                format!("{invocation_id}.event-completed"),
                RuntimeEventKind::Completed {
                    usage: Usage::default(),
                },
            ));
            Ok(Box::new(FaultingSession {
                events: events.into(),
            }))
        }
    }

    struct FaultingSession {
        events: VecDeque<RuntimeEvent>,
    }

    impl RuntimeSession for FaultingSession {
        fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
            Ok(self.events.pop_front())
        }

        fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
            Err(RuntimeError::Unsupported("faulting test runtime resume"))
        }

        fn interrupt(&mut self) -> Result<(), RuntimeError> {
            self.events.clear();
            Ok(())
        }
    }

    fn executable_fixture(path: &Path, contents: &[u8]) {
        std::fs::write(path, contents).expect("write program fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(path)
                .expect("program metadata")
                .permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(path, permissions).expect("program permissions");
        }
    }

    fn stated(role: ProgramRole, path: PathBuf) -> AdmittedProgram {
        let digest = ymp_domain::digest_bytes(&std::fs::read(&path).expect("program bytes"));
        AdmittedProgram::admit(role, path, &ProgramRequirement::StatedDigest(digest))
            .expect("admit a program bound by its stated digest")
    }

    fn chain_descriptor(directory: &Path) -> ymp_runtime_api::LaunchDescriptor {
        let executable = directory.join("runtime");
        executable_fixture(&executable, b"#!/bin/sh\nexit 0\n");
        let shell = directory.join("chain-shell");
        executable_fixture(&shell, b"#!/bin/sh\nexec /bin/sh \"$@\"\n");
        let sanitiser = directory.join("chain-sanitiser");
        executable_fixture(&sanitiser, b"#!/bin/sh\nexec /usr/bin/env \"$@\"\n");
        ymp_runtime_api::LaunchDescriptor {
            schema_version: 1,
            invocation_id: "invocation-chain".to_owned(),
            attempt_id: "attempt-chain".to_owned(),
            executable: executable.clone(),
            executable_digest: ymp_domain::digest_bytes(
                &std::fs::read(&executable).expect("runtime bytes"),
            ),
            coordination_executable: None,
            coordination_executable_digest: None,
            launch_chain: vec![
                stated(ProgramRole::LaunchShell, shell),
                stated(ProgramRole::EnvironmentSanitiser, sanitiser),
            ],
            arguments: vec!["exec".to_owned(), "-".to_owned()],
            environment: Vec::new(),
            working_directory: directory.to_owned(),
        }
    }

    /// The controller admits the chain independently of the driver that declared it, so a program
    /// replaced between preparation and launch is refused even if the driver accepted it.
    #[test]
    fn a_launch_descriptor_whose_chain_program_changed_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let descriptor = chain_descriptor(temporary.path());
        for index in 0..descriptor.launch_chain.len() {
            let descriptor = chain_descriptor(temporary.path());
            validate_launch_descriptor(
                &descriptor,
                &descriptor.attempt_id,
                &descriptor.invocation_id,
                temporary.path(),
            )
            .expect("an unchanged chain is admitted");
            let replaced = &descriptor.launch_chain[index].path;
            let bytes = std::fs::read(replaced).expect("chain program bytes");
            std::fs::write(replaced, [bytes.as_slice(), b"# substituted\n"].concat())
                .expect("substitute chain program");
            let error = validate_launch_descriptor(
                &descriptor,
                &descriptor.attempt_id,
                &descriptor.invocation_id,
                temporary.path(),
            )
            .expect_err("a replaced chain program is refused")
            .to_string();
            assert!(error.contains("changed after admission"), "{error}");
        }
    }

    /// A resumed launch may change its session operand and nothing else. Without this the chain the
    /// run was admitted with could be exchanged at the first resume.
    #[test]
    fn a_resume_that_changes_the_launch_chain_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let initial = chain_descriptor(temporary.path());
        let mut resumed = initial.clone();
        resumed.arguments = vec![
            "exec".to_owned(),
            "resume".to_owned(),
            "session-1".to_owned(),
            "-".to_owned(),
        ];
        validate_runtime_launch(
            RuntimeKind::Codex,
            Some(&initial),
            &resumed,
            true,
            Some("session-1"),
        )
        .expect("an unchanged chain resumes");
        let mut exchanged = resumed.clone();
        exchanged.launch_chain.pop();
        let error = validate_runtime_launch(
            RuntimeKind::Codex,
            Some(&initial),
            &exchanged,
            true,
            Some("session-1"),
        )
        .expect_err("a resumed launch may not change its chain")
        .to_string();
        assert!(error.contains("launch chain"), "{error}");
    }

    /// The search path is led by directories this account owns, so a name resolved through it is
    /// not an identity. A program planted there answers to the name and must be refused rather than
    /// admitted, and rather than stepped over in favour of a later candidate.
    #[test]
    fn a_program_planted_earlier_in_the_search_path_is_refused() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let planted_directory = temporary.path().join("planted");
        std::fs::create_dir(&planted_directory).expect("planted directory");
        let planted = planted_directory.join("git");
        executable_fixture(&planted, b"#!/bin/sh\nexec /usr/bin/git \"$@\"\n");

        let system = admit_workspace_program().expect("the system program is admitted");
        assert_eq!(system.identity, ProgramIdentity::SystemPath);
        assert!(system.path.is_absolute());

        let search = std::env::join_paths([planted_directory.as_path(), Path::new("/usr/bin")])
            .expect("search path");
        let error = admit_workspace_program_from(&search)
            .expect_err("a planted program is refused")
            .to_string();
        assert!(error.contains("this account can write"), "{error}");
        assert!(
            error.contains(planted.to_str().expect("planted path")),
            "{error}"
        );
    }

    /// Declares a launch descriptor without admitting the programs its launch chain enters. The
    /// product drivers admit them; this exists so that a driver which does not is refused rather
    /// than reaching the same launch through an unadmitted shell.
    struct ChainlessRuntime {
        executable: PathBuf,
        chain: Vec<AdmittedProgram>,
    }

    impl RuntimeDriver for ChainlessRuntime {
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
                version: Some("codex-cli 0.147.0".to_owned()),
                readiness: Readiness::Ready,
                detail: "chainless runtime for supervisor regression".to_owned(),
            })
        }

        fn start(
            &self,
            _request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            Err(RuntimeError::Unsupported("chainless runtime start"))
        }

        fn prepare_launch(
            &self,
            request: &InvocationRequest,
        ) -> Result<Option<ymp_runtime_api::LaunchDescriptor>, RuntimeError> {
            Ok(Some(ymp_runtime_api::LaunchDescriptor {
                schema_version: 1,
                invocation_id: request.invocation_id.clone(),
                attempt_id: request.attempt_id.clone(),
                executable: self.executable.clone(),
                executable_digest: ymp_domain::digest_bytes(&std::fs::read(&self.executable)?),
                coordination_executable: None,
                coordination_executable_digest: None,
                launch_chain: self.chain.clone(),
                arguments: vec!["-".to_owned()],
                environment: Vec::new(),
                working_directory: request.workspace.clone(),
            }))
        }

        fn start_prepared(
            &self,
            _request: InvocationRequest,
            _descriptor: Option<&ymp_runtime_api::LaunchDescriptor>,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            panic!("a launch without an admitted chain must never be started");
        }
    }

    /// Two ways a driver could reach the same launch through a program this account could have
    /// planted: admit no chain at all, or bind the chain to a digest the driver chose itself rather
    /// than to a location the account cannot write. Both are refused before the runtime starts.
    #[test]
    fn a_launch_chain_the_account_could_have_chosen_is_refused_before_the_runtime_starts() {
        for (label, chain) in [
            ("no admitted chain", Vec::new()),
            ("a chain the driver bound to its own digest", Vec::new()),
        ] {
            let temporary = tempfile::tempdir().expect("temporary directory");
            let source = temporary.path().join("source");
            std::fs::create_dir(&source).expect("source directory");
            std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
            let executable = temporary.path().join("chainless-runtime");
            executable_fixture(&executable, b"#!/bin/sh\nexit 0\n");
            let chain = if chain.is_empty() && label.starts_with("a chain") {
                let shell = temporary.path().join("chain-shell");
                executable_fixture(&shell, b"#!/bin/sh\nexec /bin/sh \"$@\"\n");
                vec![stated(ProgramRole::LaunchShell, shell)]
            } else {
                chain
            };
            let application = Arc::new(Mutex::new(
                Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                    .expect("create application"),
            ));
            let started = start_managed_candidate(
                Arc::clone(&application),
                Box::new(ChainlessRuntime { executable, chain }),
                ManagedCandidateRequest {
                    contract: ManagedContract {
                        contract_id: "contract-chainless".to_owned(),
                        contract_digest: "c".repeat(64),
                        source,
                        prompt: "finish".to_owned(),
                        capture_exclusions: Vec::new(),
                        verifier: None,
                    },
                    bridge_executable: std::env::current_exe().expect("current executable"),
                },
            );
            let Err(error) = started else {
                panic!("{label} must be refused");
            };
            let error = error.to_string();
            assert!(
                error.contains("did not admit the programs"),
                "{label}: {error}"
            );
            assert_eq!(
                application.lock().expect("application lock").state().status,
                RunStatus::InfrastructureError,
                "{label}"
            );
        }
    }

    /// The workspace program is resolved through `PATH` once and executed by the absolute path its
    /// digest was taken from, so a replacement between admission and use is refused.
    #[test]
    fn a_replaced_workspace_program_refuses_to_build_the_baseline() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let admitted = admit_workspace_program().expect("admit the workspace program");
        assert!(admitted.path.is_absolute());
        assert!(admitted.verify().is_ok());

        let workspace = temporary.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let stand_in = temporary.path().join("git-stand-in");
        executable_fixture(&stand_in, b"#!/bin/sh\nexit 0\n");
        let stand_in = stated(ProgramRole::Workspace, stand_in);
        initialize_private_git(&workspace, &stand_in).expect("an unchanged program is executed");
        executable_fixture(&stand_in.path, b"#!/bin/sh\nexit 1\n");
        let error = initialize_private_git(&workspace, &stand_in)
            .expect_err("a replaced workspace program is refused")
            .to_string();
        assert!(error.contains("changed after admission"), "{error}");
    }

    #[test]
    fn contract_loads_relative_source_and_rejects_parent_exclusion() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        let path = temporary.path().join("contract.json");
        std::fs::write(
            &path,
            br#"{"schema_version":1,"contract_id":"contract-1","source":"source","prompt":"make the requested change","capture_exclusions":["target"]}"#,
        )
        .expect("write contract");
        let contract = ManagedContract::load(&path).expect("load contract");
        assert_eq!(
            contract.source,
            source.canonicalize().expect("canonical source")
        );
        assert_eq!(contract.capture_exclusions, ["target"]);

        let program = temporary.path().join("verifier");
        std::fs::write(&program, b"fixture").expect("write verifier fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&program)
                .expect("verifier metadata")
                .permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&program, permissions).expect("verifier permissions");
        }
        let negative = temporary.path().join("negative");
        std::fs::create_dir(&negative).expect("negative control directory");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "contract_id": "contract-1",
                "source": "source",
                "prompt": "make the requested change",
                "capture_exclusions": ["target"],
                "verifier": {
                    "program": "verifier",
                    "arguments": ["check"],
                    "negative_control": "negative",
                    "oracle_digest": "a".repeat(64)
                }
            }))
            .expect("serialize verifier contract"),
        )
        .expect("write verifier contract");
        let contract = ManagedContract::load(&path).expect("load verifier contract");
        let verifier = contract.verifier.expect("verifier configuration");
        assert_eq!(verifier.program, program.canonicalize().expect("program"));
        assert_eq!(
            verifier.negative_control,
            negative.canonicalize().expect("negative")
        );
        assert_eq!(verifier.wall_time_ms, 60_000);

        std::fs::write(
            &path,
            br#"{"schema_version":1,"contract_id":"contract-1","source":"source","prompt":"make the requested change","capture_exclusions":["../escape"]}"#,
        )
        .expect("write invalid contract");
        assert!(ManagedContract::load(&path).is_err());
    }

    #[test]
    fn completed_runtime_without_controller_action_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let application = Arc::new(Mutex::new(
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let contract = ManagedContract {
            contract_id: "contract-1".to_owned(),
            contract_digest: "c".repeat(64),
            source,
            prompt: "finish".to_owned(),
            capture_exclusions: vec!["target".to_owned()],
            verifier: None,
        };
        let runtime = FakeRuntime::with_script(vec![ScriptStep::Complete(Usage::default())]);
        let handle = start_unattested_managed_candidate(
            Arc::clone(&application),
            Box::new(runtime),
            ManagedCandidateRequest {
                contract,
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_failed = false;
        let mut saw_candidate = false;
        while Instant::now() < deadline && !handle.is_finished() {
            while let Some(event) = handle.try_next() {
                match event {
                    ManagedRunEvent::Runtime(event)
                        if matches!(event.event, RuntimeEventKind::Failed { .. }) =>
                    {
                        saw_failed = true;
                    }
                    ManagedRunEvent::CandidateAvailable { .. } => saw_candidate = true,
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        while let Some(event) = handle.try_next() {
            if matches!(event, ManagedRunEvent::CandidateAvailable { .. }) {
                saw_candidate = true;
            }
        }
        assert!(handle.is_finished());
        assert!(saw_failed);
        assert!(!saw_candidate);
        assert_eq!(
            application.lock().expect("application lock").state().status,
            RunStatus::InfrastructureError
        );
        assert!(
            application
                .lock()
                .expect("application lock")
                .state()
                .candidate_digest
                .is_none()
        );
        let transcript = temporary
            .path()
            .join("data/runtime-evidence")
            .join(handle.attempt_id())
            .join("events.jsonl");
        let transcript = std::fs::read_to_string(&transcript).expect("runtime transcript");
        assert!(!transcript.contains(".opaque"));
        let records: Vec<serde_json::Value> = transcript
            .lines()
            .map(|line| serde_json::from_str(line).expect("runtime evidence record"))
            .collect();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["event"]["type"], "started");
        assert_eq!(records[1]["event"]["type"], "failed");
        assert_eq!(records[1]["predecessor_digest"], records[0]["digest"]);
        assert!(
            application
                .lock()
                .expect("application lock")
                .export_evidence(temporary.path().join("export"))
                .is_err()
        );
        handle.join().expect("join worker");
    }

    fn assert_invalid_progress_is_terminal(
        runtime: Box<dyn RuntimeDriver>,
        _expected_detail: &str,
        recorded_events: usize,
    ) {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let data_root = temporary.path().join("data");
        let application = Arc::new(Mutex::new(
            Application::create(&data_root, "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let contract = ManagedContract {
            contract_id: "contract-1".to_owned(),
            contract_digest: "c".repeat(64),
            source,
            prompt: "finish".to_owned(),
            capture_exclusions: vec!["target".to_owned()],
            verifier: None,
        };
        let handle = start_unattested_managed_candidate(
            Arc::clone(&application),
            runtime,
            ManagedCandidateRequest {
                contract,
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !handle.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut failure = None;
        let mut saw_candidate = false;
        while let Some(event) = handle.try_next() {
            match event {
                ManagedRunEvent::CandidateAvailable { .. } => saw_candidate = true,
                ManagedRunEvent::Failed { detail } => failure = Some(detail),
                _ => {}
            }
        }
        assert!(handle.is_finished());
        assert!(!saw_candidate);
        let failure = failure.expect("typed managed-run failure");
        assert_eq!(failure, "managed_runtime_supervision_failed");
        let state = application.lock().expect("application lock");
        assert_eq!(state.state().status, RunStatus::InfrastructureError);
        assert!(state.state().candidate_digest.is_none());
        assert!(state.state().active_attempts.is_empty());
        drop(state);
        let transcript = temporary
            .path()
            .join("data/runtime-evidence")
            .join(handle.attempt_id())
            .join("events.jsonl");
        assert_eq!(
            std::fs::read_to_string(transcript)
                .expect("runtime transcript")
                .lines()
                .count(),
            recorded_events
        );
        handle.join().expect("join worker");
        drop(application);
        let recovered = Application::open(data_root).expect("reopen terminal application");
        assert_eq!(recovered.state().status, RunStatus::InfrastructureError);
        assert!(recovered.state().candidate_digest.is_none());
        assert!(recovered.state().active_attempts.is_empty());
    }

    #[test]
    fn duplicate_runtime_progress_is_terminal_without_candidate() {
        assert_invalid_progress_is_terminal(
            Box::new(FakeRuntime::with_script(vec![
                ScriptStep::Output("duplicate me".to_owned()),
                ScriptStep::DuplicateLast,
                ScriptStep::Complete(Usage::default()),
            ])),
            "runtime repeated event_id",
            2,
        );
    }

    #[test]
    fn other_invalid_runtime_progress_is_terminal_without_candidate() {
        for (fault, expected_detail, recorded_events) in [
            (ProgressFault::Gap, "does not match expected sequence", 1),
            (
                ProgressFault::Reordered,
                "does not match expected sequence",
                2,
            ),
            (
                ProgressFault::RepeatedEventId,
                "runtime repeated event_id",
                1,
            ),
            (
                ProgressFault::EmptyEventId,
                "runtime event_id must contain",
                1,
            ),
            (
                ProgressFault::WrongInvocation,
                "runtime event invocation_id does not match",
                1,
            ),
        ] {
            assert_invalid_progress_is_terminal(
                Box::new(FaultingRuntime { fault }),
                expected_detail,
                recorded_events,
            );
        }
    }

    #[test]
    fn agent_submission_captures_only_the_bound_workspace() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let application = Arc::new(Mutex::new(
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let contract = ManagedContract {
            contract_id: "contract-1".to_owned(),
            contract_digest: "c".repeat(64),
            source,
            prompt: "finish and submit".to_owned(),
            capture_exclusions: vec!["target".to_owned()],
            verifier: None,
        };
        let runtime = SubmittingRuntime {
            inner: FakeRuntime::with_script(vec![ScriptStep::Complete(Usage::default())]),
        };
        let handle = start_unattested_managed_candidate(
            Arc::clone(&application),
            Box::new(runtime),
            ManagedCandidateRequest {
                contract,
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !handle.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut agent_candidate = None;
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::CandidateAvailable {
                candidate_digest,
                change_count,
            } = event
            {
                assert_eq!(change_count, None);
                agent_candidate = Some(candidate_digest);
            }
        }
        let candidate_digest = agent_candidate.expect("agent-submitted candidate event");
        assert_eq!(
            application
                .lock()
                .expect("application lock")
                .state()
                .candidate_digest
                .as_deref(),
            Some(candidate_digest.as_str())
        );
        let materialized = temporary.path().join("materialized");
        application
            .lock()
            .expect("application lock")
            .artifact_store()
            .materialize(&candidate_digest, &materialized)
            .expect("materialize submitted candidate");
        assert_eq!(
            std::fs::read(materialized.join("input.txt")).expect("candidate file"),
            b"agent candidate\n"
        );
        handle.join().expect("join worker");
    }

    #[test]
    fn managed_handle_wakes_one_session_once_per_command_identity() {
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
            Box::new(LifecycleRuntime {
                inner: FakeRuntime::default(),
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
        let attempt_id = handle.attempt_id().to_owned();
        let invocation_id = handle.invocation_id().to_owned();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut session_id = None;
        let mut saw_yield = false;
        let mut resumed_outputs = 0;
        let mut completed = 0;
        while Instant::now() < deadline && !saw_yield {
            while let Some(event) = handle.try_next() {
                if let ManagedRunEvent::Runtime(event) = event {
                    assert_eq!(event.invocation_id, invocation_id);
                    match event.event {
                        RuntimeEventKind::Started { opaque_session_id } => {
                            session_id = Some(opaque_session_id)
                        }
                        RuntimeEventKind::Yielded { .. } => saw_yield = true,
                        _ => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(saw_yield, "managed runtime did not yield");
        handle
            .wake("wake-command-1", "resume once")
            .expect("first wake");
        handle
            .wake("wake-command-1", "resume once")
            .expect("idempotent repeated wake");
        assert!(handle.wake("wake-command-1", "different input").is_err());

        while Instant::now() < deadline && !handle.is_finished() {
            while let Some(event) = handle.try_next() {
                if let ManagedRunEvent::Runtime(event) = event {
                    assert_eq!(event.invocation_id, invocation_id);
                    match event.event {
                        RuntimeEventKind::Output { text } if text == "fake runtime resumed" => {
                            resumed_outputs += 1;
                        }
                        RuntimeEventKind::Completed { .. } => completed += 1,
                        _ => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Runtime(event) = event {
                assert_eq!(event.invocation_id, invocation_id);
                match event.event {
                    RuntimeEventKind::Output { text } if text == "fake runtime resumed" => {
                        resumed_outputs += 1;
                    }
                    RuntimeEventKind::Completed { .. } => completed += 1,
                    _ => {}
                }
            }
        }
        assert!(handle.is_finished());
        assert_eq!(resumed_outputs, 1);
        assert_eq!(completed, 1);
        assert_eq!(session_id, Some(format!("{attempt_id}.session")));
        assert_eq!(attempt_id, handle.attempt_id());
        let evidence = std::fs::read_to_string(
            temporary
                .path()
                .join("data/runtime-evidence")
                .join(&attempt_id)
                .join("events.jsonl"),
        )
        .expect("runtime evidence");
        for record in evidence.lines() {
            let record: serde_json::Value =
                serde_json::from_str(record).expect("runtime evidence record");
            assert_eq!(record["invocation_id"], invocation_id);
        }
        handle.join().expect("join worker");
    }
}
