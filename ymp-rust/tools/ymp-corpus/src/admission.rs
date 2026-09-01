use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use ymp_agent_mcp::{MCP_PROTOCOL_VERSION, McpServer, tool_catalog};
use ymp_agent_rpc::{AgentRpcServer, SocketToolHandler};
use ymp_application::{Application, WorkspaceSubmission};
use ymp_board::budget::{Allowance, CommunicationAllowance};
use ymp_board::{
    BoardCommand, BoardEvent, InitialMember, OpenScope, RegisterParticipant, Rights, ScopeKind,
};
use ymp_domain::{Budget, Command as DomainCommand};
use ymp_runtime_api::{
    InvocationRequest, Readiness, RuntimeDriver, RuntimeEvent, RuntimeEventKind, RuntimeSession,
    Usage,
};
use ymp_runtime_codex::{
    CodexRuntime, PINNED_CODEX_API_ORIGIN, PINNED_CODEX_MODEL, PINNED_CODEX_PROMPT_POLICY,
    PINNED_CODEX_VERSION,
};
use ymp_runtime_fake::{FakeRuntime, ScriptStep};

use crate::report_json;

const ADMISSION_ID: &str = "weak-diagnostic-admission-v1";
const FROZEN_MANIFEST_SHA256: &str =
    "48f6107e12764b40f508b0f07ec75156735739750d43f4872278b2b39947e24b";
const ROOT_PARTICIPANT: &str = "participant-root";
const READER: &str = "reader-b";
const OUTSIDER: &str = "unauthorized-reader";
const SCOPE: &str = "transport-scope";
const REPORT_FILE: &str = "admission-report.json";
const REPORT_DIGEST_FILE: &str = "admission-report.sha256";
const PROBE_FILE: &str = "stage-two-probe.json";
const TOOL_RESULT_LIMIT: u64 = 1024;

#[derive(Debug, Subcommand)]
pub enum AdmissionCommand {
    Check {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
    },
    Rehearse {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
        #[arg(long)]
        root: PathBuf,
    },
}

