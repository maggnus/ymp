#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use ymp_runtime_api::{
    AdmittedProgram, BoundedOutputLine, CancellationToken, DiagnosticSummary, InFlightExcess,
    InvocationRequest, LaunchChain, LaunchDescriptor, LaunchEnvironmentVariable, McpBinding,
    ModelSpend, ProbeReport, Readiness, RuntimeDriver, RuntimeError, RuntimeEvent,
    RuntimeEventKind, RuntimeFailureKind, RuntimeKind, RuntimeSession, Usage,
    configure_process_group, create_launch_marker, end_process_tree_or_keep, evidence_digest,
    managed_launch_command, read_bounded_lines, register_launch_marker, terminate_process_tree,
    verify_admitted_programs,
};

pub const PINNED_CODEX_VERSION: &str = "codex-cli 0.147.0";
/// The model route the managed profile requests. `gpt-5.6-sol` was refused with HTTP 400
/// (`invalid_request_error`: not supported when using Codex with a ChatGPT account), so the route
/// is the balanced agentic coding model the account does list, which accepts the `low` reasoning
/// effort this profile pins.
pub const PINNED_CODEX_MODEL: &str = "gpt-5.6-terra";
pub const PINNED_CODEX_PROMPT_POLICY: &str = "ymp-codex-low-v1";
pub const PINNED_CODEX_API_ORIGIN: &str = "https://api.openai.com/v1";
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_WALL_TIME_LIMIT_MS: u64 = 10 * 60 * 1000;
const LAUNCH_DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
const HARNESS_INSTRUCTIONS: &str = "Execution policy: work without delegation or subagents. Do not send progress reports. Batch independent file reads and batch the final formatting, tests, lint, and diff checks. Use only the files and tools needed for the requested outcome. Do not commit. Stop immediately after a concise final report.";
/// How a managed attempt publishes what it produced. The operator's request is delivered word for
/// word and is not required to describe the product's own submission path, so the requirement
/// travels here instead: an attempt that only writes files leaves nothing the run can accept, and
/// the budget it spent buys no candidate.
const PUBLICATION_INSTRUCTIONS: &str = "Publication policy: the work of this attempt becomes a candidate only when it is published with the submit tool of the ymp MCP server. Files left in the workspace are not a result, and no report replaces that call. Call submit once, as soon as the requested outcome is reached, whether or not the request above mentions publishing.";
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
    verified_executable: Arc<Mutex<Option<VerifiedExecutable>>>,
    profile: CodexProfile,
    auth_source: Option<PathBuf>,
    runtime_path: OsString,
    launch_chain: LaunchChain,
    prepared_launches: Arc<Mutex<HashMap<String, CodexLaunch>>>,
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
            verified_executable: Arc::new(Mutex::new(None)),
            profile: CodexProfile::default(),
            auth_source: discover_auth_source(),
            runtime_path: std::env::var_os("PATH")
                .unwrap_or_else(|| OsString::from("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin")),
            launch_chain: LaunchChain::default(),
            prepared_launches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_profile(executable: impl Into<PathBuf>, profile: CodexProfile) -> Self {
        let mut runtime = Self::new(executable);
        runtime.profile = profile;
        runtime
    }

    /// Replaces the programs the launch preamble enters. The product path uses the system shell and
    /// sanitiser; this exists so that a check can substitute a chain program after admission, which
    /// the system paths do not allow.
    #[doc(hidden)]
    pub fn with_launch_chain(mut self, chain: LaunchChain) -> Self {
        self.launch_chain = chain;
        self
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

    fn admitted_executable(&self) -> Result<VerifiedExecutable, RuntimeError> {
        let mut admitted = self.verified_executable.lock().map_err(|_| {
            RuntimeError::InvalidProfile("verified Codex executable lock failed".to_owned())
        })?;
        if let Some(executable) = admitted.as_ref() {
            return Ok(executable.clone());
        }
        let executable = VerifiedExecutable::admit(&self.executable, "codex")?;
        *admitted = Some(executable.clone());
        Ok(executable)
    }
}

#[derive(Clone, Debug)]
struct VerifiedExecutable {
    execution_path: PathBuf,
    digest: String,
    _directory: Arc<tempfile::TempDir>,
}

impl VerifiedExecutable {
    fn admit(source_path: &Path, label: &str) -> Result<Self, RuntimeError> {
        let mut source = File::open(source_path)?;
        if !source.metadata()?.is_file() {
            return Err(RuntimeError::InvalidProfile(format!(
                "{label} executable is not a regular file: {}",
                source_path.display()
            )));
        }
        let directory = Arc::new(
            tempfile::Builder::new()
                .prefix("ymp-admitted-executable-")
                .tempdir()?,
        );
        set_private_directory_permissions(directory.path())?;
        let execution_path = directory.path().join(label);
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&execution_path)?;
        std::io::copy(&mut source, &mut destination)?;
        destination.flush()?;
        destination.sync_all()?;
        drop(destination);
        set_executable_file_permissions(&execution_path)?;
        let digest = executable_digest(&execution_path)?;
        Ok(Self {
            execution_path,
            digest,
            _directory: directory,
        })
    }
}

