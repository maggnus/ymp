#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ymp_runtime::{
    AcceptanceContract, AdmissionDenial, AdmissionFailure, AgentId, AgentIneligibility, Allowance,
    Application, ApplicationMetadata, AssignmentRequest, CodexBackend, CodexRegistry, Constraints,
    Criterion, CriterionId, ErrorClass, ExclusionReason, ExecutionBackend, ExecutionScenario, Goal,
    InvocationId, InvocationLimits, Journal, ModelOffering, ObservationOutcome, ObservedUsage,
    OfferingId, Registry, ReservationPurpose, ResourceAmount, Role, SessionExecutionView,
    SessionId, SessionStatus, Settings, StartOutcome, SystemClock, Task, TaskId, Termination,
    UncertaintyCause, WorkspaceAccess, WorkspaceOperation, WorkspaceScope, application_metadata,
    read_execution,
};
use ymp_storage::SqliteJournal;

pub const DEFAULT_DATA_DIR: &str = ".ymp/sessions";
pub const DEFAULT_MAX_TURNS: u32 = 16;
pub const DEFAULT_MAX_OUTPUT_CHARS: u64 = 200_000;
pub const DEFAULT_MAX_WALL_CLOCK: Duration = Duration::from_secs(30 * 60);

const DEFAULT_RESERVATION: ResourceAmount = ResourceAmount::new(1);
const OBSERVATION_INTERVAL: Duration = Duration::from_millis(100);
const CANCELLATION_LEAD: Duration = Duration::from_secs(5);
const CODEX_AGENT_ID: &str = "codex";
const CODEX_OFFERING_ID: &str = "codex-cli-local";
const IMPLEMENTER_ROLE: &str = "implementer";
const COMPLETION_CRITERION_ID: &str = "requested-work-completed";
const COMPLETION_CRITERION: &str = "the agent completed the requested work";
const DATABASE_FILE_NAME: &str = "journal.db";

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Fully rendered process result. Operational failures use exit code 1,
/// command-line usage errors use 2, and only successful commands use 0.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutput {
    stdout: String,
    stderr: String,
    exit_code: u8,
}

impl CommandOutput {
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub const fn exit_code(&self) -> u8 {
        self.exit_code
    }

    fn success(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        }
    }

    fn observed(stdout: String, succeeded: bool) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: if succeeded { 0 } else { 1 },
        }
    }

    fn failure(failure: CliFailure) -> Self {
        Self {
            stdout: String::new(),
            stderr: format!("error: {}\n", failure.message),
            exit_code: failure.exit_code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CliFailure {
    message: String,
    exit_code: u8,
}

impl CliFailure {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }

    fn operational(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
        }
    }

    fn with_session_context(self, session_id: &SessionId, data_dir: &Path) -> Self {
        Self {
            message: format!(
                "{}\nSession: {}\nData directory: {}",
                self.message,
                session_id,
                data_dir.display()
            ),
            exit_code: self.exit_code,
        }
    }
}

enum ParsedCommand {
    Help,
    RunHelp,
    ShowHelp,
    Version,
    Run {
        goal: String,
        data_dir: PathBuf,
    },
    Show {
        session_id: String,
        data_dir: PathBuf,
    },
}

/// Executes one CLI command without exiting the process.
pub fn command(arguments: &[OsString]) -> CommandOutput {
    match parse_command(arguments).and_then(execute) {
        Ok(output) => output,
        Err(failure) => CommandOutput::failure(failure),
    }
}

fn execute(command: ParsedCommand) -> Result<CommandOutput, CliFailure> {
    let metadata = application_metadata();
    match command {
        ParsedCommand::Help => Ok(CommandOutput::success(help(metadata))),
        ParsedCommand::RunHelp => Ok(CommandOutput::success(run_help(metadata))),
        ParsedCommand::ShowHelp => Ok(CommandOutput::success(show_help(metadata))),
        ParsedCommand::Version => Ok(CommandOutput::success(format!(
            "{} {}\n",
            metadata.name(),
            metadata.version()
        ))),
        ParsedCommand::Run { goal, data_dir } => run_command(goal, data_dir),
        ParsedCommand::Show {
            session_id,
            data_dir,
        } => show_command(session_id, data_dir),
    }
}

