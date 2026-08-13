#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
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
    BoundedOutputLine, CancellationToken, DiagnosticSummary, InFlightExcess, InvocationRequest,
    LaunchDescriptor, LaunchEnvironmentVariable, McpBinding, ProbeReport, Readiness, RuntimeDriver,
    RuntimeError, RuntimeEvent, RuntimeEventKind, RuntimeFailureKind, RuntimeKind, RuntimeSession,
    Usage, configure_process_group, create_launch_marker, evidence_digest, managed_launch_command,
    read_bounded_lines, register_launch_marker, terminate_process_tree,
};

pub const PINNED_CLAUDE_VERSION: &str = "2.1.227 (Claude Code)";
pub const PINNED_CLAUDE_MODEL: &str = "claude-opus-5";
pub const PINNED_CLAUDE_PROMPT_POLICY: &str = "ymp-claude-low-v1";
pub const PINNED_CLAUDE_API_PROVIDER: &str = "firstParty";

/// Owner gate `G3` approves one project-wide monetary bound and a permitted in-flight overshoot.
/// A profile that declares more than these ceilings is rejected before a run.
pub const G3_MAX_BUDGET_MICROUSD: u64 = 1_000_000;
pub const G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD: u64 = 50_000;

const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_WALL_TIME_LIMIT_MS: u64 = 10 * 60 * 1000;
const LAUNCH_DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
const MAX_CREDENTIAL_BYTES: usize = 1024 * 1024;
const HARNESS_INSTRUCTIONS: &str = "Execution policy: work without delegation, agents, skills, or background tasks. Do not send progress reports. Batch independent file reads and batch the final formatting, tests, lint, and diff checks. Use only the files and tools needed for the requested outcome. Do not commit. Stop immediately after a concise final report.";

/// Built-in tools this profile admits. `Task` and every other delegation tool are absent, so the
/// runtime cannot start a native subagent even though its agent definitions remain installed.
pub const APPROVED_BUILTIN_TOOLS: [&str; 6] = ["Bash", "Edit", "Glob", "Grep", "Read", "Write"];

/// Invocation-scoped coordination tools, named as Claude Code exposes MCP tools.
pub const COORDINATION_TOOLS: [&str; 4] = [
    "mcp__ymp__read_control",
    "mcp__ymp__read_events",
    "mcp__ymp__submit",
    "mcp__ymp__yield",
];

const DELEGATION_TOOLS: [&str; 6] = [
    "Task",
    "Agent",
    "SendMessage",
    "ListAgents",
    "Workflow",
    "RemoteTrigger",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeProfile {
    pub expected_version: String,
    pub model: String,
    pub effort: String,
    pub permission_mode: String,
    pub prompt_policy: String,
    /// Comma-separated `--setting-sources` value. Only the empty value is approved, because any
    /// source re-admits user, project, or local configuration into the managed invocation.
    pub setting_sources: String,
    pub input_format: String,
    pub output_format: String,
    pub builtin_tools: Vec<String>,
    pub native_subagents: bool,
    pub strict_mcp_config: bool,
    pub max_budget_microusd: u64,
    pub max_in_flight_overshoot_microusd: u64,
    pub output_limit_bytes: usize,
    pub wall_time_limit_ms: u64,
}

impl Default for ClaudeProfile {
    fn default() -> Self {
        Self {
            expected_version: PINNED_CLAUDE_VERSION.to_owned(),
            model: PINNED_CLAUDE_MODEL.to_owned(),
            effort: "low".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            prompt_policy: PINNED_CLAUDE_PROMPT_POLICY.to_owned(),
            setting_sources: String::new(),
            input_format: "text".to_owned(),
            output_format: "stream-json".to_owned(),
            builtin_tools: APPROVED_BUILTIN_TOOLS
                .iter()
                .map(|tool| (*tool).to_owned())
                .collect(),
            native_subagents: false,
            strict_mcp_config: true,
            max_budget_microusd: G3_MAX_BUDGET_MICROUSD,
            max_in_flight_overshoot_microusd: G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            wall_time_limit_ms: DEFAULT_WALL_TIME_LIMIT_MS,
        }
    }
}

impl ClaudeProfile {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.expected_version.trim().is_empty() {
            return Err(RuntimeError::InvalidProfile(
                "expected version must not be empty".to_owned(),
            ));
        }
        if self.model != PINNED_CLAUDE_MODEL {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude model {}",
                self.model
            )));
        }
        if self.effort != "low" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude effort {}",
                self.effort
            )));
        }
        if self.permission_mode != "acceptEdits" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude permission mode {}",
                self.permission_mode
            )));
        }
        if self.prompt_policy != PINNED_CLAUDE_PROMPT_POLICY {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude prompt policy {}",
                self.prompt_policy
            )));
        }
        if !self.setting_sources.is_empty() {
            return Err(RuntimeError::InvalidProfile(format!(
                "ambient Claude setting sources are not approved: {}",
                self.setting_sources
            )));
        }
        if !self.strict_mcp_config {
            return Err(RuntimeError::InvalidProfile(
                "ambient Claude MCP servers are not approved; strict MCP configuration is required"
                    .to_owned(),
            ));
        }
        if self.native_subagents {
            return Err(RuntimeError::InvalidProfile(
                "Claude native subagents are not approved for a managed invocation".to_owned(),
            ));
        }
        if self.input_format != "text" || self.output_format != "stream-json" {
            return Err(RuntimeError::InvalidProfile(format!(
                "incompatible Claude structured event interface {}/{}",
                self.input_format, self.output_format
            )));
        }
        for tool in &self.builtin_tools {
            if DELEGATION_TOOLS.contains(&tool.as_str()) {
                return Err(RuntimeError::InvalidProfile(format!(
                    "delegation tool {tool} enables Claude native subagents"
                )));
            }
            if !APPROVED_BUILTIN_TOOLS.contains(&tool.as_str()) {
                return Err(RuntimeError::InvalidProfile(format!(
                    "unapproved Claude built-in tool {tool}"
                )));
            }
        }
        if self.max_budget_microusd == 0 {
            return Err(RuntimeError::InvalidProfile(
                "Claude monetary bound must be positive so that overshoot is enforceable"
                    .to_owned(),
            ));
        }
        if self.max_budget_microusd > G3_MAX_BUDGET_MICROUSD {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude monetary bound {} exceeds the G3 ceiling {G3_MAX_BUDGET_MICROUSD}",
                self.max_budget_microusd
            )));
        }
        if self.max_in_flight_overshoot_microusd > G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude in-flight overshoot {} exceeds the G3 permitted overshoot {G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD}",
                self.max_in_flight_overshoot_microusd
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

    fn budget_argument(&self) -> String {
        format!("{:.6}", self.max_budget_microusd as f64 / 1_000_000.0)
    }

    fn enforced_cost_ceiling_microusd(&self) -> u64 {
        self.max_budget_microusd
            .saturating_add(self.max_in_flight_overshoot_microusd)
    }

    /// The build identifier Claude Code reports inside its `system`/`init` event, which omits the
    /// product suffix that `--version` prints.
    fn expected_build(&self) -> &str {
        self.expected_version
            .split_whitespace()
            .next()
            .unwrap_or(&self.expected_version)
    }
}