#[derive(Debug)]
struct CodexLaunch {
    executable: VerifiedExecutable,
    coordination_executable: Option<VerifiedExecutable>,
    launch_chain: Vec<AdmittedProgram>,
    profile: CodexProfile,
    workspace: PathBuf,
    attempt_id: String,
    invocation_id: String,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct EnvironmentValue {
    name: String,
    value: String,
    confidential: bool,
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

    fn values(
        &self,
        mcp: Option<&McpBinding>,
        attempt_id: Option<&str>,
        invocation_id: Option<&str>,
    ) -> Result<Vec<EnvironmentValue>, RuntimeError> {
        let mut values = vec![
            environment_value("HOME", self.root.path(), false)?,
            environment_value("CODEX_HOME", &self.codex_home, false)?,
            environment_value("TMPDIR", &self.temporary, false)?,
            EnvironmentValue {
                name: "PATH".to_owned(),
                value: self
                    .runtime_path
                    .to_str()
                    .ok_or_else(|| {
                        RuntimeError::InvalidProfile(
                            "allowlisted PATH must be valid UTF-8".to_owned(),
                        )
                    })?
                    .to_owned(),
                confidential: false,
            },
            plain_environment_value("NO_COLOR", "1", false),
            plain_environment_value("OPENAI_BASE_URL", PINNED_CODEX_API_ORIGIN, false),
        ];
        if let (Some(mcp), Some(attempt_id), Some(invocation_id)) = (mcp, attempt_id, invocation_id)
        {
            values.extend([
                environment_value("YMP_AGENT_SOCKET", &mcp.socket_path, true)?,
                plain_environment_value("YMP_AGENT_TOKEN", &mcp.token, true),
                plain_environment_value("YMP_ATTEMPT_ID", attempt_id, false),
                plain_environment_value("YMP_INVOCATION_ID", invocation_id, false),
            ]);
        }
        values.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(values)
    }

    fn apply(command: &mut Command, values: &[EnvironmentValue]) {
        command.env_clear();
        for variable in values {
            command.env(&variable.name, &variable.value);
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
    fn descriptor(&self, session_id: Option<&str>) -> Result<LaunchDescriptor, RuntimeError> {
        let arguments = self.arguments(session_id)?;
        let environment = self.environment.values(
            self.mcp.as_ref(),
            Some(&self.attempt_id),
            Some(&self.invocation_id),
        )?;
        Ok(LaunchDescriptor {
            schema_version: LAUNCH_DESCRIPTOR_SCHEMA_VERSION,
            invocation_id: self.invocation_id.clone(),
            attempt_id: self.attempt_id.clone(),
            executable: self.executable.execution_path.clone(),
            executable_digest: self.executable.digest.clone(),
            coordination_executable: self
                .coordination_executable
                .as_ref()
                .map(|executable| executable.execution_path.clone()),
            coordination_executable_digest: self
                .coordination_executable
                .as_ref()
                .map(|executable| executable.digest.clone()),
            launch_chain: self.launch_chain.clone(),
            arguments,
            environment: environment
                .iter()
                .map(|variable| LaunchEnvironmentVariable {
                    name: variable.name.clone(),
                    value: (!variable.confidential).then(|| variable.value.clone()),
                    value_digest: evidence_digest(variable.value.as_bytes()),
                    confidential: variable.confidential,
                })
                .collect(),
            working_directory: self.workspace.clone(),
        })
    }

    fn arguments(&self, session_id: Option<&str>) -> Result<Vec<String>, RuntimeError> {
        let workspace = self.workspace.to_str().ok_or_else(|| {
            RuntimeError::InvalidProfile("Codex workspace path must be UTF-8".to_owned())
        })?;
        let mut arguments = vec![
            "exec".to_owned(),
            "--json".to_owned(),
            "--ignore-user-config".to_owned(),
            "--ignore-rules".to_owned(),
            "--sandbox".to_owned(),
            self.profile.sandbox.as_arg().to_owned(),
            "--model".to_owned(),
            self.profile.model.clone(),
            "-c".to_owned(),
            format!(
                "model_reasoning_effort=\"{}\"",
                self.profile.reasoning_effort
            ),
            "-c".to_owned(),
            format!("approval_policy=\"{}\"", self.profile.approval_policy),
            "-c".to_owned(),
            "shell_environment_policy.inherit=\"none\"".to_owned(),
            "-C".to_owned(),
            workspace.to_owned(),
        ];
        for feature in DISABLED_AMBIENT_FEATURES {
            arguments.push("--disable".to_owned());
            arguments.push(feature.to_owned());
        }
        if let Some(mcp) = &self.mcp {
            add_mcp_config_arguments(&mut arguments, mcp)?;
        }
        if let Some(session_id) = session_id {
            arguments.push("resume".to_owned());
            arguments.push(session_id.to_owned());
        }
        arguments.push("-".to_owned());
        Ok(arguments)
    }

    /// What the product tells the managed runtime after the operator's request. The publication
    /// requirement is added only for a launch that carries the coordination bridge, because an
    /// invocation without it has no submission tool to name.
    fn managed_instructions(&self) -> String {
        match self.mcp {
            Some(_) => format!("{PUBLICATION_INSTRUCTIONS} {HARNESS_INSTRUCTIONS}"),
            None => HARNESS_INSTRUCTIONS.to_owned(),
        }
    }

    fn spawn(
        &self,
        descriptor: &LaunchDescriptor,
        session_id: Option<&str>,
        prompt: &str,
    ) -> Result<CodexProcess, RuntimeError> {
        let expected = self.descriptor(session_id)?;
        if descriptor != &expected {
            return Err(RuntimeError::InvalidProfile(
                "prepared Codex launch descriptor was modified".to_owned(),
            ));
        }
        if executable_digest(&descriptor.executable)? != descriptor.executable_digest {
            return Err(RuntimeError::InvalidProfile(
                "Codex executable changed after launch preparation".to_owned(),
            ));
        }
        if let (Some(executable), Some(digest)) = (
            descriptor.coordination_executable.as_deref(),
            descriptor.coordination_executable_digest.as_deref(),
        ) && executable_digest(executable)? != digest
        {
            return Err(RuntimeError::InvalidProfile(
                "MCP executable changed after launch preparation".to_owned(),
            ));
        }
        let environment = self.environment.values(
            self.mcp.as_ref(),
            Some(&self.attempt_id),
            Some(&self.invocation_id),
        )?;
        // The shell and the environment sanitiser execute before the runtime image is loaded, so
        // they are part of the trusted path and are refused unless they still hold the bytes that
        // were admitted for this launch.
        verify_admitted_programs(&descriptor.launch_chain)?;
        // Every descendant of the managed process inherits this marker, whatever becomes of the
        // processes between it and the run, so the run can still identify what it started.
        let marker = create_launch_marker()?;
        let mut command = match managed_launch_command(
            &descriptor.executable,
            &descriptor.arguments,
            &marker,
            &descriptor.launch_chain,
        ) {
            Ok(command) => command,
            Err(error) => {
                let _ = std::fs::remove_file(&marker);
                return Err(error);
            }
        };
        CodexEnvironment::apply(&mut command, &environment);
        command
            .current_dir(&descriptor.working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_group(&mut command);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = std::fs::remove_file(&marker);
                return Err(error.into());
            }
        };
        register_launch_marker(&child, marker);
        if let Err(error) = verify_admitted_programs(&descriptor.launch_chain) {
            end_process_tree_or_keep(&mut child);
            return Err(error);
        }
        if executable_digest(&descriptor.executable)? != descriptor.executable_digest {
            end_process_tree_or_keep(&mut child);
            return Err(RuntimeError::InvalidProfile(
                "Codex executable changed while the prepared launch was starting".to_owned(),
            ));
        }
        if let (Some(executable), Some(digest)) = (
            descriptor.coordination_executable.as_deref(),
            descriptor.coordination_executable_digest.as_deref(),
        ) && executable_digest(executable)? != digest
        {
            end_process_tree_or_keep(&mut child);
            return Err(RuntimeError::InvalidProfile(
                "MCP executable changed while the prepared launch was starting".to_owned(),
            ));
        }
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stdin was not piped".to_owned()))?;
        // The operator's request goes first and unchanged; what the product requires of every
        // managed attempt follows it as the product's own instruction.
        stdin.write_all(prompt.as_bytes())?;
        stdin.write_all(b"\n\n")?;
        stdin.write_all(self.managed_instructions().as_bytes())?;
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
        let environment_values = environment.values(None, None, None)?;
        if !self.executable.is_file() {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: None,
                readiness: Readiness::NotInstalled,
                detail: "executable not found".to_owned(),
            });
        }
        let admitted = self.admitted_executable()?;
        let mut version_command = Command::new(&admitted.execution_path);
        CodexEnvironment::apply(&mut version_command, &environment_values);
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
        let mut auth_command = Command::new(&admitted.execution_path);
        CodexEnvironment::apply(&mut auth_command, &environment_values);
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
        let readiness = self.probe()?;
        if readiness.readiness != Readiness::Ready {
            return Err(RuntimeError::InvalidProfile(format!(
                "Codex profile is not ready: {}",
                readiness.detail
            )));
        }
        let descriptor = self.prepare_launch(&request)?.ok_or_else(|| {
            RuntimeError::InvalidProfile("Codex launch descriptor was not prepared".to_owned())
        })?;
        self.start_prepared(request, Some(&descriptor))
    }