fn parse_command(arguments: &[OsString]) -> Result<ParsedCommand, CliFailure> {
    let Some(command) = arguments.first() else {
        return Ok(ParsedCommand::Help);
    };
    if command == OsStr::new("--help") {
        return if arguments.len() == 1 {
            Ok(ParsedCommand::Help)
        } else {
            Err(CliFailure::usage(format!(
                "--help does not accept additional arguments\n\n{}",
                usage(application_metadata())
            )))
        };
    }
    if command == OsStr::new("--version") {
        return if arguments.len() == 1 {
            Ok(ParsedCommand::Version)
        } else {
            Err(CliFailure::usage(format!(
                "--version does not accept additional arguments\n\n{}",
                usage(application_metadata())
            )))
        };
    }

    let name = command.to_str().ok_or_else(|| {
        CliFailure::usage(format!(
            "command must be valid UTF-8\n\n{}",
            usage(application_metadata())
        ))
    })?;
    match name {
        "run" if arguments.get(1) == Some(&OsString::from("--help")) => {
            if arguments.len() == 2 {
                Ok(ParsedCommand::RunHelp)
            } else {
                Err(CliFailure::usage(
                    "run --help does not accept additional arguments",
                ))
            }
        }
        "show" if arguments.get(1) == Some(&OsString::from("--help")) => {
            if arguments.len() == 2 {
                Ok(ParsedCommand::ShowHelp)
            } else {
                Err(CliFailure::usage(
                    "show --help does not accept additional arguments",
                ))
            }
        }
        "run" => {
            let (subject, data_dir) = parse_subject_and_data_dir("run", &arguments[1..])?;
            let goal = subject.into_string().map_err(|_| {
                CliFailure::usage("run task text must be valid UTF-8\n\nUsage: ymp run \"<task>\" [--data-dir PATH]")
            })?;
            if goal.trim().is_empty() {
                return Err(CliFailure::usage(
                    "run task text must not be blank\n\nUsage: ymp run \"<task>\" [--data-dir PATH]",
                ));
            }
            Ok(ParsedCommand::Run { goal, data_dir })
        }
        "show" => {
            let (subject, data_dir) = parse_subject_and_data_dir("show", &arguments[1..])?;
            let session_id = subject.into_string().map_err(|_| {
                CliFailure::usage("session ID must be valid UTF-8\n\nUsage: ymp show <session-id> [--data-dir PATH]")
            })?;
            if session_id.trim().is_empty() {
                return Err(CliFailure::usage(
                    "session ID must not be blank\n\nUsage: ymp show <session-id> [--data-dir PATH]",
                ));
            }
            Ok(ParsedCommand::Show {
                session_id,
                data_dir,
            })
        }
        _ => Err(CliFailure::usage(format!(
            "unsupported command '{}'\n\n{}",
            command.to_string_lossy(),
            usage(application_metadata())
        ))),
    }
}

fn parse_subject_and_data_dir(
    command: &str,
    arguments: &[OsString],
) -> Result<(OsString, PathBuf), CliFailure> {
    let mut subject = None;
    let mut data_dir = None;
    let mut options_ended = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if !options_ended && argument == OsStr::new("--") {
            options_ended = true;
            index += 1;
            continue;
        }
        if !options_ended && argument == OsStr::new("--data-dir") {
            if data_dir.is_some() {
                return Err(command_usage(
                    command,
                    "--data-dir may be specified only once",
                ));
            }
            let value = arguments
                .get(index + 1)
                .ok_or_else(|| command_usage(command, "--data-dir requires a path"))?;
            if value.is_empty() {
                return Err(command_usage(command, "--data-dir path must not be empty"));
            }
            data_dir = Some(PathBuf::from(value));
            index += 2;
            continue;
        }
        if !options_ended
            && let Some(value) = argument
                .to_str()
                .and_then(|value| value.strip_prefix("--data-dir="))
        {
            if data_dir.is_some() {
                return Err(command_usage(
                    command,
                    "--data-dir may be specified only once",
                ));
            }
            if value.is_empty() {
                return Err(command_usage(command, "--data-dir path must not be empty"));
            }
            data_dir = Some(PathBuf::from(value));
            index += 1;
            continue;
        }
        if !options_ended && argument.to_string_lossy().starts_with('-') {
            return Err(command_usage(
                command,
                &format!("unsupported option '{}'", argument.to_string_lossy()),
            ));
        }
        if subject.replace(argument.clone()).is_some() {
            return Err(command_usage(
                command,
                &format!("{command} accepts exactly one positional argument"),
            ));
        }
        index += 1;
    }

    let subject = subject.ok_or_else(|| {
        command_usage(
            command,
            if command == "run" {
                "run requires task text"
            } else {
                "show requires a session ID"
            },
        )
    })?;
    Ok((
        subject,
        data_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR)),
    ))
}

fn command_usage(command: &str, message: &str) -> CliFailure {
    let syntax = if command == "run" {
        "Usage: ymp run \"<task>\" [--data-dir PATH]"
    } else {
        "Usage: ymp show <session-id> [--data-dir PATH]"
    };
    CliFailure::usage(format!("{message}\n\n{syntax}"))
}

