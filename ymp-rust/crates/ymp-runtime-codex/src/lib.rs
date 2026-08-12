#![forbid(unsafe_code)]

use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use ymp_runtime_api::{
    BoundedOutputLine, CancellationToken, InvocationRequest, McpBinding, ProbeReport, Readiness,
    RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeEventKind, RuntimeKind, RuntimeSession,
    Usage, configure_process_group, read_bounded_lines, terminate_process_tree,
};

pub const PINNED_CODEX_VERSION: &str = "codex-cli 0.147.0";
pub const PINNED_CODEX_MODEL: &str = "gpt-5.6-sol";
pub const PINNED_CODEX_PROMPT_POLICY: &str = "ymp-codex-low-v1";
pub const PINNED_CODEX_API_ORIGIN: &str = "https://api.openai.com/v1";
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_WALL_TIME_LIMIT_MS: u64 = 10 * 60 * 1000;
const HARNESS_INSTRUCTIONS: &str = "Execution policy: work without delegation or subagents. Do not send progress reports. Batch independent file reads and batch the final formatting, tests, lint, and diff checks. Use only the files and tools needed for the requested outcome. Do not commit. Stop immediately after a concise final report.";
const DISABLED_AMBIENT_FEATURES: [&str; 35] = [
    "apps",
    "auth_elicitation",
    "browser_use",
    "browser_use_external",
    "browser_use_full_cdp_access",
    "code_mode_host",
    "computer_use",
    "deferred_executor",
    "enable_fanout",
    "fast_mode",
    "goals",
    "guardian_approval",
    "hooks",
    "image_generation",
    "in_app_browser",
    "in_app_updates",
    "multi_agent",
    "multi_agent_v2",
    "network_proxy",
    "personality",
    "plugin_sharing",
    "plugins",
    "recommended_plugins",
    "remote_compaction_v2",
    "remote_control",
    "remote_models",
    "remote_plugin",
    "skill_mcp_dependency_install",
    "skill_search",
    "shell_snapshot",
    "standalone_web_search",
    "tool_call_mcp_elicitation",
    "tool_suggest",
    "view_image",
    "web_search_request",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexSandbox {
    ReadOnly,
    WorkspaceWrite,
}

impl CodexSandbox {
    fn as_arg(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexProfile {
    pub expected_version: String,
    pub model: String,
    pub reasoning_effort: String,
    pub approval_policy: String,
    pub prompt_policy: String,
    pub sandbox: CodexSandbox,
    pub output_limit_bytes: usize,
    pub wall_time_limit_ms: u64,
}

impl Default for CodexProfile {
    fn default() -> Self {
        Self {
            expected_version: PINNED_CODEX_VERSION.to_owned(),
            model: PINNED_CODEX_MODEL.to_owned(),
            reasoning_effort: "low".to_owned(),
            approval_policy: "never".to_owned(),
            prompt_policy: PINNED_CODEX_PROMPT_POLICY.to_owned(),
            sandbox: CodexSandbox::WorkspaceWrite,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            wall_time_limit_ms: DEFAULT_WALL_TIME_LIMIT_MS,
        }
    }
}

impl CodexProfile {
    fn validate(&self) -> Result<(), RuntimeError> {
        if self.expected_version.trim().is_empty() {
            return Err(RuntimeError::InvalidProfile(
                "expected version must not be empty".to_owned(),
            ));
        }
        if self.model != PINNED_CODEX_MODEL {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex model {}",
                self.model
            )));
        }
        if self.reasoning_effort != "low" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex reasoning effort {}",
                self.reasoning_effort
            )));
        }
        if self.approval_policy != "never" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex approval policy {}",
                self.approval_policy
            )));
        }
        if self.prompt_policy != PINNED_CODEX_PROMPT_POLICY {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex prompt policy {}",
                self.prompt_policy
            )));
        }
        if self.output_limit_bytes == 0 {
            return Err(RuntimeError::InvalidProfile(
                "output limit must be positive".to_owned(),
            ));
        }
        if self.wall_time_limit_ms == 0 {
            return Err(RuntimeError::InvalidProfile(
                "wall-time limit must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CodexRuntime {
    executable: PathBuf,
    profile: CodexProfile,
    auth_source: Option<PathBuf>,
    runtime_path: OsString,
}

impl Default for CodexRuntime {
    fn default() -> Self {
        Self::new("codex")
    }
}

impl CodexRuntime {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        let executable = resolve_executable(executable.into());
        Self {
            executable,
            profile: CodexProfile::default(),
            auth_source: discover_auth_source(),
            runtime_path: std::env::var_os("PATH")
                .unwrap_or_else(|| OsString::from("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin")),
        }
    }

    pub fn with_profile(executable: impl Into<PathBuf>, profile: CodexProfile) -> Self {
        let mut runtime = Self::new(executable);
        runtime.profile = profile;
        runtime
    }

    pub fn profile(&self) -> &CodexProfile {
        &self.profile
    }

    fn isolated_environment(&self) -> Result<CodexEnvironment, RuntimeError> {
        CodexEnvironment::create(
            self.auth_source.as_deref(),
            self.runtime_path.clone(),
            &self.profile,
        )
    }
}

#[derive(Debug)]
struct CodexLaunch {
    executable: PathBuf,
    profile: CodexProfile,
    workspace: PathBuf,
    attempt_id: String,
    mcp: Option<McpBinding>,
    environment: CodexEnvironment,
}

#[derive(Debug)]
struct CodexEnvironment {
    root: tempfile::TempDir,
    codex_home: PathBuf,
    temporary: PathBuf,
    runtime_path: OsString,
}

impl CodexEnvironment {
    fn create(
        auth_source: Option<&Path>,
        runtime_path: OsString,
        profile: &CodexProfile,
    ) -> Result<Self, RuntimeError> {
        profile.validate()?;
        let root = tempfile::Builder::new()
            .prefix("ymp-codex-home-")
            .tempdir()?;
        let codex_home = root.path().join(".codex");
        let temporary = root.path().join("tmp");
        fs::create_dir(&codex_home)?;
        fs::create_dir(&temporary)?;
        set_private_directory_permissions(root.path())?;
        set_private_directory_permissions(&codex_home)?;
        set_private_directory_permissions(&temporary)?;
        if let Some(auth_source) = auth_source {
            let auth = fs::read(auth_source)?;
            if auth.len() > 1024 * 1024 {
                return Err(RuntimeError::InvalidProfile(
                    "Codex authentication material exceeds its 1 MiB limit".to_owned(),
                ));
            }
            let destination = codex_home.join("auth.json");
            fs::write(&destination, auth)?;
            set_private_file_permissions(&destination)?;
        }
        Ok(Self {
            root,
            codex_home,
            temporary,
            runtime_path,
        })
    }

    fn apply(&self, command: &mut Command, mcp: Option<&McpBinding>, attempt_id: Option<&str>) {
        command
            .env_clear()
            .env("HOME", self.root.path())
            .env("CODEX_HOME", &self.codex_home)
            .env("TMPDIR", &self.temporary)
            .env("PATH", &self.runtime_path)
            .env("NO_COLOR", "1")
            .env("OPENAI_BASE_URL", PINNED_CODEX_API_ORIGIN);
        if let (Some(mcp), Some(attempt_id)) = (mcp, attempt_id) {
            command
                .env("YMP_AGENT_SOCKET", &mcp.socket_path)
                .env("YMP_AGENT_TOKEN", &mcp.token)
                .env("YMP_ATTEMPT_ID", attempt_id);
        }
    }
}

struct CodexProcess {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: JoinHandle<Vec<u8>>,
    started_at: Instant,
}

impl CodexLaunch {
    fn spawn(&self, session_id: Option<&str>, prompt: &str) -> Result<CodexProcess, RuntimeError> {
        let mut command = Command::new(&self.executable);
        self.environment
            .apply(&mut command, self.mcp.as_ref(), Some(&self.attempt_id));
        command
            .arg("exec")
            .arg("--json")
            .arg("--ignore-user-config")
            .arg("--ignore-rules")
            .args(["--sandbox", self.profile.sandbox.as_arg()])
            .args(["--model", &self.profile.model])
            .arg("-c")
            .arg(format!(
                "model_reasoning_effort=\"{}\"",
                self.profile.reasoning_effort
            ))
            .arg("-c")
            .arg(format!(
                "approval_policy=\"{}\"",
                self.profile.approval_policy
            ))
            .arg("-c")
            .arg("shell_environment_policy.inherit=\"none\"")
            .arg("-C")
            .arg(&self.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for feature in DISABLED_AMBIENT_FEATURES {
            command.arg("--disable").arg(feature);
        }
        if let Some(mcp) = &self.mcp {
            add_mcp_config(&mut command, mcp)?;
        }
        if let Some(session_id) = session_id {
            command.arg("resume").arg(session_id);
        }
        command.arg("-");
        configure_process_group(&mut command);
        let mut child = command.spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stdin was not piped".to_owned()))?;
        stdin.write_all(prompt.as_bytes())?;
        stdin.write_all(b"\n\n")?;
        stdin.write_all(HARNESS_INSTRUCTIONS.as_bytes())?;
        stdin.flush()?;
        drop(stdin);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stdout was not piped".to_owned()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stderr was not piped".to_owned()))?;
        let stderr_limit = self.profile.output_limit_bytes;
        let stderr_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr
                .take((stderr_limit.saturating_add(1)) as u64)
                .read_to_end(&mut bytes);
            bytes
        });
        Ok(CodexProcess {
            child,
            lines: read_bounded_lines(stdout, self.profile.output_limit_bytes),
            stderr_reader,
            started_at: Instant::now(),
        })
    }
}

