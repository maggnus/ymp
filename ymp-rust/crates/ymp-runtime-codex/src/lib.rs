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
use ymp_agent_rpc::{AgentToolCapabilities, endpoint_capabilities};
use ymp_runtime_api::{
    AdmittedProgram, BoundedOutputLine, CancellationToken, DiagnosticSummary, InFlightExcess,
    InvocationRequest, LaunchChain, LaunchDescriptor, LaunchEnvironmentVariable, McpBinding,
    ModelSpend, ProbeReport, ProbeTransportIdentity, Readiness, RuntimeDriver, RuntimeError,
    RuntimeEvent, RuntimeEventKind, RuntimeFailureKind, RuntimeKind, RuntimeSession,
    TOOL_HOST_PROBE_ENVIRONMENT, TOOL_HOST_PROBE_INTERNAL_ARGUMENTS,
    TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND, TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION,
    TOOL_HOST_PROBE_SERVER_VERSION, TOOL_HOST_PROBE_WORKSPACE_SERVER, ToolHostProbeInvocation,
    ToolHostProbeRuntimeIdentity, ToolHostProbeTool, Usage, configure_process_group,
    create_launch_marker, end_process_tree_or_keep, evidence_digest, managed_launch_command,
    probe_transport_digest, read_bounded_lines, register_launch_marker, terminate_process_tree,
    tool_host_probe_tool_schema_digest, verify_admitted_programs,
};

/// Canonical behavioral surface an installed Codex executable must satisfy. The observed release
/// string is deliberately absent: it is evidence recorded beside this contract, not an acceptance
/// predicate. The JSON is kept as one canonical byte string so downstream admission can bind its
/// digest without re-serializing a second representation.
pub const CODEX_COMPATIBILITY_CONTRACT: &str = r#"{"contract_version":1,"app_server_help":{"required":["generate-json-schema","--listen <URL>","stdio://"]},"app_server_schema":{"events":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"],"tool":{"required":["arguments","id","server","status","tool","type"],"title":"McpToolCallThreadItem"},"usage":{"pointer":"/definitions/v2/TokenUsageBreakdown","required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]}},"cancellation":{"terminal":"cancelled","tree":"all_descendants"},"descendant_termination":{"boundaries":["timeout","interrupt","drop"],"tree":"all_descendants"},"exec_help":{"required":["--json","--ignore-user-config","--ignore-rules","resume"]},"features":{"removed":["enable_fanout","remote_control","remote_models"],"required_not_removed":["hooks","multi_agent","multi_agent_v2","plugins","remote_plugin","shell_snapshot"]},"launch_feature_flags":{"removed_disable":["enable_fanout","remote_control","remote_models"],"required_disable":["hooks","multi_agent","multi_agent_v2","plugins","remote_plugin","shell_snapshot"]},"resume":{"help_required":["SESSION_ID","--json","--ignore-user-config","--ignore-rules"],"identity":"stable_thread_id"},"runtime_events":{"allowed":["thread.started","turn.started","item.started","item.updated","item.completed","turn.completed","turn.failed","error"],"mcp_status":["inProgress","completed","failed"],"tool_required":["arguments","server","status","tool"],"usage_required":["input_tokens","cached_input_tokens","output_tokens","reasoning_output_tokens"]}}"#;