fn run_command(goal: String, data_dir: PathBuf) -> Result<CommandOutput, CliFailure> {
    let current_dir = std::env::current_dir().map_err(|error| {
        CliFailure::operational(format!("cannot read working directory: {error}"))
    })?;
    let workspace = current_dir.canonicalize().map_err(|error| {
        CliFailure::operational(format!("cannot resolve working directory: {error}"))
    })?;
    let data_dir = absolute_path(&current_dir, &data_dir);
    let seed = identifier_seed();
    let session_id = SessionId::new(format!("session-{seed}"))
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let task = build_task(&goal, &seed)?;
    let limits = default_limits()?;

    let journal = SqliteJournal::open(&data_dir).map_err(|error| {
        CliFailure::operational(format!("cannot open session journal: {error}"))
    })?;
    let data_dir = data_dir.canonicalize().map_err(|error| {
        CliFailure::operational(format!("cannot resolve session data directory: {error}"))
    })?;
    let workspace_mode = WorkspaceMode::for_paths(&workspace, &data_dir);
    let access = workspace_access(&workspace, workspace_mode)?;
    let request = assignment_request(access.clone(), limits)?;
    let application = Application::new(journal.clone());
    let clock = Arc::new(SystemClock::new());
    let registry = CodexRegistry::new(codex_agent()?, codex_offering()?, "codex");
    let backend = CodexBackend::new("codex", clock.clone())
        .with_prompt(goal)
        .with_extra_args(codex_extra_args(workspace_mode));
    let scenario = ExecutionScenario::over(
        journal,
        backend,
        [access],
        registry,
        DEFAULT_RESERVATION,
        clock,
    );

    let report = run_one(
        &application,
        &scenario,
        session_id.clone(),
        task,
        request,
        limits,
        std::thread::sleep,
    )
    .map_err(|failure| failure.with_session_context(&session_id, &data_dir))?;
    let succeeded = report.outcome.succeeded();
    Ok(CommandOutput::observed(
        format_run_report(&report, &workspace, workspace_mode, &data_dir),
        succeeded,
    ))
}

fn show_command(session_id: String, data_dir: PathBuf) -> Result<CommandOutput, CliFailure> {
    let session_id = SessionId::new(session_id)
        .map_err(|error| CliFailure::usage(format!("invalid session ID: {error}")))?;
    let current_dir = std::env::current_dir().map_err(|error| {
        CliFailure::operational(format!("cannot read working directory: {error}"))
    })?;
    let data_dir = absolute_path(&current_dir, &data_dir);
    let database_path = data_dir.join(DATABASE_FILE_NAME);
    match database_path.try_exists() {
        Ok(true) if database_path.is_file() => {}
        Ok(_) => {
            return Err(CliFailure::operational(format!(
                "no session journal exists at {}; run 'ymp run' with this data directory first",
                database_path.display()
            )));
        }
        Err(error) => {
            return Err(CliFailure::operational(format!(
                "cannot inspect session journal at {}: {error}",
                database_path.display()
            )));
        }
    }
    let journal = SqliteJournal::open(&data_dir).map_err(|error| {
        CliFailure::operational(format!("cannot open session journal: {error}"))
    })?;
    let application = Application::new(journal.clone());
    let session = application
        .read_session(&session_id)
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let execution = read_execution(&journal, &session_id)
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let data_dir = data_dir.canonicalize().unwrap_or(data_dir);
    Ok(CommandOutput::success(format_session_history(
        &session, &execution, &data_dir,
    )))
}

fn absolute_path(current_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        current_dir.join(path)
    }
}

fn identifier_seed() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{elapsed:x}-{:x}-{sequence:x}", std::process::id())
}

fn build_task(goal: &str, seed: &str) -> Result<Task, CliFailure> {
    let criterion = Criterion::new(
        CriterionId::new(COMPLETION_CRITERION_ID)
            .map_err(|error| CliFailure::operational(error.to_string()))?,
        COMPLETION_CRITERION,
    )
    .map_err(|error| CliFailure::operational(error.to_string()))?;
    Ok(Task::new(
        TaskId::new(format!("task-{seed}"))
            .map_err(|error| CliFailure::operational(error.to_string()))?,
        Goal::new(goal).map_err(|error| CliFailure::operational(error.to_string()))?,
        AcceptanceContract::new(vec![criterion])
            .map_err(|error| CliFailure::operational(error.to_string()))?,
        Constraints::new(Vec::new()).map_err(|error| CliFailure::operational(error.to_string()))?,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceMode {
    ReadOnly,
    ReadWrite,
}

impl WorkspaceMode {
    fn for_paths(workspace: &Path, data_dir: &Path) -> Self {
        if data_dir.starts_with(workspace) {
            Self::ReadOnly
        } else {
            Self::ReadWrite
        }
    }

    fn operations(self) -> &'static [WorkspaceOperation] {
        match self {
            Self::ReadOnly => &[WorkspaceOperation::Read],
            Self::ReadWrite => &[WorkspaceOperation::Read, WorkspaceOperation::Write],
        }
    }

    fn sandbox(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::ReadWrite => "workspace-write",
        }
    }

    fn report(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only (the data directory is inside the workspace)",
            Self::ReadWrite => "read and write",
        }
    }
}