#[derive(Clone, Debug)]
pub struct ClaudeRuntime {
    executable: PathBuf,
    verified_executable: Arc<Mutex<Option<VerifiedExecutable>>>,
    profile: ClaudeProfile,
    credential: Option<CredentialSource>,
    runtime_path: OsString,
    prepared_launches: Arc<Mutex<HashMap<String, ClaudeLaunch>>>,
}

impl Default for ClaudeRuntime {
    fn default() -> Self {
        Self::new(pinned_executable_path())
    }
}

impl ClaudeRuntime {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        let executable = resolve_executable(executable.into());
        Self {
            executable,
            verified_executable: Arc::new(Mutex::new(None)),
            profile: ClaudeProfile::default(),
            credential: CredentialSource::discover(),
            runtime_path: std::env::var_os("PATH")
                .unwrap_or_else(|| OsString::from("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin")),
            prepared_launches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_profile(executable: impl Into<PathBuf>, profile: ClaudeProfile) -> Self {
        let mut runtime = Self::new(executable);
        runtime.profile = profile;
        runtime
    }

    pub fn profile(&self) -> &ClaudeProfile {
        &self.profile
    }

    /// Delegates no authentication material into the generated home. The managed invocation is
    /// then authenticated only if the executable itself is, which is how a fixture avoids reading
    /// the operator's credential and how the isolation of the generated home is falsified.
    pub fn without_delegated_credential(mut self) -> Self {
        self.credential = None;
        self
    }

    fn isolated_environment(&self) -> Result<ClaudeEnvironment, RuntimeError> {
        ClaudeEnvironment::create(
            self.credential.as_ref(),
            self.runtime_path.clone(),
            &self.profile,
        )
    }

    fn admitted_executable(&self) -> Result<VerifiedExecutable, RuntimeError> {
        let mut admitted = self.verified_executable.lock().map_err(|_| {
            RuntimeError::InvalidProfile("verified Claude executable lock failed".to_owned())
        })?;
        if let Some(executable) = admitted.as_ref() {
            return Ok(executable.clone());
        }
        let executable = VerifiedExecutable::admit(&self.executable, "claude")?;
        *admitted = Some(executable.clone());
        Ok(executable)
    }

    fn not_installed(&self) -> ProbeReport {
        ProbeReport {
            kind: RuntimeKind::ClaudeCode,
            executable: self.executable.display().to_string(),
            version: None,
            readiness: Readiness::NotInstalled,
            detail: "executable not found".to_owned(),
        }
    }
}

/// Authentication material for the managed invocation. Claude Code resolves its `claude.ai` OAuth
/// credential from the home directory, so a synthetic home is unauthenticated until the driver
/// delegates the operator's credential into it. This is deliberately a weaker, explicitly labelled
/// profile: `SECURITY.md` forbids treating a delegated refresh token as strict containment.
#[derive(Clone, Debug, Eq, PartialEq)]
enum CredentialSource {
    File(PathBuf),
    #[cfg(target_os = "macos")]
    MacosKeychain,
}

impl CredentialSource {
    fn discover() -> Option<Self> {
        let configuration = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")));
        if let Some(configuration) = configuration {
            let credentials = configuration.join(".credentials.json");
            if credentials.is_file() {
                return Some(Self::File(credentials));
            }
        }
        #[cfg(target_os = "macos")]
        {
            Some(Self::MacosKeychain)
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    fn read(&self) -> Result<Option<Vec<u8>>, RuntimeError> {
        let material = match self {
            Self::File(path) => fs::read(path)?,
            #[cfg(target_os = "macos")]
            Self::MacosKeychain => {
                let output = Command::new("/usr/bin/security")
                    .args([
                        "find-generic-password",
                        "-s",
                        "Claude Code-credentials",
                        "-w",
                    ])
                    .stdin(Stdio::null())
                    .stderr(Stdio::null())
                    .output()?;
                if !output.status.success() {
                    return Ok(None);
                }
                let mut material = output.stdout;
                while material.last().is_some_and(u8::is_ascii_whitespace) {
                    material.pop();
                }
                material
            }
        };
        if material.is_empty() {
            return Ok(None);
        }
        if material.len() > MAX_CREDENTIAL_BYTES {
            return Err(RuntimeError::InvalidProfile(
                "Claude authentication material exceeds its 1 MiB limit".to_owned(),
            ));
        }
        Ok(Some(material))
    }

    fn label(&self) -> &'static str {
        match self {
            Self::File(_) => "delegated_home_credential",
            #[cfg(target_os = "macos")]
            Self::MacosKeychain => "delegated_host_keychain_credential",
        }
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
struct ClaudeLaunch {
    executable: VerifiedExecutable,
    coordination_executable: Option<VerifiedExecutable>,
    profile: ClaudeProfile,
    workspace: PathBuf,
    attempt_id: String,
    invocation_id: String,
    mcp: Option<McpBinding>,
    environment: ClaudeEnvironment,
}

#[derive(Debug)]
struct ClaudeEnvironment {
    root: tempfile::TempDir,
    configuration: PathBuf,
    temporary: PathBuf,
    runtime_path: OsString,
    credential_label: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EnvironmentValue {
    name: String,
    value: String,
    confidential: bool,
}

impl ClaudeEnvironment {
    fn create(
        credential: Option<&CredentialSource>,
        runtime_path: OsString,
        profile: &ClaudeProfile,
    ) -> Result<Self, RuntimeError> {
        profile.validate()?;
        let root = tempfile::Builder::new()
            .prefix("ymp-claude-home-")
            .tempdir()?;
        let configuration = root.path().join(".claude");
        let temporary = root.path().join("tmp");
        fs::create_dir(&configuration)?;
        fs::create_dir(&temporary)?;
        set_private_directory_permissions(root.path())?;
        set_private_directory_permissions(&configuration)?;
        set_private_directory_permissions(&temporary)?;
        let mut credential_label = None;
        if let Some(credential) = credential
            && let Some(material) = credential.read()?
        {
            let destination = configuration.join(".credentials.json");
            fs::write(&destination, material)?;
            set_private_file_permissions(&destination)?;
            credential_label = Some(credential.label());
        }
        Ok(Self {
            root,
            configuration,
            temporary,
            runtime_path,
            credential_label,
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
            environment_value("CLAUDE_CONFIG_DIR", &self.configuration, false)?,
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
            plain_environment_value("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1", false),
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

struct ClaudeProcess {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: JoinHandle<Vec<u8>>,
    started_at: Instant,
}

impl ClaudeLaunch {
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
        let mut arguments = vec![
            "--print".to_owned(),
            "--input-format".to_owned(),
            self.profile.input_format.clone(),
            "--output-format".to_owned(),
            self.profile.output_format.clone(),
            "--verbose".to_owned(),
            "--setting-sources".to_owned(),
            self.profile.setting_sources.clone(),
            "--disable-slash-commands".to_owned(),
            "--strict-mcp-config".to_owned(),
            "--mcp-config".to_owned(),
            mcp_config(self.mcp.as_ref())?,
            "--model".to_owned(),
            self.profile.model.clone(),
            "--effort".to_owned(),
            self.profile.effort.clone(),
            "--permission-mode".to_owned(),
            self.profile.permission_mode.clone(),
            "--max-budget-usd".to_owned(),
            self.profile.budget_argument(),
            "--tools".to_owned(),
            self.profile.builtin_tools.join(","),
        ];
        if self.mcp.is_some() {
            arguments.push("--allowed-tools".to_owned());
            arguments.push(COORDINATION_TOOLS.join(","));
        }
        if let Some(session_id) = session_id {
            arguments.push("--resume".to_owned());
            arguments.push(session_id.to_owned());
        }
        Ok(arguments)
    }

    fn spawn(
        &self,
        descriptor: &LaunchDescriptor,
        session_id: Option<&str>,
        prompt: &str,
    ) -> Result<ClaudeProcess, RuntimeError> {
        let expected = self.descriptor(session_id)?;
        if descriptor != &expected {
            return Err(RuntimeError::InvalidProfile(
                "prepared Claude launch descriptor was modified".to_owned(),
            ));
        }
        if executable_digest(&descriptor.executable)? != descriptor.executable_digest {
            return Err(RuntimeError::InvalidProfile(
                "Claude executable changed after launch preparation".to_owned(),
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
        // Every descendant of the managed process inherits this marker, whatever becomes of the
        // processes between it and the run, so the run can still identify what it started.
        let marker = create_launch_marker()?;
        let mut command =
            managed_launch_command(&descriptor.executable, &descriptor.arguments, &marker);
        ClaudeEnvironment::apply(&mut command, &environment);
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
        // The managed process is live from here, so its wall-time budget and reported wall time
        // are measured from this point rather than from the end of the launch checks below.
        let started_at = Instant::now();
        // Claude Code abandons an unfed standard input a few seconds after start, so the prompt is
        // delivered before the post-launch digest recheck reads the admitted executables again.
        // The recheck below still terminates the process before any event, lifecycle command, or
        // usage record from a substituted executable can be accepted.
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Claude stdin was not piped".to_owned()))?;
        let delivered = (|| -> std::io::Result<()> {
            stdin.write_all(prompt.as_bytes())?;
            stdin.write_all(b"\n\n")?;
            stdin.write_all(HARNESS_INSTRUCTIONS.as_bytes())?;
            stdin.write_all(b"\n")?;
            stdin.flush()
        })();
        drop(stdin);
        if let Err(error) = delivered {
            let _ = terminate_process_tree(&mut child);
            return Err(RuntimeError::Process(error));
        }
        if executable_digest(&descriptor.executable)? != descriptor.executable_digest {
            let _ = terminate_process_tree(&mut child);
            return Err(RuntimeError::InvalidProfile(
                "Claude executable changed while the prepared launch was starting".to_owned(),
            ));
        }
        if let (Some(executable), Some(digest)) = (
            descriptor.coordination_executable.as_deref(),
            descriptor.coordination_executable_digest.as_deref(),
        ) && executable_digest(executable)? != digest
        {
            let _ = terminate_process_tree(&mut child);
            return Err(RuntimeError::InvalidProfile(
                "MCP executable changed while the prepared launch was starting".to_owned(),
            ));
        }
        let stdout = child.stdout.take().ok_or_else(|| {
            RuntimeError::MalformedEvent("Claude stdout was not piped".to_owned())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            RuntimeError::MalformedEvent("Claude stderr was not piped".to_owned())
        })?;
        let stderr_limit = self.profile.output_limit_bytes;
        let stderr_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr
                .take((stderr_limit.saturating_add(1)) as u64)
                .read_to_end(&mut bytes);
            bytes
        });
        Ok(ClaudeProcess {
            child,
            lines: read_bounded_lines(stdout, self.profile.output_limit_bytes),
            stderr_reader,
            started_at,
        })
    }
}

impl RuntimeDriver for ClaudeRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::ClaudeCode
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        self.profile.validate()?;
        if !self.executable.is_file() {
            return Ok(self.not_installed());
        }
        let environment = self.isolated_environment()?;
        let environment_values = environment.values(None, None, None)?;
        let admitted = self.admitted_executable()?;
        let mut version_command = Command::new(&admitted.execution_path);
        ClaudeEnvironment::apply(&mut version_command, &environment_values);
        let output = match version_command
            .arg("--version")
            .stdin(Stdio::null())
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(self.not_installed());
            }
            Err(error) => return Err(RuntimeError::Process(error)),
        };
        if !output.status.success() {
            return Ok(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
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
                kind: RuntimeKind::ClaudeCode,
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
        ClaudeEnvironment::apply(&mut auth_command, &environment_values);
        let auth = auth_command
            .args(["auth", "status"])
            .stdin(Stdio::null())
            .output()?;
        let status: Option<Value> = serde_json::from_slice(&auth.stdout).ok();
        let logged_in = status
            .as_ref()
            .and_then(|value| value.get("loggedIn").and_then(Value::as_bool))
            .unwrap_or(false);
        let api_provider = status
            .as_ref()
            .and_then(|value| value.get("apiProvider").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_owned();
        let auth_method = status
            .as_ref()
            .and_then(|value| value.get("authMethod").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_owned();
        if !auth.status.success() || !logged_in {
            return Ok(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
                executable: self.executable.display().to_string(),
                version: Some(version),
                readiness: Readiness::Unauthenticated,
                detail: "Claude Code authentication is unavailable in the generated home"
                    .to_owned(),
            });
        }
        if api_provider != PINNED_CLAUDE_API_PROVIDER {
            return Ok(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
                executable: self.executable.display().to_string(),
                version: Some(version),
                readiness: Readiness::Incompatible,
                detail: format!(
                    "profile requires the {PINNED_CLAUDE_API_PROVIDER} model route, found {api_provider}"
                ),
            });
        }
        Ok(ProbeReport {
            kind: RuntimeKind::ClaudeCode,
            executable: self.executable.display().to_string(),
            version: Some(version),
            readiness: Readiness::Ready,
            detail: format!(
                "pinned local profile ready: model={}, api_provider={api_provider}, auth_method={auth_method}, credential={}, effort={}, prompt_policy={}, permission_mode={}, builtin_tools={}, coordination_tools={}, native_subagents=disabled, setting_sources=none, strict_mcp_config=true, max_budget_usd={}, permitted_overshoot_microusd={}, wall_time_limit_ms={}, output_limit_bytes={}",
                self.profile.model,
                environment.credential_label.unwrap_or("none"),
                self.profile.effort,
                self.profile.prompt_policy,
                self.profile.permission_mode,
                self.profile.builtin_tools.join("+"),
                COORDINATION_TOOLS.join("+"),
                self.profile.budget_argument(),
                self.profile.max_in_flight_overshoot_microusd,
                self.profile.wall_time_limit_ms,
                self.profile.output_limit_bytes
            ),
        })
    }

    fn start(&self, request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        let readiness = self.probe()?;
        if readiness.readiness != Readiness::Ready {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude profile is not ready: {}",
                readiness.detail
            )));
        }
        let descriptor = self.prepare_launch(&request)?.ok_or_else(|| {
            RuntimeError::InvalidProfile("Claude launch descriptor was not prepared".to_owned())
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
        let launch = ClaudeLaunch {
            executable,
            coordination_executable,
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
                "duplicate prepared Claude invocation identifier".to_owned(),
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
            RuntimeError::InvalidProfile("Claude requires a prepared launch descriptor".to_owned())
        })?;
        let launch = self
            .prepared_launches
            .lock()
            .map_err(|_| RuntimeError::InvalidProfile("prepared launch lock failed".to_owned()))?
            .remove(&request.invocation_id)
            .ok_or_else(|| {
                RuntimeError::InvalidProfile(
                    "prepared Claude invocation is missing or already consumed".to_owned(),
                )
            })?;
        if descriptor.invocation_id != request.invocation_id
            || descriptor.attempt_id != request.attempt_id
            || descriptor.working_directory != request.workspace
        {
            return Err(RuntimeError::InvalidProfile(
                "prepared Claude launch identity does not match the invocation request".to_owned(),
            ));
        }
        let process = launch.spawn(descriptor, None, &request.prompt)?;
        let mut pending_events = VecDeque::new();
        pending_events.push_back(RuntimeEventKind::Launch {
            descriptor: Box::new(descriptor.clone()),
        });
        Ok(Box::new(ClaudeSession {
            child: process.child,
            lines: process.lines,
            stderr_reader: Some(process.stderr_reader),
            coordinated: launch.mcp.is_some(),
            launch,
            session_id: None,
            invocation_id: request.invocation_id,
            sequence: 0,
            output_limit_bytes: self.profile.output_limit_bytes,
            wall_time_limit_ms: self.profile.wall_time_limit_ms,
            cost_ceiling_microusd: self.profile.enforced_cost_ceiling_microusd(),
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
            in_flight: InFlightExcess::default(),
            observed_requests: HashSet::new(),
            accounted_requests: 0,
            pending_tool_calls: HashMap::new(),
            answered_tool_calls: HashSet::new(),
            turn_reported: false,
            yielded: false,
            failure_emitted: false,
        }))
    }
}

struct PendingToolCall {
    tool: String,
    arguments: Value,
}

struct ClaudeSession {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: Option<JoinHandle<Vec<u8>>>,
    coordinated: bool,
    launch: ClaudeLaunch,
    session_id: Option<String>,
    invocation_id: String,
    sequence: u64,
    output_limit_bytes: usize,
    wall_time_limit_ms: u64,
    cost_ceiling_microusd: u64,
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
    in_flight: InFlightExcess,
    observed_requests: HashSet<String>,
    accounted_requests: u64,
    pending_tool_calls: HashMap<String, PendingToolCall>,
    answered_tool_calls: HashSet<String>,
    turn_reported: bool,
    yielded: bool,
    failure_emitted: bool,
}

impl ClaudeSession {
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

    fn install_process(&mut self, process: ClaudeProcess, descriptor: LaunchDescriptor) {
        self.child = process.child;
        self.lines = process.lines;
        self.stderr_reader = Some(process.stderr_reader);
        self.started_at = process.started_at;
        self.completed = false;
        self.recoverable = false;
        self.native_resume_started = true;
        self.yielded = false;
        self.turn_reported = false;
        self.pending_tool_calls.clear();
        self.pending_events.push_back(RuntimeEventKind::Launch {
            descriptor: Box::new(descriptor),
        });
    }

    /// Model requests observed on the transcript that no terminal accounting record has yet
    /// covered, plus any monetary consumption above the profile's enforced ceiling. Both survive
    /// every terminal outcome; neither is invented when the runtime reported complete accounting.
    fn usage_snapshot(&self) -> Usage {
        let mut usage = self.usage.clone();
        usage.wall_time_ms = elapsed_millis(self.session_started_at);
        usage.in_flight_excess = InFlightExcess {
            model_requests: (self.observed_requests.len() as u64)
                .saturating_sub(self.accounted_requests),
            ..self.in_flight.clone()
        };
        usage
    }

    fn observe_request(&mut self, event: &Value) {
        if let Some(request_id) = event.get("request_id").and_then(Value::as_str) {
            self.observed_requests.insert(request_id.to_owned());
        }
    }

    fn merge_reported_usage(&mut self, event: &Value) -> Result<(), RuntimeError> {
        let reported = event.get("usage").ok_or_else(|| {
            RuntimeError::MalformedEvent("Claude result carries no usage evidence".to_owned())
        })?;
        let input_tokens = u64_field(reported, "input_tokens")?
            .saturating_add(optional_u64_field(reported, "cache_creation_input_tokens"));
        let observed = Usage {
            input_tokens,
            cached_input_tokens: optional_u64_field(reported, "cache_read_input_tokens"),
            output_tokens: u64_field(reported, "output_tokens")?,
            reasoning_output_tokens: reported
                .pointer("/output_tokens_details/thinking_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cost_microusd: event
                .get("total_cost_usd")
                .and_then(Value::as_f64)
                .map(usd_to_microusd),
            wall_time_ms: 0,
            protected_queries: 0,
            in_flight_excess: InFlightExcess::default(),
        };
        add_usage(&mut self.usage, &observed);
        self.accounted_requests = self.observed_requests.len() as u64;
        let spent = self.usage.cost_microusd.unwrap_or_default();
        self.in_flight.cost_microusd = spent.saturating_sub(self.cost_ceiling_microusd);
        self.turn_reported = true;
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
                RuntimeError::Process(_) | RuntimeError::SanitizedUnsuccessfulExit { .. }
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

    /// Rejects an invocation whose effective configuration differs from the declared profile.
    /// Claude Code reports its resolved build, model, permission mode, tool set, coordination
    /// servers and every ambient extension in `system`/`init`, which precedes model traffic.
    fn admit_session(&mut self, event: &Value) -> Result<String, RuntimeError> {
        let build = string_field(event, "claude_code_version")?;
        if build != self.launch.profile.expected_build() {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude reported build {build}, profile requires {}",
                self.launch.profile.expected_build()
            )));
        }
        let model = string_field(event, "model")?;
        if model != self.launch.profile.model {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude resolved model {model}, profile requires {}",
                self.launch.profile.model
            )));
        }
        let permission_mode = string_field(event, "permissionMode")?;
        if permission_mode != self.launch.profile.permission_mode {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude resolved permission mode {permission_mode}, profile requires {}",
                self.launch.profile.permission_mode
            )));
        }
        for ambient in ["slash_commands", "plugins", "skills"] {
            let entries = array_field(event, ambient)?;
            if !entries.is_empty() {
                return Err(RuntimeError::InvalidProfile(format!(
                    "ambient Claude {ambient} reached the managed invocation"
                )));
            }
        }
        let mut expected_tools: Vec<String> = self.launch.profile.builtin_tools.clone();
        if self.coordinated {
            expected_tools.extend(COORDINATION_TOOLS.iter().map(|tool| (*tool).to_owned()));
        }
        expected_tools.sort();
        let mut observed_tools: Vec<String> = array_field(event, "tools")?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        observed_tools.sort();
        if let Some(delegation) = observed_tools
            .iter()
            .find(|tool| DELEGATION_TOOLS.contains(&tool.as_str()))
        {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude enabled the delegation tool {delegation}"
            )));
        }
        if observed_tools != expected_tools {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude resolved tools {}, profile requires {}",
                observed_tools.join("+"),
                expected_tools.join("+")
            )));
        }
        let servers = array_field(event, "mcp_servers")?;
        let expected_servers = usize::from(self.coordinated);
        if servers.len() != expected_servers {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude resolved {} MCP servers, profile requires {expected_servers}",
                servers.len()
            )));
        }
        for server in servers {
            let name = string_field(server, "name")?;
            let status = string_field(server, "status")?;
            if name != "ymp" || status != "connected" {
                return Err(RuntimeError::InvalidProfile(format!(
                    "unapproved Claude MCP server {name} in status {status}"
                )));
            }
        }
        string_field(event, "session_id")
    }

    fn parse_assistant(&mut self, event: &Value) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if event
            .get("parent_tool_use_id")
            .is_some_and(|value| !value.is_null())
        {
            return Err(RuntimeError::InvalidProfile(
                "Claude emitted native subagent output in a managed invocation".to_owned(),
            ));
        }
        self.observe_request(event);
        let content = event
            .pointer("/message/content")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::MalformedEvent(
                    "Claude assistant event has no message content".to_owned(),
                )
            })?
            .clone();
        let mut text = Vec::new();
        for block in &content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(value) = block.get("text").and_then(Value::as_str) {
                        text.push(value.to_owned());
                    }
                }
                Some("tool_use") => {
                    let name = string_field(block, "name")?;
                    if DELEGATION_TOOLS.contains(&name.as_str()) {
                        return Err(RuntimeError::InvalidProfile(format!(
                            "Claude called the delegation tool {name}"
                        )));
                    }
                    if let Some(tool) = name.strip_prefix("mcp__ymp__") {
                        if !COORDINATION_TOOLS.contains(&name.as_str()) {
                            return Err(RuntimeError::MalformedEvent(format!(
                                "Claude called the unbound coordination tool {name}"
                            )));
                        }
                        let id = string_field(block, "id")?;
                        if self.answered_tool_calls.contains(&id)
                            || self
                                .pending_tool_calls
                                .insert(
                                    id.clone(),
                                    PendingToolCall {
                                        tool: tool.to_owned(),
                                        arguments: block
                                            .get("input")
                                            .cloned()
                                            .unwrap_or(Value::Null),
                                    },
                                )
                                .is_some()
                        {
                            return Err(RuntimeError::MalformedEvent(format!(
                                "Claude repeated the coordination tool call identifier {id}"
                            )));
                        }
                    }
                }
                _ => {}
            }
        }
        let text = text.join("\n");
        if text.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.emit(RuntimeEventKind::Output { text })))
    }

    fn parse_user(&mut self, event: &Value) -> Result<Option<RuntimeEvent>, RuntimeError> {
        let Some(content) = event.pointer("/message/content").and_then(Value::as_array) else {
            return Ok(None);
        };
        let mut replies = Vec::new();
        for block in content.clone() {
            if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            let id = string_field(&block, "tool_use_id")?;
            let Some(call) = self.pending_tool_calls.remove(&id) else {
                if self.answered_tool_calls.contains(&id) {
                    return Err(RuntimeError::MalformedEvent(format!(
                        "Claude repeated the coordination reply for {id}"
                    )));
                }
                continue;
            };
            self.answered_tool_calls.insert(id);
            let failed = block
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let payload = block.get("content").cloned().unwrap_or(Value::Null);
            let (result, error) = if failed {
                (None, Some(payload))
            } else {
                (Some(payload), None)
            };
            replies.push(RuntimeEventKind::McpToolCall {
                server: "ymp".to_owned(),
                tool: call.tool,
                status: if failed { "failed" } else { "completed" }.to_owned(),
                arguments: call.arguments,
                result,
                error,
            });
        }
        // One message may answer several coordination calls. Every reply becomes its own event, in
        // the order the message carried them, so none is dropped and none is merged into another.
        let mut replies = replies.into_iter();
        let Some(first) = replies.next() else {
            return Ok(None);
        };
        self.pending_events.extend(replies);
        Ok(Some(self.emit(first)))
    }

    /// Reads whatever the exited process left on its transcript. One turn produces exactly one
    /// session record and one terminal record; a repeat of either is a duplicate structured event
    /// rather than additional progress, so it is refused instead of being read as a second turn.
    fn duplicated_terminal_record(&mut self) -> Option<&'static str> {
        while let Ok(BoundedOutputLine::Line(line)) = self.lines.try_recv() {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match event.get("type").and_then(Value::as_str) {
                Some("result") => return Some("terminal result"),
                Some("system") if event.get("subtype").and_then(Value::as_str) == Some("init") => {
                    return Some("session record");
                }
                _ => {}
            }
        }
        None
    }

    fn parse_result(&mut self, event: &Value) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.turn_reported {
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
            return Err(RuntimeError::MalformedEvent(
                "Claude reported a second terminal result for one turn".to_owned(),
            ));
        }
        self.merge_reported_usage(event)?;
        let subtype = string_field(event, "subtype")?;
        let failed = event.get("is_error").and_then(Value::as_bool) == Some(true)
            || subtype != "success"
            || !self.pending_tool_calls.is_empty();
        if failed {
            let terminal_reason = event
                .get("terminal_reason")
                .and_then(Value::as_str)
                .unwrap_or("unreported");
            let diagnostic = DiagnosticSummary::from_bytes(
                format!("subtype={subtype}, terminal_reason={terminal_reason}").as_bytes(),
                false,
            );
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
            return Ok(Some(self.failed_event(
                RuntimeFailureKind::RuntimeReported,
                Some(diagnostic),
            )));
        }
        let spent = self.usage.cost_microusd.unwrap_or_default();
        if spent > self.cost_ceiling_microusd {
            let diagnostic = DiagnosticSummary::from_bytes(
                format!(
                    "cost_microusd={spent}, enforced_ceiling_microusd={}",
                    self.cost_ceiling_microusd
                )
                .as_bytes(),
                false,
            );
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
            return Ok(Some(
                self.failed_event(RuntimeFailureKind::Protocol, Some(diagnostic)),
            ));
        }
        let status = self.finish()?;
        if !status.success() {
            return Err(self.unsuccessful(status));
        }
        if self.session_id.is_none() {
            return Err(RuntimeError::MalformedEvent(
                "Claude completed a turn before reporting its session".to_owned(),
            ));
        }
        if let Some(duplicate) = self.duplicated_terminal_record() {
            let diagnostic = DiagnosticSummary::from_bytes(
                format!("duplicate_record={duplicate}").as_bytes(),
                false,
            );
            return Ok(Some(
                self.failed_event(RuntimeFailureKind::Protocol, Some(diagnostic)),
            ));
        }
        self.yielded = true;
        self.recoverable = true;
        Ok(Some(self.emit(RuntimeEventKind::Completed {
            usage: self.usage_snapshot(),
        })))
    }

    fn parse_event(&mut self, event: Value) -> Result<Option<RuntimeEvent>, RuntimeError> {
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::MalformedEvent("event has no type".to_owned()))?;
        match event_type {
            "system" if event.get("subtype").and_then(Value::as_str) == Some("init") => {
                let session_id = match self.admit_session(&event) {
                    Ok(session_id) => session_id,
                    Err(error) => {
                        let _ = terminate_process_tree(&mut self.child);
                        self.completed = true;
                        self.terminal = true;
                        return Err(error);
                    }
                };
                if let Some(expected) = &self.session_id {
                    if expected != &session_id {
                        let _ = terminate_process_tree(&mut self.child);
                        self.completed = true;
                        self.terminal = true;
                        return Err(RuntimeError::InvalidProfile(format!(
                            "resumed Claude session identifier changed from {expected} to {session_id}"
                        )));
                    }
                } else {
                    self.session_id = Some(session_id.clone());
                }
                Ok(Some(self.emit(RuntimeEventKind::Started {
                    opaque_session_id: session_id,
                })))
            }
            "system"
                if event.get("subtype").and_then(Value::as_str) == Some("permission_denied") =>
            {
                let tool = string_field(&event, "tool_name")?;
                if tool.starts_with("mcp__ymp__") {
                    let _ = terminate_process_tree(&mut self.child);
                    self.completed = true;
                    self.terminal = true;
                    return Err(RuntimeError::InvalidProfile(format!(
                        "the managed binding refused the coordination tool {tool}"
                    )));
                }
                Ok(None)
            }
            "system" => Ok(None),
            "assistant" => self.parse_assistant(&event),
            "user" => self.parse_user(&event),
            "result" => self.parse_result(&event),
            "stream_event" | "rate_limit_event" => Ok(None),
            other => Err(RuntimeError::MalformedEvent(format!(
                "unsupported Claude event type {other}"
            ))),
        }
    }

    fn next_event_inner(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.cancellation.is_cancelled() && !self.terminal && !self.interrupted {
            if !self.completed {
                let _ = terminate_process_tree(&mut self.child);
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
                        "Claude exited without a result event".to_owned(),
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

impl RuntimeSession for ClaudeSession {
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
                "managed Claude session identifier is unknown; refusing replacement start"
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

impl Drop for ClaudeSession {
    fn drop(&mut self) {
        if !self.completed {
            let _ = terminate_process_tree(&mut self.child);
        }
    }
}

/// Resolves the exact approved build. Claude Code installs every build under a version-named path,
/// so the pinned build is addressed directly instead of trusting whichever build the `claude` name
/// currently points at. The probe still compares the reported version, so this resolution can never
/// admit a different build.
fn pinned_executable_path() -> PathBuf {
    let build = PINNED_CLAUDE_VERSION
        .split_whitespace()
        .next()
        .unwrap_or(PINNED_CLAUDE_VERSION);
    if let Some(home) = std::env::var_os("HOME") {
        let pinned = PathBuf::from(home)
            .join(".local/share/claude/versions")
            .join(build);
        if pinned.is_file() {
            return pinned;
        }
    }
    PathBuf::from("claude")
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

fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| RuntimeError::MalformedEvent(format!("missing array field {field}")))
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

fn usd_to_microusd(cost: f64) -> u64 {
    if cost.is_finite() && cost > 0.0 {
        (cost * 1_000_000.0).round() as u64
    } else {
        0
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
            "Claude executable is not a regular file: {}",
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

fn mcp_config(binding: Option<&McpBinding>) -> Result<String, RuntimeError> {
    let servers = if let Some(binding) = binding {
        binding.validate()?;
        let executable = binding.executable.to_str().ok_or_else(|| {
            RuntimeError::InvalidProfile("MCP executable path must be UTF-8".to_owned())
        })?;
        serde_json::json!({
            "ymp": {
                "command": executable,
                "args": ["internal", "agent-mcp"]
            }
        })
    } else {
        serde_json::json!({})
    };
    serde_json::to_string(&serde_json::json!({ "mcpServers": servers }))
        .map_err(|error| RuntimeError::InvalidProfile(error.to_string()))
}

#[cfg(all(test, unix))]
mod tests {
    use super::{
        APPROVED_BUILTIN_TOOLS, ClaudeProfile, ClaudeRuntime, G3_MAX_BUDGET_MICROUSD,
        G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD, PINNED_CLAUDE_MODEL,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use ymp_runtime_api::{
        InvocationRequest, McpBinding, Readiness, RuntimeDriver, RuntimeError, RuntimeEventKind,
        RuntimeFailureKind, RuntimeSession,
    };

    const INIT_TOOLS: &str = r#"["Bash","Edit","Glob","Grep","Read","Write"]"#;

    fn fixture(directory: &Path, name: &str, body: &str) -> PathBuf {
        let path = directory.join(name);
        fs::write(
            &path,
            format!(
                r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
  exit 0
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}}'
  exit 0
fi
{body}
"##
            ),
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&path, permissions).expect("make executable");
        path
    }

    fn request(directory: &Path, invocation: &str) -> InvocationRequest {
        InvocationRequest {
            invocation_id: invocation.to_owned(),
            attempt_id: format!("attempt-{invocation}"),
            workspace: directory.to_owned(),
            mcp: None,
            prompt: "fixture".to_owned(),
            cancellation: Default::default(),
        }
    }

    fn start(
        executable: &Path,
        directory: &Path,
        invocation: &str,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        ClaudeRuntime::new(executable)
            .without_delegated_credential()
            .start(request(directory, invocation))
    }

    fn expect_launch(session: &mut dyn RuntimeSession) {
        assert!(matches!(
            session.next_event().expect("launch").expect("event").event,
            RuntimeEventKind::Launch { .. }
        ));
    }

    #[test]
    fn profile_rejects_ambient_configuration_subagents_events_usage_and_overshoot() {
        for (label, profile) in [
            (
                "ambient setting sources",
                ClaudeProfile {
                    setting_sources: "user,project".to_owned(),
                    ..ClaudeProfile::default()
                },
            ),
            (
                "ambient MCP servers",
                ClaudeProfile {
                    strict_mcp_config: false,
                    ..ClaudeProfile::default()
                },
            ),
            (
                "enabled native subagents",
                ClaudeProfile {
                    native_subagents: true,
                    ..ClaudeProfile::default()
                },
            ),
            (
                "delegation tool",
                ClaudeProfile {
                    builtin_tools: vec!["Read".to_owned(), "Task".to_owned()],
                    ..ClaudeProfile::default()
                },
            ),
            (
                "incompatible structured events",
                ClaudeProfile {
                    output_format: "json".to_owned(),
                    ..ClaudeProfile::default()
                },
            ),
            (
                "usage-free structured events",
                ClaudeProfile {
                    input_format: "stream-json".to_owned(),
                    output_format: "text".to_owned(),
                    ..ClaudeProfile::default()
                },
            ),
            (
                "unenforceable monetary bound",
                ClaudeProfile {
                    max_budget_microusd: 0,
                    ..ClaudeProfile::default()
                },
            ),
            (
                "monetary bound beyond G3",
                ClaudeProfile {
                    max_budget_microusd: G3_MAX_BUDGET_MICROUSD + 1,
                    ..ClaudeProfile::default()
                },
            ),
            (
                "overshoot beyond G3",
                ClaudeProfile {
                    max_in_flight_overshoot_microusd: G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD + 1,
                    ..ClaudeProfile::default()
                },
            ),
            (
                "unapproved model",
                ClaudeProfile {
                    model: "claude-unapproved".to_owned(),
                    ..ClaudeProfile::default()
                },
            ),
            (
                "unapproved effort",
                ClaudeProfile {
                    effort: "high".to_owned(),
                    ..ClaudeProfile::default()
                },
            ),
        ] {
            assert!(
                matches!(profile.validate(), Err(RuntimeError::InvalidProfile(_))),
                "{label} was admitted"
            );
        }
        assert!(ClaudeProfile::default().validate().is_ok());
    }

    #[test]
    fn rejected_profile_never_starts_a_process() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-never-started",
            "touch started.marker",
        );
        let runtime = ClaudeRuntime::with_profile(
            &executable,
            ClaudeProfile {
                native_subagents: true,
                ..ClaudeProfile::default()
            },
        )
        .without_delegated_credential();
        assert!(matches!(
            runtime.probe(),
            Err(RuntimeError::InvalidProfile(_))
        ));
        assert!(matches!(
            runtime.start(request(directory.path(), "invocation-rejected")),
            Err(RuntimeError::InvalidProfile(_))
        ));
        assert!(!directory.path().join("started.marker").exists());
    }

    #[test]
    fn default_runtime_exposes_the_pinned_profile() {
        let profile = ClaudeProfile::default();
        assert_eq!(profile.model, PINNED_CLAUDE_MODEL);
        assert_eq!(profile.effort, "low");
        assert_eq!(profile.permission_mode, "acceptEdits");
        assert_eq!(profile.builtin_tools, APPROVED_BUILTIN_TOOLS);
        assert!(profile.setting_sources.is_empty());
        assert!(profile.strict_mcp_config);
        assert!(!profile.native_subagents);
    }

    #[test]
    fn structured_stream_preserves_session_output_usage_and_cost() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-success",
            &format!(
                r##"cat >/dev/null
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-1","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"assistant","request_id":"req_1","parent_tool_use_id":null,"message":{{"content":[{{"type":"text","text":"done"}}]}}}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.125,"usage":{{"input_tokens":11,"cache_creation_input_tokens":100,"cache_read_input_tokens":7,"output_tokens":3,"output_tokens_details":{{"thinking_tokens":2}}}}}}'
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-success").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { opaque_session_id } if opaque_session_id == "session-1"
        ));
        assert!(matches!(
            session.next_event().expect("output").expect("event").event,
            RuntimeEventKind::Output { text } if text == "done"
        ));
        let completed = session.next_event().expect("completed").expect("event");
        let RuntimeEventKind::Completed { usage } = completed.event else {
            panic!("expected a completed event");
        };
        assert_eq!(usage.input_tokens, 111);
        assert_eq!(usage.cached_input_tokens, 7);
        assert_eq!(usage.output_tokens, 3);
        assert_eq!(usage.reasoning_output_tokens, 2);
        assert_eq!(usage.cost_microusd, Some(125_000));
        assert_eq!(usage.in_flight_excess.model_requests, 0);
        assert_eq!(usage.in_flight_excess.cost_microusd, 0);
    }

    #[test]
    fn budget_stop_preserves_reported_usage_and_unaccounted_requests() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-budget-stop",
            &format!(
                r##"cat >/dev/null
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-budget","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"assistant","request_id":"req_1","message":{{"content":[{{"type":"text","text":"working"}}]}}}}'
printf '%s\n' '{{"type":"result","subtype":"error_max_budget_usd","is_error":true,"terminal_reason":"budget_exhausted","total_cost_usd":0.9,"usage":{{"input_tokens":41,"cache_creation_input_tokens":9,"cache_read_input_tokens":5,"output_tokens":7,"output_tokens_details":{{"thinking_tokens":1}}}}}}'
exit 1
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-budget").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        assert!(matches!(
            session.next_event().expect("output").expect("event").event,
            RuntimeEventKind::Output { .. }
        ));
        let failed = session.next_event().expect("failed").expect("event");
        let RuntimeEventKind::Failed { kind, usage, .. } = failed.event else {
            panic!("expected a typed failure");
        };
        assert_eq!(kind, RuntimeFailureKind::RuntimeReported);
        assert_eq!(usage.input_tokens, 50);
        assert_eq!(usage.cached_input_tokens, 5);
        assert_eq!(usage.output_tokens, 7);
        assert_eq!(usage.cost_microusd, Some(900_000));
        assert!(usage.wall_time_ms > 0);
    }

    #[test]
    fn cost_beyond_the_enforced_ceiling_is_a_typed_failure() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-unbounded-cost",
            &format!(
                r##"cat >/dev/null
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-cost","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":4.0,"usage":{{"input_tokens":1,"output_tokens":1}}}}'
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-cost").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        let failed = session.next_event().expect("failed").expect("event");
        let RuntimeEventKind::Failed { kind, usage, .. } = failed.event else {
            panic!("expected a typed failure for an unenforced bound");
        };
        assert_eq!(kind, RuntimeFailureKind::Protocol);
        assert_eq!(usage.cost_microusd, Some(4_000_000));
        assert_eq!(
            usage.in_flight_excess.cost_microusd,
            4_000_000 - (G3_MAX_BUDGET_MICROUSD + G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD)
        );
    }

    #[test]
    fn a_missing_usage_report_is_a_typed_protocol_failure() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-no-usage",
            &format!(
                r##"cat >/dev/null
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-no-usage","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.1}}'
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-no-usage").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("typed failure")
                .expect("event")
                .event,
            RuntimeEventKind::Failed {
                kind: RuntimeFailureKind::Protocol,
                ..
            }
        ));
    }

    #[test]
    fn duplicate_terminal_result_is_a_typed_failure() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-duplicate-result",
            &format!(
                r##"cat >/dev/null
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-duplicate","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"usage":{{"input_tokens":1,"output_tokens":1}}}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"usage":{{"input_tokens":1,"output_tokens":1}}}}'
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-duplicate").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("typed failure")
                .expect("event")
                .event,
            RuntimeEventKind::Failed {
                kind: RuntimeFailureKind::Protocol,
                ..
            }
        ));
        assert!(matches!(
            session.resume("continue".to_owned()),
            Err(RuntimeError::NotYielded)
        ));
    }

    #[test]
    fn a_repeated_coordination_reply_is_a_typed_binding_failure() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let socket = directory.path().join("agent.sock");
        let bridge = fixture(directory.path(), "claude-bridge", "exit 0");
        let executable = fixture(
            directory.path(),
            "claude-repeated-reply",
            r##"cat >/dev/null
printf '%s\n' '{"type":"system","subtype":"init","session_id":"session-reply","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write","mcp__ymp__read_control","mcp__ymp__read_events","mcp__ymp__submit","mcp__ymp__yield"],"mcp_servers":[{"name":"ymp","status":"connected"}],"slash_commands":[],"plugins":[],"skills":[]}'
printf '%s\n' '{"type":"assistant","request_id":"req_1","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"mcp__ymp__submit","input":{"command_id":"agent.submit"}}]}}'
printf '%s\n' '{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":"{\"snapshot_digest\":\"a\"}"}]}]}}'
printf '%s\n' '{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":"{\"snapshot_digest\":\"a\"}"}]}]}}'
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"usage":{"input_tokens":1,"output_tokens":1}}'
"##,
        );
        let mut session = ClaudeRuntime::new(&executable)
            .without_delegated_credential()
            .start(InvocationRequest {
                invocation_id: "invocation-reply".to_owned(),
                attempt_id: "attempt-reply".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: Some(McpBinding {
                    executable: bridge,
                    socket_path: socket,
                    token: "fixture-token".to_owned(),
                }),
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        let call = session.next_event().expect("tool call").expect("event");
        assert!(matches!(
            call.event,
            RuntimeEventKind::McpToolCall { ref tool, ref status, .. }
                if tool == "submit" && status == "completed"
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("typed failure")
                .expect("event")
                .event,
            RuntimeEventKind::Failed {
                kind: RuntimeFailureKind::Protocol,
                ..
            }
        ));
    }

    #[test]
    fn an_ambient_or_delegating_session_is_rejected_before_model_use() {
        for (label, init) in [
            (
                "ambient plugins",
                r#"{"type":"system","subtype":"init","session_id":"s","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write"],"mcp_servers":[],"slash_commands":[],"plugins":["ambient"],"skills":[]}"#,
            ),
            (
                "ambient MCP server",
                r#"{"type":"system","subtype":"init","session_id":"s","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write"],"mcp_servers":[{"name":"other","status":"connected"}],"slash_commands":[],"plugins":[],"skills":[]}"#,
            ),
            (
                "native subagent tool",
                r#"{"type":"system","subtype":"init","session_id":"s","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write","Task"],"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}"#,
            ),
            (
                "substituted model",
                r#"{"type":"system","subtype":"init","session_id":"s","claude_code_version":"2.1.227","model":"claude-sonnet-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write"],"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}"#,
            ),
            (
                "substituted build",
                r#"{"type":"system","subtype":"init","session_id":"s","claude_code_version":"2.1.229","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write"],"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}"#,
            ),
            (
                "widened permission mode",
                r#"{"type":"system","subtype":"init","session_id":"s","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"bypassPermissions","tools":["Bash","Edit","Glob","Grep","Read","Write"],"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}"#,
            ),
        ] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let executable = fixture(
                directory.path(),
                "claude-ambient",
                &format!(
                    r##"cat >/dev/null
printf '%s\n' '{init}'
printf '%s\n' 'model-traffic-started' > model.traffic
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.5,"usage":{{"input_tokens":1,"output_tokens":1}}}}'
"##
                ),
            );
            let mut session =
                start(&executable, directory.path(), "invocation-ambient").expect("start fixture");
            expect_launch(session.as_mut());
            let failure = session.next_event().expect("typed failure").expect("event");
            assert!(
                matches!(
                    failure.event,
                    RuntimeEventKind::Failed {
                        kind: RuntimeFailureKind::Protocol,
                        ..
                    }
                ),
                "{label} was admitted"
            );
        }
    }

    #[test]
    fn a_descendant_does_not_survive_a_failed_parent() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-descendant",
            &format!(
                r##"cat >/dev/null
# The descendant releases the inherited transcript pipes, so its survival is observable
# independently of how long the supervisor waits for end of output.
sleep 300 >/dev/null 2>&1 &
printf '%s\n' "$!" > descendant.pid
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-descendant","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
exit 19
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-descendant").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::SanitizedUnsuccessfulExit { .. })
        ));
        drop(session);
        let descendant = fs::read_to_string(directory.path().join("descendant.pid"))
            .expect("descendant identifier");
        let descendant = descendant.trim().to_owned();
        let mut alive = true;
        for _ in 0..40 {
            alive = std::process::Command::new("/bin/kill")
                .args(["-0", &descendant])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(!alive, "a descendant survived its failed managed parent");
    }

    #[test]
    fn recoverable_process_failure_resumes_the_same_managed_session() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-resume",
            &format!(
                r##"count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
printf '%s\n' "$count" > invocation.count
printf '%s\n' "$@" > "invocation-$count.args"
cat > "invocation-$count.stdin"
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-resume","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
if [ "$count" -eq 1 ]; then
  printf '%s\n' 'committed progress' > committed.progress
  exit 17
fi
test "$(cat committed.progress)" = 'committed progress' || exit 29
printf '%s\n' '{{"type":"assistant","request_id":"req_2","message":{{"content":[{{"type":"text","text":"resumed progress"}}]}}}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.125,"usage":{{"input_tokens":11,"cache_read_input_tokens":7,"output_tokens":3}}}}'
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-resume").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { opaque_session_id } if opaque_session_id == "session-resume"
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::SanitizedUnsuccessfulExit { .. })
        ));
        session
            .resume("continue after interruption".to_owned())
            .expect("resume fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("resumed start").expect("event").event,
            RuntimeEventKind::Started { opaque_session_id } if opaque_session_id == "session-resume"
        ));
        assert!(matches!(
            session.next_event().expect("resumed output").expect("event").event,
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
        let resumed = fs::read_to_string(directory.path().join("invocation-2.args"))
            .expect("resumed arguments");
        assert!(resumed.contains("--resume\nsession-resume\n"));
        assert!(!resumed.contains("--no-session-persistence"));
        assert_eq!(
            fs::read_to_string(directory.path().join("invocation.count"))
                .expect("invocation count")
                .trim(),
            "2"
        );
    }

    #[test]
    fn a_replaced_session_identifier_is_terminal_and_never_resumed_again() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-mismatch",
            &format!(
                r##"count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
printf '%s\n' "$count" > invocation.count
cat >/dev/null
if [ "$count" -eq 1 ]; then
  printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-known","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
  exit 17
fi
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-replacement","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
sleep 30
"##
            ),
        );
        let mut session =
            start(&executable, directory.path(), "invocation-mismatch").expect("start fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { opaque_session_id } if opaque_session_id == "session-known"
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::SanitizedUnsuccessfulExit { .. })
        ));
        session.resume("resume".to_owned()).expect("resume fixture");
        expect_launch(session.as_mut());
        assert!(matches!(
            session
                .next_event()
                .expect("typed failure")
                .expect("event")
                .event,
            RuntimeEventKind::Failed {
                kind: RuntimeFailureKind::Protocol,
                ..
            }
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
    fn an_uninstalled_executable_is_reported_rather_than_started() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let runtime = ClaudeRuntime::new(directory.path().join("absent-claude"))
            .without_delegated_credential();
        let probe = runtime.probe().expect("probe");
        assert_eq!(probe.readiness, Readiness::NotInstalled);
    }
}
