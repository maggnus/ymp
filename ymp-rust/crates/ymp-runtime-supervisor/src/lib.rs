#![forbid(unsafe_code)]

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use uuid::Uuid;
use ymp_application::Application;
use ymp_application::WorkspaceSubmission;
use ymp_domain::{Command, MAX_IDENTIFIER_CHARS, RunStatus, digest_bytes};
use ymp_runtime_api::{
    CancellationToken, InvocationRequest, McpBinding, Readiness, RuntimeDriver, RuntimeEvent,
    RuntimeEventKind, RuntimeKind, Usage,
};

const CONTRACT_SCHEMA_VERSION: u32 = 1;
const MAX_CONTRACT_BYTES: usize = 1024 * 1024;
const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_RUNTIME_EVIDENCE_BYTES: u64 = 4 * 1024 * 1024;

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
    Interrupted,
}

#[derive(Serialize)]
struct RuntimeEvidenceDigestInput<'a> {
    schema_version: u32,
    run_id: &'a str,
    attempt_id: &'a str,
    runtime_kind: RuntimeKind,
    contract_id: &'a str,
    contract_digest: &'a str,
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
    runtime_kind: RuntimeKind,
    contract_id: &'a str,
    contract_digest: &'a str,
    sequence: u64,
    event_id: &'a str,
    predecessor_digest: &'a Option<String>,
    event: &'a RuntimeEvidenceKind,
    digest: &'a str,
}

pub struct ManagedRunHandle {
    attempt_id: String,
    runtime_kind: RuntimeKind,
    cancellation: CancellationToken,
    application: Arc<Mutex<Application>>,
    receiver: Receiver<ManagedRunEvent>,
    finished: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ManagedRunHandle {
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
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

pub fn start_managed_candidate(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    request: ManagedCandidateRequest,
) -> anyhow::Result<ManagedRunHandle> {
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
    let contract_id = request.contract.contract_id.clone();
    let contract_digest = request.contract.contract_digest.clone();
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let invocation_id = format!("invocation-{}", Uuid::new_v4());
    let (data_root, run_id, base_digest, workspace) = {
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
        let exclusions: Vec<_> = request
            .contract
            .capture_exclusions
            .iter()
            .map(String::as_str)
            .collect();
        let base = application
            .artifact_store()
            .capture_source_excluding(&request.contract.source, &exclusions)?;
        let workspace = data_root.join("workspaces").join(&attempt_id);
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)?;
        initialize_private_git(&workspace)?;
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
        )
    };

    let token = Uuid::new_v4().simple().to_string();
    let socket_path = std::env::temp_dir()
        .join("ymp-runtime")
        .join(format!("{}.sock", Uuid::new_v4().simple()));
    let exclusions = request.contract.capture_exclusions.clone();
    let rpc_server = match ymp_agent_rpc::AgentRpcServer::start_with_submission(
        &socket_path,
        &token,
        &attempt_id,
        Arc::clone(&application),
        WorkspaceSubmission::new(&base_digest, &workspace, exclusions.clone()),
    ) {
        Ok(server) => server,
        Err(error) => {
            record_infrastructure_failure(&application, &attempt_id, &error.to_string());
            return Err(error.into());
        }
    };
    let cancellation = CancellationToken::default();
    let mut session = match driver.start(InvocationRequest {
        invocation_id,
        attempt_id: attempt_id.clone(),
        workspace: workspace.clone(),
        mcp: Some(McpBinding {
            executable: bridge_executable,
            socket_path,
            token,
        }),
        prompt: request.contract.prompt,
        cancellation: cancellation.clone(),
    }) {
        Ok(session) => session,
        Err(error) => {
            record_infrastructure_failure(&application, &attempt_id, &error.to_string());
            return Err(error.into());
        }
    };
    let runtime_kind = driver.kind();
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
    let (sender, receiver) = channel();
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = Arc::clone(&finished);
    let worker_application = Arc::clone(&application);
    let worker_attempt = attempt_id.clone();
    let worker_cancellation = cancellation.clone();
    let worker = thread::Builder::new()
        .name(format!("ymp-runtime-{worker_attempt}"))
        .spawn(move || {
            let _rpc_server = rpc_server;
            let result = (|| -> anyhow::Result<()> {
                let mut completed = false;
                let mut predecessor_digest = None;
                while let Some(event) = session.next_event()? {
                    predecessor_digest = Some(append_runtime_evidence(
                        &mut evidence_file,
                        &run_id,
                        &worker_attempt,
                        runtime_kind,
                        &contract_id,
                        &contract_digest,
                        &predecessor_digest,
                        &event,
                    )?);
                    if matches!(event.event, RuntimeEventKind::Completed { .. }) {
                        completed = true;
                    }
                    let interrupted = matches!(event.event, RuntimeEventKind::Interrupted);
                    let _ = sender.send(ManagedRunEvent::Runtime(event));
                    if interrupted {
                        return Ok(());
                    }
                }
                if worker_cancellation.is_cancelled() {
                    return Ok(());
                }
                if !completed {
                    bail!("runtime ended without a completed event");
                }
                let (candidate_digest, change_count) = {
                    let mut application = worker_application
                        .lock()
                        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
                    if application.state().status != RunStatus::Running {
                        return Ok(());
                    }
                    if let Some(candidate_digest) = application.state().candidate_digest.clone() {
                        (candidate_digest, None)
                    } else {
                        let exclusion_refs: Vec<_> =
                            exclusions.iter().map(String::as_str).collect();
                        let outcome = application.submit_workspace_candidate_excluding(
                            format!("{worker_attempt}.submit-workspace"),
                            &worker_attempt,
                            &base_digest,
                            &workspace,
                            &exclusion_refs,
                        )?;
                        (
                            outcome.candidate.snapshot_digest,
                            Some(outcome.submission.change_count),
                        )
                    }
                };
                let _ = sender.send(ManagedRunEvent::CandidateAvailable {
                    candidate_digest,
                    change_count,
                });
                Ok(())
            })();
            if let Err(error) = result {
                let detail = error.to_string();
                record_infrastructure_failure(&worker_application, &worker_attempt, &detail);
                let _ = sender.send(ManagedRunEvent::Failed { detail });
            }
            let _ = sender.send(ManagedRunEvent::Finished);
            worker_finished.store(true, Ordering::Release);
        })?;