fn workspace_access(
    workspace: &Path,
    workspace_mode: WorkspaceMode,
) -> Result<WorkspaceAccess, CliFailure> {
    let scope = workspace
        .to_str()
        .ok_or_else(|| CliFailure::operational("working directory path must be valid UTF-8"))?;
    WorkspaceAccess::new(
        WorkspaceScope::new(scope).map_err(|error| CliFailure::operational(error.to_string()))?,
        workspace_mode.operations().iter().copied(),
    )
    .map_err(|error| CliFailure::operational(error.to_string()))
}

fn codex_extra_args(workspace_mode: WorkspaceMode) -> Vec<String> {
    [
        "--ephemeral",
        "-c",
        "approval_policy=\"never\"",
        "--sandbox",
        workspace_mode.sandbox(),
        "-c",
        "sandbox_workspace_write.writable_roots=[]",
        "-c",
        "sandbox_workspace_write.exclude_slash_tmp=true",
        "-c",
        "sandbox_workspace_write.exclude_tmpdir_env_var=true",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn default_limits() -> Result<InvocationLimits, CliFailure> {
    InvocationLimits::new(
        DEFAULT_MAX_TURNS,
        DEFAULT_MAX_OUTPUT_CHARS,
        DEFAULT_MAX_WALL_CLOCK,
    )
    .map_err(|error| CliFailure::operational(error.to_string()))
}

fn codex_agent() -> Result<AgentId, CliFailure> {
    AgentId::new(CODEX_AGENT_ID).map_err(|error| CliFailure::operational(error.to_string()))
}

fn codex_offering() -> Result<ModelOffering, CliFailure> {
    ModelOffering::new(
        OfferingId::new(CODEX_OFFERING_ID)
            .map_err(|error| CliFailure::operational(error.to_string()))?,
        Vec::new(),
    )
    .map_err(|error| CliFailure::operational(error.to_string()))
}

fn assignment_request(
    access: WorkspaceAccess,
    limits: InvocationLimits,
) -> Result<AssignmentRequest, CliFailure> {
    let allowance = Allowance::new(DEFAULT_RESERVATION, ReservationPurpose::Production, limits)
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    AssignmentRequest::new(
        codex_agent()?,
        Role::new(IMPLEMENTER_ROLE).map_err(|error| CliFailure::operational(error.to_string()))?,
        Settings::new(),
        allowance,
        vec![access],
    )
    .map_err(|error| CliFailure::operational(error.to_string()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RunOutcome {
    Terminated(Termination),
    FailedAtStart(ErrorClass),
    UncertainAtStart(ErrorClass),
    Uncertain(UncertaintyCause),
}

impl RunOutcome {
    fn succeeded(&self) -> bool {
        matches!(self, Self::Terminated(Termination::Completed))
    }

    fn render(&self) -> String {
        match self {
            Self::Terminated(termination) => termination.to_string(),
            Self::FailedAtStart(class) => format!("failed (class {class})"),
            Self::UncertainAtStart(class) => format!(
                "uncertain (start failed with class {class}; whether execution started is unknown)"
            ),
            Self::Uncertain(cause) => format!("uncertain ({cause})"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RunReport {
    session_id: SessionId,
    session_status: SessionStatus,
    revision: ymp_runtime::Revision,
    invocation: InvocationId,
    outcome: RunOutcome,
    usage: ObservedUsage,
}

#[allow(clippy::too_many_arguments)]
fn run_one<J, B, R, W>(
    application: &Application<J>,
    scenario: &ExecutionScenario<J, B, R>,
    session_id: SessionId,
    task: Task,
    request: AssignmentRequest,
    limits: InvocationLimits,
    mut wait: W,
) -> Result<RunReport, CliFailure>
where
    J: Journal + Clone,
    B: ExecutionBackend,
    R: Registry,
    W: FnMut(Duration),
{
    let opened = application
        .open_session(session_id.clone(), task)
        .map_err(|error| CliFailure::operational(format!("cannot open session: {error}")))?;
    scenario
        .scan()
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let assignment = scenario
        .admit(&session_id, request, opened.revision())
        .map_err(map_admission_failure)?;
    let invocation = assignment.invocation().clone();
    let revision = scenario
        .execution_view(&session_id)
        .map_err(|error| CliFailure::operational(error.to_string()))?
        .revision();

    match scenario
        .invoke(&session_id, &invocation, revision)
        .map_err(|error| CliFailure::operational(error.to_string()))?
    {
        StartOutcome::FailedAtStart { class } => {
            return report_from_history(
                scenario,
                &session_id,
                invocation,
                RunOutcome::FailedAtStart(class),
            );
        }
        StartOutcome::UncertainAtStart { class } => {
            return report_from_history(
                scenario,
                &session_id,
                invocation,
                RunOutcome::UncertainAtStart(class),
            );
        }
        StartOutcome::Started => {}
    }

    let started_at = scenario.clock_elapsed();
    let cancellation_at = limits.max_wall_clock().saturating_sub(CANCELLATION_LEAD);
    let mut cancellation_requested = false;
    loop {
        let elapsed = scenario.clock_elapsed().saturating_sub(started_at);
        if !cancellation_requested && elapsed >= cancellation_at {
            let revision = scenario
                .execution_view(&session_id)
                .map_err(|error| CliFailure::operational(error.to_string()))?
                .revision();
            scenario
                .cancel(&session_id, &invocation, revision)
                .map_err(|error| CliFailure::operational(error.to_string()))?;
            cancellation_requested = true;
        }

        let revision = scenario
            .execution_view(&session_id)
            .map_err(|error| CliFailure::operational(error.to_string()))?
            .revision();
        match scenario
            .observe(&session_id, &invocation, revision)
            .map_err(|error| CliFailure::operational(error.to_string()))?
        {
            ObservationOutcome::Terminated { termination, .. } => {
                let revision = scenario
                    .execution_view(&session_id)
                    .map_err(|error| CliFailure::operational(error.to_string()))?
                    .revision();
                scenario
                    .settle(&session_id, &invocation, revision)
                    .map_err(|error| CliFailure::operational(error.to_string()))?;
                return report_from_history(
                    scenario,
                    &session_id,
                    invocation,
                    RunOutcome::Terminated(termination),
                );
            }
            ObservationOutcome::UncertainAfterDeadline { .. } => {
                return report_from_history(
                    scenario,
                    &session_id,
                    invocation,
                    RunOutcome::Uncertain(UncertaintyCause::BoundedWaitExpired),
                );
            }
            ObservationOutcome::WaitingForTermination { .. } => {
                let elapsed = scenario.clock_elapsed().saturating_sub(started_at);
                let next_boundary = if cancellation_requested {
                    limits.max_wall_clock()
                } else {
                    cancellation_at
                };
                let remaining = next_boundary.saturating_sub(elapsed);
                wait(OBSERVATION_INTERVAL.min(remaining));
            }
        }
    }
}

fn map_admission_failure(error: AdmissionFailure) -> CliFailure {
    match error {
        AdmissionFailure::Denied(AdmissionDenial::IneligibleAgent {
            ineligibility: AgentIneligibility::Excluded(ExclusionReason::NotReady { detail }),
            ..
        }) => CliFailure::operational(format!(
            "Codex is not ready: {detail}\nNo invocation was admitted."
        )),
        other => CliFailure::operational(other.to_string()),
    }
}

fn report_from_history<J, B, R>(
    scenario: &ExecutionScenario<J, B, R>,
    session_id: &SessionId,
    invocation: InvocationId,
    outcome: RunOutcome,
) -> Result<RunReport, CliFailure>
where
    J: Journal + Clone,
    B: ExecutionBackend,
    R: Registry,
{
    let session = scenario
        .read_session(session_id)
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let execution = scenario
        .execution_view(session_id)
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let invocation_view = execution.invocation(&invocation).ok_or_else(|| {
        CliFailure::operational(format!(
            "invocation '{invocation}' is missing from session history"
        ))
    })?;
    let usage = invocation_view
        .settled_usage()
        .or_else(|| invocation_view.observed_usage())
        .copied()
        .unwrap_or_else(ObservedUsage::unknown);
    Ok(RunReport {
        session_id: session_id.clone(),
        session_status: session.status(),
        revision: execution.revision(),
        invocation,
        outcome,
        usage,
    })
}

fn format_run_report(
    report: &RunReport,
    workspace: &Path,
    workspace_mode: WorkspaceMode,
    data_dir: &Path,
) -> String {
    format!(
        "Session: {}\nSession status: {}\nRevision: {}\nInvocation: {}\nOutcome: {}\nUsage: {}\nWorkspace: {}\nWorkspace access: {}\nData directory: {}\nAcceptance: not evaluated\n",
        report.session_id,
        session_status(report.session_status),
        report.revision,
        report.invocation,
        report.outcome.render(),
        format_usage(&report.usage),
        workspace.display(),
        workspace_mode.report(),
        data_dir.display(),
    )
}

fn format_session_history(
    session: &ymp_runtime::SessionView,
    execution: &SessionExecutionView,
    data_dir: &Path,
) -> String {
    let mut output = format!(
        "Session: {}\nStatus: {}\nRevision: {}\nGoal: {:?}\nData directory: {}\nInvocations: {}\n",
        session.session_id(),
        session_status(session.status()),
        session.revision(),
        session.task().goal().request(),
        data_dir.display(),
        execution.invocation_count(),
    );
    for (index, (invocation_id, invocation)) in execution.invocations().enumerate() {
        let outcome = invocation_outcome(invocation);
        let usage = invocation
            .settled_usage()
            .or_else(|| invocation.observed_usage())
            .copied()
            .unwrap_or_else(ObservedUsage::unknown);
        output.push_str(&format!(
            "\nInvocation {}: {}\n  Agent: {}\n  Role: {}\n  Status: {}\n  Outcome: {}\n  Usage: {}\n  Accounted: {}\n",
            index + 1,
            invocation_id,
            invocation.assignment().agent(),
            invocation.assignment().role(),
            invocation.status(),
            outcome,
            format_usage(&usage),
            if invocation.settled_usage().is_some() {
                "yes"
            } else {
                "no"
            },
        ));
    }
    output.push_str("\nAcceptance: not evaluated\n");
    output
}

fn invocation_outcome(invocation: &ymp_runtime::InvocationView) -> String {
    if let Some(termination) = invocation.termination() {
        termination.to_string()
    } else if let Some(class) = invocation.failure_class() {
        format!("failed (class {class})")
    } else if let Some(cause) = invocation.uncertainty_cause() {
        format!("uncertain ({cause})")
    } else {
        "not observed".to_owned()
    }
}

fn format_usage(usage: &ObservedUsage) -> String {
    let turns = usage
        .turns()
        .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
    let output_chars = usage
        .output_chars()
        .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
    let wall_clock = usage.wall_clock().map_or_else(
        || "unknown".to_owned(),
        |value| format!("{} ms", value.as_millis()),
    );
    format!("turns {turns}, output characters {output_chars}, wall clock {wall_clock}")
}

fn session_status(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Open => "open",
        SessionStatus::Cancelled => "cancelled",
    }
}

fn help(metadata: ApplicationMetadata) -> String {
    format!(
        "{} {}\n\n{}\n\nCommands:\n  run   Execute one admitted Codex invocation for a task.\n  show  Read a persisted session and its invocation history.\n\n{}\n\n{}\n",
        metadata.name(),
        metadata.version(),
        usage(metadata),
        storage_help(),
        metadata.capability_limit(),
    )
}

fn run_help(metadata: ApplicationMetadata) -> String {
    format!(
        "{} run\n\nUsage: {} run \"<task>\" [--data-dir PATH]\n\nCreates a Task whose goal is the exact argument, whose sole criterion is\n\"{}\", and whose Constraints contain no\nuser-supplied conditions. Empty Constraints do not authorize unrestricted\nexecution. The current working directory is passed to one Codex invocation. When\nthe data directory is inside the workspace, including at the default location,\nthe workspace is admitted read-only so the invocation cannot alter its\nauthoritative journal. A data directory outside the workspace enables read and\nwrite access. Codex runs without approval prompts and without persisting its own\nsession rollout files.\n\nDefault limits: {} observed turns, {} accepted output characters, and {} minutes\nof wall-clock time. YMP requests cancellation {} seconds before the deadline. A\ncancelled outcome requires observed termination; otherwise the persisted state is\nuncertain.\n\n{}\n\n{}\n",
        metadata.name(),
        metadata.name(),
        COMPLETION_CRITERION,
        DEFAULT_MAX_TURNS,
        DEFAULT_MAX_OUTPUT_CHARS,
        DEFAULT_MAX_WALL_CLOCK.as_secs() / 60,
        CANCELLATION_LEAD.as_secs(),
        storage_help(),
        metadata.capability_limit(),
    )
}

fn show_help(metadata: ApplicationMetadata) -> String {
    format!(
        "{} show\n\nUsage: {} show <session-id> [--data-dir PATH]\n\nReopens an existing journal and prints the session status plus every persisted\ninvocation's status, observed outcome, and observed usage. The command does not\ncreate a missing data directory or journal.\n\n{}\n",
        metadata.name(),
        metadata.name(),
        storage_help(),
    )
}

fn usage(metadata: ApplicationMetadata) -> String {
    format!(
        "Usage:\n  {} run \"<task>\" [--data-dir PATH]\n  {} show <session-id> [--data-dir PATH]\n  {} [--help | --version]",
        metadata.name(),
        metadata.name(),
        metadata.name(),
    )
}

fn storage_help() -> String {
    format!(
        "Session storage:\n  The default data directory is ./{DEFAULT_DATA_DIR}, relative to the current\n  working directory. It contains journal.db; SQLite may create journal.db-wal\n  and journal.db-shm while the journal is open or leave them after interruption.\n  Session history persists across ymp processes until the user deletes the whole\n  data directory. Do not delete or copy individual database files while ymp runs."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ymp_runtime::{
        ManualClock, ScriptedBackend, ScriptedOutcome, ScriptedProvider, ScriptedRegistry,
    };

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ymp-cli-{name}-{}-{}",
                std::process::id(),
                ID_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).expect("test directory is created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_limits() -> InvocationLimits {
        InvocationLimits::new(4, 4_000, Duration::from_secs(60)).expect("valid limits")
    }

    fn scripted_run<J>(
        journal: J,
        workspace: &Path,
        outcome: ScriptedOutcome,
        ready: bool,
        session_name: &str,
    ) -> Result<(RunReport, Application<J>, J), CliFailure>
    where
        J: Journal + Clone,
    {
        let access = workspace_access(workspace, WorkspaceMode::ReadWrite)?;
        let provider = ScriptedProvider::new(codex_agent()?, codex_offering()?)
            .with_readiness(ready, "the scripted Codex registry is not ready")
            .with_effective_workspace_accesses([access.clone()]);
        let backend = ScriptedBackend::for_provider(&provider).with_outcome(outcome);
        let clock = ManualClock::new();
        let scenario = ExecutionScenario::over(
            journal.clone(),
            backend,
            [access.clone()],
            ScriptedRegistry::new([provider]),
            DEFAULT_RESERVATION,
            Arc::new(clock),
        );
        let application = Application::new(journal.clone());
        let session_id = SessionId::new(session_name).expect("valid session ID");
        let report = run_one(
            &application,
            &scenario,
            session_id,
            build_task("edit the requested file", "test")?,
            assignment_request(access, test_limits())?,
            test_limits(),
            |_| {},
        )?;
        Ok((report, application, journal))
    }

    #[test]
    fn happy_path_runs_one_scripted_admitted_invocation() {
        let workspace = TestDir::new("happy-workspace");
        let usage = ObservedUsage::unknown()
            .with_turns(2)
            .with_output_chars(321)
            .with_wall_clock(Duration::from_millis(450));
        let (report, application, journal) = scripted_run(
            ymp_runtime::MemoryJournal::new(),
            workspace.path(),
            ScriptedOutcome::completes(None, usage),
            true,
            "session-happy",
        )
        .expect("scripted run completes");

        assert_eq!(
            report.outcome,
            RunOutcome::Terminated(Termination::Completed)
        );
        assert_eq!(report.usage, usage);
        assert_eq!(report.revision, ymp_runtime::Revision::new(6));
        let session = application
            .read_session(&report.session_id)
            .expect("session reads");
        assert_eq!(session.task().goal().request(), "edit the requested file");
        assert_eq!(session.task().acceptance_contract().criteria().len(), 1);
        assert_eq!(
            session.task().acceptance_contract().criteria()[0].description(),
            COMPLETION_CRITERION
        );
        assert!(session.task().constraints().is_empty());
        assert_eq!(
            read_execution(&journal, &report.session_id)
                .expect("execution reads")
                .invocation_count(),
            1
        );
    }

    #[test]
    fn failed_scripted_outcome_reports_its_class_and_usage() {
        let workspace = TestDir::new("failed-workspace");
        let usage = ObservedUsage::unknown()
            .with_turns(1)
            .with_output_chars(17)
            .with_wall_clock(Duration::from_millis(80));
        let (report, _, _) = scripted_run(
            ymp_runtime::MemoryJournal::new(),
            workspace.path(),
            ScriptedOutcome::fails("provider-overloaded", usage),
            true,
            "session-failed",
        )
        .expect("scripted failure is an observed report");

        assert_eq!(
            report.outcome,
            RunOutcome::Terminated(Termination::Failed {
                class: ErrorClass::new("provider-overloaded").expect("valid class")
            })
        );
        assert_eq!(
            report.outcome.render(),
            "failed (class provider-overloaded)"
        );
        assert_eq!(report.usage, usage);
        assert!(!report.outcome.succeeded());
    }

    #[test]
    fn not_ready_registry_is_typed_and_admits_no_invocation() {
        let workspace = TestDir::new("not-ready-workspace");
        let journal = ymp_runtime::MemoryJournal::new();
        let result = scripted_run(
            journal.clone(),
            workspace.path(),
            ScriptedOutcome::never_reports(),
            false,
            "session-not-ready",
        );
        let failure = result.expect_err("not-ready registry refuses admission");

        assert_eq!(failure.exit_code, 1);
        assert_eq!(
            failure.message,
            "Codex is not ready: the scripted Codex registry is not ready\nNo invocation was admitted."
        );
        let session_id = SessionId::new("session-not-ready").expect("valid ID");
        assert_eq!(
            read_execution(&journal, &session_id)
                .expect("opened session persists")
                .invocation_count(),
            0
        );
    }

    #[test]
    fn show_reopens_sqlite_and_reads_persisted_invocation_history() {
        let workspace = TestDir::new("restart-workspace");
        let data = TestDir::new("restart-data");
        let session_id = SessionId::new("session-restart").expect("valid session ID");
        {
            let journal = SqliteJournal::open(data.path()).expect("journal opens");
            let usage = ObservedUsage::unknown()
                .with_turns(3)
                .with_output_chars(99)
                .with_wall_clock(Duration::from_millis(700));
            let (report, _, _) = scripted_run(
                journal,
                workspace.path(),
                ScriptedOutcome::completes(None, usage),
                true,
                session_id.as_str(),
            )
            .expect("scripted run completes");
            assert_eq!(report.revision, ymp_runtime::Revision::new(6));
        }

        let output = show_command(session_id.as_str().to_owned(), data.path().to_path_buf())
            .expect("show reopens the journal");
        assert_eq!(output.exit_code(), 0);
        assert!(output.stdout().contains("Session: session-restart"));
        assert!(output.stdout().contains("Revision: 6"));
        assert!(output.stdout().contains("Invocations: 1"));
        assert!(output.stdout().contains("Status: terminated"));
        assert!(output.stdout().contains("Outcome: completed"));
        assert!(
            output
                .stdout()
                .contains("Usage: turns 3, output characters 99, wall clock 700 ms")
        );
        assert!(output.stdout().contains("Accounted: yes"));
    }

    #[test]
    fn show_missing_journal_does_not_create_hidden_state() {
        let root = TestDir::new("show-missing");
        let missing = root.path().join("missing");
        let output = command(&[
            OsString::from("show"),
            OsString::from("session-missing"),
            OsString::from("--data-dir"),
            missing.as_os_str().to_owned(),
        ]);

        assert_eq!(output.exit_code(), 1);
        assert!(output.stderr().contains("no session journal exists"));
        assert!(!missing.exists());
    }

    #[test]
    fn help_documents_limits_storage_lifetime_and_honest_completion() {
        let output = command(&[OsString::from("run"), OsString::from("--help")]);

        assert_eq!(output.exit_code(), 0);
        assert!(output.stdout().contains("16 observed turns"));
        assert!(
            output
                .stdout()
                .contains("200000 accepted output characters")
        );
        assert!(output.stdout().contains("30 minutes"));
        assert!(output.stdout().contains("./.ymp/sessions"));
        assert!(output.stdout().contains("journal.db-wal"));
        assert!(output.stdout().contains("until the user deletes the whole"));
        assert!(output.stdout().contains("workspace is admitted read-only"));
        assert!(
            output
                .stdout()
                .contains("data directory outside the workspace")
        );
        assert!(output.stdout().contains("write access"));
        assert!(output.stdout().contains("without persisting"));
        assert!(output.stdout().contains("session rollout"));
        assert!(output.stdout().contains("does not establish acceptance"));
    }

    #[test]
    fn workspace_policy_keeps_the_journal_outside_the_invocation_write_set() {
        let root = TestDir::new("workspace-policy");
        let workspace = root.path().join("workspace");
        let internal_data = workspace.join(DEFAULT_DATA_DIR);
        let external_data = root.path().join("data");

        assert_eq!(
            WorkspaceMode::for_paths(&workspace, &internal_data),
            WorkspaceMode::ReadOnly
        );
        assert_eq!(
            WorkspaceMode::for_paths(&workspace, &external_data),
            WorkspaceMode::ReadWrite
        );
        let arguments = codex_extra_args(WorkspaceMode::ReadWrite);
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-c", "approval_policy=\"never\""])
        );
        assert_eq!(arguments.first().map(String::as_str), Some("--ephemeral"));
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["--sandbox", "workspace-write"])
        );
        assert!(
            arguments
                .windows(2)
                .any(|pair| { pair == ["-c", "sandbox_workspace_write.writable_roots=[]"] })
        );
        assert!(
            arguments.iter().any(|argument| {
                argument == "sandbox_workspace_write.exclude_tmpdir_env_var=true"
            })
        );
    }
}