    fn prepare_launch(
        &self,
        request: &InvocationRequest,
    ) -> Result<Option<LaunchDescriptor>, RuntimeError> {
        self.profile.validate()?;
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
        if request.invocation_id.is_empty() || request.invocation_id.len() > 128 {
            return Err(RuntimeError::InvalidProfile(
                "invocation identifier must contain between 1 and 128 bytes".to_owned(),
            ));
        }
        if let Some(mcp) = &request.mcp {
            mcp.validate()?;
        }
        let executable = self.admitted_executable()?;
        let (mcp, coordination_executable) = match &request.mcp {
            Some(binding) => {
                let executable = VerifiedExecutable::admit(&binding.executable, "ymp-agent-mcp")?;
                let mut binding = binding.clone();
                binding.executable = executable.execution_path.clone();
                (Some(binding), Some(executable))
            }
            None => (None, None),
        };
        let launch = CodexLaunch {
            executable,
            coordination_executable,
            launch_chain: self.launch_chain.admit()?,
            profile: self.profile.clone(),
            workspace: request.workspace.clone(),
            attempt_id: request.attempt_id.clone(),
            invocation_id: request.invocation_id.clone(),
            mcp,
            environment: self.isolated_environment()?,
        };
        let descriptor = launch.descriptor(None)?;
        let mut prepared = self
            .prepared_launches
            .lock()
            .map_err(|_| RuntimeError::InvalidProfile("prepared launch lock failed".to_owned()))?;
        if prepared
            .insert(request.invocation_id.clone(), launch)
            .is_some()
        {
            return Err(RuntimeError::InvalidProfile(
                "duplicate prepared Codex invocation identifier".to_owned(),
            ));
        }
        Ok(Some(descriptor))
    }

    fn start_prepared(
        &self,
        request: InvocationRequest,
        descriptor: Option<&LaunchDescriptor>,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        let descriptor = descriptor.ok_or_else(|| {
            RuntimeError::InvalidProfile("Codex requires a prepared launch descriptor".to_owned())
        })?;
        let launch = self
            .prepared_launches
            .lock()
            .map_err(|_| RuntimeError::InvalidProfile("prepared launch lock failed".to_owned()))?
            .remove(&request.invocation_id)
            .ok_or_else(|| {
                RuntimeError::InvalidProfile(
                    "prepared Codex invocation is missing or already consumed".to_owned(),
                )
            })?;
        if descriptor.invocation_id != request.invocation_id
            || descriptor.attempt_id != request.attempt_id
            || descriptor.working_directory != request.workspace
        {
            return Err(RuntimeError::InvalidProfile(
                "prepared Codex launch identity does not match the invocation request".to_owned(),
            ));
        }
        let process = launch.spawn(descriptor, None, &request.prompt)?;
        let mut pending_events = VecDeque::new();
        pending_events.push_back(RuntimeEventKind::Launch {
            descriptor: Box::new(descriptor.clone()),
        });
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
            pending_events,
            usage: Usage::default(),
            current_turn_usage: Usage::default(),
            in_flight: InFlightExcess::default(),
            yielded: false,
            failure_emitted: false,
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
    pending_events: VecDeque<RuntimeEventKind>,
    usage: Usage,
    current_turn_usage: Usage,
    in_flight: InFlightExcess,
    yielded: bool,
    failure_emitted: bool,
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
        terminate_process_tree(&mut self.child)?;
        Ok(status)
    }

    fn stderr(&mut self) -> DiagnosticSummary {
        let Some(reader) = self.stderr_reader.take() else {
            return DiagnosticSummary::from_bytes(&[], false);
        };
        let mut bytes = reader.join().unwrap_or_default();
        let truncated = bytes.len() > self.output_limit_bytes;
        bytes.truncate(self.output_limit_bytes);
        DiagnosticSummary::from_bytes(&bytes, truncated)
    }

    fn unsuccessful(&mut self, status: ExitStatus) -> RuntimeError {
        RuntimeError::SanitizedUnsuccessfulExit {
            status: status.to_string(),
            diagnostic: self.stderr(),
        }
    }

    fn install_process(&mut self, process: CodexProcess, descriptor: LaunchDescriptor) {
        self.child = process.child;
        self.lines = process.lines;
        self.stderr_reader = Some(process.stderr_reader);
        self.started_at = process.started_at;
        self.completed = false;
        self.recoverable = false;
        self.native_resume_started = true;
        self.yielded = false;
        self.pending_events.push_back(RuntimeEventKind::Launch {
            descriptor: Box::new(descriptor),
        });
    }

    fn usage_snapshot(&self) -> Usage {
        let mut usage = self.usage.clone();
        add_usage(&mut usage, &self.current_turn_usage);
        usage.wall_time_ms = elapsed_millis(self.session_started_at);
        usage.in_flight_excess = self.in_flight.clone();
        usage
    }

    /// A started turn holds one model request the runtime has not yet accounted for. That count is
    /// the product's own, derived from the observed turn boundary rather than reported by the
    /// runtime, and it stands only until the turn's accounting record arrives; see `merge_usage`.
    fn begin_turn(&mut self, event: &Value) {
        if let Some(usage) = event.get("usage") {
            self.current_turn_usage = observed_usage(usage);
            self.in_flight = usage
                .get("in_flight_excess")
                .map(in_flight_excess)
                .unwrap_or_else(|| InFlightExcess {
                    model_requests: 1,
                    ..InFlightExcess::default()
                });
        } else {
            self.in_flight = InFlightExcess {
                model_requests: 1,
                ..InFlightExcess::default()
            };
        }
    }

    /// Applies a turn's accounting record. The record accounts for everything the turn consumed, so
    /// it also settles the excess: the counters it states replace the current ones, and a record
    /// that states none leaves none. Keeping the earlier value instead would record the request the
    /// product counted at the turn start as if the runtime had reported it unaccounted.
    fn merge_usage(&mut self, value: &Value) -> Result<(), RuntimeError> {
        let mut observed = observed_usage(value);
        observed.input_tokens = u64_field(value, "input_tokens")?;
        observed.output_tokens = u64_field(value, "output_tokens")?;
        let observed_in_flight = value.get("in_flight_excess").map(in_flight_excess);
        self.current_turn_usage = observed;
        add_usage(&mut self.usage, &self.current_turn_usage);
        self.current_turn_usage = Usage::default();
        self.in_flight = observed_in_flight.unwrap_or_default();
        Ok(())
    }

    fn failed_event(
        &mut self,
        kind: RuntimeFailureKind,
        diagnostic: Option<DiagnosticSummary>,
    ) -> RuntimeEvent {
        self.failure_emitted = true;
        self.terminal = true;
        self.emit(RuntimeEventKind::Failed {
            kind,
            usage: self.usage_snapshot(),
            diagnostic,
        })
    }