impl AdmissionCommand {
    pub fn execute(&self) -> Result<()> {
        match self {
            Self::Check { manifest, digest } => {
                let loaded = load_manifest(manifest, digest)?;
                let current = std::env::current_dir().context("resolve admission working root")?;
                let stage_one = evaluate_stage_one(&loaded.manifest, Path::new("codex"), &current);
                let report = AdmissionReport::check(&loaded, stage_one);
                println!("{}", report_json(&report)?);
                ensure!(
                    report.stage_one.compatible,
                    "installed route is incompatible: {}",
                    report.stage_one.issues.join("; ")
                );
            }
            Self::Rehearse {
                manifest,
                digest,
                root,
            } => {
                let loaded = load_manifest(manifest, digest)?;
                let roots = EvaluationRoots::prepare(root, manifest)?;
                let stage_one =
                    evaluate_stage_one(&loaded.manifest, Path::new("codex"), &roots.project);
                let transport = rehearse_transport(&loaded.manifest, &roots)?;
                let stage_two = evaluate_probe(&loaded, &roots.export.join(PROBE_FILE));
                let report = AdmissionReport::rehearsal(&loaded, stage_one, stage_two, transport);
                validate_report_claims(&loaded.manifest, &report)?;
                write_immutable_report(&roots.export, &report)?;
                println!("{}", report_json(&report)?);
                ensure!(
                    report.stage_one.compatible,
                    "installed route is incompatible: {}",
                    report.stage_one.issues.join("; ")
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionManifest {
    pub schema_version: u32,
    pub admission_id: String,
    pub artifact_role: String,
    pub freeze_state: String,
    pub model_calls_at_freeze: u64,
    pub runtime: RuntimeContract,
    pub tool_host: ToolHostContract,
    pub git_trust: GitTrustPolicy,
    pub read_limits: ReadLimits,
    pub transport_schedules: Vec<TransportSchedule>,
    pub stage_two: ProbeBudget,
    pub report: ReportContract,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeContract {
    pub driver_crate: String,
    pub driver_version: String,
    pub cli_program: String,
    pub cli_version: String,
    pub model: String,
    pub route: String,
    pub api_origin: String,
    pub reasoning_effort: String,
    pub approval_policy: String,
    pub prompt_policy: String,
    pub sandbox: String,
    pub wall_time_limit_ms: u64,
    pub output_limit_bytes: u64,
    pub required_exec_flags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostContract {
    pub transport: String,
    pub binding_required: bool,
    pub endpoint_scope: String,
    pub mcp_protocol_version: String,
    pub tool_schema_sha256: String,
    pub required_tools: Vec<String>,
    pub request_participant_capability_gated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitTrustPolicy {
    pub repository_required: bool,
    pub skip_repository_check: bool,
    pub user_config: String,
    pub project_rules: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadLimits {
    pub max_reads_per_reader: u32,
    pub max_reads_unauthorized_reader: u32,
    pub max_bytes_per_read: u64,
    pub schedule_content_independent: bool,
    pub polling_until_success: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransportSchedule {
    pub scenario: Scenario,
    pub reader_read_cap: u32,
    pub windows: Vec<ProcessWindow>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessWindow {
    pub ordinal: u32,
    pub actor: String,
    pub action: WindowAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    S1,
    S2,
    S3,
}

impl Scenario {
    fn as_str(self) -> &'static str {
        match self {
            Self::S1 => "s1",
            Self::S2 => "s2",
            Self::S3 => "s3",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowAction {
    StartFake,
    Publish,
    ReadBoard,
    UnauthorizedRead,
    CompleteFake,
    YieldFakeAndTool,
    AssertNoBoardWake,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeBudget {
    pub evidence_file: String,
    pub stage: String,
    pub budget_source: String,
    pub max_model_calls: u64,
    pub max_input_tokens: u64,
    pub max_cached_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_reasoning_tokens: u64,
    pub max_wall_time_ms: u64,
    pub task_output_forbidden: bool,
    pub arm_observation_forbidden: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportContract {
    pub file: String,
    pub digest_file: String,
    pub immutable_create_new: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeEvidence {
    pub schema_version: u32,
    pub probe_id: String,
    pub admission_manifest_sha256: String,
    pub source: ProbeSource,
    pub status: ProbeOutcome,
    pub stage: String,
    pub budget_source: String,
    pub nonce_input: String,
    pub nonce_readback: Option<String>,
    pub no_task_output: bool,
    pub arm_observation: bool,
    pub model_calls: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub wall_time_ms: u64,
    pub route: String,
    pub cli_version: String,
    pub driver_version: String,
    pub cost: ProbeCost,
    pub input_digest: String,
    pub output_digest: Option<String>,
    pub event_digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeSource {
    Fake,
    ToolHost,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeOutcome {
    Succeeded,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeCost {
    pub availability: CostAvailability,
    pub currency: Option<String>,
    pub minor_units: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CostAvailability {
    Reported,
    Unavailable,
}

#[derive(Clone, Debug)]
struct LoadedManifest {
    manifest: AdmissionManifest,
    digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionReport {
    pub schema_version: u32,
    pub admission_id: String,
    pub manifest_sha256: String,
    pub model_calls: u64,
    pub stage_one: StageOneReport,
    pub stage_two: StageTwoReport,
    pub transport: Option<TransportReport>,
    pub model_ready: bool,
    pub arm_schedule: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StageOneReport {
    pub compatible: bool,
    pub installed_cli_version: Option<String>,
    pub expected_cli_version: String,
    pub driver_version: String,
    pub route: String,
    pub tool_schema_sha256: String,
    pub git_repository: bool,
    pub binding_required: bool,
    pub model_calls: u64,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StageTwoReport {
    pub status: StageTwoStatus,
    pub evidence_digest: Option<String>,
    pub reason: String,
    pub can_open_model_gate: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageTwoStatus {
    Missing,
    Refused,
    Mismatched,
    Unaccounted,
    FakeValidated,
    Unattested,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransportReport {
    pub passed: bool,
    pub schedule_sha256: String,
    pub scenarios: Vec<ScenarioReport>,
    pub model_calls: u64,
    pub production_path: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioReport {
    pub scenario: Scenario,
    pub executed_windows: Vec<ProcessWindow>,
    pub reader_reads: u32,
    pub unauthorized_reads: u32,
    pub published_records: u32,
    pub delivery_records: u32,
    pub delivered_message_ids: Vec<String>,
    pub unauthorized_message_count: u32,
    pub board_wake_count: u32,
    pub recruitment_advertised_without_capability: bool,
    pub terminal_a: String,
    pub terminal_b: String,
    pub fake_runtime_event_digest: String,
    pub passed: bool,
}

impl AdmissionReport {
    fn check(loaded: &LoadedManifest, stage_one: StageOneReport) -> Self {
        Self {
            schema_version: 1,
            admission_id: ADMISSION_ID.to_owned(),
            manifest_sha256: loaded.digest.clone(),
            model_calls: 0,
            stage_one,
            stage_two: StageTwoReport::missing(),
            transport: None,
            model_ready: false,
            arm_schedule: None,
        }
    }

    fn rehearsal(
        loaded: &LoadedManifest,
        stage_one: StageOneReport,
        stage_two: StageTwoReport,
        transport: TransportReport,
    ) -> Self {
        Self {
            schema_version: 1,
            admission_id: ADMISSION_ID.to_owned(),
            manifest_sha256: loaded.digest.clone(),
            model_calls: 0,
            stage_one,
            stage_two,
            transport: Some(transport),
            model_ready: false,
            arm_schedule: None,
        }
    }
}

impl StageTwoReport {
    fn missing() -> Self {
        Self {
            status: StageTwoStatus::Missing,
            evidence_digest: None,
            reason: "stage-two no-task-output nonce probe evidence is missing".to_owned(),
            can_open_model_gate: false,
        }
    }
}

fn load_manifest(manifest_path: &Path, digest_path: &Path) -> Result<LoadedManifest> {
    let bytes = fs::read(manifest_path)
        .with_context(|| format!("read admission manifest {}", manifest_path.display()))?;
    let supplied = fs::read_to_string(digest_path)
        .with_context(|| format!("read admission digest {}", digest_path.display()))?;
    let digest = sha256_bytes(&bytes);
    validate_sha256(supplied.trim(), "admission manifest sidecar")?;
    ensure!(
        supplied.trim() == digest,
        "admission manifest digest mismatch"
    );
    ensure!(
        digest == FROZEN_MANIFEST_SHA256,
        "admission manifest changed after freeze"
    );
    let manifest: AdmissionManifest =
        serde_json::from_slice(&bytes).context("parse strict admission manifest")?;
    validate_manifest(&manifest)?;
    Ok(LoadedManifest { manifest, digest })
}

fn validate_manifest(manifest: &AdmissionManifest) -> Result<()> {
    ensure!(manifest.schema_version == 1, "unsupported admission schema");
    ensure!(
        manifest.admission_id == ADMISSION_ID,
        "admission identity changed"
    );
    ensure!(
        manifest.artifact_role == "development_only_zero_model_admission",
        "admission artifact claims an unsupported role"
    );
    ensure!(
        manifest.freeze_state == "frozen_before_model_call" && manifest.model_calls_at_freeze == 0,
        "admission was not frozen before model use"
    );
    let runtime = &manifest.runtime;
    ensure!(
        runtime.driver_crate == "ymp-runtime-codex",
        "runtime driver changed"
    );
    ensure!(
        runtime.driver_version == env!("CARGO_PKG_VERSION"),
        "driver version changed"
    );
    ensure!(runtime.cli_program == "codex", "runtime CLI changed");
    ensure!(
        runtime.cli_version == PINNED_CODEX_VERSION,
        "runtime CLI version changed"
    );
    ensure!(runtime.model == PINNED_CODEX_MODEL, "model route changed");
    ensure!(runtime.route == "openai_responses_chatgpt", "route changed");
    ensure!(
        runtime.api_origin == PINNED_CODEX_API_ORIGIN,
        "API origin changed"
    );
    ensure!(
        runtime.reasoning_effort == "low",
        "reasoning effort changed"
    );
    ensure!(
        runtime.approval_policy == "never",
        "approval policy changed"
    );
    ensure!(
        runtime.prompt_policy == PINNED_CODEX_PROMPT_POLICY,
        "prompt policy changed"
    );
    ensure!(
        runtime.sandbox == "workspace-write",
        "sandbox policy changed"
    );
    ensure!(
        runtime.wall_time_limit_ms == 600_000,
        "runtime window changed"
    );
    ensure!(
        runtime.output_limit_bytes == 16 * 1024 * 1024,
        "output cap changed"
    );
    ensure!(
        runtime.required_exec_flags
            == [
                "--json",
                "--ignore-user-config",
                "--ignore-rules",
                "--sandbox",
                "--model",
                "--disable",
                "--cd",
            ],
        "required CLI flags changed"
    );
    ensure!(
        manifest.tool_host.binding_required,
        "tool-host binding is missing"
    );
    ensure!(
        manifest.tool_host.transport == "stdio_mcp_private_rpc_application"
            && manifest.tool_host.endpoint_scope == "attempt_scoped_unix_socket",
        "tool-host transport changed"
    );
    ensure!(
        manifest.tool_host.mcp_protocol_version == MCP_PROTOCOL_VERSION,
        "MCP protocol version changed"
    );
    validate_sha256(&manifest.tool_host.tool_schema_sha256, "tool schema digest")?;
    ensure!(
        manifest.tool_host.required_tools
            == [
                "read_control",
                "read_events",
                "read_board",
                "publish",
                "request_participant",
                "yield",
                "submit",
            ],
        "tool-host schema set changed"
    );
    ensure!(
        manifest.tool_host.request_participant_capability_gated,
        "recruitment is advertised without an endpoint capability"
    );
    ensure!(
        manifest.git_trust
            == (GitTrustPolicy {
                repository_required: true,
                skip_repository_check: false,
                user_config: "ignored".to_owned(),
                project_rules: "ignored".to_owned(),
            }),
        "Git/config trust policy changed"
    );
    ensure!(
        manifest.read_limits
            == (ReadLimits {
                max_reads_per_reader: 2,
                max_reads_unauthorized_reader: 1,
                max_bytes_per_read: TOOL_RESULT_LIMIT,
                schedule_content_independent: true,
                polling_until_success: false,
            }),
        "read/window limits changed"
    );
    ensure!(
        manifest.transport_schedules == expected_schedules(),
        "transport schedule changed or can be selected after observation"
    );
    ensure!(
        manifest.stage_two.evidence_file == PROBE_FILE
            && manifest.stage_two.stage == "no_task_output_nonce_tool_host"
            && manifest.stage_two.budget_source == "admission_probe"
            && manifest.stage_two.max_model_calls == 1
            && manifest.stage_two.max_input_tokens == 32_768
            && manifest.stage_two.max_cached_input_tokens == 32_768
            && manifest.stage_two.max_output_tokens == 1_024
            && manifest.stage_two.max_reasoning_tokens == 1_024
            && manifest.stage_two.max_wall_time_ms == 120_000
            && manifest.stage_two.task_output_forbidden
            && manifest.stage_two.arm_observation_forbidden,
        "stage-two probe budget changed"
    );
    ensure!(
        manifest.report
            == (ReportContract {
                file: REPORT_FILE.to_owned(),
                digest_file: REPORT_DIGEST_FILE.to_owned(),
                immutable_create_new: true,
            }),
        "admission report contract changed"
    );
    Ok(())
}

fn expected_schedules() -> Vec<TransportSchedule> {
    vec![
        schedule(
            Scenario::S1,
            1,
            &[
                ("a", WindowAction::StartFake),
                ("a", WindowAction::Publish),
                ("a", WindowAction::CompleteFake),
                ("b", WindowAction::StartFake),
                ("b", WindowAction::ReadBoard),
                ("u", WindowAction::UnauthorizedRead),
                ("b", WindowAction::CompleteFake),
            ],
        ),
        schedule(
            Scenario::S2,
            2,
            &[
                ("b", WindowAction::StartFake),
                ("b", WindowAction::ReadBoard),
                ("a", WindowAction::StartFake),
                ("a", WindowAction::Publish),
                ("a", WindowAction::CompleteFake),
                ("b", WindowAction::ReadBoard),
                ("u", WindowAction::UnauthorizedRead),
                ("b", WindowAction::CompleteFake),
            ],
        ),
        schedule(
            Scenario::S3,
            1,
            &[
                ("b", WindowAction::StartFake),
                ("b", WindowAction::ReadBoard),
                ("b", WindowAction::YieldFakeAndTool),
                ("a", WindowAction::StartFake),
                ("a", WindowAction::Publish),
                ("a", WindowAction::CompleteFake),
                ("controller", WindowAction::AssertNoBoardWake),
                ("u", WindowAction::UnauthorizedRead),
            ],
        ),
    ]
}

fn schedule(
    scenario: Scenario,
    reader_read_cap: u32,
    actions: &[(&str, WindowAction)],
) -> TransportSchedule {
    TransportSchedule {
        scenario,
        reader_read_cap,
        windows: actions
            .iter()
            .enumerate()
            .map(|(index, (actor, action))| ProcessWindow {
                ordinal: index as u32 + 1,
                actor: (*actor).to_owned(),
                action: *action,
            })
            .collect(),
    }
}

fn evaluate_stage_one(
    manifest: &AdmissionManifest,
    executable: &Path,
    git_root: &Path,
) -> StageOneReport {
    let runtime = CodexRuntime::new(executable);
    let profile = runtime.profile();
    let mut issues = Vec::new();
    if profile.expected_version != manifest.runtime.cli_version
        || profile.model != manifest.runtime.model
        || profile.reasoning_effort != manifest.runtime.reasoning_effort
        || profile.approval_policy != manifest.runtime.approval_policy
        || profile.prompt_policy != manifest.runtime.prompt_policy
        || profile.wall_time_limit_ms != manifest.runtime.wall_time_limit_ms
        || profile.output_limit_bytes as u64 != manifest.runtime.output_limit_bytes
    {
        issues.push("manifest does not match the production Codex profile".to_owned());
    }
    let probe = runtime.probe();
    let (installed_cli_version, readiness) = match probe {
        Ok(probe) => {
            if probe.readiness != Readiness::Ready {
                issues.push(format!(
                    "runtime probe {:?}: {}",
                    probe.readiness, probe.detail
                ));
            }
            (probe.version, probe.readiness)
        }
        Err(error) => {
            issues.push(format!("runtime probe failed: {error}"));
            (None, Readiness::Unavailable)
        }
    };
    let help = Command::new(runtime.executable())
        .args(["exec", "--help"])
        .stdin(Stdio::null())
        .output();
    match help {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            for flag in &manifest.runtime.required_exec_flags {
                if !text.contains(flag) {
                    issues.push(format!("required Codex flag is unavailable: {flag}"));
                }
            }
        }
        Ok(output) => issues.push(format!("Codex help probe exited with {}", output.status)),
        Err(error) => issues.push(format!("Codex help probe failed: {error}")),
    }
    let tool_schema_sha256 = sha256_bytes(
        &serde_json::to_vec(&tool_catalog()).expect("tool catalog serialization cannot fail"),
    );
    if tool_schema_sha256 != manifest.tool_host.tool_schema_sha256 {
        issues.push("production MCP tool schema digest changed".to_owned());
    }
    let git_repository = Command::new("git")
        .args(["-C"])
        .arg(git_root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdin(Stdio::null())
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout == b"true\n");
    if manifest.git_trust.repository_required && !git_repository {
        issues.push("Git trust requirement is missing: project is not a Git repository".to_owned());
    }
    StageOneReport {
        compatible: issues.is_empty() && readiness == Readiness::Ready,
        installed_cli_version,
        expected_cli_version: manifest.runtime.cli_version.clone(),
        driver_version: manifest.runtime.driver_version.clone(),
        route: manifest.runtime.route.clone(),
        tool_schema_sha256,
        git_repository,
        binding_required: manifest.tool_host.binding_required,
        model_calls: 0,
        issues,
    }
}

#[derive(Clone, Debug)]
struct EvaluationRoots {
    project: PathBuf,
    home: PathBuf,
    ymp_home: PathBuf,
    tmp: PathBuf,
    build: PathBuf,
    export: PathBuf,
}

impl EvaluationRoots {
    fn prepare(root: &Path, manifest: &Path) -> Result<Self> {
        validate_absolute_normal(root, "evaluation root")?;
        fs::create_dir_all(root)?;
        ensure!(
            !fs::symlink_metadata(root)?.file_type().is_symlink(),
            "evaluation root must not be a symbolic link"
        );
        let root = fs::canonicalize(root)?;
        if let Ok(manifest) = fs::canonicalize(manifest) {
            ensure!(
                !root.starts_with(manifest.parent().unwrap_or_else(|| Path::new("/"))),
                "evaluation root must be external to the corpus worktree"
            );
        }
        let roots = Self {
            project: root.join("project"),
            home: root.join("home"),
            ymp_home: root.join("ymp-home"),
            tmp: root.join("tmp"),
            build: root.join("build"),
            export: root.join("export"),
        };
        for path in roots.paths() {
            fs::create_dir_all(path)?;
            ensure!(
                !fs::symlink_metadata(path)?.file_type().is_symlink(),
                "isolated root must not be a symbolic link: {}",
                path.display()
            );
        }
        let canonical = roots
            .paths()
            .iter()
            .map(fs::canonicalize)
            .collect::<std::io::Result<Vec<_>>>()?;
        ensure!(
            canonical.iter().collect::<BTreeSet<_>>().len() == 6,
            "isolated roots overlap"
        );
        Ok(roots)
    }

    fn paths(&self) -> [&Path; 6] {
        [
            &self.project,
            &self.home,
            &self.ymp_home,
            &self.tmp,
            &self.build,
            &self.export,
        ]
    }
}

fn rehearse_transport(
    manifest: &AdmissionManifest,
    roots: &EvaluationRoots,
) -> Result<TransportReport> {
    let schedule_sha256 = sha256_bytes(&serde_json::to_vec(&manifest.transport_schedules)?);
    let mut scenarios = Vec::new();
    for schedule in &manifest.transport_schedules {
        scenarios.push(rehearse_scenario(manifest, schedule, roots)?);
    }
    let report = TransportReport {
        passed: scenarios.iter().all(|scenario| scenario.passed),
        schedule_sha256,
        scenarios,
        model_calls: 0,
        production_path: vec![
            "ymp-agent-mcp".to_owned(),
            "ymp-agent-rpc".to_owned(),
            "ymp-application".to_owned(),
            "ymp-board".to_owned(),
        ],
    };
    Ok(report)
}

struct Endpoint {
    mcp: McpServer<SocketToolHandler>,
    _rpc: AgentRpcServer,
    next_id: u64,
    advertised_tools: BTreeSet<String>,
}

impl Endpoint {
    fn basic(
        socket: &Path,
        token: &str,
        attempt: &str,
        application: Arc<Mutex<Application>>,
    ) -> Result<Self> {
        let rpc = AgentRpcServer::start(socket, token, attempt, application)?;
        Self::finish(rpc, SocketToolHandler::new(socket, token, attempt))
    }

    fn invocation(
        socket: &Path,
        token: &str,
        attempt: &str,
        invocation: &str,
        application: Arc<Mutex<Application>>,
        submission: WorkspaceSubmission,
    ) -> Result<Self> {
        let rpc = AgentRpcServer::start_with_submission_for_invocation(
            socket,
            token,
            attempt,
            invocation,
            application,
            submission,
        )?;
        Self::finish(
            rpc,
            SocketToolHandler::for_invocation(socket, token, attempt, invocation),
        )
    }

    fn finish(rpc: AgentRpcServer, handler: SocketToolHandler) -> Result<Self> {
        let mut endpoint = Self {
            mcp: McpServer::new(handler),
            _rpc: rpc,
            next_id: 1,
            advertised_tools: BTreeSet::new(),
        };
        let initialized = endpoint.raw(json!({
            "jsonrpc": "2.0",
            "id": endpoint.next_id,
            "method": "initialize"
        }))?;
        ensure!(
            initialized.get("result").is_some(),
            "MCP initialization failed"
        );
        endpoint.next_id += 1;
        let catalog = endpoint.raw(json!({
            "jsonrpc": "2.0",
            "id": endpoint.next_id,
            "method": "tools/list"
        }))?;
        endpoint.next_id += 1;
        endpoint.advertised_tools = catalog["result"]["tools"]
            .as_array()
            .context("MCP tools/list returned no tool array")?
            .iter()
            .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
            .collect();
        Ok(endpoint)
    }

    fn call(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let response = self.raw(json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }))?;
        self.next_id += 1;
        ensure!(
            response["result"]["isError"] == false,
            "MCP tool {name} failed: {}",
            response["result"]["structuredContent"]
        );
        Ok(response["result"]["structuredContent"].clone())
    }

    fn raw(&mut self, request: Value) -> Result<Value> {
        let line = self
            .mcp
            .handle_line(&request.to_string())
            .context("MCP request unexpectedly had no response")?;
        serde_json::from_str(&line).context("parse MCP response")
    }
}

struct FakeSlice {
    session: Box<dyn RuntimeSession>,
    events: Vec<RuntimeEvent>,
}

impl FakeSlice {
    fn start(workspace: &Path, actor: &str, yields: bool) -> Result<Self> {
        let script = if yields {
            vec![ScriptStep::Yield(format!("{actor}-cursor"))]
        } else {
            vec![ScriptStep::Complete(Usage::default())]
        };
        let runtime = FakeRuntime::with_script(script);
        ensure!(
            runtime.probe()?.readiness == Readiness::Ready,
            "fake runtime is not ready"
        );
        let mut session = runtime.start(InvocationRequest {
            invocation_id: format!("invocation-{actor}"),
            attempt_id: actor.to_owned(),
            workspace: workspace.to_path_buf(),
            mcp: None,
            prompt: "zero-model transport rehearsal".to_owned(),
            cancellation: Default::default(),
        })?;
        let started = session
            .next_event()?
            .context("fake runtime omitted started event")?;
        ensure!(matches!(started.event, RuntimeEventKind::Started { .. }));
        Ok(Self {
            session,
            events: vec![started],
        })
    }

    fn complete(&mut self) -> Result<()> {
        let terminal = self
            .session
            .next_event()?
            .context("fake runtime omitted completed event")?;
        ensure!(matches!(terminal.event, RuntimeEventKind::Completed { .. }));
        self.events.push(terminal);
        ensure!(
            self.session.next_event()?.is_none(),
            "fake runtime emitted after terminal"
        );
        Ok(())
    }

    fn yield_now(&mut self) -> Result<()> {
        let terminal = self
            .session
            .next_event()?
            .context("fake runtime omitted yielded event")?;
        ensure!(matches!(terminal.event, RuntimeEventKind::Yielded { .. }));
        self.events.push(terminal);
        ensure!(
            self.session.next_event()?.is_none(),
            "yielded fake runtime kept running"
        );
        Ok(())
    }
}

struct ReadCounter {
    reader: u32,
    unauthorized: u32,
}

impl ReadCounter {
    fn reader(&mut self, schedule: &TransportSchedule, manifest: &AdmissionManifest) -> Result<()> {
        self.reader = self.reader.saturating_add(1);
        ensure!(
            self.reader <= schedule.reader_read_cap
                && self.reader <= manifest.read_limits.max_reads_per_reader,
            "transport reader exceeded its predeclared hard read cap"
        );
        Ok(())
    }

    fn unauthorized(&mut self, manifest: &AdmissionManifest) -> Result<()> {
        self.unauthorized = self.unauthorized.saturating_add(1);
        ensure!(
            self.unauthorized <= manifest.read_limits.max_reads_unauthorized_reader,
            "unauthorized reader exceeded its predeclared hard read cap"
        );
        Ok(())
    }
}

fn rehearse_scenario(
    manifest: &AdmissionManifest,
    schedule: &TransportSchedule,
    roots: &EvaluationRoots,
) -> Result<ScenarioReport> {
    let name = schedule.scenario.as_str();
    let run_root = roots.ymp_home.join(name);
    let mut application =
        Application::create(&run_root, format!("admission-{name}"), Budget::new(3, 0))?;
    for participant in [ROOT_PARTICIPANT, READER, OUTSIDER] {
        application.execute(
            format!("start-{participant}"),
            DomainCommand::StartAttempt {
                attempt_id: participant.to_owned(),
            },
        )?;
    }
    for participant in [READER, OUTSIDER] {
        application.record_board(&BoardCommand::RegisterParticipant(RegisterParticipant {
            participant_id: participant.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            endowment: CommunicationAllowance::ZERO.with(Allowance::DeliveredBytes, 8 * 1024),
        }))?;
    }
    application.record_board(&BoardCommand::OpenScope(OpenScope {
        controller: "ymp".to_owned(),
        scope_id: SCOPE.to_owned(),
        kind: ScopeKind::Task,
        sponsor: ROOT_PARTICIPANT.to_owned(),
        review_policy: None,
        initial_members: vec![
            InitialMember {
                grant_id: format!("grant-root-{name}"),
                participant: ROOT_PARTICIPANT.to_owned(),
                rights: Rights::READ_AND_PUBLISH,
            },
            InitialMember {
                grant_id: format!("grant-reader-{name}"),
                participant: READER.to_owned(),
                rights: Rights::READ,
            },
        ],
        member_expires_at: 10_000,
    }))?;
    let notifications = application.subscribe(16)?;
    let workspace = roots.project.join(name);
    let source = roots.tmp.join(format!("source-{name}"));
    fs::create_dir_all(&source)?;
    fs::write(source.join("marker.txt"), b"zero-model\n")?;
    let base = application.artifact_store().capture_source(&source)?;
    application
        .artifact_store()
        .materialize(&base.manifest_digest, &workspace)?;
    let submission = WorkspaceSubmission::new(&base.manifest_digest, &workspace, Vec::new());
    let application = Arc::new(Mutex::new(application));
    let socket_root = roots.tmp.join(format!("s-{name}"));
    let mut a = Endpoint::basic(
        &socket_root.join("a.sock"),
        &format!("token-a-{name}"),
        ROOT_PARTICIPANT,
        Arc::clone(&application),
    )?;
    let mut b = Endpoint::invocation(
        &socket_root.join("b.sock"),
        &format!("token-b-{name}"),
        READER,
        &format!("invocation-b-{name}"),
        Arc::clone(&application),
        submission,
    )?;
    let mut unauthorized = Endpoint::basic(
        &socket_root.join("u.sock"),
        &format!("token-u-{name}"),
        OUTSIDER,
        Arc::clone(&application),
    )?;
    let mut slices: BTreeMap<String, FakeSlice> = BTreeMap::new();
    let mut counter = ReadCounter {
        reader: 0,
        unauthorized: 0,
    };
    let mut delivered_message_ids = Vec::new();
    let mut unauthorized_message_count = 0_u32;
    let mut board_wake_count = 0_u32;
    let recruitment_advertised_without_capability = [&a, &b, &unauthorized]
        .iter()
        .any(|endpoint| endpoint.advertised_tools.contains("request_participant"));
    let content = format!("{name}-fixed-transport-payload");
    let message_id = format!("message-{name}");
    let mut terminal_a = "not_started".to_owned();
    let mut terminal_b = "not_started".to_owned();

    for window in &schedule.windows {
        match (window.actor.as_str(), window.action) {
            (actor @ ("a" | "b"), WindowAction::StartFake) => {
                ensure!(!slices.contains_key(actor), "fake slice started twice");
                let yields = schedule.scenario == Scenario::S3 && actor == "b";
                slices.insert(
                    actor.to_owned(),
                    FakeSlice::start(&workspace, actor, yields)?,
                );
                if actor == "a" {
                    terminal_a = "running".to_owned();
                } else {
                    terminal_b = "running".to_owned();
                }
            }
            ("a", WindowAction::Publish) => {
                a.call(
                    "publish",
                    json!({
                        "command_id": message_id,
                        "audience": {
                            "audience": "named",
                            "scope_id": SCOPE,
                            "recipients": [READER]
                        },
                        "kind": "observation",
                        "content": content,
                        "salience_ms": 5_000,
                        "references": [],
                        "relation": { "relation": "standalone" },
                        "claimed_decision_basis": []
                    }),
                )?;
            }
            ("b", WindowAction::ReadBoard) => {
                counter.reader(schedule, manifest)?;
                let read = b.call(
                    "read_board",
                    json!({ "limit_bytes": manifest.read_limits.max_bytes_per_read }),
                )?;
                let messages = read["messages"]
                    .as_array()
                    .context("read_board returned no message array")?;
                delivered_message_ids.extend(messages.iter().filter_map(|message| {
                    message["message"]["message_id"].as_str().map(str::to_owned)
                }));
            }
            ("u", WindowAction::UnauthorizedRead) => {
                counter.unauthorized(manifest)?;
                let read = unauthorized.call(
                    "read_board",
                    json!({ "limit_bytes": manifest.read_limits.max_bytes_per_read }),
                )?;
                unauthorized_message_count = read["messages"]
                    .as_array()
                    .context("unauthorized read returned no message array")?
                    .len() as u32;
            }
            (actor @ ("a" | "b"), WindowAction::CompleteFake) => {
                slices
                    .get_mut(actor)
                    .context("fake slice completed before start")?
                    .complete()?;
                if actor == "a" {
                    terminal_a = "completed_without_task_output".to_owned();
                } else {
                    terminal_b = "completed_without_task_output".to_owned();
                }
            }
            ("b", WindowAction::YieldFakeAndTool) => {
                slices
                    .get_mut("b")
                    .context("fake slice yielded before start")?
                    .yield_now()?;
                let confirmation =
                    b.call("yield", json!({ "command_id": format!("yield-{name}") }))?;
                ensure!(
                    confirmation["attempt_id"] == READER,
                    "yield confirmation lost bound attempt identity"
                );
                terminal_b = "yielded_without_task_output".to_owned();
            }
            ("controller", WindowAction::AssertNoBoardWake) => {
                board_wake_count = drain_notifications(&notifications)?;
                ensure!(
                    board_wake_count == 0,
                    "board publication woke the control plane"
                );
            }
            _ => bail!(
                "schedule action is not executable for {} window {}",
                name,
                window.ordinal
            ),
        }
    }
    ensure!(
        counter.reader == schedule.reader_read_cap,
        "reader did not consume exact schedule"
    );
    ensure!(
        counter.unauthorized == manifest.read_limits.max_reads_unauthorized_reader,
        "unauthorized negative did not consume its exact schedule"
    );
    let evidence_path = roots.export.join(format!("{name}-board.json"));
    let evidence = application
        .lock()
        .map_err(|_| anyhow::anyhow!("application lock failed"))?
        .export_board_evidence(&evidence_path)?;
    evidence.replay()?;
    let published_records = evidence
        .facts
        .iter()
        .filter(|record| matches!(&record.fact, BoardEvent::MessagePublished { message_id: id, .. } if id == &message_id))
        .count() as u32;
    let delivery_records = evidence
        .facts
        .iter()
        .filter(|record| matches!(&record.fact, BoardEvent::DeliveryRecorded { reader, message_ids, .. } if reader == READER && message_ids == std::slice::from_ref(&message_id)))
        .count() as u32;
    let fake_runtime_event_digest = sha256_bytes(&serde_json::to_vec(
        &slices
            .values()
            .flat_map(|slice| slice.events.clone())
            .collect::<Vec<_>>(),
    )?);
    let expected_delivery = matches!(schedule.scenario, Scenario::S1 | Scenario::S2);
    let passed = published_records == 1
        && delivery_records == u32::from(expected_delivery)
        && unauthorized_message_count == 0
        && board_wake_count == 0
        && !recruitment_advertised_without_capability
        && terminal_a == "completed_without_task_output"
        && if schedule.scenario == Scenario::S3 {
            terminal_b == "yielded_without_task_output" && delivered_message_ids.is_empty()
        } else {
            terminal_b == "completed_without_task_output"
                && delivered_message_ids == [message_id.clone()]
        };
    Ok(ScenarioReport {
        scenario: schedule.scenario,
        executed_windows: schedule.windows.clone(),
        reader_reads: counter.reader,
        unauthorized_reads: counter.unauthorized,
        published_records,
        delivery_records,
        delivered_message_ids,
        unauthorized_message_count,
        board_wake_count,
        recruitment_advertised_without_capability,
        terminal_a,
        terminal_b,
        fake_runtime_event_digest,
        passed,
    })
}

fn drain_notifications(receiver: &Receiver<u64>) -> Result<u32> {
    let mut count = 0_u32;
    loop {
        match receiver.try_recv() {
            Ok(_) => count = count.saturating_add(1),
            Err(TryRecvError::Empty) => return Ok(count),
            Err(TryRecvError::Disconnected) => bail!("control notification channel disconnected"),
        }
    }
}

fn evaluate_probe(loaded: &LoadedManifest, path: &Path) -> StageTwoReport {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StageTwoReport::missing();
        }
        Err(error) => {
            return StageTwoReport {
                status: StageTwoStatus::Mismatched,
                evidence_digest: None,
                reason: format!("stage-two probe evidence cannot be read: {error}"),
                can_open_model_gate: false,
            };
        }
    };
    let digest = sha256_bytes(&bytes);
    let evidence: ProbeEvidence = match serde_json::from_slice(&bytes) {
        Ok(evidence) => evidence,
        Err(error) => {
            return StageTwoReport {
                status: StageTwoStatus::Mismatched,
                evidence_digest: Some(digest),
                reason: format!("strict stage-two probe schema rejected evidence: {error}"),
                can_open_model_gate: false,
            };
        }
    };
    match validate_probe(loaded, &evidence) {
        Ok(()) if evidence.source == ProbeSource::Fake => StageTwoReport {
            status: StageTwoStatus::FakeValidated,
            evidence_digest: Some(digest),
            reason: "fake nonce evidence validates the schema but cannot open the model gate"
                .to_owned(),
            can_open_model_gate: false,
        },
        Ok(()) => StageTwoReport {
            status: StageTwoStatus::Unattested,
            evidence_digest: Some(digest),
            reason: "tool-host evidence is schema-valid but the current public seam provides no controller-attested probe record"
                .to_owned(),
            can_open_model_gate: false,
        },
        Err(error) => StageTwoReport {
            status: classify_probe_error(&error.to_string()),
            evidence_digest: Some(digest),
            reason: error.to_string(),
            can_open_model_gate: false,
        },
    }
}

fn validate_probe(loaded: &LoadedManifest, evidence: &ProbeEvidence) -> Result<()> {
    let budget = &loaded.manifest.stage_two;
    ensure!(
        evidence.schema_version == 1,
        "mismatched probe schema version"
    );
    ensure!(
        !evidence.probe_id.is_empty(),
        "mismatched empty probe identity"
    );
    ensure!(
        evidence.admission_manifest_sha256 == loaded.digest,
        "mismatched admission manifest binding"
    );
    ensure!(
        evidence.status == ProbeOutcome::Succeeded,
        "refused nonce probe"
    );
    ensure!(evidence.stage == budget.stage, "mismatched probe stage");
    ensure!(
        evidence.budget_source == budget.budget_source,
        "unaccounted probe budget source"
    );
    ensure!(
        evidence.no_task_output,
        "mismatched probe emitted task output"
    );
    ensure!(
        !evidence.arm_observation,
        "mismatched probe counted as an arm observation"
    );
    ensure!(
        evidence.nonce_readback.as_deref() == Some(evidence.nonce_input.as_str()),
        "mismatched nonce readback"
    );
    ensure!(
        evidence.input_digest == sha256_bytes(evidence.nonce_input.as_bytes()),
        "mismatched nonce input digest"
    );
    let output = evidence.nonce_readback.as_deref().unwrap_or_default();
    ensure!(
        evidence.output_digest.as_deref() == Some(sha256_bytes(output.as_bytes()).as_str()),
        "mismatched nonce output digest"
    );
    validate_sha256(&evidence.event_digest, "probe event digest")?;
    let expected_calls = if evidence.source == ProbeSource::Fake {
        0
    } else {
        1
    };
    ensure!(
        evidence.model_calls == expected_calls,
        "unaccounted probe model calls"
    );
    ensure!(
        evidence.model_calls <= budget.max_model_calls,
        "probe call budget exceeded"
    );
    ensure!(
        evidence.input_tokens <= budget.max_input_tokens
            && evidence.cached_input_tokens <= budget.max_cached_input_tokens
            && evidence.output_tokens <= budget.max_output_tokens
            && evidence.reasoning_tokens <= budget.max_reasoning_tokens
            && evidence.wall_time_ms <= budget.max_wall_time_ms,
        "unaccounted probe usage exceeded its separate budget"
    );
    if evidence.source == ProbeSource::ToolHost {
        ensure!(
            evidence
                .input_tokens
                .saturating_add(evidence.cached_input_tokens)
                .saturating_add(evidence.output_tokens)
                .saturating_add(evidence.reasoning_tokens)
                > 0,
            "unaccounted tool-host probe has no token evidence"
        );
    }
    ensure!(
        evidence.route == loaded.manifest.runtime.route,
        "mismatched probe route"
    );
    ensure!(
        evidence.cli_version == loaded.manifest.runtime.cli_version
            && evidence.driver_version == loaded.manifest.runtime.driver_version,
        "mismatched probe CLI/driver version"
    );
    match evidence.cost.availability {
        CostAvailability::Reported => ensure!(
            evidence
                .cost
                .currency
                .as_deref()
                .is_some_and(|currency| !currency.is_empty())
                && evidence.cost.minor_units.is_some(),
            "unaccounted reported probe cost"
        ),
        CostAvailability::Unavailable => ensure!(
            evidence.cost.currency.is_none() && evidence.cost.minor_units.is_none(),
            "unaccounted unavailable probe cost"
        ),
    }
    Ok(())
}

fn classify_probe_error(error: &str) -> StageTwoStatus {
    if error.contains("refused") {
        StageTwoStatus::Refused
    } else if error.contains("unaccounted") || error.contains("budget") {
        StageTwoStatus::Unaccounted
    } else {
        StageTwoStatus::Mismatched
    }
}

fn validate_report_claims(manifest: &AdmissionManifest, report: &AdmissionReport) -> Result<()> {
    ensure!(report.schema_version == 1, "report schema changed");
    ensure!(
        report.admission_id == ADMISSION_ID,
        "report identity changed"
    );
    ensure!(
        report.manifest_sha256 == FROZEN_MANIFEST_SHA256,
        "report lost its frozen manifest binding"
    );
    ensure!(
        report.model_calls == 0,
        "admission command made a model call"
    );
    ensure!(
        report.arm_schedule.is_none(),
        "admission emitted an arm schedule"
    );
    ensure!(
        report.stage_one.model_calls == 0
            && report.stage_one.route == manifest.runtime.route
            && report.stage_one.tool_schema_sha256 == manifest.tool_host.tool_schema_sha256
            && report.stage_one.binding_required,
        "stage-one report changed a decisive compatibility claim"
    );
    ensure!(
        !report.stage_two.can_open_model_gate,
        "current public seam cannot attest stage-two evidence"
    );
    ensure!(
        !report.model_ready || report.stage_two.can_open_model_gate,
        "model gate opened without stage-two evidence"
    );
    ensure!(!report.model_ready, "unattested model gate opened");
    let Some(transport) = &report.transport else {
        return Ok(());
    };
    ensure!(
        transport.model_calls == 0,
        "transport rehearsal made a model call"
    );
    ensure!(
        transport.schedule_sha256
            == sha256_bytes(&serde_json::to_vec(&manifest.transport_schedules)?),
        "transport schedule digest changed"
    );
    ensure!(
        transport.production_path
            == [
                "ymp-agent-mcp",
                "ymp-agent-rpc",
                "ymp-application",
                "ymp-board"
            ],
        "transport report does not name the production path"
    );
    ensure!(
        transport.scenarios.len() == 3,
        "transport report omitted a scenario"
    );
    for (schedule, scenario) in manifest
        .transport_schedules
        .iter()
        .zip(&transport.scenarios)
    {
        ensure!(
            scenario.scenario == schedule.scenario,
            "scenario report reordered"
        );
        ensure!(
            scenario.executed_windows == schedule.windows,
            "executed schedule changed"
        );
        ensure!(
            scenario.reader_reads == schedule.reader_read_cap,
            "read cap claim changed"
        );
        ensure!(
            scenario.unauthorized_reads == 1,
            "unauthorized negative was omitted"
        );
        ensure!(
            scenario.published_records == 1,
            "MessagePublished receipt is missing"
        );
        ensure!(
            scenario.unauthorized_message_count == 0,
            "unauthorized reader received bytes"
        );
        ensure!(
            !scenario.recruitment_advertised_without_capability,
            "recruitment was advertised without an endpoint capability"
        );
        if matches!(scenario.scenario, Scenario::S1 | Scenario::S2) {
            ensure!(
                scenario.delivery_records == 1,
                "DeliveryRecorded receipt is missing"
            );
            ensure!(
                scenario.delivered_message_ids
                    == [format!("message-{}", scenario.scenario.as_str())],
                "delivery identity is missing"
            );
        } else {
            ensure!(
                scenario.delivery_records == 0,
                "S3 unexpectedly delivered a message"
            );
            ensure!(
                scenario.delivered_message_ids.is_empty(),
                "S3 reader observed publication"
            );
            ensure!(
                scenario.board_wake_count == 0,
                "S3 publication produced a board wake"
            );
        }
        validate_sha256(
            &scenario.fake_runtime_event_digest,
            "fake runtime event digest",
        )?;
        ensure!(
            scenario.terminal_a == "completed_without_task_output"
                && if scenario.scenario == Scenario::S3 {
                    scenario.terminal_b == "yielded_without_task_output"
                } else {
                    scenario.terminal_b == "completed_without_task_output"
                },
            "scenario terminal claim is not honest"
        );
        ensure!(scenario.passed, "scenario report is not compliant");
    }
    ensure!(transport.passed, "transport report is not compliant");
    Ok(())
}

fn write_immutable_report(export: &Path, report: &AdmissionReport) -> Result<()> {
    let bytes = format!("{}\n", report_json(report)?).into_bytes();
    let report_path = export.join(REPORT_FILE);
    let digest_path = export.join(REPORT_DIGEST_FILE);
    let mut report_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report_path)
        .with_context(|| format!("create immutable report {}", report_path.display()))?;
    report_file.write_all(&bytes)?;
    report_file.sync_all()?;
    let mut digest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&digest_path)
        .with_context(|| format!("create immutable report digest {}", digest_path.display()))?;
    digest_file.write_all(format!("{}\n", sha256_bytes(&bytes)).as_bytes())?;
    digest_file.sync_all()?;
    Ok(())
}

fn validate_absolute_normal(path: &Path, label: &str) -> Result<()> {
    ensure!(path.is_absolute(), "{label} must be absolute");
    for component in path.components() {
        ensure!(
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::Normal(_)
            ),
            "{label} must not contain traversal"
        );
    }
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<()> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} is not lowercase SHA-256"
    );
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        AdmissionReport, CostAvailability, LoadedManifest, ProbeCost, ProbeEvidence, ProbeOutcome,
        ProbeSource, ReadCounter, StageOneReport, StageTwoReport, StageTwoStatus,
        expected_schedules, load_manifest, validate_probe, validate_report_claims,
    };

    fn loaded() -> LoadedManifest {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("corpus/development/weak-diagnostic-admission-v1");
        load_manifest(&root.join("manifest.json"), &root.join("manifest.sha256"))
            .expect("load frozen admission")
    }

    fn fake_probe(loaded: &LoadedManifest) -> ProbeEvidence {
        let nonce = "nonce-fixed-for-schema-validation".to_owned();
        ProbeEvidence {
            schema_version: 1,
            probe_id: "fake-probe-1".to_owned(),
            admission_manifest_sha256: loaded.digest.clone(),
            source: ProbeSource::Fake,
            status: ProbeOutcome::Succeeded,
            stage: loaded.manifest.stage_two.stage.clone(),
            budget_source: loaded.manifest.stage_two.budget_source.clone(),
            nonce_input: nonce.clone(),
            nonce_readback: Some(nonce.clone()),
            no_task_output: true,
            arm_observation: false,
            model_calls: 0,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
            wall_time_ms: 1,
            route: loaded.manifest.runtime.route.clone(),
            cli_version: loaded.manifest.runtime.cli_version.clone(),
            driver_version: loaded.manifest.runtime.driver_version.clone(),
            cost: ProbeCost {
                availability: CostAvailability::Unavailable,
                currency: None,
                minor_units: None,
            },
            input_digest: super::sha256_bytes(nonce.as_bytes()),
            output_digest: Some(super::sha256_bytes(nonce.as_bytes())),
            event_digest: super::sha256_bytes(b"fake-event"),
        }
    }

    use std::path::PathBuf;

    #[cfg(unix)]
    fn write_codex_fixture(
        path: &std::path::Path,
        version: &str,
        include_flags: bool,
        exact_tool_schema: bool,
    ) {
        use std::os::unix::fs::PermissionsExt;

        let flags = if include_flags {
            "resume --json --ignore-user-config --ignore-rules --sandbox --model --disable --cd"
        } else {
            "resume --json --ignore-user-config --sandbox --model --disable --cd"
        };
        let tool_schema = if exact_tool_schema {
            r#"{"definitions":{"v2":{"TokenUsageBreakdown":{"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]}}},"items":[{"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]}],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]}"#
        } else {
            r#"{"definitions":{"v2":{"TokenUsageBreakdown":{"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]}}},"items":[{"title":"McpToolCallThreadItem","required":["id","server","status","tool","type"]}],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]}"#
        };
        fs::write(
            path,
            format!(
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf '%s\\n' '{version}'\nelif [ \"$1\" = \"exec\" ] && [ \"$2\" = \"--help\" ]; then\n  printf '%s\\n' '{flags}'\nelif [ \"$1\" = \"exec\" ] && [ \"$2\" = \"resume\" ] && [ \"$3\" = \"--help\" ]; then\n  printf '%s\\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'\nelif [ \"$1\" = \"features\" ] && [ \"$2\" = \"list\" ]; then\n  printf '%s\\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'\nelif [ \"$1\" = \"app-server\" ] && [ \"$2\" = \"--help\" ]; then\n  printf '%s\\n' 'generate-json-schema --listen <URL> stdio://'\nelif [ \"$1\" = \"app-server\" ] && [ \"$2\" = \"generate-json-schema\" ]; then\n  mkdir -p \"$4\"\n  printf '%s\\n' '{tool_schema}' > \"$4/codex_app_server_protocol.schemas.json\"\nelif [ \"$1\" = \"login\" ]; then\n  exit 0\nelse\n  exit 2\nfi\n"
            ),
        )
        .expect("write Codex fixture");
        let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("fixture permissions");
    }

    #[test]
    fn production_tool_schema_digest_matches_the_freeze() {
        let loaded = loaded();
        assert_eq!(
            super::sha256_bytes(
                &serde_json::to_vec(&ymp_agent_mcp::tool_catalog()).expect("tool catalog")
            ),
            loaded.manifest.tool_host.tool_schema_sha256
        );
    }

    #[test]
    #[cfg(unix)]
    fn exact_cli_route_and_flags_are_required_without_a_model_call() {
        let loaded = loaded();
        let directory = tempdir().expect("temporary directory");
        let project = directory.path().join("project");
        fs::create_dir(&project).expect("project");
        assert!(
            std::process::Command::new("git")
                .args(["-C"])
                .arg(&project)
                .args(["init", "--quiet"])
                .status()
                .expect("Git init")
                .success()
        );
        let exact = directory.path().join("codex-exact");
        write_codex_fixture(&exact, &loaded.manifest.runtime.cli_version, true, true);
        let accepted = super::evaluate_stage_one(&loaded.manifest, &exact, &project);
        assert!(accepted.compatible, "{:?}", accepted.issues);
        assert_eq!(accepted.model_calls, 0);

        let stale = directory.path().join("codex-stale");
        write_codex_fixture(&stale, "codex-cli 0.147.0", true, true);
        assert!(!super::evaluate_stage_one(&loaded.manifest, &stale, &project).compatible);
        let future = directory.path().join("codex-future");
        write_codex_fixture(&future, "codex-cli 0.152.0", true, true);
        assert!(!super::evaluate_stage_one(&loaded.manifest, &future, &project).compatible);
        let removed_flag = directory.path().join("codex-missing-flag");
        write_codex_fixture(
            &removed_flag,
            &loaded.manifest.runtime.cli_version,
            false,
            true,
        );
        assert!(!super::evaluate_stage_one(&loaded.manifest, &removed_flag, &project).compatible);
        let changed_schema = directory.path().join("codex-changed-schema");
        write_codex_fixture(
            &changed_schema,
            &loaded.manifest.runtime.cli_version,
            true,
            false,
        );
        assert!(!super::evaluate_stage_one(&loaded.manifest, &changed_schema, &project).compatible);
        assert!(!super::evaluate_stage_one(&loaded.manifest, &exact, directory.path()).compatible);

        for version in ["codex-cli 0.147.0", "codex-cli 0.152.0"] {
            let mut manifest = loaded.manifest.clone();
            manifest.runtime.cli_version = version.to_owned();
            assert!(super::validate_manifest(&manifest).is_err(), "{version}");
        }
    }

    #[test]
    fn strict_manifest_probe_and_report_schemas_reject_unknown_fields() {
        let loaded = loaded();
        let manifest = serde_json::to_value(&loaded.manifest).expect("manifest value");
        let mut manifest_unknown = manifest;
        manifest_unknown["unknown"] = json!(true);
        assert!(serde_json::from_value::<super::AdmissionManifest>(manifest_unknown).is_err());

        let probe = fake_probe(&loaded);
        let mut probe_unknown = serde_json::to_value(probe).expect("probe value");
        probe_unknown["unknown"] = json!(true);
        assert!(serde_json::from_value::<ProbeEvidence>(probe_unknown).is_err());

        let report = AdmissionReport::check(
            &loaded,
            StageOneReport {
                compatible: true,
                installed_cli_version: Some(loaded.manifest.runtime.cli_version.clone()),
                expected_cli_version: loaded.manifest.runtime.cli_version.clone(),
                driver_version: loaded.manifest.runtime.driver_version.clone(),
                route: loaded.manifest.runtime.route.clone(),
                tool_schema_sha256: loaded.manifest.tool_host.tool_schema_sha256.clone(),
                git_repository: true,
                binding_required: true,
                model_calls: 0,
                issues: Vec::new(),
            },
        );
        let mut report_unknown = serde_json::to_value(report).expect("report value");
        report_unknown["unknown"] = json!(true);
        assert!(serde_json::from_value::<AdmissionReport>(report_unknown).is_err());
    }

    #[test]
    fn digest_schedule_and_extra_read_are_load_bearing() {
        let loaded = loaded();
        let directory = tempdir().expect("temporary directory");
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("corpus/development/weak-diagnostic-admission-v1/manifest.json");
        let manifest = fs::read_to_string(manifest_path).expect("read current frozen manifest");
        let previous_manifest = manifest.replacen("codex-cli 0.151.0", "codex-cli 0.147.0", 1);
        assert_eq!(
            super::sha256_bytes(previous_manifest.as_bytes()),
            "eddafc93dbefcbbf75d29b646ac5a5dc1964d254a484249bb96a5c72ea2d6cda"
        );
        fs::write(directory.path().join("manifest.json"), previous_manifest)
            .expect("write previous frozen manifest");
        fs::write(
            directory.path().join("manifest.sha256"),
            "eddafc93dbefcbbf75d29b646ac5a5dc1964d254a484249bb96a5c72ea2d6cda\n",
        )
        .expect("write previous frozen-manifest digest");
        assert!(
            load_manifest(
                &directory.path().join("manifest.json"),
                &directory.path().join("manifest.sha256")
            )
            .is_err()
        );

        let mut changed = loaded.manifest.clone();
        changed.transport_schedules[0].windows.swap(0, 1);
        let changed_schedule = super::validate_manifest(&changed);
        if std::env::var("YMP_ADMISSION_MUTATION_CHILD").as_deref() == Ok("manifest") {
            changed_schedule.expect("deliberately changed schedule must exit nonzero");
        } else {
            assert!(changed_schedule.is_err());
        }

        let schedule = &expected_schedules()[0];
        let mut reads = ReadCounter {
            reader: 0,
            unauthorized: 0,
        };
        reads
            .reader(schedule, &loaded.manifest)
            .expect("one scheduled read");
        assert!(reads.reader(schedule, &loaded.manifest).is_err());
    }

    #[test]
    fn missing_refused_mismatched_and_unaccounted_probe_keep_gate_closed() {
        let loaded = loaded();
        let valid = fake_probe(&loaded);
        validate_probe(&loaded, &valid).expect("fake evidence validates schema");
        for mutation in ["refused", "mismatched", "unaccounted"] {
            let mut evidence = valid.clone();
            match mutation {
                "refused" => evidence.status = ProbeOutcome::Refused,
                "mismatched" => evidence.nonce_readback = Some("different".to_owned()),
                "unaccounted" => evidence.budget_source = "arm_budget".to_owned(),
                _ => unreachable!(),
            }
            let result = validate_probe(&loaded, &evidence);
            if mutation == "unaccounted"
                && std::env::var("YMP_ADMISSION_MUTATION_CHILD").as_deref() == Ok("probe")
            {
                result.expect("deliberately unaccounted probe must exit nonzero");
            } else {
                assert!(result.is_err(), "accepted {mutation}");
            }
        }
        for status in [
            StageTwoStatus::Missing,
            StageTwoStatus::Refused,
            StageTwoStatus::Mismatched,
            StageTwoStatus::Unaccounted,
            StageTwoStatus::FakeValidated,
            StageTwoStatus::Unattested,
        ] {
            let report = StageTwoReport {
                status,
                evidence_digest: None,
                reason: "negative".to_owned(),
                can_open_model_gate: false,
            };
            assert!(!report.can_open_model_gate);
        }
    }

    #[test]
    fn removing_delivery_receipt_or_advertising_without_binding_rejects_report() {
        let loaded = loaded();
        let mut manifest = loaded.manifest.clone();
        manifest.tool_host.binding_required = false;
        assert!(super::validate_manifest(&manifest).is_err());

        let mut report = AdmissionReport {
            schema_version: 1,
            admission_id: super::ADMISSION_ID.to_owned(),
            manifest_sha256: loaded.digest.clone(),
            model_calls: 0,
            stage_one: StageOneReport {
                compatible: true,
                installed_cli_version: Some(loaded.manifest.runtime.cli_version.clone()),
                expected_cli_version: loaded.manifest.runtime.cli_version.clone(),
                driver_version: loaded.manifest.runtime.driver_version.clone(),
                route: loaded.manifest.runtime.route.clone(),
                tool_schema_sha256: loaded.manifest.tool_host.tool_schema_sha256.clone(),
                git_repository: true,
                binding_required: true,
                model_calls: 0,
                issues: Vec::new(),
            },
            stage_two: StageTwoReport::missing(),
            transport: Some(super::TransportReport {
                passed: true,
                schedule_sha256: super::sha256_bytes(
                    &serde_json::to_vec(&loaded.manifest.transport_schedules)
                        .expect("schedule bytes"),
                ),
                scenarios: expected_schedules()
                    .iter()
                    .map(|schedule| super::ScenarioReport {
                        scenario: schedule.scenario,
                        executed_windows: schedule.windows.clone(),
                        reader_reads: schedule.reader_read_cap,
                        unauthorized_reads: 1,
                        published_records: 1,
                        delivery_records: u32::from(matches!(
                            schedule.scenario,
                            super::Scenario::S1 | super::Scenario::S2
                        )),
                        delivered_message_ids: if schedule.scenario == super::Scenario::S3 {
                            Vec::new()
                        } else {
                            vec![format!("message-{}", schedule.scenario.as_str())]
                        },
                        unauthorized_message_count: 0,
                        board_wake_count: 0,
                        recruitment_advertised_without_capability: false,
                        terminal_a: "completed_without_task_output".to_owned(),
                        terminal_b: if schedule.scenario == super::Scenario::S3 {
                            "yielded_without_task_output".to_owned()
                        } else {
                            "completed_without_task_output".to_owned()
                        },
                        fake_runtime_event_digest: super::sha256_bytes(b"events"),
                        passed: true,
                    })
                    .collect(),
                model_calls: 0,
                production_path: vec![
                    "ymp-agent-mcp".to_owned(),
                    "ymp-agent-rpc".to_owned(),
                    "ymp-application".to_owned(),
                    "ymp-board".to_owned(),
                ],
            }),
            model_ready: false,
            arm_schedule: None,
        };
        validate_report_claims(&loaded.manifest, &report).expect("valid report claims");
        report.transport.as_mut().expect("transport").scenarios[0].delivery_records = 0;
        let missing_receipt = validate_report_claims(&loaded.manifest, &report);
        if std::env::var("YMP_ADMISSION_MUTATION_CHILD").as_deref() == Ok("transport") {
            missing_receipt.expect("removed delivery receipt must exit nonzero");
        } else {
            assert!(missing_receipt.is_err());
        }
    }

    #[test]
    fn deliberate_negative_mutations_capture_nonzero_exits() {
        let executable = std::env::current_exe().expect("current test executable");
        for (case, test) in [
            (
                "manifest",
                "admission::tests::digest_schedule_and_extra_read_are_load_bearing",
            ),
            (
                "probe",
                "admission::tests::missing_refused_mismatched_and_unaccounted_probe_keep_gate_closed",
            ),
            (
                "transport",
                "admission::tests::removing_delivery_receipt_or_advertising_without_binding_rejects_report",
            ),
        ] {
            let output = std::process::Command::new(&executable)
                .args(["--exact", test, "--nocapture"])
                .env("YMP_ADMISSION_MUTATION_CHILD", case)
                .output()
                .expect("run deliberate mutation child");
            assert!(
                !output.status.success(),
                "{case} mutation returned a false green"
            );
            eprintln!("FALSIFIER {case} exit={:?}", output.status.code());
        }
    }

    #[test]
    #[cfg(unix)]
    fn rehearsal_reaches_mcp_rpc_application_and_fake_runtime() {
        let loaded = loaded();
        let directory = tempdir().expect("temporary directory");
        let root = directory.path().join("evaluation");
        fs::create_dir_all(root.join("project")).expect("project root");
        let status = std::process::Command::new("git")
            .args(["-C"])
            .arg(root.join("project"))
            .args(["init", "--quiet"])
            .status()
            .expect("run Git");
        assert!(status.success());
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("corpus/development/weak-diagnostic-admission-v1/manifest.json");
        let roots =
            super::EvaluationRoots::prepare(&root, &manifest_path).expect("prepare isolated roots");
        let transport =
            super::rehearse_transport(&loaded.manifest, &roots).expect("run transport rehearsal");
        assert!(transport.passed);
        assert_eq!(transport.model_calls, 0);
        assert_eq!(transport.scenarios.len(), 3);
        assert_eq!(transport.scenarios[0].delivery_records, 1);
        assert_eq!(transport.scenarios[1].delivery_records, 1);
        assert_eq!(transport.scenarios[2].delivery_records, 0);
        assert_eq!(transport.scenarios[2].board_wake_count, 0);
    }
}