pub fn codex_compatibility_contract_digest() -> String {
    evidence_digest(CODEX_COMPATIBILITY_CONTRACT.as_bytes())
}
/// Compatibility alias retained for the accepted v1 product fixtures and admission reader. The
/// Codex driver and supervisor never use it for runtime admission; W1-EVL-04q owns its remaining
/// product consumer and turns v1 into immutable historical evidence.
#[doc(hidden)]
pub const PINNED_CODEX_VERSION: &str = "codex-cli 0.151.0";
/// The model route the managed profile requests. `gpt-5.6-sol` was refused with HTTP 400
/// (`invalid_request_error`: not supported when using Codex with a ChatGPT account), so the route
/// is the balanced agentic coding model the account does list, which accepts the `low` reasoning
/// effort this profile pins.
pub const PINNED_CODEX_MODEL: &str = "gpt-5.6-terra";
pub const PINNED_CODEX_PROMPT_POLICY: &str = "ymp-codex-low-v2";
pub const PINNED_CODEX_API_ORIGIN: &str = "https://api.openai.com/v1";
const PINNED_CODEX_ROUTE: &str = "openai_responses_chatgpt";
const CODEX_DRIVER: &str = "ymp-runtime-codex";
const CODEX_DRIVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_WALL_TIME_LIMIT_MS: u64 = 10 * 60 * 1000;
const LAUNCH_DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
const HARNESS_INSTRUCTIONS: &str = "Execution policy: work without delegation or subagents. Do not send progress reports. Batch independent file reads and batch the final formatting, tests, lint, and diff checks. Use only the files and tools needed for the requested outcome. Do not commit. Stop immediately after a concise final report.";
/// How a managed attempt publishes what it produced. The operator's request is delivered word for
/// word and is not required to describe the product's own submission path, so the requirement
/// travels here instead: an attempt that only writes files leaves nothing the run can accept, and
/// the budget it spent buys no candidate.
const PUBLICATION_INSTRUCTIONS: &str = "Publication policy: the work of this attempt becomes a candidate only when it is published with the submit tool of the ymp MCP server. Files left in the workspace are not a result, and no report replaces that call. Call submit once, as soon as the requested outcome is reached, whether or not the request above mentions publishing.";
const DISABLED_AMBIENT_FEATURES: [&str; 32] = [
    "apps",
    "auth_elicitation",
    "browser_use",
    "browser_use_external",
    "browser_use_full_cdp_access",
    "code_mode_host",
    "computer_use",
    "deferred_executor",
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
const REMOVED_CODEX_FEATURES: [&str; 3] = ["enable_fanout", "remote_control", "remote_models"];
const REQUIRED_CODEX_FEATURES: [&str; 6] = [
    "hooks",
    "multi_agent",
    "multi_agent_v2",
    "plugins",
    "remote_plugin",
    "shell_snapshot",
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
    tool_host_executable: PathBuf,
    verified_executable: Arc<Mutex<Option<VerifiedExecutable>>>,
    profile: CodexProfile,
    auth_source: Option<PathBuf>,
    runtime_path: OsString,
    launch_chain: LaunchChain,
    prepared_launches: Arc<Mutex<HashMap<String, CodexLaunch>>>,
    #[cfg(test)]
    pre_spawn_tool_host_replacement: Option<Vec<u8>>,
}

impl Default for CodexRuntime {
    /// The installed engine, as the registry discovers it. Discovery belongs to the registry and
    /// to nothing else, so an engine is selected through one channel whose result a record states.
    fn default() -> Self {
        Self::new(ymp_runtime_registry::discover(
            ymp_runtime_registry::Engine::Codex.program(),
        ))
    }
}

impl CodexRuntime {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        let executable = ymp_runtime_registry::resolve(executable.into());
        Self {
            executable,
            tool_host_executable: std::env::current_exe().unwrap_or_default(),
            verified_executable: Arc::new(Mutex::new(None)),
            profile: CodexProfile::default(),
            auth_source: discover_auth_source(),
            runtime_path: std::env::var_os("PATH")
                .unwrap_or_else(|| OsString::from("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin")),
            launch_chain: LaunchChain::default(),
            prepared_launches: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            pre_spawn_tool_host_replacement: None,
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

    /// Where the managed invocation's authentication material comes from, named rather than read.
    /// The registry records the origin so an operator can see which account a spend would reach;
    /// the credential itself never leaves the launch.
    pub fn credential_origin(&self) -> Option<&'static str> {
        self.auth_source
            .as_ref()
            .map(|_| "delegated_home_credential")
    }

    /// The digest of the installed executable, computed from its bytes rather than reported by it.
    pub fn executable_digest(&self) -> Result<String, RuntimeError> {
        Ok(self.admitted_executable()?.digest)
    }

    /// The routes this engine can serve.
    ///
    /// The installed build publishes no catalog and lists no routes of its own: which models the
    /// account may reach is answered by the provider at request time, and asking would spend the
    /// operator's budget. What the registry can record without spending is the route the managed
    /// profile pins, and it records it as pinned rather than as measured, so a reader is never told
    /// that a list nobody measured was measured.
    pub fn model_catalog(&self) -> Vec<String> {
        vec![self.profile.model.clone()]
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

    fn observation(
        &self,
        executable: &Path,
        environment: &[EnvironmentValue],
        label: &str,
        arguments: &[&str],
    ) -> Result<String, RuntimeError> {
        let mut command = Command::new(executable);
        CodexEnvironment::apply(&mut command, environment);
        let output = command.args(arguments).output()?;
        if !output.status.success() {
            return Err(RuntimeError::InvalidProfile(format!(
                "codex_compatibility_{label}_probe_failed: exited with {}",
                output.status
            )));
        }
        String::from_utf8(output.stdout).map_err(|_| RuntimeError::NonUtf8Output)
    }

    fn observe_compatibility_surface(
        &self,
        executable: &Path,
        environment: &[EnvironmentValue],
    ) -> Result<(), RuntimeError> {
        let exec_help =
            self.observation(executable, environment, "exec_help", &["exec", "--help"])?;
        for required in ["--json", "--ignore-user-config", "--ignore-rules", "resume"] {
            if !exec_help.contains(required) {
                return Err(RuntimeError::InvalidProfile(format!(
                    "codex_compatibility_exec_help_mismatch: missing {required}"
                )));
            }
        }
        let resume_help = self.observation(
            executable,
            environment,
            "resume_help",
            &["exec", "resume", "--help"],
        )?;
        for required in [
            "SESSION_ID",
            "--json",
            "--ignore-user-config",
            "--ignore-rules",
        ] {
            if !resume_help.contains(required) {
                return Err(RuntimeError::InvalidProfile(format!(
                    "codex_compatibility_resume_help_mismatch: missing {required}"
                )));
            }
        }

        let features =
            self.observation(executable, environment, "features", &["features", "list"])?;
        for feature in REMOVED_CODEX_FEATURES {
            if feature_state(&features, feature).as_deref() != Some("removed") {
                return Err(RuntimeError::InvalidProfile(format!(
                    "codex_compatibility_feature_state_mismatch: {feature} is not removed"
                )));
            }
        }
        for feature in REQUIRED_CODEX_FEATURES {
            match feature_state(&features, feature).as_deref() {
                Some("removed") | None => {
                    return Err(RuntimeError::InvalidProfile(format!(
                        "codex_compatibility_feature_state_mismatch: {feature} cannot be disabled"
                    )));
                }
                Some(_) => {}
            }
        }

        let app_server_help = self.observation(
            executable,
            environment,
            "app_server_help",
            &["app-server", "--help"],
        )?;
        for required in ["generate-json-schema", "--listen <URL>", "stdio://"] {
            if !app_server_help.contains(required) {
                return Err(RuntimeError::InvalidProfile(format!(
                    "codex_compatibility_app_server_help_mismatch: missing {required}"
                )));
            }
        }

        let schema_directory = tempfile::Builder::new()
            .prefix("ymp-codex-compatibility-schema-")
            .tempdir()?;
        let schema_path = schema_directory.path().to_str().ok_or_else(|| {
            RuntimeError::InvalidProfile(
                "Codex App Server schema path must be valid UTF-8".to_owned(),
            )
        })?;
        self.observation(
            executable,
            environment,
            "app_server_schema",
            &["app-server", "generate-json-schema", "--out", schema_path],
        )?;
        let schema = fs::read(
            schema_directory
                .path()
                .join("codex_app_server_protocol.schemas.json"),
        )?;
        if schema.len() > self.profile.output_limit_bytes {
            return Err(RuntimeError::InvalidProfile(
                "codex_compatibility_app_server_schema_mismatch: schema exceeds output limit"
                    .to_owned(),
            ));
        }
        let schema: Value = serde_json::from_slice(&schema).map_err(|error| {
            RuntimeError::InvalidProfile(format!(
                "codex_compatibility_app_server_schema_mismatch: invalid JSON: {error}"
            ))
        })?;
        validate_compatibility_app_server_schema(&schema)?;
        Ok(())
    }

    fn tool_host_probe_runtime_identity(
        &self,
        workspace: &Path,
    ) -> Result<ToolHostProbeRuntimeIdentity, RuntimeError> {
        let probe = self.probe()?;
        if probe.readiness != Readiness::Ready {
            return Err(RuntimeError::InvalidProfile(format!(
                "Codex tool-host probe profile is not ready: {}",
                probe.detail
            )));
        }
        let cli_version = probe.version.ok_or_else(|| {
            RuntimeError::InvalidProfile(
                "Codex compatibility probe did not report an observed version".to_owned(),
            )
        })?;
        let executable_digest = executable_digest(&self.executable)?;
        let probe_transport =
            measure_tool_host_probe_transport(&self.tool_host_executable, workspace)?;
        let probe_transport_digest = probe_transport_digest(&probe_transport);
        Ok(ToolHostProbeRuntimeIdentity {
            runtime_kind: RuntimeKind::Codex,
            route: PINNED_CODEX_ROUTE.to_owned(),
            profile: self.profile.prompt_policy.clone(),
            cli: self.executable.display().to_string(),
            cli_version,
            compatibility_contract_digest: codex_compatibility_contract_digest(),
            executable_digest,
            driver: CODEX_DRIVER.to_owned(),
            driver_version: CODEX_DRIVER_VERSION.to_owned(),
            tool_schema_digest: tool_host_probe_tool_schema_digest(),
            probe_transport,
            probe_transport_digest,
        })
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
    agent_capabilities: AgentToolCapabilities,
    environment: CodexEnvironment,
}

#[derive(Debug)]
struct CodexProbeLaunch {
    executable: VerifiedExecutable,
    tool_host_source: PathBuf,
    tool_host_executable: VerifiedExecutable,
    launch_chain: Vec<AdmittedProgram>,
    profile: CodexProfile,
    workspace: PathBuf,
    relative_path: PathBuf,
    nonce: String,
    expected_transport: ProbeTransportIdentity,
    environment: CodexEnvironment,
    #[cfg(test)]
    pre_spawn_tool_host_replacement: Option<Vec<u8>>,
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

fn measure_tool_host_probe_transport(
    executable: &Path,
    workspace: &Path,
) -> Result<ProbeTransportIdentity, RuntimeError> {
    let metadata = fs::symlink_metadata(executable)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(RuntimeError::InvalidProfile(
            "tool-host probe executable must be a regular non-symlink file".to_owned(),
        ));
    }
    let canonical_executable = executable.canonicalize()?;
    if canonical_executable != executable {
        return Err(RuntimeError::InvalidProfile(
            "tool-host probe executable changed during canonicalization".to_owned(),
        ));
    }
    let workspace_metadata = fs::symlink_metadata(workspace)?;
    if !workspace_metadata.is_dir() || workspace_metadata.file_type().is_symlink() {
        return Err(RuntimeError::InvalidProfile(
            "tool-host probe workspace must be a regular non-symlink directory".to_owned(),
        ));
    }
    let canonical_workspace = workspace.canonicalize()?;
    if canonical_workspace != workspace {
        return Err(RuntimeError::InvalidProfile(
            "tool-host probe workspace changed during canonicalization".to_owned(),
        ));
    }
    let executable_digest = evidence_digest(&fs::read(&canonical_executable)?);
    Ok(ProbeTransportIdentity {
        mcp_protocol_version: TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION.to_owned(),
        server_name: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
        server_version: TOOL_HOST_PROBE_SERVER_VERSION.to_owned(),
        tool_schema_digest: tool_host_probe_tool_schema_digest(),
        ordered_tools: [
            ToolHostProbeTool::WorkspaceWrite,
            ToolHostProbeTool::WorkspaceRead,
        ],
        server_executable_digest: executable_digest.clone(),
        launcher_executable_digest: executable_digest,
        internal_subcommand: TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND.to_owned(),
        arguments: TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        inherited_environment: TOOL_HOST_PROBE_ENVIRONMENT
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        canonical_workspace_root_digest: evidence_digest(
            canonical_workspace.as_os_str().as_encoded_bytes(),
        ),
    })
}

fn require_tool_host_probe_transport(
    expected: &ProbeTransportIdentity,
    actual: &ProbeTransportIdentity,
) -> Result<(), RuntimeError> {
    if expected != actual || probe_transport_digest(expected) != probe_transport_digest(actual) {
        return Err(RuntimeError::InvalidProfile(
            "tool-host probe transport changed after controller reservation".to_owned(),
        ));
    }
    Ok(())
}

fn compare_codex_tool_host_probe_identity(
    expected: &ToolHostProbeRuntimeIdentity,
    observed: &ToolHostProbeRuntimeIdentity,
) -> Result<(), RuntimeError> {
    for (field, matches) in [
        (
            "runtime_kind",
            expected.runtime_kind == observed.runtime_kind,
        ),
        ("route", expected.route == observed.route),
        ("profile", expected.profile == observed.profile),
        ("cli", expected.cli == observed.cli),
        (
            "compatibility_contract_digest",
            expected.compatibility_contract_digest == observed.compatibility_contract_digest,
        ),
        (
            "executable_digest",
            expected.executable_digest == observed.executable_digest,
        ),
        ("driver", expected.driver == observed.driver),
        (
            "driver_version",
            expected.driver_version == observed.driver_version,
        ),
        (
            "tool_schema_digest",
            expected.tool_schema_digest == observed.tool_schema_digest,
        ),
        (
            "probe_transport",
            expected.probe_transport == observed.probe_transport,
        ),
        (
            "probe_transport_digest",
            expected.probe_transport_digest == observed.probe_transport_digest,
        ),
    ] {
        if !matches {
            return Err(RuntimeError::InvalidProfile(format!(
                "Codex tool-host probe identity differs in {field}"
            )));
        }
    }
    if observed.cli_version.is_empty() || observed.cli_version.len() > 4096 {
        return Err(RuntimeError::InvalidProfile(
            "Codex tool-host probe observed version is invalid".to_owned(),
        ));
    }
    Ok(())
}

impl CodexProbeLaunch {
    fn arguments(&self) -> Result<Vec<String>, RuntimeError> {
        let workspace = self.workspace.to_str().ok_or_else(|| {
            RuntimeError::InvalidProfile("Codex probe workspace path must be UTF-8".to_owned())
        })?;
        let mut arguments = vec![
            "exec".to_owned(),
            "--json".to_owned(),
            "--ignore-user-config".to_owned(),
            "--ignore-rules".to_owned(),
            "--sandbox".to_owned(),
            CodexSandbox::WorkspaceWrite.as_arg().to_owned(),
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
        arguments.push("-c".to_owned());
        arguments.push(self.mcp_configuration()?);
        arguments.push("-".to_owned());
        validate_compatibility_launch_arguments(&arguments)?;
        Ok(arguments)
    }

    fn mcp_configuration(&self) -> Result<String, RuntimeError> {
        let executable = self
            .tool_host_executable
            .execution_path
            .to_str()
            .ok_or_else(|| {
                RuntimeError::InvalidProfile(
                    "tool-host probe executable path must be UTF-8".to_owned(),
                )
            })?;
        let workspace = self.workspace.to_str().ok_or_else(|| {
            RuntimeError::InvalidProfile("tool-host probe workspace must be UTF-8".to_owned())
        })?;
        let relative_path = self.relative_path.to_str().ok_or_else(|| {
            RuntimeError::InvalidProfile("tool-host probe path must be UTF-8".to_owned())
        })?;
        let server = serde_json::to_string(TOOL_HOST_PROBE_WORKSPACE_SERVER)
            .expect("fixed server name is serializable");
        let command =
            serde_json::to_string(executable).expect("UTF-8 executable path is serializable");
        let child_arguments = serde_json::to_string(&TOOL_HOST_PROBE_INTERNAL_ARGUMENTS)
            .expect("fixed child arguments are serializable");
        let enabled_tools = serde_json::to_string(&[
            ToolHostProbeTool::WorkspaceWrite.to_string(),
            ToolHostProbeTool::WorkspaceRead.to_string(),
        ])
        .expect("fixed tool names are serializable");
        let workspace = serde_json::to_string(workspace).expect("UTF-8 workspace is serializable");
        let relative_path =
            serde_json::to_string(relative_path).expect("UTF-8 relative path is serializable");
        let nonce = serde_json::to_string(&self.nonce).expect("nonce is serializable");
        Ok(format!(
            "mcp_servers={{{server}={{required=true,enabled_tools={enabled_tools},default_tools_approval_mode=\"approve\",command={command},args={child_arguments},env={{YMP_TOOL_HOST_PROBE_WORKSPACE_ROOT={workspace},YMP_TOOL_HOST_PROBE_PATH={relative_path},YMP_TOOL_HOST_PROBE_NONCE={nonce}}},env_vars=[]}}}}"
        ))
    }

    fn directive(&self) -> String {
        let path =
            serde_json::to_string(&self.relative_path).expect("controller path is serializable");
        let nonce = serde_json::to_string(&self.nonce).expect("controller nonce is serializable");
        format!(
            "Compatibility probe only; this is not a user task or experimental arm. Use only the \
             {server} MCP server. Call workspace_write exactly once with path {path} and content \
             {nonce}; then call workspace_read exactly once with path {path}. Use no other tool. \
             Produce no ordinary output, commentary, explanation, result, or collaboration data. \
             End immediately after workspace_read returns the same nonce.",
            server = TOOL_HOST_PROBE_WORKSPACE_SERVER,
        )
    }

    fn spawn(&self) -> Result<CodexProcess, RuntimeError> {
        let actual = measure_tool_host_probe_transport(&self.tool_host_source, &self.workspace)?;
        require_tool_host_probe_transport(&self.expected_transport, &actual)?;
        if executable_digest(&self.executable.execution_path)? != self.executable.digest {
            return Err(RuntimeError::InvalidProfile(
                "Codex executable changed before tool-host probe spawn".to_owned(),
            ));
        }
        if executable_digest(&self.tool_host_executable.execution_path)?
            != self.expected_transport.server_executable_digest
        {
            return Err(RuntimeError::InvalidProfile(
                "tool-host probe child changed before spawn".to_owned(),
            ));
        }
        verify_admitted_programs(&self.launch_chain)?;
        let marker = create_launch_marker()?;
        let arguments = self.arguments()?;
        let mut command = match managed_launch_command(
            &self.executable.execution_path,
            &arguments,
            &marker,
            &self.launch_chain,
        ) {
            Ok(command) => command,
            Err(error) => {
                let _ = fs::remove_file(&marker);
                return Err(error);
            }
        };
        let environment = self.environment.values(None, None, None)?;
        CodexEnvironment::apply(&mut command, &environment);
        command
            .current_dir(&self.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_group(&mut command);

        // This is the final source/config/root measurement before process creation. The private
        // child copy is already bound to those bytes, so a source substitution cannot become the
        // configured MCP process after this check.
        #[cfg(test)]
        if let Some(replacement) = &self.pre_spawn_tool_host_replacement {
            fs::write(&self.tool_host_source, replacement)?;
        }
        let actual = measure_tool_host_probe_transport(&self.tool_host_source, &self.workspace)?;
        require_tool_host_probe_transport(&self.expected_transport, &actual)?;
        let started_at = Instant::now();
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = fs::remove_file(&marker);
                return Err(error.into());
            }
        };
        register_launch_marker(&child, marker);
        if let Err(error) = verify_admitted_programs(&self.launch_chain) {
            end_process_tree_or_keep(&mut child);
            return Err(error);
        }
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stdin was not piped".to_owned()))?;
        stdin.write_all(self.directive().as_bytes())?;
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
            started_at,
        })
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
            add_mcp_config_arguments(&mut arguments, mcp, self.agent_capabilities)?;
        }
        if let Some(session_id) = session_id {
            arguments.push("resume".to_owned());
            arguments.push(session_id.to_owned());
        }
        arguments.push("-".to_owned());
        validate_compatibility_launch_arguments(&arguments)?;
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
        if version.is_empty() || version.len() > 4096 {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: Some(version.clone()),
                readiness: Readiness::Incompatible,
                detail: "codex_compatibility_version_observation_invalid".to_owned(),
            });
        }
        if let Err(error) =
            self.observe_compatibility_surface(&admitted.execution_path, &environment_values)
        {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: Some(version),
                readiness: Readiness::Incompatible,
                detail: error.to_string(),
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
                "behaviorally compatible local profile ready: model={}, api_origin={}, reasoning_effort={}, approval_policy={}, prompt_policy={}, sandbox={}, environment=synthetic_allowlist_v1, wall_time_limit_ms={}, output_limit_bytes={}",
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

    fn tool_host_probe_identity(&self) -> Result<ToolHostProbeRuntimeIdentity, RuntimeError> {
        let workspace = std::env::current_dir()?.canonicalize()?;
        self.tool_host_probe_runtime_identity(&workspace)
    }

    fn start_tool_host_probe(
        &self,
        invocation: ToolHostProbeInvocation,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        if invocation.allowed_tools
            != [
                ToolHostProbeTool::WorkspaceWrite,
                ToolHostProbeTool::WorkspaceRead,
            ]
        {
            return Err(RuntimeError::InvalidProfile(
                "Codex tool-host probe requires the fixed write/read tool order".to_owned(),
            ));
        }
        let request = invocation.request;
        let observed = self.tool_host_probe_runtime_identity(&invocation.workspace)?;
        compare_codex_tool_host_probe_identity(&request.expected_runtime, &observed)?;

        let tool_host_executable =
            VerifiedExecutable::admit(&self.tool_host_executable, "ymp-tool-host-probe")?;
        if tool_host_executable.digest
            != request
                .expected_runtime
                .probe_transport
                .server_executable_digest
        {
            return Err(RuntimeError::InvalidProfile(
                "tool-host probe executable changed after controller reservation".to_owned(),
            ));
        }
        let actual =
            measure_tool_host_probe_transport(&self.tool_host_executable, &invocation.workspace)?;
        require_tool_host_probe_transport(&request.expected_runtime.probe_transport, &actual)?;
        let launch = CodexProbeLaunch {
            executable: self.admitted_executable()?,
            tool_host_source: self.tool_host_executable.clone(),
            tool_host_executable,
            launch_chain: self.launch_chain.admit()?,
            profile: self.profile.clone(),
            workspace: invocation.workspace,
            relative_path: request.workspace_path.clone(),
            nonce: request.nonce.clone(),
            expected_transport: request.expected_runtime.probe_transport.clone(),
            environment: self.isolated_environment()?,
            #[cfg(test)]
            pre_spawn_tool_host_replacement: self.pre_spawn_tool_host_replacement.clone(),
        };
        let process = launch.spawn()?;
        Ok(Box::new(CodexSession {
            child: process.child,
            lines: process.lines,
            stderr_reader: Some(process.stderr_reader),
            launch: None,
            _probe_launch: Some(launch),
            session_id: None,
            invocation_id: request.invocation_id,
            sequence: 0,
            output_limit_bytes: self.profile.output_limit_bytes,
            wall_time_limit_ms: request.deadline_ms,
            started_at: process.started_at,
            session_started_at: process.started_at,
            cancellation: request.cancellation,
            completed: false,
            terminal: false,
            recoverable: false,
            native_resume_started: false,
            interrupted: false,
            interruption_emitted: false,
            pending_events: VecDeque::new(),
            usage: Usage::default(),
            current_turn_usage: Usage::default(),
            in_flight: InFlightExcess::default(),
            yielded: false,
            failure_emitted: false,
            strict_probe_events: true,
        }))
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
        let agent_capabilities = accepted_agent_capabilities(
            request.mcp.as_ref(),
            &request.attempt_id,
            &request.invocation_id,
        );
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
            agent_capabilities,
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
            launch: Some(launch),
            _probe_launch: None,
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
            strict_probe_events: false,
        }))
    }
}