impl RuntimeDriver for CodexRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Codex
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        self.profile.validate()?;
        let environment = self.isolated_environment()?;
        let mut version_command = Command::new(&self.executable);
        environment.apply(&mut version_command, None, None);
        let output = match version_command.arg("--version").output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProbeReport {
                    kind: RuntimeKind::Codex,
                    executable: self.executable.display().to_string(),
                    version: None,
                    readiness: Readiness::NotInstalled,
                    detail: "executable not found".to_owned(),
                });
            }
            Err(error) => return Err(RuntimeError::Process(error)),
        };
        if !output.status.success() {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: None,
                readiness: Readiness::Unavailable,
                detail: format!("version probe exited with {}", output.status),
            });
        }
        let version = String::from_utf8(output.stdout)
            .map_err(|_| RuntimeError::NonUtf8Output)?
            .trim()
            .to_owned();
        if version != self.profile.expected_version {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: Some(version.clone()),
                readiness: Readiness::Incompatible,
                detail: format!(
                    "profile requires {}, found {version}",
                    self.profile.expected_version
                ),
            });
        }
        let mut auth_command = Command::new(&self.executable);
        environment.apply(&mut auth_command, None, None);
        let auth = auth_command.args(["login", "status"]).output()?;
        if !auth.status.success() {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: Some(version),
                readiness: Readiness::Unauthenticated,
                detail: "Codex authentication is unavailable".to_owned(),
            });
        }
        Ok(ProbeReport {
            kind: RuntimeKind::Codex,
            executable: self.executable.display().to_string(),
            version: Some(version),
            readiness: Readiness::Ready,
            detail: format!(
                "pinned local profile ready: model={}, api_origin={}, reasoning_effort={}, approval_policy={}, prompt_policy={}, sandbox={}, environment=synthetic_allowlist_v1, wall_time_limit_ms={}, output_limit_bytes={}",
                self.profile.model,
                PINNED_CODEX_API_ORIGIN,
                self.profile.reasoning_effort,
                self.profile.approval_policy,
                self.profile.prompt_policy,
                self.profile.sandbox.as_arg(),
                self.profile.wall_time_limit_ms,
                self.profile.output_limit_bytes
            ),
        })
    }

    fn start(&self, request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        self.profile.validate()?;
        let readiness = self.probe()?;
        if readiness.readiness != Readiness::Ready {
            return Err(RuntimeError::InvalidProfile(format!(
                "Codex profile is not ready: {}",
                readiness.detail
            )));
        }
        if !request.workspace.is_dir() {
            return Err(RuntimeError::InvalidProfile(format!(
                "workspace is not a directory: {}",
                request.workspace.display()
            )));
        }
        if request.attempt_id.is_empty() || request.attempt_id.len() > 128 {
            return Err(RuntimeError::InvalidProfile(
                "attempt identifier must contain between 1 and 128 bytes".to_owned(),
            ));
        }
        let launch = CodexLaunch {
            executable: self.executable.clone(),
            profile: self.profile.clone(),
            workspace: request.workspace,
            attempt_id: request.attempt_id,
            mcp: request.mcp,
            environment: self.isolated_environment()?,
        };
        let process = launch.spawn(None, &request.prompt)?;
        Ok(Box::new(CodexSession {
            child: process.child,
            lines: process.lines,
            stderr_reader: Some(process.stderr_reader),
            launch,
            session_id: None,
            invocation_id: request.invocation_id,
            sequence: 0,
            output_limit_bytes: self.profile.output_limit_bytes,
            wall_time_limit_ms: self.profile.wall_time_limit_ms,
            started_at: process.started_at,
            session_started_at: process.started_at,
            cancellation: request.cancellation,
            completed: false,
            terminal: false,
            recoverable: false,
            native_resume_started: false,
            interrupted: false,
            interruption_emitted: false,
        }))
    }
}