    fn finish_after_error(&mut self, error: &RuntimeError) {
        if self.terminal || self.interrupted || self.cancellation.is_cancelled() {
            return;
        }
        if !self.completed {
            end_process_tree_or_keep(&mut self.child);
            self.completed = true;
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
        self.recoverable = self.session_id.is_some()
            && !self.native_resume_started
            && matches!(
                error,
                RuntimeError::Process(_) | RuntimeError::SanitizedUnsuccessfulExit { .. }
            );
        self.terminal = self.session_id.is_some() && !self.recoverable;
    }

    fn next_line(&mut self) -> Result<Option<String>, RuntimeError> {
        let limit = Duration::from_millis(self.wall_time_limit_ms);
        loop {
            if self.cancellation.is_cancelled() {
                end_process_tree_or_keep(&mut self.child);
                self.completed = true;
                self.interrupted = true;
                return Ok(None);
            }
            let Some(remaining) = limit.checked_sub(self.started_at.elapsed()) else {
                end_process_tree_or_keep(&mut self.child);
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
                    end_process_tree_or_keep(&mut self.child);
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
                        end_process_tree_or_keep(&mut self.child);
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
                self.merge_usage(usage)?;
                let status = self.finish()?;
                if !status.success() {
                    return Err(self.unsuccessful(status));
                }
                if self.session_id.is_none() {
                    return Err(RuntimeError::MalformedEvent(
                        "Codex completed a turn before reporting its session".to_owned(),
                    ));
                }
                self.yielded = true;
                self.recoverable = true;
                Ok(Some(self.emit(RuntimeEventKind::Completed {
                    usage: self.usage_snapshot(),
                })))
            }
            "turn.started" => {
                self.begin_turn(&event);
                Ok(None)
            }
            "item.started" | "item.updated" => Ok(None),
            "error" | "turn.failed" => {
                if let Some(usage) = event.get("usage") {
                    self.merge_usage(usage)?;
                }
                let bytes = serde_json::to_vec(&event).map_err(|error| {
                    RuntimeError::MalformedEvent(format!(
                        "failed to summarize runtime failure event: {error}"
                    ))
                })?;
                end_process_tree_or_keep(&mut self.child);
                self.completed = true;
                Ok(Some(self.failed_event(
                    RuntimeFailureKind::RuntimeReported,
                    Some(DiagnosticSummary::from_bytes(&bytes, false)),
                )))
            }
            other => Err(RuntimeError::MalformedEvent(format!(
                "unsupported Codex event type {other}"
            ))),
        }
    }

    fn next_event_inner(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.cancellation.is_cancelled() && !self.terminal && !self.interrupted {
            if !self.completed {
                end_process_tree_or_keep(&mut self.child);
            }
            self.completed = true;
            self.terminal = true;
            self.interrupted = true;
            self.yielded = false;
        }
        if self.interrupted {
            if self.interruption_emitted {
                return Ok(None);
            }
            self.interruption_emitted = true;
            return Ok(Some(self.emit(RuntimeEventKind::Cancelled {
                usage: self.usage_snapshot(),
            })));
        }
        if self.completed {
            return Ok(None);
        }
        loop {
            let Some(line) = self.next_line()? else {
                if self.interrupted {
                    self.terminal = true;
                    self.interruption_emitted = true;
                    return Ok(Some(self.emit(RuntimeEventKind::Cancelled {
                        usage: self.usage_snapshot(),
                    })));
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
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(self.emit(event)));
        }
        let result = self.next_event_inner();
        if let Err(error) = &result {
            self.finish_after_error(error);
        }
        match result {
            Err(RuntimeError::TimedOut { limit_ms }) => {
                self.terminal = true;
                Ok(Some(self.emit(RuntimeEventKind::TimedOut {
                    limit_ms,
                    usage: self.usage_snapshot(),
                })))
            }
            Err(RuntimeError::OutputLimitExceeded { limit_bytes }) => {
                let diagnostic = DiagnosticSummary::from_bytes(
                    format!("output_limit_bytes={limit_bytes}").as_bytes(),
                    false,
                );
                Ok(Some(self.failed_event(
                    RuntimeFailureKind::OutputLimit,
                    Some(diagnostic),
                )))
            }
            Err(error) if self.recoverable => Err(error),
            Err(error) if !self.failure_emitted => {
                let kind = match error {
                    RuntimeError::SanitizedUnsuccessfulExit { .. } | RuntimeError::Process(_) => {
                        RuntimeFailureKind::ProcessExit
                    }
                    RuntimeError::RuntimeReportedFailure(_) => RuntimeFailureKind::RuntimeReported,
                    _ => RuntimeFailureKind::Protocol,
                };
                let diagnostic = match &error {
                    RuntimeError::SanitizedUnsuccessfulExit { diagnostic, .. } => {
                        diagnostic.clone()
                    }
                    _ => DiagnosticSummary::from_bytes(error.to_string().as_bytes(), false),
                };
                Ok(Some(self.failed_event(kind, Some(diagnostic))))
            }
            other => other,
        }
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
        if !self.recoverable && !self.yielded {
            return Err(RuntimeError::NotYielded);
        }
        let descriptor = self.launch.descriptor(Some(&session_id))?;
        let process = self.launch.spawn(&descriptor, Some(&session_id), &input)?;
        self.install_process(process, descriptor);
        Ok(())
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        self.cancellation.cancel();
        if !self.terminal && !self.interrupted {
            if !self.completed {
                terminate_process_tree(&mut self.child)?;
            }
            self.completed = true;
            self.terminal = true;
            self.interrupted = true;
            self.yielded = false;
        }
        Ok(())
    }

    fn usage(&self) -> Usage {
        self.usage_snapshot()
    }
}

impl Drop for CodexSession {
    fn drop(&mut self) {
        if !self.completed {
            end_process_tree_or_keep(&mut self.child);
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

#[cfg(unix)]
fn set_executable_file_permissions(path: &Path) -> Result<(), RuntimeError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o500))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable_file_permissions(_path: &Path) -> Result<(), RuntimeError> {
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

fn in_flight_excess(value: &Value) -> InFlightExcess {
    InFlightExcess {
        model_requests: optional_u64_field(value, "model_requests"),
        input_tokens: optional_u64_field(value, "input_tokens"),
        cached_input_tokens: optional_u64_field(value, "cached_input_tokens"),
        output_tokens: optional_u64_field(value, "output_tokens"),
        reasoning_output_tokens: optional_u64_field(value, "reasoning_output_tokens"),
        cost_microusd: optional_u64_field(value, "cost_microusd"),
    }
}

/// Reads the per-model breakdown a turn reports beside its total. A turn that names no model is
/// read as an empty breakdown rather than assigned to the admitted profile, so a cost that arrived
/// without evidence of the route that spent it stays visibly unattributed in the record.
fn model_breakdown(value: &Value) -> Vec<ModelSpend> {
    let Some(reported) = value.get("cost_by_model").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut breakdown: Vec<ModelSpend> = reported
        .iter()
        .filter_map(|spend| {
            let model = spend.get("model").and_then(Value::as_str)?;
            Some(ModelSpend {
                model: model.to_owned(),
                cost_microusd: optional_u64_field(spend, "cost_microusd"),
            })
        })
        .collect();
    breakdown.sort();
    breakdown
}

fn observed_usage(value: &Value) -> Usage {
    Usage {
        input_tokens: optional_u64_field(value, "input_tokens"),
        cached_input_tokens: optional_u64_field(value, "cached_input_tokens"),
        output_tokens: optional_u64_field(value, "output_tokens"),
        reasoning_output_tokens: optional_u64_field(value, "reasoning_output_tokens"),
        cost_microusd: value.get("cost_microusd").and_then(Value::as_u64),
        cost_by_model: model_breakdown(value),
        wall_time_ms: 0,
        protected_queries: optional_u64_field(value, "protected_queries"),
        in_flight_excess: value
            .get("in_flight_excess")
            .map(in_flight_excess)
            .unwrap_or_default(),
    }
}

fn add_usage(total: &mut Usage, increment: &Usage) {
    total.input_tokens = total.input_tokens.saturating_add(increment.input_tokens);
    total.cached_input_tokens = total
        .cached_input_tokens
        .saturating_add(increment.cached_input_tokens);
    total.output_tokens = total.output_tokens.saturating_add(increment.output_tokens);
    total.reasoning_output_tokens = total
        .reasoning_output_tokens
        .saturating_add(increment.reasoning_output_tokens);
    if let Some(cost) = increment.cost_microusd {
        total.cost_microusd = Some(total.cost_microusd.unwrap_or_default().saturating_add(cost));
    }
    total.absorb_model_spend(&increment.cost_by_model);
    total.protected_queries = total
        .protected_queries
        .saturating_add(increment.protected_queries);
}

fn elapsed_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn executable_digest(executable: &Path) -> Result<String, RuntimeError> {
    if !executable.is_file() {
        return Err(RuntimeError::InvalidProfile(format!(
            "Codex executable is not a regular file: {}",
            executable.display()
        )));
    }
    Ok(evidence_digest(&fs::read(executable)?))
}

fn environment_value(
    name: &str,
    value: &Path,
    confidential: bool,
) -> Result<EnvironmentValue, RuntimeError> {
    Ok(EnvironmentValue {
        name: name.to_owned(),
        value: value
            .to_str()
            .ok_or_else(|| {
                RuntimeError::InvalidProfile(format!(
                    "allowlisted environment value for {name} must be UTF-8"
                ))
            })?
            .to_owned(),
        confidential,
    })
}

fn plain_environment_value(name: &str, value: &str, confidential: bool) -> EnvironmentValue {
    EnvironmentValue {
        name: name.to_owned(),
        value: value.to_owned(),
        confidential,
    }
}

fn add_mcp_config_arguments(
    arguments: &mut Vec<String>,
    binding: &McpBinding,
) -> Result<(), RuntimeError> {
    binding.validate()?;
    let executable = binding.executable.to_str().ok_or_else(|| {
        RuntimeError::InvalidProfile("MCP executable path must be UTF-8".to_owned())
    })?;
    for setting in [
        "mcp_servers.ymp.required=true".to_owned(),
        "mcp_servers.ymp.enabled_tools=[\"read_control\",\"read_events\",\"yield\",\"submit\"]"
            .to_owned(),
        "mcp_servers.ymp.default_tools_approval_mode=\"approve\"".to_owned(),
        format!(
            "mcp_servers.ymp.command={}",
            serde_json::to_string(executable).expect("string serialization cannot fail")
        ),
        "mcp_servers.ymp.args=[\"internal\",\"agent-mcp\"]".to_owned(),
    ] {
        arguments.push("-c".to_owned());
        arguments.push(setting);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CodexProfile, CodexRuntime, PINNED_CODEX_MODEL};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use ymp_runtime_api::{
        CancellationToken, InvocationRequest, RuntimeDriver, RuntimeError, RuntimeEventKind,
        RuntimeFailureKind, RuntimeSession,
    };

    fn expect_launch(session: &mut dyn RuntimeSession) {
        assert!(matches!(
            session.next_event().expect("launch").expect("event").event,
            RuntimeEventKind::Launch { .. }
        ));
    }

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

    /// The requirement to publish through the submission tool reaches the managed runtime as the
    /// product's own instruction, so a request that never mentions publishing still carries it.
    /// The request itself is delivered word for word, and an invocation without the coordination
    /// bridge is told nothing about a tool it does not have.
    #[test]
    fn the_product_tells_a_coordinated_runtime_how_a_candidate_is_published() {
        const REQUEST: &str = "Replace the content of input.txt with the single line: after.";
        assert!(
            !REQUEST.contains("submit"),
            "the measured request already names publication itself"
        );

        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-delivery-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat > delivered.stdin
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-delivery"}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let mut delivered = Vec::new();
        for (name, mcp) in [
            (
                "coordinated",
                Some(ymp_runtime_api::McpBinding {
                    executable: executable.clone(),
                    socket_path: directory.path().join("agent.sock"),
                    token: "fixture-token".to_owned(),
                }),
            ),
            ("plain", None),
        ] {
            let workspace = directory.path().join(name);
            fs::create_dir(&workspace).expect("workspace directory");
            let mut session = CodexRuntime::new(&executable)
                .start(InvocationRequest {
                    invocation_id: format!("invocation-delivery-{name}"),
                    attempt_id: format!("attempt-delivery-{name}"),
                    workspace: workspace.clone(),
                    mcp,
                    prompt: REQUEST.to_owned(),
                    cancellation: Default::default(),
                })
                .expect("start fixture");
            while session.next_event().expect("delivery event").is_some() {}
            delivered.push(
                fs::read_to_string(workspace.join("delivered.stdin")).expect("delivered input"),
            );
        }
        let [coordinated, plain] = <[String; 2]>::try_from(delivered).expect("two deliveries");

        for (label, text) in [("coordinated", &coordinated), ("uncoordinated", &plain)] {
            assert!(
                text.starts_with(&format!("{REQUEST}\n\n")),
                "the {label} invocation did not receive the request word for word: {text}"
            );
            assert!(
                text.contains("Execution policy:"),
                "the {label} invocation received no execution policy: {text}"
            );
        }
        assert!(
            coordinated.contains("Publication policy:"),
            "a coordinated invocation was not told how a candidate is published: {coordinated}"
        );
        assert!(
            !plain.contains("Publication policy:"),
            "an invocation without the coordination bridge was told to call a tool it does not \
             have: {plain}"
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
  printf '%s\n' "$@" > invocation.args
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-1"}'
  printf '%s\n' '{"type":"turn.started"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"read_control","status":"completed","arguments":{},"result":{"run_id":"run-1"},"error":null}}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"submit-1"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"done"}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"cached_input_tokens":7,"output_tokens":3,"reasoning_output_tokens":2,"cost_microusd":91,"protected_queries":2}}'
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
        expect_launch(session.as_mut());
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
        assert!(matches!(
            session.next_event().expect("submit").expect("event").event,
            RuntimeEventKind::McpToolCall { tool, status, .. }
                if tool == "submit" && status == "completed"
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
                    && usage.cost_microusd == Some(91)
                    && usage.protected_queries == 2
        ));
        assert!(session.next_event().expect("terminal").is_none());
        let arguments = fs::read_to_string(directory.path().join("invocation.args"))
            .expect("captured invocation arguments");
        assert!(arguments.contains("mcp_servers.ymp.required=true"));
        assert!(arguments.contains(
            "mcp_servers.ymp.enabled_tools=[\"read_control\",\"read_events\",\"yield\",\"submit\"]"
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

        for prompt in ["missing-usage", "incompatible-event"] {
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
            expect_launch(session.as_mut());
            assert!(matches!(
                session.next_event().expect("started").expect("event").event,
                RuntimeEventKind::Started { .. }
            ));
            assert!(matches!(
                session.next_event().expect("failure").expect("event").event,
                RuntimeEventKind::Failed {
                    kind: RuntimeFailureKind::Protocol,
                    diagnostic: Some(diagnostic),
                    ..
                } if diagnostic.digest.len() == 64
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
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"resume-submit"},"result":{"committed":true},"error":null}}'
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

        expect_launch(session.as_mut());
        let started = session.next_event().expect("started").expect("event");
        assert_eq!(started.sequence, 2);
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
            Err(RuntimeError::SanitizedUnsuccessfulExit { .. })
        ));

        session
            .resume("continue after interruption".to_owned())
            .expect("resume fixture");
        expect_launch(session.as_mut());
        let resumed = session.next_event().expect("resumed start").expect("event");
        assert_eq!(resumed.sequence, 5);
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
            session.next_event().expect("submit").expect("event").event,
            RuntimeEventKind::McpToolCall { tool, .. } if tool == "submit"
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
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("failure").expect("event").event,
            RuntimeEventKind::Failed {
                kind: RuntimeFailureKind::ProcessExit,
                ..
            }
        ));
        assert!(matches!(
            session.resume("do not replace".to_owned()),
            Err(RuntimeError::NotYielded)
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
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { opaque_session_id }
                if opaque_session_id == "thread-missing"
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::SanitizedUnsuccessfulExit { .. })
        ));
        session.resume("resume".to_owned()).expect("native resume");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("failure").expect("event").event,
            RuntimeEventKind::Failed {
                kind: RuntimeFailureKind::ProcessExit,
                diagnostic: Some(diagnostic),
                ..
            } if diagnostic.bytes > 0 && diagnostic.digest.len() == 64
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
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("timeout").expect("event").event,
            RuntimeEventKind::TimedOut { limit_ms: 100, .. }
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
        expect_launch(session.as_mut());
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
        assert!(matches!(event.event, RuntimeEventKind::Cancelled { .. }));

        let descendant = fs::read_to_string(pid_path).expect("descendant pid");
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", descendant.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        assert!(!alive, "descendant process survived cancellation");
    }

    #[test]
    fn prepared_launch_matches_actual_process_and_rejects_all_mutations() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-launch-descriptor-fixture");
        let script = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  printf '%s\n' "$0" > actual.executable
  printf '%s\n' "$@" > actual.arguments
  env | sort > actual.environment
  cat >/dev/null
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-launch"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"launch-submit"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}'
fi
"##;
        fs::write(&executable, script).expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let make_request = |invocation_id: &str| InvocationRequest {
            invocation_id: invocation_id.to_owned(),
            attempt_id: "attempt-launch".to_owned(),
            workspace: directory.path().to_owned(),
            mcp: Some(ymp_runtime_api::McpBinding {
                executable: executable.clone(),
                socket_path: directory.path().join("agent.sock"),
                token: "launch-fixture-secret".to_owned(),
            }),
            prompt: "fixture".to_owned(),
            cancellation: Default::default(),
        };

        let runtime = CodexRuntime::new(&executable);
        let initial_request = make_request("invocation-launch-actual");
        let descriptor = runtime
            .prepare_launch(&initial_request)
            .expect("prepare launch")
            .expect("Codex descriptor");
        assert_eq!(descriptor.invocation_id, initial_request.invocation_id);
        assert_eq!(descriptor.attempt_id, initial_request.attempt_id);
        let token_entry = descriptor
            .environment
            .iter()
            .find(|variable| variable.name == "YMP_AGENT_TOKEN")
            .expect("token evidence");
        assert!(token_entry.confidential);
        assert_eq!(token_entry.value, None);
        assert!(
            !serde_json::to_string(&descriptor)
                .expect("serialize descriptor")
                .contains("launch-fixture-secret")
        );
        let mut session = runtime
            .start_prepared(initial_request, Some(&descriptor))
            .expect("start prepared launch");
        let launched = session.next_event().expect("launch").expect("event");
        assert!(matches!(
            launched.event,
            RuntimeEventKind::Launch { descriptor: actual } if *actual == descriptor
        ));
        while session.next_event().expect("runtime event").is_some() {}

        assert_eq!(
            fs::read_to_string(directory.path().join("actual.executable"))
                .expect("actual executable")
                .trim(),
            descriptor.executable.to_str().expect("UTF-8 executable")
        );
        let actual_arguments: Vec<_> =
            fs::read_to_string(directory.path().join("actual.arguments"))
                .expect("actual arguments")
                .lines()
                .map(str::to_owned)
                .collect();
        assert_eq!(actual_arguments, descriptor.arguments);
        let environment_bytes =
            fs::read(directory.path().join("actual.environment")).expect("actual environment");
        let actual_environment =
            String::from_utf8(environment_bytes.clone()).expect("UTF-8 environment");
        for variable in &descriptor.environment {
            let actual = actual_environment
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{}=", variable.name)))
                .expect("descriptor environment entry reached child");
            assert_eq!(
                ymp_runtime_api::evidence_digest(actual.as_bytes()),
                variable.value_digest
            );
            if let Some(expected) = &variable.value {
                assert_eq!(actual, expected);
            }
        }
        fs::remove_file(directory.path().join("actual.environment"))
            .expect("remove raw test observation");

        for (suffix, mutate) in [
            ("argument", 0_u8),
            ("environment", 1_u8),
            ("invocation", 2_u8),
        ] {
            let request = make_request(&format!("invocation-launch-{suffix}"));
            let mut descriptor = runtime
                .prepare_launch(&request)
                .expect("prepare mutation")
                .expect("descriptor");
            match mutate {
                0 => descriptor.arguments.push("--mutated".to_owned()),
                1 => descriptor.environment[0].value_digest = "0".repeat(64),
                2 => descriptor.invocation_id.push_str("-mutated"),
                _ => unreachable!(),
            }
            assert!(matches!(
                runtime.start_prepared(request, Some(&descriptor)),
                Err(RuntimeError::InvalidProfile(_))
            ));
        }

        let request = make_request("invocation-launch-executable");
        let descriptor = runtime
            .prepare_launch(&request)
            .expect("prepare executable mutation")
            .expect("descriptor");
        let mut permissions = fs::metadata(&descriptor.executable)
            .expect("admitted executable metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&descriptor.executable, permissions)
            .expect("make admitted executable mutable for negative control");
        fs::write(&descriptor.executable, format!("{script}\n# substituted\n"))
            .expect("substitute admitted executable");
        assert!(matches!(
            runtime.start_prepared(request, Some(&descriptor)),
            Err(RuntimeError::InvalidProfile(_))
        ));

        let request = make_request("invocation-launch-mcp-executable");
        let descriptor = runtime
            .prepare_launch(&request)
            .expect("prepare MCP executable mutation")
            .expect("descriptor");
        let coordination_executable = descriptor
            .coordination_executable
            .as_ref()
            .expect("coordination executable");
        let mut permissions = fs::metadata(coordination_executable)
            .expect("admitted MCP executable metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(coordination_executable, permissions)
            .expect("make admitted MCP executable mutable for negative control");
        fs::write(coordination_executable, "#!/bin/sh\nexit 73\n")
            .expect("substitute admitted MCP executable");
        assert!(matches!(
            runtime.start_prepared(request, Some(&descriptor)),
            Err(RuntimeError::InvalidProfile(_))
        ));
    }

    #[test]
    fn terminal_outcomes_preserve_available_accounting() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-accounting-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  input=$(cat)
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-accounting"}'
  printf '%s\n' '{"type":"turn.started","usage":{"input_tokens":7,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":1,"cost_microusd":23,"protected_queries":2,"in_flight_excess":{"model_requests":1,"input_tokens":5,"cached_input_tokens":1,"output_tokens":2,"reasoning_output_tokens":1,"cost_microusd":11}}}'
  case "$input" in
    *success*)
      printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"accounting-submit"},"result":{"committed":true},"error":null}}'
      printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":2,"cost_microusd":31,"protected_queries":3,"in_flight_excess":{"model_requests":2,"cost_microusd":3}}}'
      ;;
    *error*)
      printf '%s\n' '{"type":"turn.failed","usage":{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":2,"cost_microusd":31,"protected_queries":3,"in_flight_excess":{"model_requests":1,"input_tokens":5,"cached_input_tokens":1,"output_tokens":2,"reasoning_output_tokens":1,"cost_microusd":11}},"error":{"code":"fixture"}}'
      ;;
    *) sleep 30 ;;
  esac
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        for outcome in ["success", "error", "cancel", "timeout"] {
            let cancellation = CancellationToken::default();
            let profile = CodexProfile {
                wall_time_limit_ms: if outcome == "timeout" { 150 } else { 5_000 },
                ..CodexProfile::default()
            };
            let runtime = CodexRuntime::with_profile(&executable, profile);
            let mut session = runtime
                .start(InvocationRequest {
                    invocation_id: format!("invocation-{outcome}"),
                    attempt_id: format!("attempt-{outcome}"),
                    workspace: directory.path().to_owned(),
                    mcp: None,
                    prompt: outcome.to_owned(),
                    cancellation: cancellation.clone(),
                })
                .expect("start accounting fixture");
            expect_launch(session.as_mut());
            assert!(matches!(
                session.next_event().expect("started").expect("event").event,
                RuntimeEventKind::Started { .. }
            ));
            let terminal = if outcome == "cancel" {
                let worker = std::thread::spawn(move || session.next_event());
                std::thread::sleep(std::time::Duration::from_millis(75));
                cancellation.cancel();
                worker
                    .join()
                    .expect("cancel worker")
                    .expect("cancel event")
                    .expect("terminal event")
            } else {
                loop {
                    let event = session
                        .next_event()
                        .expect("accounting event")
                        .expect("terminal event");
                    if matches!(
                        &event.event,
                        RuntimeEventKind::Completed { .. }
                            | RuntimeEventKind::Failed { .. }
                            | RuntimeEventKind::TimedOut { .. }
                    ) {
                        break event;
                    }
                }
            };
            let usage = match terminal.event {
                RuntimeEventKind::Completed { usage }
                | RuntimeEventKind::Failed { usage, .. }
                | RuntimeEventKind::TimedOut { usage, .. }
                | RuntimeEventKind::Cancelled { usage } => usage,
                other => panic!("unexpected terminal event: {other:?}"),
            };
            assert!(usage.wall_time_ms > 0);
            assert!(usage.cost_microusd.is_some());
            assert!(usage.input_tokens > 0);
            assert!(usage.output_tokens > 0);
            assert!(usage.protected_queries > 0);
            if outcome == "success" {
                assert_eq!(usage.in_flight_excess.model_requests, 2);
                assert_eq!(usage.in_flight_excess.cost_microusd, 3);
            } else {
                assert!(usage.in_flight_excess.model_requests > 0);
                assert!(usage.in_flight_excess.cost_microusd > 0);
            }
        }
    }

    /// A turn's accounting record covers everything the turn consumed, so a terminal report that
    /// states no in-flight excess leaves none behind. The count the product keeps between the turn
    /// start and that record — one open model request — is its own, and must not survive into a
    /// record the runtime closed. A released Codex build reports no excess field at all, which is
    /// the first fixture below; the second states an excess while the turn runs and then closes the
    /// turn without one.
    #[test]
    fn a_terminal_record_without_an_excess_field_records_no_excess() {
        let directory = tempfile::tempdir().expect("temporary directory");
        for (fixture, started_usage) in [
            (
                "silent",
                r#"{"input_tokens":7,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":1,"cost_microusd":23,"protected_queries":2}"#,
            ),
            (
                "reported",
                r#"{"input_tokens":7,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":1,"cost_microusd":23,"protected_queries":2,"in_flight_excess":{"model_requests":4,"input_tokens":5,"cost_microusd":11}}"#,
            ),
        ] {
            let executable = directory.path().join(format!("codex-{fixture}-fixture"));
            fs::write(
                &executable,
                format!(
                    r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' '{{"type":"thread.started","thread_id":"thread-{fixture}"}}'
  printf '%s\n' '{{"type":"turn.started","usage":{started_usage}}}'
  printf '%s\n' '{{"type":"turn.completed","usage":{{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":2,"cost_microusd":31,"protected_queries":3}}}}'
fi
"##
                ),
            )
            .expect("write fixture");
            let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&executable, permissions).expect("make executable");

            let runtime = CodexRuntime::new(&executable);
            let mut session = runtime
                .start(InvocationRequest {
                    invocation_id: format!("invocation-{fixture}"),
                    attempt_id: format!("attempt-{fixture}"),
                    workspace: directory.path().to_owned(),
                    mcp: None,
                    prompt: "accounting".to_owned(),
                    cancellation: CancellationToken::default(),
                })
                .expect("start accounting fixture");
            expect_launch(session.as_mut());
            let usage = loop {
                let event = session
                    .next_event()
                    .expect("accounting event")
                    .expect("terminal event");
                if let RuntimeEventKind::Completed { usage } = event.event {
                    break usage;
                }
            };
            assert!(usage.input_tokens > 0, "{fixture} input tokens");
            assert!(usage.output_tokens > 0, "{fixture} output tokens");
            assert_eq!(
                usage.in_flight_excess,
                crate::InFlightExcess::default(),
                "{fixture} terminal excess"
            );
        }
    }

    /// Builds a substitutable stand-in for a system program of the launch chain. The operating
    /// system refuses to execute a copy of `/bin/sh` or `/usr/bin/env`, measured as a kill by
    /// signal, so a check enters the real program through a script whose own bytes can be replaced
    /// after admission.
    /// Binds the stand-ins by the digests they were written with, because their location cannot
    /// carry the binding: they stand in a directory this account owns, which is exactly what the
    /// product rule refuses.
    fn stand_in_chain(shell: &Path, sanitiser: &Path) -> ymp_runtime_api::LaunchChain {
        let digest = |path: &Path| {
            ymp_runtime_api::evidence_digest(&fs::read(path).expect("stand-in bytes"))
        };
        ymp_runtime_api::LaunchChain::stated(
            (shell.to_owned(), digest(shell)),
            Some((sanitiser.to_owned(), digest(sanitiser))),
        )
    }

    fn chain_stand_in(directory: &Path, name: &str, program: &str) -> PathBuf {
        let path = directory.join(name);
        let entered = directory.join(format!("{name}.entered"));
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' entered >> '{}'\nexec {program} \"$@\"\n",
                entered.display()
            ),
        )
        .expect("write launch chain stand-in");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).expect("make executable");
        path
    }

    fn admitted_runtime_fixture(path: &Path) {
        fs::write(
            path,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' admitted > admitted-runtime.marker
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-chain"}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}'
fi
"##,
        )
        .expect("write admitted fixture");
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("make executable");
    }

    #[test]
    fn every_admitted_launch_chain_program_executes_and_refuses_replacement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-chain-fixture");
        admitted_runtime_fixture(&executable);
        let shell = chain_stand_in(directory.path(), "chain-shell", "/bin/sh");
        let sanitiser = chain_stand_in(directory.path(), "chain-sanitiser", "/usr/bin/env");
        let runtime =
            CodexRuntime::new(&executable).with_launch_chain(stand_in_chain(&shell, &sanitiser));
        runtime.probe().expect("probe chain runtime");

        let admitted_workspace = directory.path().join("admitted");
        fs::create_dir(&admitted_workspace).expect("admitted workspace");
        let request = InvocationRequest {
            invocation_id: "invocation-chain-admitted".to_owned(),
            attempt_id: "attempt-chain-admitted".to_owned(),
            workspace: admitted_workspace.clone(),
            mcp: None,
            prompt: "run through the admitted chain".to_owned(),
            cancellation: Default::default(),
        };
        let descriptor = runtime
            .prepare_launch(&request)
            .expect("prepare admitted chain")
            .expect("launch descriptor");
        assert_eq!(
            descriptor
                .launch_chain
                .iter()
                .map(|program| (program.role, program.path.clone()))
                .collect::<Vec<_>>(),
            vec![
                (ymp_runtime_api::ProgramRole::LaunchShell, shell.clone()),
                (
                    ymp_runtime_api::ProgramRole::EnvironmentSanitiser,
                    sanitiser.clone()
                ),
            ]
        );
        let mut session = runtime
            .start_prepared(request, Some(&descriptor))
            .expect("execute the admitted chain");
        while session.next_event().expect("runtime event").is_some() {}
        drop(session);
        assert!(
            admitted_workspace.join("admitted-runtime.marker").is_file(),
            "the admitted chain did not reach the runtime"
        );
        // The marker alone would also appear if the launch had ignored the admitted chain and
        // entered the system shell, so each stand-in records that it was the program that ran.
        for name in ["chain-shell", "chain-sanitiser"] {
            assert_eq!(
                fs::read_to_string(directory.path().join(format!("{name}.entered")))
                    .unwrap_or_default()
                    .lines()
                    .count(),
                1,
                "the launch did not enter the admitted {name}"
            );
        }

        for (index, expected) in [(0_usize, "launch shell"), (1, "environment sanitiser")] {
            let workspace = directory.path().join(format!("replaced-{index}"));
            fs::create_dir(&workspace).expect("negative workspace");
            let request = InvocationRequest {
                invocation_id: format!("invocation-chain-replaced-{index}"),
                attempt_id: format!("attempt-chain-replaced-{index}"),
                workspace: workspace.clone(),
                mcp: None,
                prompt: "run through a replaced chain".to_owned(),
                cancellation: Default::default(),
            };
            let descriptor = runtime
                .prepare_launch(&request)
                .expect("prepare replaced chain")
                .expect("launch descriptor");
            let replaced = &descriptor.launch_chain[index].path;
            let bytes = fs::read(replaced).expect("admitted chain program");
            fs::write(replaced, [bytes.as_slice(), b"# substituted\n"].concat())
                .expect("substitute admitted chain program");
            let error = runtime
                .start_prepared(request, Some(&descriptor))
                .err()
                .expect("a replaced chain program refuses the launch");
            match &error {
                RuntimeError::InvalidProfile(detail) => assert!(
                    detail.contains(expected) && detail.contains("changed after admission"),
                    "unexpected refusal: {detail}"
                ),
                other => panic!("unexpected error: {other:?}"),
            }
            assert!(
                !workspace.join("admitted-runtime.marker").exists(),
                "the runtime ran through a replaced chain program"
            );
            // The next round admits the chain again against the digests this check stated, so the
            // replaced program is put back rather than left for the following round to trip over.
            fs::write(replaced, &bytes).expect("restore the admitted chain program");
        }
    }

    #[test]
    fn admitted_runtime_bytes_execute_after_source_path_replacement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-admitted-runtime-fixture");
        let admitted = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' admitted > admitted-runtime.marker
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-admitted"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"admitted-submit"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}'
fi
"##;
        fs::write(&executable, admitted).expect("write admitted fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");
        let runtime = CodexRuntime::new(&executable);
        let request = InvocationRequest {
            invocation_id: "invocation-admitted-runtime".to_owned(),
            attempt_id: "attempt-admitted-runtime".to_owned(),
            workspace: directory.path().to_owned(),
            mcp: None,
            prompt: "run admitted bytes".to_owned(),
            cancellation: Default::default(),
        };
        runtime.probe().expect("probe admitted runtime");
        let descriptor = runtime
            .prepare_launch(&request)
            .expect("prepare admitted runtime")
            .expect("launch descriptor");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' substituted > substituted-runtime.marker\nexit 73\n",
        )
        .expect("replace source path");
        let mut session = runtime
            .start_prepared(request, Some(&descriptor))
            .expect("execute admitted runtime object");
        while session.next_event().expect("runtime event").is_some() {}
        assert!(directory.path().join("admitted-runtime.marker").is_file());
        assert!(!directory.path().join("substituted-runtime.marker").exists());
    }

    #[test]
    fn configured_bridge_uses_admitted_bytes_after_source_path_replacement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-bridge-config-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  bridge=''
  for argument in "$@"; do
    case "$argument" in
      mcp_servers.ymp.command=*)
        bridge=${argument#mcp_servers.ymp.command=}
        bridge=${bridge#\"}
        bridge=${bridge%\"}
        ;;
    esac
  done
  "$bridge"
  printf '%s\n' admitted > admitted-runtime.marker
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-bridge"}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}'
fi
"##,
        )
        .expect("write Codex fixture");
        let bridge = directory.path().join("ymp-bridge-fixture");
        fs::write(
            &bridge,
            "#!/bin/sh\nprintf '%s\\n' admitted > admitted-bridge.marker\n",
        )
        .expect("write admitted bridge");
        for path in [&executable, &bridge] {
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(path, permissions).expect("make executable");
        }
        let runtime = CodexRuntime::new(&executable);
        let request = InvocationRequest {
            invocation_id: "invocation-admitted-bridge".to_owned(),
            attempt_id: "attempt-admitted-bridge".to_owned(),
            workspace: directory.path().to_owned(),
            mcp: Some(ymp_runtime_api::McpBinding {
                executable: bridge.clone(),
                socket_path: directory.path().join("agent.sock"),
                token: "bridge-token".to_owned(),
            }),
            prompt: "inspect bridge binding".to_owned(),
            cancellation: Default::default(),
        };
        runtime.probe().expect("probe admitted runtime");
        let descriptor = runtime
            .prepare_launch(&request)
            .expect("prepare bridge binding")
            .expect("launch descriptor");
        let configured_bridge = descriptor
            .arguments
            .windows(2)
            .find_map(|arguments| {
                (arguments[0] == "-c")
                    .then_some(arguments[1].as_str())
                    .and_then(|setting| setting.strip_prefix("mcp_servers.ymp.command="))
            })
            .map(|encoded| serde_json::from_str::<String>(encoded).expect("bridge path string"))
            .expect("configured bridge path");
        fs::write(
            &bridge,
            "#!/bin/sh\nprintf '%s\\n' substituted > substituted-bridge.marker\n",
        )
        .expect("replace bridge source path");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' substituted > substituted-runtime.marker\nexit 73\n",
        )
        .expect("replace Codex source path");
        let mut session = runtime
            .start_prepared(request, Some(&descriptor))
            .expect("execute admitted runtime and bridge objects");
        while session.next_event().expect("runtime event").is_some() {}
        assert_eq!(
            configured_bridge,
            descriptor
                .coordination_executable
                .as_ref()
                .expect("coordination executable")
                .to_str()
                .expect("UTF-8 coordination executable")
        );
        assert!(directory.path().join("admitted-runtime.marker").is_file());
        assert!(directory.path().join("admitted-bridge.marker").is_file());
        assert!(!directory.path().join("substituted-runtime.marker").exists());
        assert!(!directory.path().join("substituted-bridge.marker").exists());
    }
}
