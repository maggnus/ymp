#![forbid(unsafe_code)]

use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
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
    ModelSpend, ProbeReport, ProgramRequirement, ProgramRole, Readiness, RuntimeDriver,
    RuntimeError, RuntimeEvent, RuntimeEventKind, RuntimeFailureKind, RuntimeKind, RuntimeSession,
    Usage, configure_process_group, create_launch_marker, end_process_tree_or_keep,
    evidence_digest, managed_launch_command, read_bounded_lines, register_launch_marker,
    terminate_process_tree, verify_admitted_programs,
};

/// The oldest Claude Code release this profile admits, written the way `claude --version` prints
/// it. Admission measures the installed build against this floor instead of comparing the printed
/// string to one frozen build: a release at or above the floor is admitted, an older one is
/// refused, and a build that does not name this product is refused whatever it reports.
pub const MINIMUM_CLAUDE_VERSION: &str = "2.1.227 (Claude Code)";
/// The product name `claude --version` prints after the build number. A file that answers
/// `--version` with anything else is not the runtime this profile admits.
pub const CLAUDE_PRODUCT_NAME: &str = "Claude Code";
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
/// How a managed attempt publishes what it produced. The operator's request is delivered word for
/// word and is not required to describe the product's own submission path, so the requirement
/// travels here instead: an attempt that only writes files leaves nothing the run can accept, and
/// the budget it spent buys no candidate.
const PUBLICATION_INSTRUCTIONS: &str = "Publication policy: the work of this attempt becomes a candidate only when it is published with the mcp__ymp__submit tool. Files left in the workspace are not a result, and no report replaces that call. Call mcp__ymp__submit once, as soon as the requested outcome is reached, whether or not the request above mentions publishing.";

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

/// The program search path a managed invocation receives. The operator's own `PATH` is never
/// delegated: everything it names would otherwise be reachable from the child and would be written
/// into the profile record as if the run had approved it.
pub const APPROVED_SEARCH_PATH: [&str; 5] =
    ["/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"];