struct CodexSession {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: Option<JoinHandle<Vec<u8>>>,
    launch: CodexLaunch,
    session_id: Option<String>,
    invocation_id: String,
    sequence: u64,
    output_limit_bytes: usize,
    wall_time_limit_ms: u64,
    started_at: Instant,
    session_started_at: Instant,
    cancellation: CancellationToken,
    completed: bool,
    terminal: bool,
    recoverable: bool,
    native_resume_started: bool,
    interrupted: bool,
    interruption_emitted: bool,
}

impl CodexSession {
    fn emit(&mut self, event: RuntimeEventKind) -> RuntimeEvent {
        self.sequence += 1;
        RuntimeEvent {
            sequence: self.sequence,
            event_id: format!("{}.event-{}", self.invocation_id, self.sequence),
            invocation_id: self.invocation_id.clone(),
            event,
        }
    }

    fn finish(&mut self) -> Result<ExitStatus, RuntimeError> {
        let status = self.child.wait()?;
        self.completed = true;
        Ok(status)
    }

    fn stderr(&mut self) -> String {
        let Some(reader) = self.stderr_reader.take() else {
            return String::new();
        };
        reader
            .join()
            .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
            .unwrap_or_else(|_| "stderr reader failed".to_owned())
    }

    fn unsuccessful(&mut self, status: ExitStatus) -> RuntimeError {
        RuntimeError::UnsuccessfulExit {
            status: status.to_string(),
            stderr: self.stderr(),
        }
    }

    fn install_process(&mut self, process: CodexProcess) {
        self.child = process.child;
        self.lines = process.lines;
        self.stderr_reader = Some(process.stderr_reader);
        self.started_at = process.started_at;
        self.completed = false;
        self.recoverable = false;
        self.native_resume_started = true;
    }