struct CodexSession {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: Option<JoinHandle<Vec<u8>>>,
    launch: Option<CodexLaunch>,
    _probe_launch: Option<CodexProbeLaunch>,
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
    strict_probe_events: bool,
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
        observed.cached_input_tokens = u64_field(value, "cached_input_tokens")?;
        observed.output_tokens = u64_field(value, "output_tokens")?;
        observed.reasoning_output_tokens = u64_field(value, "reasoning_output_tokens")?;
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
                        if self.strict_probe_events && text.is_empty() {
                            return Ok(None);
                        }
                        Ok(Some(self.emit(RuntimeEventKind::Output { text })))
                    }
                    Some("mcp_tool_call") => {
                        let server = string_field(item, "server")?;
                        let tool = string_field(item, "tool")?;
                        let status = string_field(item, "status")?;
                        if !matches!(status.as_str(), "inProgress" | "completed" | "failed") {
                            return Err(RuntimeError::MalformedEvent(format!(
                                "mcp_tool_call has unsupported status {status}"
                            )));
                        }
                        let arguments = item.get("arguments").cloned().ok_or_else(|| {
                            RuntimeError::MalformedEvent(
                                "mcp_tool_call has no arguments".to_owned(),
                            )
                        })?;
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
                    Some("reasoning") => Ok(None),
                    other if self.strict_probe_events => {
                        Err(RuntimeError::MalformedEvent(format!(
                            "tool-host probe emitted unexpected completed item {}",
                            other.unwrap_or("<missing>")
                        )))
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
            "item.started" | "item.updated" => {
                if self.strict_probe_events
                    && let Some(item) = event.get("item")
                    && !matches!(
                        item.get("type").and_then(Value::as_str),
                        Some("mcp_tool_call" | "reasoning" | "agent_message")
                    )
                {
                    return Err(RuntimeError::MalformedEvent(format!(
                        "tool-host probe emitted unexpected {} item {}",
                        event_type,
                        item.get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<missing>")
                    )));
                }
                Ok(None)
            }
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
        if self.strict_probe_events
            || self.terminal
            || self.interrupted
            || self.cancellation.is_cancelled()
        {
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
        let (descriptor, process) = {
            let launch = self.launch.as_ref().ok_or(RuntimeError::NotYielded)?;
            let descriptor = launch.descriptor(Some(&session_id))?;
            let process = launch.spawn(&descriptor, Some(&session_id), &input)?;
            (descriptor, process)
        };
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

fn feature_state(observed: &str, feature: &str) -> Option<String> {
    observed
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next()? == feature).then(|| {
                let fields: Vec<_> = fields.collect();
                fields[..fields.len().saturating_sub(1)].join(" ")
            })
        })
        .filter(|state| {
            matches!(
                state.as_str(),
                "removed" | "deprecated" | "stable" | "experimental" | "under development"
            )
        })
}