    Ok(ManagedRunHandle {
        attempt_id,
        runtime_kind,
        cancellation,
        application,
        receiver,
        finished,
        worker: Some(worker),
    })
}

#[allow(clippy::too_many_arguments)]
fn append_runtime_evidence(
    file: &mut File,
    run_id: &str,
    attempt_id: &str,
    runtime_kind: RuntimeKind,
    contract_id: &str,
    contract_digest: &str,
    predecessor_digest: &Option<String>,
    runtime_event: &RuntimeEvent,
) -> anyhow::Result<String> {
    let event = runtime_evidence_kind(&runtime_event.event)?;
    let input = RuntimeEvidenceDigestInput {
        schema_version: 1,
        run_id,
        attempt_id,
        runtime_kind,
        contract_id,
        contract_digest,
        sequence: runtime_event.sequence,
        event_id: &runtime_event.event_id,
        predecessor_digest,
        event: &event,
    };
    let digest = digest_bytes(&serde_json::to_vec(&input)?);
    let envelope = RuntimeEvidenceEnvelope {
        schema_version: 1,
        run_id,
        attempt_id,
        runtime_kind,
        contract_id,
        contract_digest,
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

fn runtime_evidence_kind(event: &RuntimeEventKind) -> anyhow::Result<RuntimeEvidenceKind> {
    Ok(match event {
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
        RuntimeEventKind::Interrupted => RuntimeEvidenceKind::Interrupted,
    })
}

fn digest_json(value: &Value) -> anyhow::Result<String> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
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

fn initialize_private_git(workspace: &Path) -> anyhow::Result<()> {
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
        let status = ProcessCommand::new("git")
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
        ManagedCandidateRequest, ManagedContract, ManagedRunEvent, start_managed_candidate,
    };
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use ymp_agent_api::{AgentToolCall, AgentToolHandler, SubmitArguments};
    use ymp_agent_rpc::SocketToolHandler;
    use ymp_application::Application;
    use ymp_domain::{Budget, RunStatus};
    use ymp_runtime_api::{
        InvocationRequest, ProbeReport, RuntimeDriver, RuntimeError, RuntimeEventKind, RuntimeKind,
        RuntimeSession, Usage,
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
            let mut client =
                SocketToolHandler::new(&binding.socket_path, &binding.token, &request.attempt_id);
            client
                .call(AgentToolCall::Submit(SubmitArguments {
                    command_id: "agent.submit".to_owned(),
                }))
                .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
            self.inner.start(request)
        }
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
    fn completed_runtime_produces_controller_captured_candidate() {
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
        let handle = start_managed_candidate(
            Arc::clone(&application),
            Box::new(runtime),
            ManagedCandidateRequest {
                contract,
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_completed = false;
        let mut saw_candidate = false;
        while Instant::now() < deadline && !handle.is_finished() {
            while let Some(event) = handle.try_next() {
                match event {
                    ManagedRunEvent::Runtime(event)
                        if matches!(event.event, RuntimeEventKind::Completed { .. }) =>
                    {
                        saw_completed = true;
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
        assert!(saw_completed);
        assert!(saw_candidate);
        assert_eq!(
            application.lock().expect("application lock").state().status,
            RunStatus::Running
        );
        assert!(
            application
                .lock()
                .expect("application lock")
                .state()
                .candidate_digest
                .is_some()
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
        assert_eq!(records[1]["event"]["type"], "completed");
        assert_eq!(records[1]["predecessor_digest"], records[0]["digest"]);
        let export = temporary.path().join("export");
        application
            .lock()
            .expect("application lock")
            .export_evidence(&export)
            .expect("export candidate and runtime evidence");
        assert!(
            export
                .join("runtime-evidence")
                .join(handle.attempt_id())
                .join("events.jsonl")
                .is_file()
        );
        handle.join().expect("join worker");
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
        let handle = start_managed_candidate(
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
}