    fn finish_after_error(&mut self, error: &RuntimeError) {
        if self.terminal || self.interrupted || self.cancellation.is_cancelled() {
            return;
        }
        if !self.completed {
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
        self.recoverable = self.session_id.is_some()
            && !self.native_resume_started
            && matches!(
                error,
                RuntimeError::Process(_) | RuntimeError::UnsuccessfulExit { .. }
            );
        self.terminal = self.session_id.is_some() && !self.recoverable;
    }

    fn next_line(&mut self) -> Result<Option<String>, RuntimeError> {
        let limit = Duration::from_millis(self.wall_time_limit_ms);
        loop {
            if self.cancellation.is_cancelled() {
                let _ = terminate_process_tree(&mut self.child);
                self.completed = true;
                self.interrupted = true;
                return Ok(None);
            }
            let Some(remaining) = limit.checked_sub(self.started_at.elapsed()) else {
                let _ = terminate_process_tree(&mut self.child);
                self.completed = true;
                return Err(RuntimeError::TimedOut {
                    limit_ms: self.wall_time_limit_ms,
                });
            };
            match self
                .lines
                .recv_timeout(remaining.min(Duration::from_millis(100)))
            {
                Ok(BoundedOutputLine::Line(line)) => return Ok(Some(line)),
                Ok(BoundedOutputLine::End) => return Ok(None),
                Ok(BoundedOutputLine::ReadFailed(error)) => {
                    return Err(RuntimeError::MalformedEvent(error));
                }
                Ok(BoundedOutputLine::LimitExceeded) => {
                    let _ = terminate_process_tree(&mut self.child);
                    self.completed = true;
                    return Err(RuntimeError::OutputLimitExceeded {
                        limit_bytes: self.output_limit_bytes,
                    });
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(None),
            }
        }
    }

    fn parse_event(&mut self, event: Value) -> Result<Option<RuntimeEvent>, RuntimeError> {
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::MalformedEvent("event has no type".to_owned()))?;
        match event_type {
            "thread.started" => {
                let session_id = string_field(&event, "thread_id")?;
                if let Some(expected) = &self.session_id {
                    if expected != &session_id {
                        let _ = terminate_process_tree(&mut self.child);
                        self.completed = true;
                        self.terminal = true;
                        return Err(RuntimeError::InvalidProfile(format!(
                            "resumed Codex session identifier changed from {expected} to {session_id}"
                        )));
                    }
                } else {
                    self.session_id = Some(session_id.clone());
                }
                Ok(Some(self.emit(RuntimeEventKind::Started {
                    opaque_session_id: session_id,
                })))
            }
            "item.completed" => {
                let Some(item) = event.get("item") else {
                    return Err(RuntimeError::MalformedEvent(
                        "item.completed has no item".to_owned(),
                    ));
                };
                match item.get("type").and_then(Value::as_str) {
                    Some("agent_message") => {
                        let text = string_field(item, "text")?;
                        Ok(Some(self.emit(RuntimeEventKind::Output { text })))
                    }
                    Some("mcp_tool_call") => {
                        let server = string_field(item, "server")?;
                        let tool = string_field(item, "tool")?;
                        let status = string_field(item, "status")?;
                        let arguments = item.get("arguments").cloned().unwrap_or(Value::Null);
                        let result = item.get("result").filter(|value| !value.is_null()).cloned();
                        let error = item.get("error").filter(|value| !value.is_null()).cloned();
                        Ok(Some(self.emit(RuntimeEventKind::McpToolCall {
                            server,
                            tool,
                            status,
                            arguments,
                            result,
                            error,
                        })))
                    }
                    _ => Ok(None),
                }
            }
            "turn.completed" => {
                let usage = event.get("usage").ok_or_else(|| {
                    RuntimeError::MalformedEvent("turn.completed has no usage".to_owned())
                })?;
                let usage = Usage {
                    input_tokens: u64_field(usage, "input_tokens")?,
                    cached_input_tokens: optional_u64_field(usage, "cached_input_tokens"),
                    output_tokens: u64_field(usage, "output_tokens")?,
                    reasoning_output_tokens: optional_u64_field(usage, "reasoning_output_tokens"),
                    cost_microusd: None,
                    wall_time_ms: elapsed_millis(self.session_started_at),
                    protected_queries: 0,
                    in_flight_excess: Default::default(),
                };
                let status = self.finish()?;
                if !status.success() {
                    return Err(self.unsuccessful(status));
                }
                self.terminal = true;
                Ok(Some(self.emit(RuntimeEventKind::Completed { usage })))
            }
            "turn.started" | "item.started" | "item.updated" => Ok(None),
            "error" | "turn.failed" => Err(RuntimeError::MalformedEvent(event.to_string())),
            other => Err(RuntimeError::MalformedEvent(format!(
                "unsupported Codex event type {other}"
            ))),
        }
    }

    fn next_event_inner(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.cancellation.is_cancelled() && !self.completed {
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
            self.terminal = true;
            self.interrupted = true;
        }
        if self.interrupted {
            if self.interruption_emitted {
                return Ok(None);
            }
            self.interruption_emitted = true;
            return Ok(Some(self.emit(RuntimeEventKind::Interrupted)));
        }
        if self.completed {
            return Ok(None);
        }
        loop {
            let Some(line) = self.next_line()? else {
                if self.interrupted {
                    self.terminal = true;
                    self.interruption_emitted = true;
                    return Ok(Some(self.emit(RuntimeEventKind::Interrupted)));
                }
                let status = self.finish()?;
                if status.success() {
                    return Err(RuntimeError::MalformedEvent(
                        "Codex exited without turn.completed".to_owned(),
                    ));
                }
                return Err(self.unsuccessful(status));
            };
            let event: Value = serde_json::from_str(&line)
                .map_err(|error| RuntimeError::MalformedEvent(error.to_string()))?;
            if let Some(event) = self.parse_event(event)? {
                return Ok(Some(event));
            }
        }
    }
}

impl RuntimeSession for CodexSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        let result = self.next_event_inner();
        if let Err(error) = &result {
            self.finish_after_error(error);
        }
        result
    }

    fn resume(&mut self, input: String) -> Result<(), RuntimeError> {
        if self.terminal || self.interrupted || self.cancellation.is_cancelled() {
            return Err(RuntimeError::NotYielded);
        }
        let session_id = self.session_id.clone().ok_or_else(|| {
            RuntimeError::InvalidProfile(
                "managed Codex session identifier is unknown; refusing replacement start"
                    .to_owned(),
            )
        })?;
        if !self.recoverable {
            return Err(RuntimeError::NotYielded);
        }
        let process = self.launch.spawn(Some(&session_id), &input)?;
        self.install_process(process);
        Ok(())
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        self.cancellation.cancel();
        if !self.completed && !self.interrupted {
            terminate_process_tree(&mut self.child)?;
            self.completed = true;
            self.terminal = true;
            self.interrupted = true;
        }
        Ok(())
    }
}