fn validate_compatibility_launch_arguments(arguments: &[String]) -> Result<(), RuntimeError> {
    for removed in REMOVED_CODEX_FEATURES {
        if arguments
            .windows(2)
            .any(|pair| pair[0] == "--disable" && pair[1] == removed)
        {
            return Err(RuntimeError::InvalidProfile(format!(
                "codex_compatibility_removed_flag: --disable {removed}"
            )));
        }
    }
    for required in REQUIRED_CODEX_FEATURES {
        if !arguments
            .windows(2)
            .any(|pair| pair[0] == "--disable" && pair[1] == required)
        {
            return Err(RuntimeError::InvalidProfile(format!(
                "codex_compatibility_feature_not_disabled: {required}"
            )));
        }
    }
    Ok(())
}

fn required_fields(value: &Value, expected: &[&str]) -> bool {
    let Some(fields) = value.get("required").and_then(Value::as_array) else {
        return false;
    };
    expected
        .iter()
        .all(|expected| fields.iter().any(|field| field.as_str() == Some(expected)))
}

fn schema_contains_object(value: &Value, title: &str, required: &[&str]) -> bool {
    match value {
        Value::Object(object) => {
            if object.get("title").and_then(Value::as_str) == Some(title)
                && required_fields(value, required)
            {
                return true;
            }
            object
                .values()
                .any(|value| schema_contains_object(value, title, required))
        }
        Value::Array(values) => values
            .iter()
            .any(|value| schema_contains_object(value, title, required)),
        _ => false,
    }
}