/// Every environment name a managed invocation may carry. A launch whose environment names
/// anything else is refused before the child starts, whichever path placed it there.
pub const ALLOWED_ENVIRONMENT_NAMES: [&str; 10] = [
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    "CLAUDE_CONFIG_DIR",
    "HOME",
    "NO_COLOR",
    "PATH",
    "TMPDIR",
    "YMP_AGENT_SOCKET",
    "YMP_AGENT_TOKEN",
    "YMP_ATTEMPT_ID",
    "YMP_INVOCATION_ID",
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
    /// The lowest Claude Code release admission accepts. It is a floor and not an identity: the
    /// build that actually runs is whatever the installed executable reports, measured before the
    /// launch and required again from the session.
    pub minimum_version: String,
    pub model: String,
    pub effort: String,
    pub permission_mode: String,
    pub prompt_policy: String,
    /// Comma-separated `--setting-sources` value. Only the empty value is approved, because any
    /// source re-admits user, project, or local configuration into the managed invocation.
    pub setting_sources: String,
    pub input_format: String,
    pub output_format: String,
    /// Directories the child may search for programs. Only the approved system directories are
    /// admitted, so the operator's own search path cannot be delegated by configuration either.
    pub search_path: Vec<String>,
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
            minimum_version: MINIMUM_CLAUDE_VERSION.to_owned(),
            model: PINNED_CLAUDE_MODEL.to_owned(),
            effort: "low".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            prompt_policy: PINNED_CLAUDE_PROMPT_POLICY.to_owned(),
            setting_sources: String::new(),
            input_format: "text".to_owned(),
            output_format: "stream-json".to_owned(),
            search_path: APPROVED_SEARCH_PATH
                .iter()
                .map(|directory| (*directory).to_owned())
                .collect(),
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
        if InstalledBuild::parse(&self.minimum_version).is_none() {
            return Err(RuntimeError::InvalidProfile(format!(
                "the minimum version must name a {CLAUDE_PRODUCT_NAME} build: {}",
                self.minimum_version
            )));
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
        if self.search_path.is_empty() {
            return Err(RuntimeError::InvalidProfile(
                "the managed search path must name at least one approved directory".to_owned(),
            ));
        }
        for (index, directory) in self.search_path.iter().enumerate() {
            if !APPROVED_SEARCH_PATH.contains(&directory.as_str()) {
                return Err(RuntimeError::InvalidProfile(format!(
                    "unapproved Claude search-path directory {directory}"
                )));
            }
            if self.search_path[..index].contains(directory) {
                return Err(RuntimeError::InvalidProfile(format!(
                    "repeated Claude search-path directory {directory}"
                )));
            }
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

    /// The `PATH` value the child receives, built from the approved directories alone.
    fn search_path_value(&self) -> String {
        self.search_path.join(":")
    }

    fn enforced_cost_ceiling_microusd(&self) -> u64 {
        self.max_budget_microusd
            .saturating_add(self.max_in_flight_overshoot_microusd)
    }

    /// Answers whether an installed release is new enough for this profile. The comparison is
    /// ordinal, so `2.1.232` satisfies a floor of `2.1.227` while `2.1.9` does not, which string
    /// ordering would get wrong.
    fn admits(&self, installed: &InstalledBuild) -> bool {
        InstalledBuild::parse(&self.minimum_version)
            .is_some_and(|floor| installed.is_at_least(&floor))
    }
}

/// A Claude Code release as its executable reports it: the build identifier that the `system`/`init`
/// event repeats, and the ordinal reading of that identifier used for the floor comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
struct InstalledBuild {
    build: String,
    ordinal: Vec<u64>,
}

impl InstalledBuild {
    /// Reads what `claude --version` printed. Anything that does not name this product, or whose
    /// build identifier is not a dotted sequence of numbers, is not a release this profile can
    /// order against the floor and is therefore not parsed at all.
    fn parse(reported: &str) -> Option<Self> {
        let reported = reported.trim();
        let (build, product) = reported.split_once(char::is_whitespace)?;
        if product.trim() != format!("({CLAUDE_PRODUCT_NAME})") {
            return None;
        }
        let ordinal = build
            .split('.')
            .map(|component| component.parse::<u64>().ok())
            .collect::<Option<Vec<u64>>>()?;
        if ordinal.is_empty() {
            return None;
        }
        Some(Self {
            build: build.to_owned(),
            ordinal,
        })
    }

    fn is_at_least(&self, floor: &Self) -> bool {
        let width = self.ordinal.len().max(floor.ordinal.len());
        let read = |ordinal: &[u64], index: usize| ordinal.get(index).copied().unwrap_or(0);
        for index in 0..width {
            match read(&self.ordinal, index).cmp(&read(&floor.ordinal, index)) {
                std::cmp::Ordering::Less => return false,
                std::cmp::Ordering::Greater => return true,
                std::cmp::Ordering::Equal => {}
            }
        }
        true
    }
}

/// What running `--version` on the admitted executable produced.
enum VersionMeasurement {
    NotInstalled,
    ProbeFailed(ExitStatus),
    Reported(String),
}

#[derive(Clone, Debug)]
pub struct ClaudeRuntime {
    executable: PathBuf,
    verified_executable: Arc<Mutex<Option<VerifiedExecutable>>>,
    profile: ClaudeProfile,
    credential: Option<CredentialSource>,
    launch_chain: LaunchChain,
    prepared_launches: Arc<Mutex<HashMap<String, ClaudeLaunch>>>,
}

impl Default for ClaudeRuntime {
    fn default() -> Self {
        Self::new(installed_executable_path())
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
            launch_chain: LaunchChain::default(),
            prepared_launches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_profile(executable: impl Into<PathBuf>, profile: ClaudeProfile) -> Self {
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
        ClaudeEnvironment::create(self.credential.as_ref(), &self.profile)
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

    /// Reads the release the admitted executable reports. The measurement runs against the copy
    /// that admission took and holds by digest, so the readiness report and the launch describe the
    /// same bytes rather than whatever the original path may point at afterwards.
    fn measure_version(
        &self,
        admitted: &VerifiedExecutable,
        environment_values: &[EnvironmentValue],
    ) -> Result<VersionMeasurement, RuntimeError> {
        let mut command = Command::new(&admitted.execution_path);
        ClaudeEnvironment::apply(&mut command, environment_values);
        let output = match command.arg("--version").stdin(Stdio::null()).output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(VersionMeasurement::NotInstalled);
            }
            Err(error) => return Err(RuntimeError::Process(error)),
        };
        if !output.status.success() {
            return Ok(VersionMeasurement::ProbeFailed(output.status));
        }
        Ok(VersionMeasurement::Reported(
            String::from_utf8(output.stdout)
                .map_err(|_| RuntimeError::NonUtf8Output)?
                .trim()
                .to_owned(),
        ))
    }

    /// Measures the installed release and refuses everything the profile does not admit. Both the
    /// readiness probe and the launch resolve the running build through this one path, so a launch
    /// can never proceed on a build the probe would have refused.
    fn admitted_build(
        &self,
        admitted: &VerifiedExecutable,
        environment_values: &[EnvironmentValue],
    ) -> Result<Result<(InstalledBuild, String), ProbeReport>, RuntimeError> {
        let reported = match self.measure_version(admitted, environment_values)? {
            VersionMeasurement::NotInstalled => return Ok(Err(self.not_installed())),
            VersionMeasurement::ProbeFailed(status) => {
                return Ok(Err(ProbeReport {
                    kind: RuntimeKind::ClaudeCode,
                    executable: self.executable.display().to_string(),
                    version: None,
                    readiness: Readiness::Unavailable,
                    detail: format!("version probe exited with {status}"),
                }));
            }
            VersionMeasurement::Reported(reported) => reported,
        };
        let Some(installed) = InstalledBuild::parse(&reported) else {
            return Ok(Err(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
                executable: self.executable.display().to_string(),
                version: Some(reported.clone()),
                readiness: Readiness::Incompatible,
                detail: format!(
                    "profile requires a {CLAUDE_PRODUCT_NAME} build of at least {}, found {reported}",
                    self.profile.minimum_version
                ),
            }));
        };
        if !self.profile.admits(&installed) {
            return Ok(Err(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
                executable: self.executable.display().to_string(),
                version: Some(reported.clone()),
                readiness: Readiness::Incompatible,
                detail: format!(
                    "profile requires at least {}, found {reported}",
                    self.profile.minimum_version
                ),
            }));
        }
        Ok(Ok((installed, reported)))
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
    #[cfg(target_os = "macos")]
    const MACOS_KEYCHAIN_READER: &'static str = "/usr/bin/security";
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

    /// The program this source executes to obtain the credential, admitted before it runs. It hands
    /// credential material to the managed process, so it belongs to the trusted chain and is named
    /// in the launch record rather than treated as an incidental utility.
    fn reader(&self) -> Result<Option<AdmittedProgram>, RuntimeError> {
        match self {
            Self::File(_) => Ok(None),
            #[cfg(target_os = "macos")]
            Self::MacosKeychain => Ok(Some(AdmittedProgram::admit(
                ProgramRole::CredentialReader,
                Self::MACOS_KEYCHAIN_READER,
                &ProgramRequirement::SystemPath,
            )?)),
        }
    }

    fn read(&self) -> Result<Option<Vec<u8>>, RuntimeError> {
        let reader = self.reader()?;
        let material = match self {
            Self::File(path) => fs::read(path)?,
            #[cfg(target_os = "macos")]
            Self::MacosKeychain => {
                let reader = reader
                    .as_ref()
                    .ok_or_else(|| {
                        RuntimeError::InvalidProfile(
                            "the keychain credential reader was not admitted".to_owned(),
                        )
                    })?
                    .path
                    .clone();
                let output = Command::new(reader)
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
    /// The build the admitted executable reported before this launch was prepared. The session has
    /// to report the same one, so the release that was measured is the release that ran.
    measured_build: InstalledBuild,
    coordination_executable: Option<VerifiedExecutable>,
    launch_chain: Vec<AdmittedProgram>,
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
    search_path: String,
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
            search_path: profile.search_path_value(),
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
            plain_environment_value("PATH", &self.search_path, false),
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
        admit_environment_names(values.iter().map(|variable| variable.name.as_str()))?;
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
    ) -> Result<ClaudeProcess, RuntimeError> {
        // The environment of the prepared launch is admitted by name before it is compared with the
        // expected one, so a variable inserted into the descriptor is refused by the rule it breaks
        // rather than as an unnamed difference.
        admit_environment_names(
            descriptor
                .environment
                .iter()
                .map(|variable| variable.name.as_str()),
        )?;
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
        // The operator's request goes first and unchanged; what the product requires of every
        // managed attempt follows it as the product's own instruction.
        let instructions = self.managed_instructions();
        let delivered = (|| -> std::io::Result<()> {
            stdin.write_all(prompt.as_bytes())?;
            stdin.write_all(b"\n\n")?;
            stdin.write_all(instructions.as_bytes())?;
            stdin.write_all(b"\n")?;
            stdin.flush()
        })();
        drop(stdin);
        if let Err(error) = delivered {
            end_process_tree_or_keep(&mut child);
            return Err(RuntimeError::Process(error));
        }
        if let Err(error) = verify_admitted_programs(&descriptor.launch_chain) {
            end_process_tree_or_keep(&mut child);
            return Err(error);
        }
        if executable_digest(&descriptor.executable)? != descriptor.executable_digest {
            end_process_tree_or_keep(&mut child);
            return Err(RuntimeError::InvalidProfile(
                "Claude executable changed while the prepared launch was starting".to_owned(),
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
        let version = match self.admitted_build(&admitted, &environment_values)? {
            Ok((_, reported)) => reported,
            Err(report) => return Ok(report),
        };
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
                "pinned local profile ready: minimum_version={}, executable_digest={}, model={}, api_provider={api_provider}, auth_method={auth_method}, credential={}, effort={}, prompt_policy={}, permission_mode={}, builtin_tools={}, coordination_tools={}, native_subagents=disabled, setting_sources=none, strict_mcp_config=true, max_budget_usd={}, permitted_overshoot_microusd={}, wall_time_limit_ms={}, output_limit_bytes={}",
                self.profile.minimum_version,
                admitted.digest,
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
        let environment = self.isolated_environment()?;
        let measured_build =
            match self.admitted_build(&executable, &environment.values(None, None, None)?)? {
                Ok((build, _)) => build,
                Err(report) => {
                    return Err(RuntimeError::InvalidProfile(format!(
                        "Claude profile is not ready: {}",
                        report.detail
                    )));
                }
            };
        let (mcp, coordination_executable) = match &request.mcp {
            Some(binding) => {
                let executable = VerifiedExecutable::admit(&binding.executable, "ymp-agent-mcp")?;
                let mut binding = binding.clone();
                binding.executable = executable.execution_path.clone();
                (Some(binding), Some(executable))
            }
            None => (None, None),
        };
        // The reader of the operator's credential runs before the runtime image is loaded and its
        // output reaches the managed process, so it is admitted with the chain and named in the
        // launch record rather than treated as an incidental utility.
        let mut launch_chain = self.launch_chain.admit()?;
        if let Some(credential) = &self.credential
            && let Some(reader) = credential.reader()?
        {
            launch_chain.push(reader);
        }
        let launch = ClaudeLaunch {
            executable,
            measured_build,
            coordination_executable,
            launch_chain,
            profile: self.profile.clone(),
            workspace: request.workspace.clone(),
            attempt_id: request.attempt_id.clone(),
            invocation_id: request.invocation_id.clone(),
            mcp,
            environment,
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

    /// Binds a turn's reported cost to the models that produced it. The matched-budget comparison
    /// reads this number as evidence of one model route, so a cost with no breakdown at all, a
    /// total whose breakdown does not add up to it, and a breakdown naming a model this profile
    /// never admitted are all refused instead of being recorded as unattributed numbers.
    fn admit_cost_attribution(
        &self,
        reported_microusd: u64,
        breakdown: &[ModelSpend],
    ) -> Result<(), RuntimeError> {
        if breakdown.is_empty() {
            if reported_microusd == 0 {
                return Ok(());
            }
            return Err(RuntimeError::MalformedEvent(format!(
                "Claude reported {reported_microusd} microUSD for a turn without naming the models that spent it"
            )));
        }
        let attributed = breakdown
            .iter()
            .map(|spend| spend.cost_microusd)
            .fold(0_u64, u64::saturating_add);
        // Each share is rounded to whole microdollars before it is summed, so the sum may trail the
        // reported total by less than one microdollar per model and by nothing else.
        if attributed.abs_diff(reported_microusd) > breakdown.len() as u64 {
            return Err(RuntimeError::MalformedEvent(format!(
                "Claude attributed {attributed} microUSD across {} model(s) for a turn it reported as {reported_microusd} microUSD",
                breakdown.len()
            )));
        }
        if let Some(unadmitted) = breakdown
            .iter()
            .find(|spend| spend.model != self.launch.profile.model && spend.cost_microusd > 0)
        {
            return Err(RuntimeError::InvalidProfile(format!(
                "the unadmitted model {} spent {} microUSD, profile admits {}",
                unadmitted.model, unadmitted.cost_microusd, self.launch.profile.model
            )));
        }
        Ok(())
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
            cost_by_model: model_breakdown(event)?,
            wall_time_ms: 0,
            protected_queries: 0,
            in_flight_excess: InFlightExcess::default(),
        };
        self.admit_cost_attribution(
            observed.cost_microusd.unwrap_or_default(),
            &observed.cost_by_model,
        )?;
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

    /// Rejects an invocation whose effective configuration differs from the declared profile.
    /// Claude Code reports its resolved build, model, permission mode, tool set, coordination
    /// servers and every ambient extension in `system`/`init`, which precedes model traffic.
    fn admit_session(&mut self, event: &Value) -> Result<String, RuntimeError> {
        let build = string_field(event, "claude_code_version")?;
        if build != self.launch.measured_build.build {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude reported build {build}, admission measured {}",
                self.launch.measured_build.build
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
            end_process_tree_or_keep(&mut self.child);
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
            end_process_tree_or_keep(&mut self.child);
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
            end_process_tree_or_keep(&mut self.child);
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
                        end_process_tree_or_keep(&mut self.child);
                        self.completed = true;
                        self.terminal = true;
                        return Err(error);
                    }
                };
                if let Some(expected) = &self.session_id {
                    if expected != &session_id {
                        end_process_tree_or_keep(&mut self.child);
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
                    end_process_tree_or_keep(&mut self.child);
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
            end_process_tree_or_keep(&mut self.child);
        }
    }
}

/// Resolves the Claude Code the operator has installed. The path is deliberately not derived from
/// the pinned version: Claude Code installs each release under a version-named file, so addressing
/// one build by name refuses the release the operator upgraded to and reports it as missing. The
/// installation an explicit `CLAUDE_CONFIG_DIR` names wins over the search path, and whichever file
/// is resolved is copied and held by digest before it runs, so the identity of what executed is
/// recorded rather than assumed from its path.
fn installed_executable_path() -> PathBuf {
    installed_executable_under(std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from))
}

fn installed_executable_under(configuration: Option<PathBuf>) -> PathBuf {
    if let Some(configuration) = configuration {
        let installed = configuration.join("claude");
        if installed.is_file() {
            return installed;
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

/// Reads the per-model breakdown of a terminal result. A missing breakdown is reported as an empty
/// one rather than assigned to the admitted model, so the attribution rule refuses the cost instead
/// of inventing evidence the runtime did not report.
fn model_breakdown(event: &Value) -> Result<Vec<ModelSpend>, RuntimeError> {
    let Some(reported) = event.get("modelUsage") else {
        return Ok(Vec::new());
    };
    let reported = reported.as_object().ok_or_else(|| {
        RuntimeError::MalformedEvent(
            "Claude per-model accounting is not a model-keyed object".to_owned(),
        )
    })?;
    let mut breakdown: Vec<ModelSpend> = reported
        .iter()
        .map(|(model, spend)| {
            let cost = spend
                .get("costUSD")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    RuntimeError::MalformedEvent(format!(
                        "Claude reported no cost for the model {model}"
                    ))
                })?;
            Ok(ModelSpend {
                model: model.clone(),
                cost_microusd: usd_to_microusd(cost),
            })
        })
        .collect::<Result<_, RuntimeError>>()?;
    breakdown.sort_by(|left, right| left.model.cmp(&right.model));
    Ok(breakdown)
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

/// Refuses an environment the profile never approved. The check is stated over the names actually
/// about to reach the child rather than over the code that built them, so a variable added to a
/// prepared launch is refused with its own name instead of being reported as an anonymous change.
fn admit_environment_names<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> Result<(), RuntimeError> {
    let mut admitted: Vec<&str> = Vec::new();
    for name in names {
        if !ALLOWED_ENVIRONMENT_NAMES.contains(&name) {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved environment variable {name} reached a managed Claude invocation"
            )));
        }
        if admitted.contains(&name) {
            return Err(RuntimeError::InvalidProfile(format!(
                "environment variable {name} was declared twice for one managed invocation"
            )));
        }
        admitted.push(name);
    }
    Ok(())
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
        APPROVED_BUILTIN_TOOLS, APPROVED_SEARCH_PATH, ClaudeProfile, ClaudeRuntime,
        G3_MAX_BUDGET_MICROUSD, G3_MAX_IN_FLIGHT_OVERSHOOT_MICROUSD, InstalledBuild,
        MINIMUM_CLAUDE_VERSION, PINNED_CLAUDE_MODEL, installed_executable_path,
        installed_executable_under,
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
        fixture_reporting(directory, name, "2.1.227 (Claude Code)", body)
    }

    /// A fixture that answers `--version` with a release of the caller's choosing, so admission can
    /// be measured against builds this host does not have installed.
    fn fixture_reporting(directory: &Path, name: &str, version: &str, body: &str) -> PathBuf {
        let path = directory.join(name);
        fs::write(
            &path,
            format!(
                r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '{version}'
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
            (
                "unapproved search-path directory",
                ClaudeProfile {
                    search_path: vec!["/usr/bin".to_owned(), "/opt/planted/bin".to_owned()],
                    ..ClaudeProfile::default()
                },
            ),
            (
                "empty search path",
                ClaudeProfile {
                    search_path: Vec::new(),
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

    /// A body that reports one completed turn, naming `build` as the release the session resolved.
    fn completing_body(build: &str) -> String {
        format!(
            r##"cat >/dev/null
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-admitted","claude_code_version":"{build}","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.0,"modelUsage":{{}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
"##
        )
    }

    /// The change this profile turns on: a release newer than the floor starts instead of being
    /// refused for not matching one frozen build.
    #[test]
    fn a_release_newer_than_the_floor_is_admitted() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture_reporting(
            directory.path(),
            "claude-newer",
            "2.1.232 (Claude Code)",
            &completing_body("2.1.232"),
        );
        let runtime = ClaudeRuntime::new(&executable).without_delegated_credential();
        let probe = runtime.probe().expect("probe the newer release");
        assert_eq!(probe.readiness, Readiness::Ready, "{}", probe.detail);
        assert_eq!(probe.version.as_deref(), Some("2.1.232 (Claude Code)"));

        let mut session = start(&executable, directory.path(), "invocation-newer")
            .expect("start the newer build");
        expect_launch(session.as_mut());
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { .. }
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("completed")
                .expect("event")
                .event,
            RuntimeEventKind::Completed { .. }
        ));
    }

    /// The negative half of the widened admission: it is a floor and not an open door. A release
    /// below the floor, and a file that answers `--version` without naming this product, are both
    /// refused, and the ordering is read as numbers rather than as text.
    #[test]
    fn a_release_below_the_floor_or_of_another_product_is_refused() {
        for (label, reported, expected) in [
            (
                "older release",
                "2.1.226 (Claude Code)",
                "profile requires at least 2.1.227 (Claude Code), found 2.1.226 (Claude Code)",
            ),
            (
                "release whose text sorts high but whose number is low",
                "2.1.99 (Claude Code)",
                "profile requires at least 2.1.227 (Claude Code), found 2.1.99 (Claude Code)",
            ),
            (
                "another product",
                "9.9.9 (Other Code)",
                "profile requires a Claude Code build of at least 2.1.227 (Claude Code), found 9.9.9 (Other Code)",
            ),
        ] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let executable =
                fixture_reporting(directory.path(), "claude-refused", reported, "exit 0\n");
            let probe = ClaudeRuntime::new(&executable)
                .without_delegated_credential()
                .probe()
                .expect("probe the refused build");
            assert_eq!(
                probe.readiness,
                Readiness::Incompatible,
                "{label} was admitted"
            );
            assert_eq!(probe.detail, expected, "{label}");

            let started = start(&executable, directory.path(), "invocation-refused");
            assert!(
                matches!(started, Err(RuntimeError::InvalidProfile(_))),
                "{label} started a session"
            );
        }
    }

    /// Admitting the installed release only stays honest while the release that was measured is the
    /// release that ran. A session whose `init` names any other build is refused in both
    /// directions, including a build that would clear the floor on its own.
    #[test]
    fn a_session_that_names_a_build_other_than_the_measured_one_is_refused() {
        for (label, reported, announced) in [
            (
                "a build below the measured one",
                "2.1.232 (Claude Code)",
                "2.1.227",
            ),
            (
                "a build above the measured one",
                "2.1.227 (Claude Code)",
                "2.1.232",
            ),
        ] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let executable = fixture_reporting(
                directory.path(),
                "claude-disagreeing",
                reported,
                &completing_body(announced),
            );
            let mut session = start(&executable, directory.path(), "invocation-disagreeing")
                .expect("start the disagreeing fixture");
            expect_launch(session.as_mut());
            assert!(
                matches!(
                    session
                        .next_event()
                        .expect("typed failure")
                        .expect("event")
                        .event,
                    RuntimeEventKind::Failed {
                        kind: RuntimeFailureKind::Protocol,
                        ..
                    }
                ),
                "{label} was admitted"
            );
        }
    }

    /// The executable is resolved from the installation, never composed from the pinned version, so
    /// upgrading the release does not make the runtime report itself as missing.
    #[test]
    fn the_executable_is_resolved_from_the_installation_and_not_from_the_version() {
        let directory = tempfile::tempdir().expect("temporary directory");
        assert_eq!(installed_executable_under(None), PathBuf::from("claude"));
        assert_eq!(
            installed_executable_under(Some(directory.path().to_owned())),
            PathBuf::from("claude"),
            "a configuration directory holding no executable falls back to the search path"
        );
        let configured = directory.path().join("claude");
        fs::write(&configured, b"#!/bin/sh\n").expect("write configured executable");
        assert_eq!(
            installed_executable_under(Some(directory.path().to_owned())),
            configured
        );
        let floor = InstalledBuild::parse(MINIMUM_CLAUDE_VERSION).expect("floor");
        let resolved = installed_executable_path();
        assert!(
            !resolved.to_string_lossy().contains(floor.build.as_str()),
            "the resolved path {} still names the pinned build",
            resolved.display()
        );
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
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.125,"modelUsage":{{"claude-opus-5":{{"inputTokens":111,"outputTokens":3,"costUSD":0.125}}}},"usage":{{"input_tokens":11,"cache_creation_input_tokens":100,"cache_read_input_tokens":7,"output_tokens":3,"output_tokens_details":{{"thinking_tokens":2}}}}}}'
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

    /// The recorded cost is only evidence of a model route while the runtime named the models that
    /// produced it and the shares they reported add up to it. A cost with no breakdown, a total its
    /// breakdown does not add up to, and a total a model this profile never admitted helped to
    /// produce are all refused.
    #[test]
    fn a_cost_its_model_breakdown_does_not_support_is_refused() {
        for (label, result) in [
            (
                "breakdown below the reported total",
                r#"{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.125,"modelUsage":{"claude-opus-5":{"inputTokens":11,"outputTokens":3,"costUSD":0.030}},"usage":{"input_tokens":11,"output_tokens":3}}"#,
            ),
            (
                "breakdown above the reported total",
                r#"{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.125,"modelUsage":{"claude-opus-5":{"inputTokens":11,"outputTokens":3,"costUSD":0.900}},"usage":{"input_tokens":11,"output_tokens":3}}"#,
            ),
            (
                "a reported cost with no breakdown at all",
                r#"{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.125,"usage":{"input_tokens":11,"output_tokens":3}}"#,
            ),
            (
                "an empty breakdown for a reported cost",
                r#"{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.125,"modelUsage":{},"usage":{"input_tokens":11,"output_tokens":3}}"#,
            ),
            (
                "spend by a model the profile never admitted",
                r#"{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.125,"modelUsage":{"claude-opus-5":{"inputTokens":11,"outputTokens":3,"costUSD":0.100},"claude-unadmitted":{"inputTokens":4,"outputTokens":1,"costUSD":0.025}},"usage":{"input_tokens":11,"output_tokens":3}}"#,
            ),
        ] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let executable = fixture(
                directory.path(),
                "claude-unattributed-cost",
                &format!(
                    r##"cat >/dev/null
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-attribution","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{result}'
"##
                ),
            );
            let mut session = start(&executable, directory.path(), "invocation-attribution")
                .expect("start fixture");
            expect_launch(session.as_mut());
            assert!(matches!(
                session.next_event().expect("started").expect("event").event,
                RuntimeEventKind::Started { .. }
            ));
            let observed = session.next_event().expect("typed failure").expect("event");
            assert!(
                matches!(
                    observed.event,
                    RuntimeEventKind::Failed {
                        kind: RuntimeFailureKind::Protocol,
                        ..
                    }
                ),
                "{label} was recorded as an attributed cost"
            );
        }
    }

    /// The child receives the approved environment and nothing else, and the same environment is
    /// what the launch descriptor carries into the profile record. The operator's own search path
    /// is not delegated, so a directory only the operator can reach is unreachable from the child.
    #[test]
    fn only_the_approved_environment_and_search_path_reach_the_child_and_its_record() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-environment",
            &format!(
                r##"cat >/dev/null
/usr/bin/env > child.env
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-environment","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.001,"modelUsage":{{"claude-opus-5":{{"inputTokens":1,"outputTokens":1,"costUSD":0.001}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
"##
            ),
        );
        let runtime = ClaudeRuntime::new(&executable).without_delegated_credential();
        let descriptor = runtime
            .prepare_launch(&request(directory.path(), "invocation-environment"))
            .expect("prepare the launch")
            .expect("launch descriptor");
        let recorded: Vec<&str> = descriptor
            .environment
            .iter()
            .map(|variable| variable.name.as_str())
            .collect();
        assert_eq!(
            recorded,
            [
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
                "CLAUDE_CONFIG_DIR",
                "HOME",
                "NO_COLOR",
                "PATH",
                "TMPDIR",
            ],
            "the profile record carries an environment the profile did not approve"
        );
        let recorded_search_path = descriptor
            .environment
            .iter()
            .find(|variable| variable.name == "PATH")
            .and_then(|variable| variable.value.clone())
            .expect("the record states the search path");
        assert_eq!(recorded_search_path, APPROVED_SEARCH_PATH.join(":"));

        let mut session = runtime
            .start_prepared(
                request(directory.path(), "invocation-environment"),
                Some(&descriptor),
            )
            .expect("start the prepared launch");
        while session.next_event().expect("runtime event").is_some() {}
        drop(session);

        let observed = fs::read_to_string(directory.path().join("child.env"))
            .expect("the child recorded its environment");
        // `PWD`, `SHLVL` and `_` are created by the observing shell rather than inherited, so the
        // admitted set is compared without them.
        let mut names: Vec<&str> = observed
            .lines()
            .filter_map(|line| line.split_once('=').map(|(name, _)| name))
            .filter(|name| !["PWD", "SHLVL", "_"].contains(name))
            .collect();
        names.sort_unstable();
        assert_eq!(names, recorded, "the child received an unapproved variable");
        let child_search_path = observed
            .lines()
            .find_map(|line| line.strip_prefix("PATH="))
            .expect("the child received a search path");
        assert_eq!(child_search_path, recorded_search_path);
        // Negative pressure on the delegation itself: whatever the operator's own search path adds
        // beyond the approved directories must be unreachable from the child.
        let operator_path = std::env::var("PATH").unwrap_or_default();
        if let Some(operator_only) = operator_path
            .split(':')
            .find(|directory| !directory.is_empty() && !APPROVED_SEARCH_PATH.contains(directory))
        {
            assert!(
                !child_search_path
                    .split(':')
                    .any(|entry| entry == operator_only),
                "the operator search-path directory {operator_only} reached the child"
            );
        }
    }

    /// A variable added to a prepared launch is refused by name, and the runtime never starts.
    #[test]
    fn an_unapproved_environment_variable_refuses_the_launch() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-injected-environment",
            "printf '%s\\n' started > started.marker\ncat >/dev/null\n",
        );
        let runtime = ClaudeRuntime::new(&executable).without_delegated_credential();
        let invocation = request(directory.path(), "invocation-injected");
        let mut descriptor = runtime
            .prepare_launch(&invocation)
            .expect("prepare the launch")
            .expect("launch descriptor");
        descriptor
            .environment
            .push(ymp_runtime_api::LaunchEnvironmentVariable {
                name: "ANTHROPIC_API_KEY".to_owned(),
                value: Some("planted".to_owned()),
                value_digest: ymp_runtime_api::evidence_digest(b"planted"),
                confidential: false,
            });
        let error = runtime
            .start_prepared(invocation, Some(&descriptor))
            .err()
            .expect("an unapproved variable refuses the launch");
        match &error {
            RuntimeError::InvalidProfile(detail) => assert!(
                detail.contains("ANTHROPIC_API_KEY"),
                "the refusal does not name the variable: {detail}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(
            !directory.path().join("started.marker").exists(),
            "the runtime started with an unapproved environment"
        );
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
printf '%s\n' '{{"type":"result","subtype":"error_max_budget_usd","is_error":true,"terminal_reason":"budget_exhausted","total_cost_usd":0.9,"modelUsage":{{"claude-opus-5":{{"inputTokens":50,"outputTokens":7,"costUSD":0.9}}}},"usage":{{"input_tokens":41,"cache_creation_input_tokens":9,"cache_read_input_tokens":5,"output_tokens":7,"output_tokens_details":{{"thinking_tokens":1}}}}}}'
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
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":4.0,"modelUsage":{{"claude-opus-5":{{"inputTokens":1,"outputTokens":1,"costUSD":4.0}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
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
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"modelUsage":{{"claude-opus-5":{{"inputTokens":1,"outputTokens":1,"costUSD":0.01}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"modelUsage":{{"claude-opus-5":{{"inputTokens":1,"outputTokens":1,"costUSD":0.01}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
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
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"modelUsage":{"claude-opus-5":{"inputTokens":1,"outputTokens":1,"costUSD":0.01}},"usage":{"input_tokens":1,"output_tokens":1}}'
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
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.5,"modelUsage":{{"claude-opus-5":{{"inputTokens":1,"outputTokens":1,"costUSD":0.5}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
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
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.125,"modelUsage":{{"claude-opus-5":{{"inputTokens":18,"outputTokens":3,"costUSD":0.125}}}},"usage":{{"input_tokens":11,"cache_read_input_tokens":7,"output_tokens":3}}}}'
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
        const RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"modelUsage":{"claude-opus-5":{"inputTokens":1,"outputTokens":1,"costUSD":0.01}},"usage":{"input_tokens":1,"output_tokens":1}}"#;

        let directory = tempfile::tempdir().expect("temporary directory");
        let bridge = fixture(directory.path(), "claude-bridge", "exit 0");
        let coordinated_workspace = directory.path().join("coordinated");
        let plain_workspace = directory.path().join("plain");
        fs::create_dir(&coordinated_workspace).expect("coordinated workspace");
        fs::create_dir(&plain_workspace).expect("plain workspace");
        let coordinated_executable = fixture(
            directory.path(),
            "claude-coordinated-delivery",
            &format!(
                r##"cat > delivered.stdin
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-delivery","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":["Bash","Edit","Glob","Grep","Read","Write","mcp__ymp__read_control","mcp__ymp__read_events","mcp__ymp__submit","mcp__ymp__yield"],"mcp_servers":[{{"name":"ymp","status":"connected"}}],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{RESULT}'
"##
            ),
        );
        let plain_executable = fixture(
            directory.path(),
            "claude-plain-delivery",
            &format!(
                r##"cat > delivered.stdin
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-delivery","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{RESULT}'
"##
            ),
        );

        let mut delivered = Vec::new();
        for (executable, workspace, mcp) in [
            (
                &coordinated_executable,
                &coordinated_workspace,
                Some(McpBinding {
                    executable: bridge.clone(),
                    socket_path: directory.path().join("agent.sock"),
                    token: "fixture-token".to_owned(),
                }),
            ),
            (&plain_executable, &plain_workspace, None),
        ] {
            let mut session = ClaudeRuntime::new(executable)
                .without_delegated_credential()
                .start(InvocationRequest {
                    invocation_id: "invocation-delivery".to_owned(),
                    attempt_id: "attempt-delivery".to_owned(),
                    workspace: workspace.clone(),
                    mcp,
                    prompt: REQUEST.to_owned(),
                    cancellation: Default::default(),
                })
                .expect("start fixture");
            expect_launch(session.as_mut());
            while let Some(event) = session.next_event().expect("delivery event") {
                if matches!(event.event, RuntimeEventKind::Completed { .. }) {
                    break;
                }
            }
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
            coordinated.contains("mcp__ymp__submit"),
            "a coordinated invocation was not told how a candidate is published: {coordinated}"
        );
        assert!(
            !plain.contains("mcp__ymp__submit"),
            "an invocation without the coordination bridge was told to call a tool it does not \
             have: {plain}"
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

    /// Builds a substitutable stand-in for a system program of the launch chain. The operating
    /// system refuses to execute a copy of `/bin/sh` or `/usr/bin/env`, measured as a kill by
    /// signal, so a check enters the real program through a script whose own bytes can be replaced
    /// after admission.
    /// The program that reads the operator's credential hands material to the managed process, so
    /// it is admitted under the same rule as the rest of the trusted chain rather than executed as
    /// an incidental utility.
    #[test]
    #[cfg(target_os = "macos")]
    fn the_credential_reader_is_admitted_before_it_reads_the_credential() {
        use super::CredentialSource;

        let reader = CredentialSource::MacosKeychain
            .reader()
            .expect("admit the keychain reader")
            .expect("the keychain source reads through a program");
        assert_eq!(reader.role, ymp_runtime_api::ProgramRole::CredentialReader);
        assert_eq!(
            reader.path,
            Path::new(CredentialSource::MACOS_KEYCHAIN_READER)
        );
        assert_eq!(
            reader.identity,
            ymp_runtime_api::ProgramIdentity::SystemPath,
            "the credential reader is bound to a location this account cannot write"
        );
        reader.verify().expect("the admitted reader verifies");

        let file = tempfile::tempdir().expect("temporary directory");
        let credential = file.path().join(".credentials.json");
        fs::write(&credential, b"{}").expect("write credential");
        assert!(
            CredentialSource::File(credential)
                .reader()
                .expect("a file source is inspected")
                .is_none(),
            "a credential read from a file executes no program"
        );
    }

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

    #[test]
    fn every_admitted_launch_chain_program_executes_and_refuses_replacement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = fixture(
            directory.path(),
            "claude-chain",
            &format!(
                r##"cat >/dev/null
printf '%s\n' admitted > admitted-runtime.marker
printf '%s\n' '{{"type":"system","subtype":"init","session_id":"session-chain","claude_code_version":"2.1.227","model":"claude-opus-5","permissionMode":"acceptEdits","tools":{INIT_TOOLS},"mcp_servers":[],"slash_commands":[],"plugins":[],"skills":[]}}'
printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed","total_cost_usd":0.001,"modelUsage":{{"claude-opus-5":{{"inputTokens":1,"outputTokens":1,"costUSD":0.001}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}'
"##
            ),
        );
        let shell = chain_stand_in(directory.path(), "chain-shell", "/bin/sh");
        let sanitiser = chain_stand_in(directory.path(), "chain-sanitiser", "/usr/bin/env");
        let runtime = ClaudeRuntime::new(&executable)
            .without_delegated_credential()
            .with_launch_chain(stand_in_chain(&shell, &sanitiser));

        let admitted_workspace = directory.path().join("admitted");
        fs::create_dir(&admitted_workspace).expect("admitted workspace");
        let prepared = runtime
            .prepare_launch(&request(&admitted_workspace, "invocation-chain-shape"))
            .expect("prepare the admitted chain")
            .expect("launch descriptor");
        assert_eq!(
            prepared
                .launch_chain
                .iter()
                .map(|program| program.role)
                .collect::<Vec<_>>(),
            vec![
                ymp_runtime_api::ProgramRole::LaunchShell,
                ymp_runtime_api::ProgramRole::EnvironmentSanitiser,
            ],
            "a run that delegates no credential enters no credential reader"
        );
        let mut session = runtime
            .start(request(&admitted_workspace, "invocation-chain-admitted"))
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
            let request = request(&workspace, &format!("invocation-chain-replaced-{index}"));
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
}