impl Drop for CodexSession {
    fn drop(&mut self) {
        if !self.completed {
            let _ = terminate_process_tree(&mut self.child);
        }
    }
}

fn resolve_executable(executable: PathBuf) -> PathBuf {
    if executable.is_absolute() || executable.components().count() > 1 {
        return executable.canonicalize().unwrap_or(executable);
    }
    let Some(path) = std::env::var_os("PATH") else {
        return executable;
    };
    std::env::split_paths(&path)
        .map(|directory| directory.join(&executable))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
        .unwrap_or(executable)
}

fn discover_auth_source() -> Option<PathBuf> {
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))?;
    let auth = codex_home.join("auth.json");
    auth.is_file().then_some(auth)
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), RuntimeError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), RuntimeError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), RuntimeError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), RuntimeError> {
    Ok(())
}

fn string_field(value: &Value, field: &str) -> Result<String, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| RuntimeError::MalformedEvent(format!("missing string field {field}")))
}

fn u64_field(value: &Value, field: &str) -> Result<u64, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| RuntimeError::MalformedEvent(format!("missing integer field {field}")))
}

fn optional_u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn elapsed_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn add_mcp_config(command: &mut Command, binding: &McpBinding) -> Result<(), RuntimeError> {
    binding.validate()?;
    let executable = binding.executable.to_str().ok_or_else(|| {
        RuntimeError::InvalidProfile("MCP executable path must be UTF-8".to_owned())
    })?;
    for setting in [
        "mcp_servers.ymp.required=true".to_owned(),
        "mcp_servers.ymp.enabled_tools=[\"read_control\",\"read_events\",\"submit\"]".to_owned(),
        "mcp_servers.ymp.default_tools_approval_mode=\"approve\"".to_owned(),
        format!(
            "mcp_servers.ymp.command={}",
            serde_json::to_string(executable).expect("string serialization cannot fail")
        ),
        "mcp_servers.ymp.args=[\"internal\",\"agent-mcp\"]".to_owned(),
    ] {
        command.arg("-c").arg(setting);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CodexProfile, CodexRuntime, PINNED_CODEX_MODEL};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use ymp_runtime_api::{
        CancellationToken, InvocationRequest, RuntimeDriver, RuntimeError, RuntimeEventKind,
    };

    #[test]
    fn profile_rejects_unapproved_model_and_effort() {
        let profile = CodexProfile {
            model: "unapproved".to_owned(),
            ..CodexProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));

        let profile = CodexProfile {
            output_limit_bytes: 0,
            ..CodexProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));

        let profile = CodexProfile {
            wall_time_limit_ms: 0,
            ..CodexProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));

        let profile = CodexProfile {
            model: PINNED_CODEX_MODEL.to_owned(),
            reasoning_effort: "high".to_owned(),
            ..CodexProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));
    }

    #[test]
    fn default_runtime_exposes_pinned_profile() {
        let runtime = CodexRuntime::default();
        assert_eq!(runtime.profile().model, PINNED_CODEX_MODEL);
        assert_eq!(runtime.profile().reasoning_effort, "low");
    }

    #[test]
    fn managed_launch_uses_a_synthetic_home_and_allowlisted_environment() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let negative_control_home =
            std::env::var_os("YMP_NEGATIVE_CONTROL_CONFIG_ROOT").map(std::path::PathBuf::from);
        if let Some(ambient_codex_home) = &negative_control_home {
            assert_eq!(
                std::env::var_os("CODEX_HOME").map(std::path::PathBuf::from),
                Some(ambient_codex_home.clone())
            );
            fs::create_dir_all(ambient_codex_home.join("hooks")).expect("ambient hooks directory");
            fs::create_dir_all(ambient_codex_home.join("plugins"))
                .expect("ambient plugins directory");
            fs::write(
                ambient_codex_home.join("config.toml"),
                b"model = 'ambient-model'\n",
            )
            .expect("ambient configuration");
            fs::write(
                ambient_codex_home.join("auth.json"),
                b"{\"OPENAI_API_KEY\":\"fixture-auth-only\"}\n",
            )
            .expect("ambient authentication fixture");
        }
        let executable = directory.path().join("codex-environment-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  printf '%s\n' "$@" > invocation.args
  {
    env | sed 's/=.*//' | sort
  } > invocation.env-keys
  {
    printf 'HOME=%s\n' "$HOME"
    printf 'CODEX_HOME=%s\n' "$CODEX_HOME"
    printf 'OPENAI_BASE_URL=%s\n' "${OPENAI_BASE_URL-unset}"
    printf 'OPENAI_ORGANIZATION=%s\n' "${OPENAI_ORGANIZATION-unset}"
    printf 'OPENAI_PROJECT=%s\n' "${OPENAI_PROJECT-unset}"
    printf 'YMP_ATTEMPT_ID=%s\n' "${YMP_ATTEMPT_ID-unset}"
    if [ -f "$CODEX_HOME/auth.json" ]; then
      printf '%s\n' 'AUTH_FILE=present'
    else
      printf '%s\n' 'AUTH_FILE=absent'
    fi
    if [ -e "$CODEX_HOME/config.toml" ] || [ -e "$CODEX_HOME/hooks" ] || [ -e "$CODEX_HOME/plugins" ]; then
      printf '%s\n' 'AMBIENT_CONFIG=present'
    else
      printf '%s\n' 'AMBIENT_CONFIG=absent'
    fi
    if [ -n "${YMP_AGENT_TOKEN-}" ]; then
      printf '%s\n' 'YMP_AGENT_TOKEN=present'
    else
      printf '%s\n' 'YMP_AGENT_TOKEN=absent'
    fi
  } > invocation.environment
  cat >/dev/null
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-environment-1"}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-environment".to_owned(),
                attempt_id: "attempt-environment".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: Some(ymp_runtime_api::McpBinding {
                    executable: executable.clone(),
                    socket_path: directory.path().join("agent.sock"),
                    token: "fixture-secret-token".to_owned(),
                }),
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        while session.next_event().expect("runtime event").is_some() {}

        let arguments = fs::read_to_string(directory.path().join("invocation.args"))
            .expect("captured arguments");
        let environment = fs::read_to_string(directory.path().join("invocation.environment"))
            .expect("captured environment facts");
        let ambient_home = std::env::var("HOME").expect("ambient HOME for negative control");
        let keys = fs::read_to_string(directory.path().join("invocation.env-keys"))
            .expect("captured environment keys");
        let mut violations = Vec::new();
        for (required, detail) in [
            (
                !arguments.contains("fixture-secret-token"),
                "MCP token reached argv",
            ),
            (
                arguments.contains("--ignore-user-config"),
                "user configuration was not rejected",
            ),
            (
                arguments.contains("--ignore-rules"),
                "ambient rules were not rejected",
            ),
            (
                arguments.contains("--disable\nhooks\n"),
                "hooks were not disabled",
            ),
            (
                arguments.contains("--disable\nmulti_agent\n"),
                "native subagents were not disabled",
            ),
            (
                arguments.contains("--disable\nmulti_agent_v2\n"),
                "native subagents v2 were not disabled",
            ),
            (
                arguments.contains("--disable\nplugins\n"),
                "plugins were not disabled",
            ),
            (
                arguments.contains("--disable\nremote_control\n"),
                "remote control was not disabled",
            ),
            (
                arguments.contains("--disable\nremote_models\n"),
                "remote models were not disabled",
            ),
            (
                arguments.contains("--disable\nremote_plugin\n"),
                "remote plugins were not disabled",
            ),
            (
                arguments.contains("--disable\nshell_snapshot\n"),
                "ambient shell snapshot was not disabled",
            ),
            (
                arguments.contains("shell_environment_policy.inherit=\"none\""),
                "shell environment inheritance was not disabled",
            ),
            (
                environment.contains("OPENAI_BASE_URL=https://api.openai.com/v1"),
                "ambient provider base URL reached the child",
            ),
            (
                environment.contains("OPENAI_ORGANIZATION=unset"),
                "ambient OpenAI organization reached the child",
            ),
            (
                environment.contains("OPENAI_PROJECT=unset"),
                "ambient OpenAI project reached the child",
            ),
            (
                environment.contains("YMP_ATTEMPT_ID=attempt-environment"),
                "attempt identity was not bound",
            ),
            (
                environment.contains("YMP_AGENT_TOKEN=present"),
                "MCP token was not delivered through the child environment",
            ),
            (
                environment.contains("AMBIENT_CONFIG=absent"),
                "ambient Codex configuration reached the synthetic home",
            ),
            (
                !environment.contains(&format!("HOME={ambient_home}\n")),
                "ambient HOME reached the child",
            ),
        ] {
            if !required {
                violations.push(detail.to_owned());
            }
        }
        if negative_control_home.is_some() && !environment.contains("AUTH_FILE=present") {
            violations.push("allowlisted authentication was not copied".to_owned());
        }
        if let Ok(ambient_codex_home) = std::env::var("CODEX_HOME")
            && environment.contains(&format!("CODEX_HOME={ambient_codex_home}\n"))
        {
            violations.push("ambient CODEX_HOME reached the child".to_owned());
        }
        for forbidden in [
            "OPENAI_ORGANIZATION",
            "OPENAI_PROJECT",
            "YMP_AMBIENT_HOOK",
            "YMP_AMBIENT_MCP",
            "YMP_AMBIENT_PLUGIN",
            "YMP_NEGATIVE_CONTROL_CONFIG_ROOT",
            "YMP_NATIVE_SUBAGENT",
            "YMP_REMOTE_EXECUTION",
        ] {
            if keys.lines().any(|key| key == forbidden) {
                violations.push(format!(
                    "managed child inherited environment key {forbidden}"
                ));
            }
        }
        assert!(
            violations.is_empty(),
            "managed child was not isolated:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn structured_process_stream_preserves_session_output_and_usage() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' "$@" > "$(dirname "$0")/invocation.args"
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-1"}'
  printf '%s\n' '{"type":"turn.started"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"read_control","status":"completed","arguments":{},"result":{"run_id":"run-1"},"error":null}}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"done"}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"cached_input_tokens":7,"output_tokens":3,"reasoning_output_tokens":2}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-1".to_owned(),
                attempt_id: "attempt-1".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: Some(ymp_runtime_api::McpBinding {
                    executable: executable.clone(),
                    socket_path: directory.path().join("agent.sock"),
                    token: "fixture-token".to_owned(),
                }),
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        let started = session.next_event().expect("started").expect("event");
        assert!(matches!(
            started.event,
            RuntimeEventKind::Started { opaque_session_id } if opaque_session_id == "thread-1"
        ));
        let tool_call = session.next_event().expect("tool call").expect("event");
        assert!(matches!(
            tool_call.event,
            RuntimeEventKind::McpToolCall {
                server,
                tool,
                status,
                result: Some(result),
                error: None,
                ..
            } if server == "ymp"
                && tool == "read_control"
                && status == "completed"
                && result["run_id"] == "run-1"
        ));
        let output = session.next_event().expect("output").expect("event");
        assert!(matches!(
            output.event,
            RuntimeEventKind::Output { text } if text == "done"
        ));
        let completed = session.next_event().expect("completed").expect("event");
        assert!(matches!(
            completed.event,
            RuntimeEventKind::Completed { usage }
                if usage.input_tokens == 11
                    && usage.cached_input_tokens == 7
                    && usage.output_tokens == 3
                    && usage.reasoning_output_tokens == 2
        ));
        assert!(session.next_event().expect("terminal").is_none());
        let arguments = fs::read_to_string(directory.path().join("invocation.args"))
            .expect("captured invocation arguments");
        assert!(arguments.contains("mcp_servers.ymp.required=true"));
        assert!(arguments.contains(
            "mcp_servers.ymp.enabled_tools=[\"read_control\",\"read_events\",\"submit\"]"
        ));
        assert!(arguments.contains("mcp_servers.ymp.default_tools_approval_mode=\"approve\""));
    }

    #[test]
    fn missing_usage_and_incompatible_structured_events_reject_the_run() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-invalid-event-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  input=$(cat)
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-invalid-event"}'
  case "$input" in
    *missing-usage*) printf '%s\n' '{"type":"turn.completed"}' ;;
    *) printf '%s\n' '{"type":"future.incompatible_event"}' ;;
  esac
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        for (prompt, expected_detail) in [
            ("missing-usage", "turn.completed has no usage"),
            ("incompatible-event", "unsupported Codex event type"),
        ] {
            let runtime = CodexRuntime::new(&executable);
            let mut session = runtime
                .start(InvocationRequest {
                    invocation_id: format!("invocation-{prompt}"),
                    attempt_id: format!("attempt-{prompt}"),
                    workspace: directory.path().to_owned(),
                    mcp: None,
                    prompt: prompt.to_owned(),
                    cancellation: Default::default(),
                })
                .expect("start fixture");
            assert!(matches!(
                session.next_event().expect("started").expect("event").event,
                RuntimeEventKind::Started { .. }
            ));
            assert!(matches!(
                session.next_event(),
                Err(RuntimeError::MalformedEvent(detail)) if detail.contains(expected_detail)
            ));
        }
    }

    #[test]
    fn recoverable_process_failure_resumes_the_same_managed_session() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-resume-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
  printf '%s\n' "$count" > invocation.count
  printf '%s\n' "$@" > "invocation-$count.args"
  {
    printf 'HOME=%s\n' "$HOME"
    printf 'CODEX_HOME=%s\n' "$CODEX_HOME"
    printf 'OPENAI_BASE_URL=%s\n' "${OPENAI_BASE_URL-unset}"
    printf 'OPENAI_ORGANIZATION=%s\n' "${OPENAI_ORGANIZATION-unset}"
    printf 'OPENAI_PROJECT=%s\n' "${OPENAI_PROJECT-unset}"
    printf 'YMP_ATTEMPT_ID=%s\n' "${YMP_ATTEMPT_ID-unset}"
    if [ -n "${YMP_AGENT_TOKEN-}" ]; then
      printf '%s\n' 'YMP_AGENT_TOKEN=present'
    else
      printf '%s\n' 'YMP_AGENT_TOKEN=absent'
    fi
  } > "invocation-$count.environment"
  cat > "invocation-$count.stdin"
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-resume-1"}'
  if [ "$count" -eq 1 ]; then
    printf '%s\n' 'committed progress' > committed.progress
    printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"committed progress"}}'
    exit 17
  fi
  test "$(cat committed.progress)" = 'committed progress' || exit 29
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"resumed progress"}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":13,"cached_input_tokens":5,"output_tokens":3,"reasoning_output_tokens":1}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-resume".to_owned(),
                attempt_id: "attempt-resume".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: Some(ymp_runtime_api::McpBinding {
                    executable: executable.clone(),
                    socket_path: directory.path().join("agent.sock"),
                    token: "fixture-token".to_owned(),
                }),
                prompt: "initial prompt".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");

        let started = session.next_event().expect("started").expect("event");
        assert_eq!(started.sequence, 1);
        assert!(matches!(
            started.event,
            RuntimeEventKind::Started { opaque_session_id }
                if opaque_session_id == "thread-resume-1"
        ));
        let committed = session.next_event().expect("output").expect("event");
        assert!(matches!(
            committed.event,
            RuntimeEventKind::Output { text } if text == "committed progress"
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::UnsuccessfulExit { .. })
        ));

        session
            .resume("continue after interruption".to_owned())
            .expect("resume fixture");
        let resumed = session.next_event().expect("resumed start").expect("event");
        assert_eq!(resumed.sequence, 3);
        assert!(matches!(
            resumed.event,
            RuntimeEventKind::Started { opaque_session_id }
                if opaque_session_id == "thread-resume-1"
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("resumed output")
                .expect("event")
                .event,
            RuntimeEventKind::Output { text } if text == "resumed progress"
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("completion")
                .expect("event")
                .event,
            RuntimeEventKind::Completed { .. }
        ));

        let resumed_arguments = fs::read_to_string(directory.path().join("invocation-2.args"))
            .expect("resumed arguments");
        let initial_arguments = fs::read_to_string(directory.path().join("invocation-1.args"))
            .expect("initial arguments");
        assert!(!initial_arguments.contains("--ephemeral"));
        assert!(resumed_arguments.contains("resume\nthread-resume-1\n"));
        assert!(!resumed_arguments.contains("--ephemeral"));
        assert!(!initial_arguments.contains("fixture-token"));
        assert!(!resumed_arguments.contains("fixture-token"));
        let initial_environment =
            fs::read_to_string(directory.path().join("invocation-1.environment"))
                .expect("initial environment");
        let resumed_environment =
            fs::read_to_string(directory.path().join("invocation-2.environment"))
                .expect("resumed environment");
        assert_eq!(initial_environment, resumed_environment);
        assert!(initial_environment.contains("OPENAI_BASE_URL=https://api.openai.com/v1"));
        assert!(initial_environment.contains("OPENAI_ORGANIZATION=unset"));
        assert!(initial_environment.contains("OPENAI_PROJECT=unset"));
        assert!(initial_environment.contains("YMP_ATTEMPT_ID=attempt-resume"));
        assert!(initial_environment.contains("YMP_AGENT_TOKEN=present"));
        assert_eq!(
            fs::read_to_string(directory.path().join("invocation.count"))
                .expect("invocation count")
                .trim(),
            "2"
        );
    }

    #[test]
    fn unknown_managed_session_does_not_start_a_replacement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-unknown-session-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
  printf '%s\n' "$count" > invocation.count
  cat >/dev/null
  exit 23
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-unknown".to_owned(),
                attempt_id: "attempt-unknown".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::UnsuccessfulExit { .. })
        ));
        assert!(matches!(
            session.resume("do not replace".to_owned()),
            Err(RuntimeError::InvalidProfile(_))
        ));
        assert_eq!(
            fs::read_to_string(directory.path().join("invocation.count"))
                .expect("invocation count")
                .trim(),
            "1"
        );
    }

    #[test]
    fn unknown_session_reported_by_native_resume_is_terminal() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-native-resume-unknown-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
  printf '%s\n' "$count" > invocation.count
  cat >/dev/null
  if [ "$count" -eq 1 ]; then
    printf '%s\n' '{"type":"thread.started","thread_id":"thread-missing"}'
    exit 17
  fi
  printf '%s\n' 'unknown session thread-missing' >&2
  exit 23
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-native-resume-unknown".to_owned(),
                attempt_id: "attempt-native-resume-unknown".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { opaque_session_id }
                if opaque_session_id == "thread-missing"
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::UnsuccessfulExit { .. })
        ));
        session.resume("resume".to_owned()).expect("native resume");
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::UnsuccessfulExit { stderr, .. })
                if stderr.contains("unknown session thread-missing")
        ));
        assert!(matches!(
            session.resume("replacement".to_owned()),
            Err(RuntimeError::NotYielded)
        ));
        assert_eq!(
            fs::read_to_string(directory.path().join("invocation.count"))
                .expect("invocation count")
                .trim(),
            "2"
        );
    }

    #[test]
    fn timeout_terminates_the_runtime_process_group() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-timeout-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  sleep 30 &
  printf '%s\n' "$!" > descendant.pid
  wait
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let profile = CodexProfile {
            wall_time_limit_ms: 100,
            ..CodexProfile::default()
        };
        let runtime = CodexRuntime::with_profile(&executable, profile);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-timeout".to_owned(),
                attempt_id: "attempt-timeout".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::TimedOut { limit_ms: 100 })
        ));

        let descendant =
            fs::read_to_string(directory.path().join("descendant.pid")).expect("descendant pid");
        let descendant = descendant.trim();
        let mut alive = true;
        for _ in 0..20 {
            alive = std::process::Command::new("/bin/kill")
                .args(["-0", descendant])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!alive, "descendant process {descendant} survived timeout");
    }

    #[test]
    fn cancellation_token_interrupts_a_blocked_runtime_tree() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-cancel-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  sleep 30 &
  printf '%s\n' "$!" > cancel-descendant.pid
  wait
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let cancellation = CancellationToken::default();
        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-cancel".to_owned(),
                attempt_id: "attempt-cancel".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: cancellation.clone(),
            })
            .expect("start fixture");
        let worker = std::thread::spawn(move || session.next_event());
        let pid_path = directory.path().join("cancel-descendant.pid");
        for _ in 0..40 {
            if pid_path.is_file() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        cancellation.cancel();
        let event = worker
            .join()
            .expect("runtime thread")
            .expect("runtime result")
            .expect("interrupted event");
        assert!(matches!(event.event, RuntimeEventKind::Interrupted));

        let descendant = fs::read_to_string(pid_path).expect("descendant pid");
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", descendant.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        assert!(!alive, "descendant process survived cancellation");
    }
}