fn schema_contains_string(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(value) => value == expected,
        Value::Object(object) => object
            .values()
            .any(|value| schema_contains_string(value, expected)),
        Value::Array(values) => values
            .iter()
            .any(|value| schema_contains_string(value, expected)),
        _ => false,
    }
}

fn validate_compatibility_app_server_schema(schema: &Value) -> Result<(), RuntimeError> {
    if !schema_contains_object(
        schema,
        "McpToolCallThreadItem",
        &["arguments", "id", "server", "status", "tool", "type"],
    ) {
        return Err(RuntimeError::InvalidProfile(
            "codex_compatibility_app_server_tool_schema_mismatch".to_owned(),
        ));
    }
    let usage = schema
        .pointer("/definitions/v2/TokenUsageBreakdown")
        .filter(|usage| {
            required_fields(
                usage,
                &[
                    "cachedInputTokens",
                    "inputTokens",
                    "outputTokens",
                    "reasoningOutputTokens",
                    "totalTokens",
                ],
            )
        });
    if usage.is_none() {
        return Err(RuntimeError::InvalidProfile(
            "codex_compatibility_app_server_usage_schema_mismatch".to_owned(),
        ));
    }
    for method in [
        "thread/resume",
        "turn/interrupt",
        "thread/tokenUsage/updated",
        "turn/completed",
    ] {
        if !schema_contains_string(schema, method) {
            return Err(RuntimeError::InvalidProfile(format!(
                "codex_compatibility_app_server_event_schema_mismatch: missing {method}"
            )));
        }
    }
    Ok(())
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
    capabilities: AgentToolCapabilities,
) -> Result<(), RuntimeError> {
    binding.validate()?;
    let executable = binding.executable.to_str().ok_or_else(|| {
        RuntimeError::InvalidProfile("MCP executable path must be UTF-8".to_owned())
    })?;
    let enabled_tools = serde_json::to_string(&coordination_tools(capabilities))
        .expect("coordination tool names are serializable");
    for setting in [
        "mcp_servers.ymp.required=true".to_owned(),
        format!("mcp_servers.ymp.enabled_tools={enabled_tools}"),
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

fn accepted_agent_capabilities(
    binding: Option<&McpBinding>,
    attempt_id: &str,
    invocation_id: &str,
) -> AgentToolCapabilities {
    let Some(binding) = binding else {
        return AgentToolCapabilities::default();
    };
    endpoint_capabilities(
        &binding.socket_path,
        &binding.token,
        attempt_id,
        invocation_id,
    )
    .unwrap_or_default()
}

fn coordination_tools(capabilities: AgentToolCapabilities) -> Vec<&'static str> {
    let mut tools = vec!["read_control", "read_events", "read_board", "publish"];
    if capabilities.request_participant {
        tools.push("request_participant");
    }
    tools.extend(["yield", "submit"]);
    tools
}

#[cfg(test)]
mod tests {
    use super::{
        CODEX_COMPATIBILITY_CONTRACT, CodexProfile, CodexRuntime, DISABLED_AMBIENT_FEATURES,
        PINNED_CODEX_MODEL, REMOVED_CODEX_FEATURES, REQUIRED_CODEX_FEATURES,
        add_mcp_config_arguments, codex_compatibility_contract_digest,
        validate_compatibility_app_server_schema, validate_compatibility_launch_arguments,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use ymp_agent_rpc::AgentToolCapabilities;
    use ymp_runtime_api::{
        CancellationToken, InvocationRequest, McpBinding, Readiness, RuntimeDriver, RuntimeError,
        RuntimeEventKind, RuntimeFailureKind, RuntimeSession, TOOL_HOST_PROBE_SCHEMA_VERSION,
        ToolHostProbeInvocation, ToolHostProbeRequest, ToolHostProbeResourceVector,
        ToolHostProbeTool, ToolHostProbeTrust,
    };

    fn expect_launch(session: &mut dyn RuntimeSession) {
        assert!(matches!(
            session.next_event().expect("launch").expect("event").event,
            RuntimeEventKind::Launch { .. }
        ));
    }

    fn write_compatibility_probe_fixture(path: &Path, version: &str, mutation: &str) {
        let exec_help = if mutation == "exec_help" {
            "resume --ignore-user-config --ignore-rules"
        } else {
            "resume --json --ignore-user-config --ignore-rules"
        };
        let features = if mutation == "feature" {
            "hooks removed false\nmulti_agent stable true\nmulti_agent_v2 stable false\nplugins stable true\nremote_plugin stable true\nshell_snapshot stable true\nenable_fanout removed false\nremote_control removed false\nremote_models removed false"
        } else {
            "hooks stable true\nmulti_agent stable true\nmulti_agent_v2 stable false\nplugins stable true\nremote_plugin stable true\nshell_snapshot stable true\nenable_fanout removed false\nremote_control removed false\nremote_models removed false"
        };
        let schema = if mutation == "schema" {
            r#"{"definitions":{"v2":{"TokenUsageBreakdown":{"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]}}},"items":[{"title":"McpToolCallThreadItem","required":["id","server","status","tool","type"]}],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]}"#
        } else {
            r#"{"definitions":{"v2":{"TokenUsageBreakdown":{"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]}}},"items":[{"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]}],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]}"#
        };
        let script = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '__VERSION__'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' '__EXEC_HELP__'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' '__FEATURES__'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '%s\n' '__SCHEMA__' > "$4/codex_app_server_protocol.schemas.json"
elif [ "$1" = "login" ] && [ "$2" = "status" ]; then
  exit 0
else
  exit 64
fi
"##
        .replace("__VERSION__", version)
        .replace("__EXEC_HELP__", exec_help)
        .replace("__FEATURES__", features)
        .replace("__SCHEMA__", schema);
        fs::write(path, script).expect("write compatibility fixture");
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("make fixture executable");
    }

    fn write_tool_host_codex_fixture(path: &Path, marker: &Path, extra: &str) {
        let compatible = path.with_extension("compatible");
        write_compatibility_probe_fixture(&compatible, "codex-cli 0.151.0", "compatible");
        let script = r##"#!/bin/sh
set -eu
if [ "$1" = "--version" ] || { [ "$1" = "exec" ] && [ "${2:-}" = "--help" ]; } || { [ "$1" = "exec" ] && [ "${2:-}" = "resume" ] && [ "${3:-}" = "--help" ]; } || { [ "$1" = "features" ] && [ "${2:-}" = "list" ]; } || { [ "$1" = "app-server" ] && { [ "${2:-}" = "--help" ] || [ "${2:-}" = "generate-json-schema" ]; }; } || { [ "$1" = "login" ] && [ "${2:-}" = "status" ]; }; then
  exec '__COMPATIBLE__' "$@"
fi
workspace=
configuration=
previous=
for argument in "$@"; do
  if [ "$previous" = "-C" ]; then workspace=$argument; fi
  case "$argument" in mcp_servers=*) configuration=$argument ;; esac
  previous=$argument
done
[ -n "$workspace" ]
case "$configuration" in
  *'"ymp.workspace"'*'workspace_write'*'workspace_read'*'tool-host-probe-mcp'*'YMP_TOOL_HOST_PROBE_WORKSPACE_ROOT'*'YMP_TOOL_HOST_PROBE_PATH'*'YMP_TOOL_HOST_PROBE_NONCE'*) ;;
  *) exit 65 ;;
esac
prompt=$(/bin/cat)
case "$prompt" in *'Compatibility probe only'*'workspace_write'*'workspace_read'*'Produce no ordinary output'*) ;; *) exit 66 ;; esac
/bin/mkdir -p "$workspace/probe"
/usr/bin/printf '%s' 'opaque-caller-nonce-7c1e' > "$workspace/probe/nonce.txt"
/usr/bin/printf '%s\n' '{"type":"thread.started","thread_id":"fixture-session"}'
/usr/bin/printf '%s\n' '{"type":"turn.started"}'
/usr/bin/printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp.workspace","tool":"workspace_write","status":"completed","arguments":{"path":"probe/nonce.txt","content":"opaque-caller-nonce-7c1e"},"result":{"bytes_written":24},"error":null}}'
/usr/bin/printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp.workspace","tool":"workspace_read","status":"completed","arguments":{"path":"probe/nonce.txt"},"result":{"content":"opaque-caller-nonce-7c1e"},"error":null}}'
__EXTRA__
/bin/sleep 0.01
/usr/bin/printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"cached_input_tokens":3,"output_tokens":5,"reasoning_output_tokens":2}}'
/usr/bin/printf '%s' started > '__MARKER__'
"##
        .replace("__COMPATIBLE__", &compatible.display().to_string())
        .replace("__EXTRA__", extra)
        .replace("__MARKER__", &marker.display().to_string());
        fs::write(path, script).expect("write tool-host Codex fixture");
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("make fixture executable");
    }

    fn tool_host_reservation(deadline_ms: u64) -> ToolHostProbeResourceVector {
        ToolHostProbeResourceVector {
            model_calls: 1,
            max_input_tokens: 64,
            max_cached_input_tokens: 32,
            max_output_tokens: 32,
            max_reasoning_output_tokens: 16,
            max_cost_microusd: None,
            max_wall_time_ms: deadline_ms,
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

    #[test]
    fn generated_tool_config_tracks_endpoint_recruitment_capability() {
        let binding = McpBinding {
            executable: PathBuf::from("/tmp/ymp"),
            socket_path: PathBuf::from("/tmp/ymp-agent.sock"),
            token: "fixture-token".to_owned(),
        };
        for (capabilities, expected) in [
            (AgentToolCapabilities::default(), false),
            (AgentToolCapabilities::RECRUITMENT, true),
        ] {
            let mut arguments = Vec::new();
            add_mcp_config_arguments(&mut arguments, &binding, capabilities)
                .expect("generate MCP configuration");
            let configuration = arguments.join(" ");
            assert_eq!(configuration.contains("request_participant"), expected);
            assert!(configuration.contains("read_control"));
            assert!(configuration.contains("submit"));
        }
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
    fn default_runtime_exposes_the_managed_profile_without_a_version_selector() {
        let runtime = CodexRuntime::default();
        assert_eq!(runtime.profile().model, PINNED_CODEX_MODEL);
        assert_eq!(runtime.profile().reasoning_effort, "low");
    }

    #[test]
    fn compatibility_contract_covers_the_measured_surface() {
        let contract: serde_json::Value =
            serde_json::from_str(CODEX_COMPATIBILITY_CONTRACT).expect("canonical contract JSON");
        assert_eq!(contract["contract_version"], 1);
        for dimension in [
            "app_server_help",
            "app_server_schema",
            "cancellation",
            "descendant_termination",
            "exec_help",
            "features",
            "launch_feature_flags",
            "resume",
            "runtime_events",
        ] {
            assert!(
                !contract[dimension].is_null(),
                "contract omitted {dimension}"
            );
        }
        assert_eq!(
            contract["features"]["removed"],
            serde_json::json!(REMOVED_CODEX_FEATURES)
        );
        assert_eq!(
            contract["features"]["required_not_removed"],
            serde_json::json!(REQUIRED_CODEX_FEATURES)
        );
        assert_eq!(codex_compatibility_contract_digest().len(), 64);
    }

    #[test]
    fn removed_flags_and_schema_mutations_have_distinct_refusals() {
        let mut arguments = vec!["exec".to_owned()];
        for feature in DISABLED_AMBIENT_FEATURES {
            arguments.extend(["--disable".to_owned(), feature.to_owned()]);
        }
        validate_compatibility_launch_arguments(&arguments)
            .expect("the measured launch flags are accepted");
        for removed in REMOVED_CODEX_FEATURES {
            let mut retained = arguments.clone();
            retained.extend(["--disable".to_owned(), removed.to_owned()]);
            let error = validate_compatibility_launch_arguments(&retained)
                .expect_err("a removed flag must be refused")
                .to_string();
            assert!(
                error.contains("codex_compatibility_removed_flag"),
                "{error}"
            );
            assert!(error.contains(removed), "{error}");
        }

        let schema = serde_json::json!({
            "definitions": {
                "v2": {
                    "TokenUsageBreakdown": {
                        "required": [
                            "cachedInputTokens", "inputTokens", "outputTokens",
                            "reasoningOutputTokens", "totalTokens"
                        ]
                    }
                }
            },
            "items": [{
                "title": "McpToolCallThreadItem",
                "required": ["arguments", "id", "server", "status", "tool", "type"]
            }],
            "methods": [
                "thread/resume", "turn/interrupt", "thread/tokenUsage/updated", "turn/completed"
            ]
        });
        validate_compatibility_app_server_schema(&schema).expect("the measured schema is accepted");

        let mut changed_tool = schema.clone();
        changed_tool["items"][0]["required"] =
            serde_json::json!(["id", "server", "status", "tool", "type"]);
        let error = validate_compatibility_app_server_schema(&changed_tool)
            .expect_err("a changed tool schema must be refused")
            .to_string();
        assert!(error.contains("tool_schema_mismatch"), "{error}");

        let mut changed_usage = schema.clone();
        changed_usage["definitions"]["v2"]["TokenUsageBreakdown"]["required"] =
            serde_json::json!([
                "inputTokens",
                "outputTokens",
                "reasoningOutputTokens",
                "totalTokens"
            ]);
        let error = validate_compatibility_app_server_schema(&changed_usage)
            .expect_err("a changed usage schema must be refused")
            .to_string();
        assert!(error.contains("usage_schema_mismatch"), "{error}");

        let mut changed_event = schema;
        changed_event["methods"] = serde_json::json!([
            "thread/resume",
            "thread/tokenUsage/updated",
            "turn/completed"
        ]);
        let error = validate_compatibility_app_server_schema(&changed_event)
            .expect_err("a changed event schema must be refused")
            .to_string();
        assert!(error.contains("event_schema_mismatch"), "{error}");
    }

    #[test]
    fn version_difference_is_evidence_and_never_an_acceptance_predicate() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut evidence = Vec::new();
        for (name, version) in [
            ("historical", "codex-cli 0.151.0"),
            ("different", "codex-cli 9.7.3"),
        ] {
            let executable = directory.path().join(format!("codex-{name}"));
            write_compatibility_probe_fixture(&executable, version, "compatible");
            let runtime = CodexRuntime::new(&executable);
            let probe = runtime.probe().expect("behavioral probe");
            assert_eq!(probe.readiness, Readiness::Ready, "{}", probe.detail);
            evidence.push((
                probe.version.expect("observed version"),
                runtime.executable_digest().expect("executable digest"),
                codex_compatibility_contract_digest(),
            ));
        }
        assert_ne!(evidence[0].0, evidence[1].0);
        assert_ne!(evidence[0].1, evidence[1].1);
        assert_eq!(evidence[0].2, evidence[1].2);
    }

    #[test]
    fn historical_version_with_behavior_drift_fails_its_exact_check() {
        let directory = tempfile::tempdir().expect("temporary directory");
        for (mutation, reason) in [
            ("exec_help", "exec_help_mismatch"),
            ("feature", "feature_state_mismatch"),
            ("schema", "tool_schema_mismatch"),
        ] {
            let executable = directory.path().join(format!("codex-{mutation}"));
            write_compatibility_probe_fixture(&executable, "codex-cli 0.151.0", mutation);
            let probe = CodexRuntime::new(&executable)
                .probe()
                .expect("behavioral probe");
            assert_eq!(probe.readiness, Readiness::Incompatible);
            assert_eq!(probe.version.as_deref(), Some("codex-cli 0.151.0"));
            assert!(
                probe.detail.contains(reason),
                "{mutation}: {}",
                probe.detail
            );
        }
    }

    #[test]
    fn tool_host_probe_process_emits_the_fixed_0151_mcp_event_shape() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let workspace = directory.path().join("workspace");
        fs::create_dir(&workspace).expect("workspace");
        let workspace = workspace.canonicalize().expect("canonical workspace");
        let executable = directory.path().join("codex");
        let marker = directory.path().join("model-call-marker");
        write_tool_host_codex_fixture(&executable, &marker, "");
        let tool_host = directory.path().join("ymp");
        fs::write(&tool_host, b"#!/bin/sh\nexit 0\n").expect("tool-host fixture");
        let mut permissions = fs::metadata(&tool_host).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&tool_host, permissions).expect("make tool-host executable");

        let mut runtime = CodexRuntime::new(&executable);
        runtime.auth_source = None;
        runtime.tool_host_executable = tool_host.canonicalize().expect("canonical tool host");
        let identity = runtime
            .tool_host_probe_runtime_identity(&workspace)
            .expect("behaviorally compatible identity");
        assert_eq!(identity.cli_version, "codex-cli 0.151.0");
        let deadline_ms = 1_000;
        let request = ToolHostProbeRequest {
            schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
            probe_id: "probe-fixture-1".to_owned(),
            invocation_id: "invocation-fixture-1".to_owned(),
            nonce: "opaque-caller-nonce-7c1e".to_owned(),
            workspace_path: PathBuf::from("probe/nonce.txt"),
            deadline_ms,
            resource_reservation: tool_host_reservation(deadline_ms),
            expected_runtime: identity,
            cancellation: CancellationToken::default(),
        };
        let mut session = runtime
            .start_tool_host_probe(ToolHostProbeInvocation {
                request,
                workspace: workspace.clone(),
                allowed_tools: [
                    ToolHostProbeTool::WorkspaceWrite,
                    ToolHostProbeTool::WorkspaceRead,
                ],
            })
            .expect("start deterministic Codex probe process");
        let mut events = Vec::new();
        while let Some(event) = session.next_event().expect("probe event") {
            events.push(event.event);
        }
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], RuntimeEventKind::Started { .. }));
        assert!(matches!(
            &events[1],
            RuntimeEventKind::McpToolCall { server, tool, .. }
                if server == "ymp.workspace" && tool == "workspace_write"
        ));
        assert!(matches!(
            &events[2],
            RuntimeEventKind::McpToolCall { server, tool, .. }
                if server == "ymp.workspace" && tool == "workspace_read"
        ));
        assert!(matches!(events[3], RuntimeEventKind::Completed { .. }));
        assert_eq!(
            fs::read_to_string(workspace.join("probe/nonce.txt")).expect("probe write"),
            "opaque-caller-nonce-7c1e"
        );
        assert!(marker.is_file(), "the fixture process completed one turn");
        assert!(session.usage().wall_time_ms > 0);
        let serialized = serde_json::to_value(ToolHostProbeTrust::UntrustedRuntimeTrace)
            .expect("trust marker serializes");
        assert_eq!(serialized, serde_json::json!("untrusted_runtime_trace"));
    }

    #[test]
    fn final_tool_host_child_rehash_rejects_a_schema_identical_substitute_before_codex_spawn() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let workspace = directory.path().join("workspace");
        fs::create_dir(&workspace).expect("workspace");
        let workspace = workspace.canonicalize().expect("canonical workspace");
        let executable = directory.path().join("codex");
        let marker = directory.path().join("model-call-marker");
        write_tool_host_codex_fixture(&executable, &marker, "");
        let tool_host = directory.path().join("ymp");
        fs::write(&tool_host, b"#!/bin/sh\n# measured child\nexit 0\n")
            .expect("measured tool-host fixture");
        let mut permissions = fs::metadata(&tool_host).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&tool_host, permissions).expect("make tool-host executable");

        let mut runtime = CodexRuntime::new(&executable);
        runtime.auth_source = None;
        runtime.tool_host_executable = tool_host.canonicalize().expect("canonical tool host");
        let identity = runtime
            .tool_host_probe_runtime_identity(&workspace)
            .expect("controller-time identity");
        runtime.pre_spawn_tool_host_replacement =
            Some(b"#!/bin/sh\n# schema-identical substitute child\nexit 0\n".to_vec());
        let deadline_ms = 1_000;
        let error = match runtime.start_tool_host_probe(ToolHostProbeInvocation {
            request: ToolHostProbeRequest {
                schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
                probe_id: "probe-fixture-2".to_owned(),
                invocation_id: "invocation-fixture-2".to_owned(),
                nonce: "opaque-caller-nonce-7c1e".to_owned(),
                workspace_path: PathBuf::from("probe/nonce.txt"),
                deadline_ms,
                resource_reservation: tool_host_reservation(deadline_ms),
                expected_runtime: identity,
                cancellation: CancellationToken::default(),
            },
            workspace,
            allowed_tools: [
                ToolHostProbeTool::WorkspaceWrite,
                ToolHostProbeTool::WorkspaceRead,
            ],
        }) {
            Ok(_) => panic!("post-reservation child substitution must be refused"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("transport changed after controller reservation"),
            "{error}"
        );
        assert!(
            !marker.exists(),
            "the fake Codex process, and therefore its model-call marker, never started"
        );
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
                REMOVED_CODEX_FEATURES
                    .iter()
                    .all(|feature| !arguments.contains(&format!("--disable\n{feature}\n"))),
                "a removed compatibility feature flag reached the child",
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
elif [ "$1" = "login" ]; then
  exit 0
else
  printf '%s\n' "$@" > invocation.args
  cat > delivered.stdin
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-delivery"}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let mut delivered = Vec::new();
        let mut invocation_arguments = Vec::new();
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
            invocation_arguments.push(
                fs::read_to_string(workspace.join("invocation.args"))
                    .expect("captured invocation arguments"),
            );
        }
        let [coordinated, plain] = <[String; 2]>::try_from(delivered).expect("two deliveries");
        let [coordinated_arguments, plain_arguments] =
            <[String; 2]>::try_from(invocation_arguments).expect("two argument lists");

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
        for tool in ["read_board", "publish"] {
            assert!(
                coordinated_arguments.contains(tool),
                "coordinated Codex configuration omitted {tool}: {coordinated_arguments}"
            );
            assert!(
                !plain_arguments.contains(tool),
                "plain Codex configuration granted {tool}: {plain_arguments}"
            );
        }
        assert!(!coordinated_arguments.contains("request_participant"));
    }

    #[test]
    fn structured_process_stream_preserves_session_output_and_usage() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
            "mcp_servers.ymp.enabled_tools=[\"read_control\",\"read_events\",\"read_board\",\"publish\",\"yield\",\"submit\"]"
        ));
        assert!(arguments.contains("mcp_servers.ymp.default_tools_approval_mode=\"approve\""));
    }

    #[test]
    fn changed_tool_event_and_usage_schemas_fail_for_their_own_reason() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-invalid-event-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
elif [ "$1" = "login" ]; then
  exit 0
else
  input=$(cat)
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-invalid-event"}'
  case "$input" in
    *changed-tool*) printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed"}}' ;;
    *changed-usage*) printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1,"reasoning_output_tokens":0}}' ;;
    *) printf '%s\n' '{"type":"future.incompatible_event"}' ;;
  esac
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        for (prompt, reason) in [
            ("changed-tool", "mcp_tool_call has no arguments"),
            ("changed-usage", "missing integer field cached_input_tokens"),
            (
                "changed-event",
                "unsupported Codex event type future.incompatible_event",
            ),
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
            expect_launch(session.as_mut());
            assert!(matches!(
                session.next_event().expect("started").expect("event").event,
                RuntimeEventKind::Started { .. }
            ));
            let failure = session.next_event().expect("failure").expect("event");
            let RuntimeEventKind::Failed {
                kind: RuntimeFailureKind::Protocol,
                diagnostic: Some(diagnostic),
                ..
            } = failure.event
            else {
                panic!("{prompt} did not produce a typed protocol failure");
            };
            let detail = format!("runtime emitted a malformed event: {reason}");
            assert_eq!(
                diagnostic.digest,
                ymp_runtime_api::evidence_digest(detail.as_bytes()),
                "{prompt} failed for another reason"
            );
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
elif [ "$1" = "login" ]; then
  exit 0
else
  printf '%s\n' "$0" > actual.executable
  printf '%s\n' "$@" > actual.arguments
  env | sort > actual.environment
  cat >/dev/null
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-launch"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"launch-submit"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' admitted > admitted-runtime.marker
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-chain"}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' admitted > admitted-runtime.marker
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-admitted"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"admitted-submit"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
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
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
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
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
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
