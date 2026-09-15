#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ymp_runtime::{
    AcceptanceAuthority, AcceptanceContract, AdmissionDenial, AdmissionFailure, AgentId,
    AgentIneligibility, Allowance, Application, ApplicationMetadata, AssignmentRequest,
    BuiltinCheckExecutor, Check, CheckMethod, CodexBackend, CodexRegistry, Constraints, Criterion,
    CriterionEvaluation, CriterionId, CriterionStatus, ErrorClass, Evidence, ExclusionReason,
    ExecutionBackend, ExecutionError, ExecutionScenario, Goal, InvocationId, InvocationLimits,
    Journal, JournalError, ModelOffering, ObservationOutcome, ObservedUsage, OfferingId, Registry,
    ReservationPurpose, ResourceAmount, Revision, Role, SessionExecutionView, SessionId,
    SessionStatus, Settings, StartOutcome, SystemClock, Task, TaskId, Termination,
    UncertaintyCause, WorkspaceAccess, WorkspaceOperation, WorkspaceScope, application_metadata,
    read_execution, sha256,
};
use ymp_storage::SqliteJournal;

pub const DEFAULT_DATA_DIR: &str = ".ymp/sessions";
pub const DEFAULT_MAX_TURNS: u32 = 16;
pub const DEFAULT_MAX_OUTPUT_CHARS: u64 = 200_000;
pub const DEFAULT_MAX_WALL_CLOCK: Duration = Duration::from_secs(30 * 60);

const DEFAULT_RESERVATION: ResourceAmount = ResourceAmount::new(1);
const OBSERVATION_INTERVAL: Duration = Duration::from_millis(100);
const CANCELLATION_LEAD: Duration = Duration::from_secs(5);
const INTERRUPT_WAIT: Duration = Duration::from_secs(5);
const EXECUTION_WRITE_ATTEMPTS: usize = 3;
const EXECUTION_WRITE_RETRY_INTERVAL: Duration = Duration::from_millis(25);
const CODEX_AGENT_ID: &str = "codex";
const CODEX_OFFERING_ID: &str = "codex-cli-local";
const IMPLEMENTER_ROLE: &str = "implementer";
const COMPLETION_CRITERION_ID: &str = "requested-work-completed";
const COMPLETION_CRITERION: &str = "the agent completed the requested work";
const DATABASE_FILE_NAME: &str = "journal.db";
const WORKSPACE_LOCK_PREFIX: &str = "workspace-";

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Fully rendered process result. Operational failures use exit code 1,
/// command-line usage errors use 2, SIGINT uses 130, and only successful
/// commands use 0.
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
        check: Option<String>,
    },
    Show {
        session_id: String,
        data_dir: PathBuf,
    },
}

/// Executes one CLI command without exiting the process.
pub fn command(arguments: &[OsString]) -> CommandOutput {
    let interrupted = AtomicBool::new(false);
    command_with_interrupt(arguments, &interrupted)
}

/// Executes one CLI command while observing the process SIGINT flag.
pub fn command_with_interrupt(arguments: &[OsString], interrupted: &AtomicBool) -> CommandOutput {
    let mut output =
        match parse_command(arguments).and_then(|command| execute(command, interrupted)) {
            Ok(output) => output,
            Err(failure) => CommandOutput::failure(failure),
        };
    if interrupted.load(Ordering::SeqCst) {
        output.exit_code = 130;
    }
    output
}

fn execute(command: ParsedCommand, interrupted: &AtomicBool) -> Result<CommandOutput, CliFailure> {
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
        ParsedCommand::Run {
            goal,
            data_dir,
            check,
        } => run_command(goal, data_dir, check, interrupted),
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
            let (subject, data_dir, check) = parse_subject_and_options("run", &arguments[1..])?;
            let goal = subject
                .into_string()
                .map_err(|_| command_usage("run", "run task text must be valid UTF-8"))?;
            if goal.trim().is_empty() {
                return Err(command_usage("run", "run task text must not be blank"));
            }
            Ok(ParsedCommand::Run {
                goal,
                data_dir,
                check,
            })
        }
        "show" => {
            let (subject, data_dir, _) = parse_subject_and_options("show", &arguments[1..])?;
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

fn parse_subject_and_options(
    command: &str,
    arguments: &[OsString],
) -> Result<(OsString, PathBuf, Option<String>), CliFailure> {
    let mut subject = None;
    let mut data_dir = None;
    let mut check = None;
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
        if !options_ended && command == "run" && argument == OsStr::new("--check") {
            if check.is_some() {
                return Err(command_usage(command, "--check may be specified only once"));
            }
            let value = arguments
                .get(index + 1)
                .ok_or_else(|| command_usage(command, "--check requires a shell command"))?;
            let value = value
                .to_str()
                .ok_or_else(|| command_usage(command, "--check command must be valid UTF-8"))?;
            if value.trim().is_empty() {
                return Err(command_usage(command, "--check command must not be blank"));
            }
            check = Some(value.to_owned());
            index += 2;
            continue;
        }
        if !options_ended
            && command == "run"
            && let Some(value) = argument
                .to_str()
                .and_then(|value| value.strip_prefix("--check="))
        {
            if check.is_some() {
                return Err(command_usage(command, "--check may be specified only once"));
            }
            if value.trim().is_empty() {
                return Err(command_usage(command, "--check command must not be blank"));
            }
            check = Some(value.to_owned());
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
        check,
    ))
}

fn command_usage(command: &str, message: &str) -> CliFailure {
    let syntax = if command == "run" {
        "Usage: ymp run \"<task>\" [--check \"<shell command>\"] [--data-dir PATH]"
    } else {
        "Usage: ymp show <session-id> [--data-dir PATH]"
    };
    CliFailure::usage(format!("{message}\n\n{syntax}"))
}

fn run_command(
    goal: String,
    data_dir: PathBuf,
    check_command: Option<String>,
    interrupted: &AtomicBool,
) -> Result<CommandOutput, CliFailure> {
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
    let check = check_command
        .map(|command| {
            Ok((
                BuiltinCheckExecutor::new()
                    .map_err(|error| CliFailure::operational(error.to_string()))?,
                Check::command(
                    vec![
                        CriterionId::new(COMPLETION_CRITERION_ID)
                            .map_err(|error| CliFailure::operational(error.to_string()))?,
                    ],
                    command,
                )
                .map_err(|error| CliFailure::operational(error.to_string()))?,
            ))
        })
        .transpose()?;
    let limits = default_limits()?;

    let journal = SqliteJournal::open(&data_dir).map_err(|error| {
        CliFailure::operational(format!("cannot open session journal: {error}"))
    })?;
    let data_dir = data_dir.canonicalize().map_err(|error| {
        CliFailure::operational(format!("cannot resolve session data directory: {error}"))
    })?;
    let _process_lock = WorkspaceProcessLock::acquire(&data_dir, &workspace)?;
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
        journal.clone(),
        backend,
        [access],
        registry,
        DEFAULT_RESERVATION,
        clock,
    );
    let session_ids = journal.session_ids().map_err(|error| {
        CliFailure::operational(format!("cannot list session journal streams: {error}"))
    })?;
    scenario
        .restore_workspace_holds(session_ids.iter())
        .map_err(|error| CliFailure::operational(error.to_string()))?;

    let mut report = run_one(
        &application,
        &scenario,
        session_id.clone(),
        task,
        request,
        limits,
        interrupted,
        std::thread::sleep,
    )
    .map_err(|failure| failure.with_session_context(&session_id, &data_dir))?;
    if report.outcome.succeeded()
        && !interrupted.load(Ordering::SeqCst)
        && let Some((executor, check)) = check.as_ref()
    {
        let evidence = executor
            .execute_protected(check, &workspace, &data_dir)
            .map_err(|error| {
                CliFailure::operational(error.to_string())
                    .with_session_context(&session_id, &data_dir)
            })?;
        let session = AcceptanceAuthority::new(&journal)
            .record_evidence(
                &session_id,
                report.invocation.clone(),
                evidence,
                report.revision,
            )
            .map_err(|error| {
                CliFailure::operational(error.to_string())
                    .with_session_context(&session_id, &data_dir)
            })?;
        report.revision = session.revision();
    }
    let session = application
        .read_session(&session_id)
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let execution = read_execution(&journal, &session_id)
        .map_err(|error| CliFailure::operational(error.to_string()))?;
    let stored_evidence = session
        .evidence()
        .map(|(_, evidence)| evidence.clone())
        .collect::<Vec<_>>();
    let check_satisfied = check.is_none()
        || CriterionEvaluation::evaluate(session.task().acceptance_contract(), &stored_evidence)
            .map_err(|error| CliFailure::operational(error.to_string()))?
            .satisfied();
    let stdout = format_run_report(
        &report,
        &session,
        &execution,
        &workspace,
        workspace_mode,
        &data_dir,
        check.is_some(),
    );
    Ok(CommandOutput::observed(
        stdout,
        report.outcome.succeeded() && check_satisfied,
    ))
}

struct WorkspaceProcessLock {
    _file: File,
}

impl WorkspaceProcessLock {
    fn acquire(data_dir: &Path, workspace: &Path) -> Result<Self, CliFailure> {
        let workspace = workspace
            .to_str()
            .ok_or_else(|| CliFailure::operational("working directory path must be valid UTF-8"))?;
        let lock_path = data_dir.join(format!(
            "{WORKSPACE_LOCK_PREFIX}{}.lock",
            sha256(workspace.as_bytes())
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| {
                CliFailure::operational(format!(
                    "cannot open workspace lock {}: {error}",
                    lock_path.display()
                ))
            })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(CliFailure::operational(format!(
                "workspace is already in use by another ymp process: {workspace}"
            ))),
            Err(TryLockError::Error(error)) => Err(CliFailure::operational(format!(
                "cannot lock workspace {workspace}: {error}"
            ))),
        }
    }
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
    interrupted: &AtomicBool,
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
    match retry_execution_write(scenario, &session_id, &mut wait, |revision| {
        scenario.invoke(&session_id, &invocation, revision)
    })? {
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
        let interrupted_now = interrupted.load(Ordering::SeqCst);
        if !cancellation_requested && (interrupted_now || elapsed >= cancellation_at) {
            if interrupted_now {
                scenario
                    .shorten_observation_deadline(&invocation, INTERRUPT_WAIT)
                    .map_err(|error| CliFailure::operational(error.to_string()))?;
            }
            retry_execution_write(scenario, &session_id, &mut wait, |revision| {
                scenario.cancel(&session_id, &invocation, revision)
            })?;
            cancellation_requested = true;
        }

        match retry_execution_write(scenario, &session_id, &mut wait, |revision| {
            scenario.observe(&session_id, &invocation, revision)
        })? {
            ObservationOutcome::Terminated { termination, .. } => {
                retry_execution_write(scenario, &session_id, &mut wait, |revision| {
                    scenario.settle(&session_id, &invocation, revision)
                })?;
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

fn retry_execution_write<J, B, R, W, T>(
    scenario: &ExecutionScenario<J, B, R>,
    session_id: &SessionId,
    wait: &mut W,
    mut operation: impl FnMut(Revision) -> Result<T, ExecutionError>,
) -> Result<T, CliFailure>
where
    J: Journal + Clone,
    B: ExecutionBackend,
    R: Registry,
    W: FnMut(Duration),
{
    for attempt in 0..EXECUTION_WRITE_ATTEMPTS {
        let revision = scenario
            .execution_view(session_id)
            .map_err(|error| CliFailure::operational(error.to_string()))?
            .revision();
        match operation(revision) {
            Ok(outcome) => return Ok(outcome),
            Err(error)
                if attempt + 1 < EXECUTION_WRITE_ATTEMPTS && retryable_execution_write(&error) =>
            {
                wait(EXECUTION_WRITE_RETRY_INTERVAL);
            }
            Err(error) => return Err(CliFailure::operational(error.to_string())),
        }
    }
    unreachable!("the execution write loop returns from every final attempt")
}

fn retryable_execution_write(error: &ExecutionError) -> bool {
    matches!(
        error,
        ExecutionError::StaleRevision { .. }
            | ExecutionError::Journal(
                JournalError::AdapterFailure { .. } | JournalError::IndeterminateCommit { .. }
            )
    )
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
    session: &ymp_runtime::SessionView,
    execution: &SessionExecutionView,
    workspace: &Path,
    workspace_mode: WorkspaceMode,
    data_dir: &Path,
    check_requested: bool,
) -> String {
    let mut output = format!(
        "Session: {}\nSession status: {}\nRevision: {}\nInvocation: {}\nOutcome: {}\nUsage: {}\nWorkspace: {}\nWorkspace access: {}\nData directory: {}\n",
        report.session_id,
        session_status(report.session_status),
        report.revision,
        report.invocation,
        report.outcome.render(),
        format_usage(&report.usage),
        workspace.display(),
        workspace_mode.report(),
        data_dir.display(),
    );
    output.push_str(&format_criterion_evaluation(
        session,
        execution,
        Some(check_requested),
    ));
    output
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
    output.push('\n');
    output.push_str(&format_criterion_evaluation(session, execution, None));
    output
}

fn format_criterion_evaluation(
    session: &ymp_runtime::SessionView,
    execution: &SessionExecutionView,
    check_requested: Option<bool>,
) -> String {
    let evidence = session
        .evidence()
        .map(|(_, evidence)| evidence.clone())
        .collect::<Vec<_>>();
    let evaluation = CriterionEvaluation::evaluate(session.task().acceptance_contract(), &evidence)
        .expect("replayed session evidence covers only declared criteria");
    let mut output = String::from("Acceptance:\n");
    for criterion in session.task().acceptance_contract().criteria() {
        let status = evaluation
            .status(criterion.id())
            .expect("every declared criterion is projected");
        match status {
            CriterionStatus::Satisfied | CriterionStatus::Failed => {
                let observed = evidence
                    .iter()
                    .find(|item| item.check().criteria().contains(criterion.id()))
                    .expect("evaluated criterion has evidence");
                let label = if status == CriterionStatus::Satisfied {
                    "satisfied with evidence"
                } else {
                    "failed with evidence"
                };
                output.push_str(&format!(
                    "  Criterion {}: {label}\n    Description: {}\n",
                    criterion.id(),
                    criterion.description()
                ));
                format_evidence(&mut output, observed);
            }
            CriterionStatus::NotEvaluated => {
                output.push_str(&format!(
                    "  Criterion {}: not evaluated ({})\n    Description: {}\n",
                    criterion.id(),
                    unevaluated_reason(execution, check_requested),
                    criterion.description()
                ));
            }
        }
    }
    output
}

fn format_evidence(output: &mut String, evidence: &Evidence) {
    match evidence.check().method() {
        CheckMethod::Command { command } => output.push_str(&format!(
            "    Check command: {command:?}\n    Exit code: {}\n    Workspace: {}\n    Check time: {} ms\n",
            evidence
                .exit_code()
                .expect("command evidence always has an exit code"),
            evidence.workspace(),
            evidence.elapsed_ms(),
        )),
        CheckMethod::ExactBytes { path, .. } => {
            let digest = evidence.files()[0].sha256().unwrap_or("missing");
            output.push_str(&format!(
                "    Exact-byte file: {}\n    Observed SHA-256: {digest}\n    Workspace: {}\n    Check time: {} ms\n",
                path.display(),
                evidence.workspace(),
                evidence.elapsed_ms(),
            ));
        }
    }
}

fn unevaluated_reason(execution: &SessionExecutionView, check_requested: Option<bool>) -> String {
    if let Some((_, invocation)) = execution.invocations().next()
        && invocation.termination() != Some(&Termination::Completed)
    {
        return format!(
            "invocation did not complete with outcome completed; observed {}",
            invocation_outcome(invocation)
        );
    }
    match check_requested {
        Some(false) => "no --check was supplied".to_owned(),
        Some(true) => "no check evidence was recorded".to_owned(),
        None => "no check evidence is recorded in the session".to_owned(),
    }
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
    let mut output = format!(
        "{} run\n\nUsage: {} run \"<task>\" [--check \"<shell command>\"] [--data-dir PATH]\n\nCreates a Task whose goal is the exact argument, whose sole criterion is\n\"{}\", and whose Constraints contain no\nuser-supplied conditions. Empty Constraints do not authorize unrestricted\nexecution. The current working directory is passed to one Codex invocation. When\nthe data directory is inside the workspace, including at the default location,\nthe workspace is admitted read-only so the invocation cannot alter its\nauthoritative journal. A data directory outside the workspace enables read and\nwrite access. Codex runs without approval prompts and without persisting its own\nsession rollout files.\n\nAfter an observed completed invocation, --check runs the exact command through\n/bin/sh in the actual workspace with the inherited environment and a {}-second\nlimit. An operating-system sandbox denies writes to the complete data-directory\npath, including absolute and symlink-resolved access. Exit code 0 satisfies the\ncriterion; any other exit code fails it. If the sandbox cannot start the inner\nshell, YMP records no evidence. YMP pins the shell and sandbox executable by\nSHA-256 before invocation and verifies them before and after the check. Executables\nand files referenced by arbitrary shell syntax are not inferred or pinned by the\nCLI. The check executor has no journal access; the kernel validates and records its\nobservation.\n\nDefault limits: {} observed turns, {} accepted output characters, and {} minutes\nof wall-clock time. YMP requests cancellation {} seconds before the deadline. A\ncancelled outcome requires observed termination; otherwise the persisted state is\nuncertain.\n\n{}\n\n{}\n",
        metadata.name(),
        metadata.name(),
        COMPLETION_CRITERION,
        ymp_runtime::DEFAULT_CHECK_TIMEOUT.as_secs(),
        DEFAULT_MAX_TURNS,
        DEFAULT_MAX_OUTPUT_CHARS,
        DEFAULT_MAX_WALL_CLOCK.as_secs() / 60,
        CANCELLATION_LEAD.as_secs(),
        storage_help(),
        metadata.capability_limit(),
    );
    output.push_str(&format!(
        "\nWhile the provider invocation is in flight, SIGINT requests cancellation through \
         the kernel, shortens the remaining observation wait to {} seconds, and exits with status \
         130 after recording either an observed termination and its resource settlement or \
         uncertainty. That cancellation path does not execute --check. SIGINT after provider \
         termination does not stop an already running check; the check reaches its own bound, its \
         observation may be recorded, and the command then exits with status 130.\n",
        INTERRUPT_WAIT.as_secs()
    ));
    output
}

fn show_help(metadata: ApplicationMetadata) -> String {
    format!(
        "{} show\n\nUsage: {} show <session-id> [--data-dir PATH]\n\nReopens an existing journal and prints the session status, every persisted\ninvocation, and recorded criterion evidence. The command does not create a missing\ndata directory or journal.\n\n{}\n",
        metadata.name(),
        metadata.name(),
        storage_help(),
    )
}

fn usage(metadata: ApplicationMetadata) -> String {
    format!(
        "Usage:\n  {} run \"<task>\" [--check \"<shell command>\"] [--data-dir PATH]\n  {} show <session-id> [--data-dir PATH]\n  {} [--help | --version]",
        metadata.name(),
        metadata.name(),
        metadata.name(),
    )
}

fn storage_help() -> String {
    format!(
        "Session storage:\n  The default data directory is ./{DEFAULT_DATA_DIR}, relative to the current\n  working directory. It contains journal.db; SQLite may create journal.db-wal\n  and journal.db-shm while the journal is open or leave them after interruption.\n  Session history persists across ymp processes until the user deletes the whole\n  data directory. Workspace lock files named workspace-<sha256>.lock coordinate\n  run processes that use this same data directory; different data directories do\n  not coordinate. Do not delete lock files or delete or copy individual database\n  files while ymp runs."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Instant;
    use ymp_runtime::{
        ManualClock, ScriptedBackend, ScriptedOutcome, ScriptedProvider, ScriptedRegistry,
    };

    const WORKSPACE_LOCK_TEST_ROLE: &str = "YMP_WORKSPACE_LOCK_TEST_ROLE";
    const WORKSPACE_LOCK_TEST_WORKSPACE: &str = "YMP_WORKSPACE_LOCK_TEST_WORKSPACE";
    const WORKSPACE_LOCK_TEST_DATA_DIR: &str = "YMP_WORKSPACE_LOCK_TEST_DATA_DIR";
    const WORKSPACE_LOCK_TEST_HELD: &str = "YMP_WORKSPACE_LOCK_TEST_HELD";
    const WORKSPACE_LOCK_TEST_CONFLICT: &str = "YMP_WORKSPACE_LOCK_TEST_CONFLICT";
    const WORKSPACE_LOCK_TEST_RELEASE: &str = "YMP_WORKSPACE_LOCK_TEST_RELEASE";
    const WORKSPACE_LOCK_TEST_RETRY: &str = "YMP_WORKSPACE_LOCK_TEST_RETRY";

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

    #[derive(Clone)]
    struct FlakyExecutionJournal {
        inner: ymp_runtime::MemoryJournal,
        fail_started: Arc<AtomicBool>,
        fail_observed: Arc<AtomicBool>,
        fail_accounted: Arc<AtomicBool>,
    }

    impl FlakyExecutionJournal {
        fn new() -> Self {
            Self {
                inner: ymp_runtime::MemoryJournal::new(),
                fail_started: Arc::new(AtomicBool::new(true)),
                fail_observed: Arc::new(AtomicBool::new(true)),
                fail_accounted: Arc::new(AtomicBool::new(true)),
            }
        }

        fn fail_selected_append(&self, events: &[ymp_runtime::SessionEvent]) -> bool {
            events.iter().any(|event| match event {
                ymp_runtime::SessionEvent::InvocationStarted { .. } => {
                    self.fail_started.swap(false, Ordering::SeqCst)
                }
                ymp_runtime::SessionEvent::InvocationObserved { .. } => {
                    self.fail_observed.swap(false, Ordering::SeqCst)
                }
                ymp_runtime::SessionEvent::InvocationAccounted { .. } => {
                    self.fail_accounted.swap(false, Ordering::SeqCst)
                }
                _ => false,
            })
        }
    }

    impl Journal for FlakyExecutionJournal {
        fn read(
            &self,
            session_id: &SessionId,
        ) -> Result<Vec<ymp_runtime::JournalEntry>, JournalError> {
            self.inner.read(session_id)
        }

        fn append(
            &self,
            session_id: &SessionId,
            expected_revision: Revision,
            events: Vec<ymp_runtime::SessionEvent>,
        ) -> Result<Revision, JournalError> {
            if self.fail_selected_append(&events) {
                return Err(JournalError::AdapterFailure {
                    message: "injected post-start append failure".to_owned(),
                });
            }
            self.inner.append(session_id, expected_revision, events)
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
        let workspace = workspace.canonicalize().map_err(|error| {
            CliFailure::operational(format!("cannot resolve test workspace: {error}"))
        })?;
        let access = workspace_access(&workspace, WorkspaceMode::ReadWrite)?;
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
        let interrupted = AtomicBool::new(false);
        let report = run_one(
            &application,
            &scenario,
            session_id,
            build_task("edit the requested file", "test")?,
            assignment_request(access, test_limits())?,
            test_limits(),
            &interrupted,
            |_| {},
        )?;
        Ok((report, application, journal))
    }

    fn command_check(command: &str) -> Check {
        Check::command(
            vec![CriterionId::new(COMPLETION_CRITERION_ID).expect("valid criterion ID")],
            command,
        )
        .expect("valid command check")
    }

    fn execute_and_record_check<J: Journal>(
        journal: &J,
        report: &RunReport,
        workspace: &Path,
        executor: &BuiltinCheckExecutor,
        check: &Check,
    ) -> ymp_runtime::SessionView {
        let before = journal.read(&report.session_id).expect("history reads");
        let evidence = executor
            .execute(check, workspace)
            .expect("check execution is observed");
        assert_eq!(
            journal.read(&report.session_id).expect("history reads"),
            before,
            "the check executor must not mutate the journal"
        );
        AcceptanceAuthority::new(journal)
            .record_evidence(
                &report.session_id,
                report.invocation.clone(),
                evidence,
                report.revision,
            )
            .expect("kernel records evidence")
    }

    fn wait_for_file(path: &Path, description: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(path.exists(), "{description}");
    }

    fn workspace_lock_test_path(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).expect("workspace lock test path is supplied"))
    }

    #[test]
    fn workspace_lock_child() {
        let Some(role) = std::env::var_os(WORKSPACE_LOCK_TEST_ROLE) else {
            return;
        };
        let workspace = workspace_lock_test_path(WORKSPACE_LOCK_TEST_WORKSPACE)
            .canonicalize()
            .expect("workspace resolves");
        let data_dir = workspace_lock_test_path(WORKSPACE_LOCK_TEST_DATA_DIR);
        let journal = SqliteJournal::open(&data_dir).expect("durable journal opens");
        let data_dir = data_dir.canonicalize().expect("data directory resolves");

        match role.to_str().expect("test role is UTF-8") {
            "holder" => {
                let _lock = WorkspaceProcessLock::acquire(&data_dir, &workspace)
                    .expect("first process takes the operating-system lock");
                let (report, _, _) = scripted_run(
                    journal,
                    &workspace,
                    ScriptedOutcome::start_outcome_unknown("provider-start-unknown"),
                    true,
                    "session-uncertain-holder",
                )
                .expect("the uncertain invocation is journaled");
                assert!(matches!(report.outcome, RunOutcome::UncertainAtStart(_)));
                std::fs::write(workspace_lock_test_path(WORKSPACE_LOCK_TEST_HELD), b"held")
                    .expect("held marker is written");
                wait_for_file(
                    &workspace_lock_test_path(WORKSPACE_LOCK_TEST_RELEASE),
                    "the holder receives its release marker",
                );
            }
            "contender" => {
                match WorkspaceProcessLock::acquire(&data_dir, &workspace) {
                    Err(error) => assert!(error.message.contains("workspace is already in use")),
                    Ok(_) => panic!("the concurrent process must not acquire the workspace lock"),
                }
                std::fs::write(
                    workspace_lock_test_path(WORKSPACE_LOCK_TEST_CONFLICT),
                    b"conflict",
                )
                .expect("conflict marker is written");
                wait_for_file(
                    &workspace_lock_test_path(WORKSPACE_LOCK_TEST_RETRY),
                    "the contender receives its retry marker",
                );

                let _lock = WorkspaceProcessLock::acquire(&data_dir, &workspace)
                    .expect("the operating-system lock is released after holder exit");
                let access = workspace_access(&workspace, WorkspaceMode::ReadWrite)
                    .expect("workspace access is valid");
                let provider = ScriptedProvider::new(
                    codex_agent().expect("valid agent"),
                    codex_offering().expect("valid offering"),
                )
                .with_effective_workspace_accesses([access.clone()]);
                let scenario = ExecutionScenario::over(
                    journal.clone(),
                    ScriptedBackend::for_provider(&provider)
                        .with_outcome(ScriptedOutcome::completes(None, ObservedUsage::unknown())),
                    [access.clone()],
                    ScriptedRegistry::new([provider]),
                    DEFAULT_RESERVATION,
                    Arc::new(ManualClock::new()),
                );
                scenario
                    .restore_workspace_holds(
                        journal.session_ids().expect("session catalog reads").iter(),
                    )
                    .expect("durable workspace holds restore");
                let session_id = SessionId::new("session-conflicting-successor")
                    .expect("valid successor session ID");
                scenario
                    .open_session(
                        session_id.clone(),
                        build_task("conflicting work", "durable-hold-successor")
                            .expect("valid task"),
                    )
                    .expect("successor session opens");
                scenario.scan().expect("registry scan succeeds");
                assert!(matches!(
                    scenario
                        .admit(
                            &session_id,
                            assignment_request(access, test_limits()).expect("valid request"),
                            ymp_runtime::Revision::new(1),
                        )
                        .expect_err("the durable uncertain hold refuses a successor"),
                    AdmissionFailure::Denied(AdmissionDenial::WorkspaceNotEnforceable {
                        refusal: ymp_runtime::WorkspaceAccessRefusal::HeldByPredecessor { .. },
                        ..
                    })
                ));
            }
            other => panic!("unknown workspace lock test role {other:?}"),
        }
    }

    #[test]
    fn concurrent_runs_restore_uncertain_workspace_hold_across_processes() {
        let workspace = TestDir::new("process-lock-workspace");
        let data_dir = TestDir::new("process-lock-data");
        let held = data_dir.path().join("held");
        let conflict = data_dir.path().join("conflict");
        let release = data_dir.path().join("release");
        let retry = data_dir.path().join("retry");
        let spawn_child = |role: &str| {
            Command::new(std::env::current_exe().expect("test executable path is available"))
                .args(["--exact", "tests::workspace_lock_child", "--nocapture"])
                .env(WORKSPACE_LOCK_TEST_ROLE, role)
                .env(WORKSPACE_LOCK_TEST_WORKSPACE, workspace.path())
                .env(WORKSPACE_LOCK_TEST_DATA_DIR, data_dir.path())
                .env(WORKSPACE_LOCK_TEST_HELD, &held)
                .env(WORKSPACE_LOCK_TEST_CONFLICT, &conflict)
                .env(WORKSPACE_LOCK_TEST_RELEASE, &release)
                .env(WORKSPACE_LOCK_TEST_RETRY, &retry)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("workspace lock child starts")
        };

        let holder = spawn_child("holder");
        wait_for_file(
            &held,
            "the first child journals uncertainty and holds the lock",
        );
        let contender = spawn_child("contender");
        wait_for_file(
            &conflict,
            "the second child observes the operating-system lock conflict",
        );
        std::fs::write(&release, b"release").expect("holder release is requested");
        let holder = holder
            .wait_with_output()
            .expect("holder child is collected");
        assert!(
            holder.status.success(),
            "holder child failed: {}{}",
            String::from_utf8_lossy(&holder.stdout),
            String::from_utf8_lossy(&holder.stderr)
        );
        std::fs::write(&retry, b"retry").expect("contender retry is requested");
        let contender = contender
            .wait_with_output()
            .expect("contender child is collected");
        assert!(
            contender.status.success(),
            "contender child failed: {}{}",
            String::from_utf8_lossy(&contender.stdout),
            String::from_utf8_lossy(&contender.stderr)
        );
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
    fn run_retries_post_start_writes_and_settlement_without_losing_holds() {
        let workspace = TestDir::new("post-start-retries-workspace");
        let workspace = workspace.path().canonicalize().expect("workspace resolves");
        let access = workspace_access(&workspace, WorkspaceMode::ReadWrite)
            .expect("workspace access is valid");
        let provider = ScriptedProvider::new(
            codex_agent().expect("valid agent"),
            codex_offering().expect("valid offering"),
        )
        .with_effective_workspace_accesses([access.clone()]);
        let backend = ScriptedBackend::for_provider(&provider)
            .with_outcome(ScriptedOutcome::completes(
                None,
                ObservedUsage::unknown().with_turns(2),
            ))
            .with_outcome(ScriptedOutcome::never_reports());
        let journal = FlakyExecutionJournal::new();
        let scenario = ExecutionScenario::over(
            journal.clone(),
            backend,
            [access.clone()],
            ScriptedRegistry::new([provider]),
            DEFAULT_RESERVATION,
            Arc::new(ManualClock::new()),
        );
        let application = Application::new(journal.clone());
        let interrupted = AtomicBool::new(false);
        let mut waits = Vec::new();
        let report = run_one(
            &application,
            &scenario,
            SessionId::new("session-post-start-retries").expect("valid session ID"),
            build_task("finish despite transient writes", "post-start-retries")
                .expect("valid task"),
            assignment_request(access, test_limits()).expect("valid request"),
            test_limits(),
            &interrupted,
            |duration| waits.push(duration),
        )
        .expect("transient execution writes are retried");

        assert_eq!(
            report.outcome,
            RunOutcome::Terminated(Termination::Completed)
        );
        assert_eq!(report.revision, Revision::new(6));
        assert_eq!(waits, vec![EXECUTION_WRITE_RETRY_INTERVAL; 3]);
        assert_eq!(
            scenario.with_backend(ScriptedBackend::pending_outcomes),
            1,
            "an idempotent duplicate start must retain the second outcome"
        );
        assert!(scenario.workspace_hold_of(&report.invocation).is_none());
        assert_eq!(scenario.treasury_held(), ResourceAmount::new(0));
        let invocation = scenario
            .execution_view(&report.session_id)
            .expect("execution history replays")
            .invocation(&report.invocation)
            .expect("invocation is recorded")
            .clone();
        assert_eq!(
            invocation.status(),
            ymp_runtime::InvocationStatus::Terminated
        );
        assert_eq!(invocation.settled_usage(), Some(&report.usage));
        let history = journal.read(&report.session_id).expect("history reads");
        assert_eq!(
            history
                .iter()
                .filter(|entry| matches!(
                    entry.event(),
                    ymp_runtime::SessionEvent::InvocationStarted { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            history
                .iter()
                .filter(|entry| matches!(
                    entry.event(),
                    ymp_runtime::SessionEvent::InvocationAccounted { .. }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn interrupt_without_termination_records_uncertain_at_the_shortened_deadline() {
        let workspace = TestDir::new("interrupt-uncertain-workspace");
        let workspace = workspace.path().canonicalize().expect("workspace resolves");
        let access = workspace_access(&workspace, WorkspaceMode::ReadWrite)
            .expect("workspace access is valid");
        let provider = ScriptedProvider::new(
            codex_agent().expect("valid agent"),
            codex_offering().expect("valid offering"),
        )
        .with_effective_workspace_accesses([access.clone()]);
        let backend = ScriptedBackend::for_provider(&provider)
            .with_outcome(ScriptedOutcome::cancelled_without_confirmation());
        let clock = ManualClock::new();
        let journal = ymp_runtime::MemoryJournal::new();
        let scenario = ExecutionScenario::over(
            journal.clone(),
            backend,
            [access.clone()],
            ScriptedRegistry::new([provider]),
            DEFAULT_RESERVATION,
            Arc::new(clock.clone()),
        );
        let application = Application::new(journal);
        let interrupted = AtomicBool::new(true);
        let limits = test_limits();
        let report = run_one(
            &application,
            &scenario,
            SessionId::new("session-interrupt-uncertain").expect("valid session ID"),
            build_task("wait for cancellation", "interrupt-uncertain").expect("valid task"),
            assignment_request(access, limits).expect("valid request"),
            limits,
            &interrupted,
            |duration| clock.advance(duration),
        )
        .expect("interrupted run reaches a durable result");

        assert_eq!(
            report.outcome,
            RunOutcome::Uncertain(UncertaintyCause::BoundedWaitExpired)
        );
        assert_eq!(scenario.clock_elapsed(), INTERRUPT_WAIT);
        let execution = scenario
            .execution_view(&report.session_id)
            .expect("execution replays");
        let invocation = execution
            .invocation(&report.invocation)
            .expect("invocation is present");
        assert_eq!(
            invocation.status(),
            ymp_runtime::InvocationStatus::Uncertain
        );
        assert!(invocation.cancel_requested());
        assert!(invocation.holds_workspace());
        assert_eq!(scenario.treasury_held(), DEFAULT_RESERVATION);
    }

    #[test]
    fn completed_run_reports_satisfied_command_evidence() {
        let workspace = TestDir::new("satisfied-check-workspace");
        let journal = ymp_runtime::MemoryJournal::new();
        let executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");
        let check = command_check("echo ok");
        let (mut report, application, journal) = scripted_run(
            journal,
            workspace.path(),
            ScriptedOutcome::completes(None, ObservedUsage::unknown()),
            true,
            "session-satisfied-check",
        )
        .expect("scripted run completes");
        let session =
            execute_and_record_check(&journal, &report, workspace.path(), &executor, &check);
        report.revision = session.revision();
        let execution = read_execution(&journal, &report.session_id).expect("execution reads");

        let output = format_run_report(
            &report,
            &application
                .read_session(&report.session_id)
                .expect("session reads"),
            &execution,
            workspace.path(),
            WorkspaceMode::ReadWrite,
            workspace.path(),
            true,
        );

        assert!(output.contains("Criterion requested-work-completed: satisfied with evidence"));
        assert!(output.contains("Check command: \"echo ok\""));
        assert!(output.contains("Exit code: 0"));
        assert!(output.contains("Workspace:"));
        assert!(output.contains("Check time:"));
    }

    #[test]
    fn completed_run_reports_failed_command_evidence() {
        let workspace = TestDir::new("failed-check-workspace");
        let journal = ymp_runtime::MemoryJournal::new();
        let executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");
        let check = command_check("exit 1");
        let (mut report, application, journal) = scripted_run(
            journal,
            workspace.path(),
            ScriptedOutcome::completes(None, ObservedUsage::unknown()),
            true,
            "session-failed-check",
        )
        .expect("scripted run completes");
        let session =
            execute_and_record_check(&journal, &report, workspace.path(), &executor, &check);
        report.revision = session.revision();
        let execution = read_execution(&journal, &report.session_id).expect("execution reads");

        let output = format_run_report(
            &report,
            &application
                .read_session(&report.session_id)
                .expect("session reads"),
            &execution,
            workspace.path(),
            WorkspaceMode::ReadWrite,
            workspace.path(),
            true,
        );

        assert!(output.contains("Criterion requested-work-completed: failed with evidence"));
        assert!(output.contains("Check command: \"exit 1\""));
        assert!(output.contains("Exit code: 1"));
        assert!(output.contains("Check time:"));
    }

    #[test]
    fn report_distinguishes_missing_check_from_failed_invocation() {
        let workspace = TestDir::new("unevaluated-workspace");
        let completed_journal = ymp_runtime::MemoryJournal::new();
        let (completed, completed_application, completed_journal) = scripted_run(
            completed_journal,
            workspace.path(),
            ScriptedOutcome::completes(None, ObservedUsage::unknown()),
            true,
            "session-no-check",
        )
        .expect("scripted run completes");
        let completed_session = completed_application
            .read_session(&completed.session_id)
            .expect("session reads");
        let completed_execution =
            read_execution(&completed_journal, &completed.session_id).expect("execution reads");
        let completed_output = format_run_report(
            &completed,
            &completed_session,
            &completed_execution,
            workspace.path(),
            WorkspaceMode::ReadWrite,
            workspace.path(),
            false,
        );
        assert!(completed_output.contains("not evaluated (no --check was supplied)"));

        let failed_journal = ymp_runtime::MemoryJournal::new();
        let (failed, failed_application, failed_journal) = scripted_run(
            failed_journal,
            workspace.path(),
            ScriptedOutcome::fails("provider-overloaded", ObservedUsage::unknown()),
            true,
            "session-failed-before-check",
        )
        .expect("scripted failure is observed");
        let failed_session = failed_application
            .read_session(&failed.session_id)
            .expect("session reads");
        let failed_execution =
            read_execution(&failed_journal, &failed.session_id).expect("execution reads");
        let failed_output = format_run_report(
            &failed,
            &failed_session,
            &failed_execution,
            workspace.path(),
            WorkspaceMode::ReadWrite,
            workspace.path(),
            true,
        );
        assert!(failed_output.contains(
            "not evaluated (invocation did not complete with outcome completed; observed failed \
             (class provider-overloaded))"
        ));
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
        let executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");
        let check = command_check("echo ok");
        {
            let journal = SqliteJournal::open(data.path()).expect("journal opens");
            let usage = ObservedUsage::unknown()
                .with_turns(3)
                .with_output_chars(99)
                .with_wall_clock(Duration::from_millis(700));
            let (report, _, journal) = scripted_run(
                journal,
                workspace.path(),
                ScriptedOutcome::completes(None, usage),
                true,
                session_id.as_str(),
            )
            .expect("scripted run completes");
            assert_eq!(report.revision, ymp_runtime::Revision::new(6));
            let session =
                execute_and_record_check(&journal, &report, workspace.path(), &executor, &check);
            assert_eq!(session.revision(), ymp_runtime::Revision::new(7));
        }

        let output = show_command(session_id.as_str().to_owned(), data.path().to_path_buf())
            .expect("show reopens the journal");
        assert_eq!(output.exit_code(), 0);
        assert!(output.stdout().contains("Session: session-restart"));
        assert!(output.stdout().contains("Revision: 7"));
        assert!(output.stdout().contains("Invocations: 1"));
        assert!(output.stdout().contains("Status: terminated"));
        assert!(output.stdout().contains("Outcome: completed"));
        assert!(
            output
                .stdout()
                .contains("Usage: turns 3, output characters 99, wall clock 700 ms")
        );
        assert!(output.stdout().contains("Accounted: yes"));
        assert!(
            output
                .stdout()
                .contains("Criterion requested-work-completed: satisfied with evidence")
        );
        assert!(output.stdout().contains("Check command: \"echo ok\""));
        assert!(output.stdout().contains("Exit code: 0"));
        assert!(output.stdout().contains("Workspace:"));
        assert!(output.stdout().contains("Check time:"));
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
        assert!(output.stdout().contains("--check"));
        assert!(output.stdout().contains("/bin/sh"));
        assert!(output.stdout().contains("120-second"));
        assert!(output.stdout().contains("not inferred or"));
        assert!(
            output
                .stdout()
                .contains("SIGINT requests cancellation through")
        );
        assert!(
            output
                .stdout()
                .contains("does not stop an already running check")
        );
        assert!(output.stdout().contains("status 130"));
        assert!(output.stdout().contains("does not establish acceptance"));
    }

    #[test]
    fn run_parser_preserves_one_explicit_check_command() {
        let parsed = parse_command(&[
            OsString::from("run"),
            OsString::from("task"),
            OsString::from("--check"),
            OsString::from("  printf 'ok'  "),
            OsString::from("--data-dir=data"),
        ])
        .expect("run options parse");
        match parsed {
            ParsedCommand::Run {
                goal,
                data_dir,
                check,
            } => {
                assert_eq!(goal, "task");
                assert_eq!(data_dir, PathBuf::from("data"));
                assert_eq!(check.as_deref(), Some("  printf 'ok'  "));
            }
            _ => panic!("expected a run command"),
        }

        let duplicate = command(&[
            OsString::from("run"),
            OsString::from("task"),
            OsString::from("--check=true"),
            OsString::from("--check"),
            OsString::from("true"),
        ]);
        assert_eq!(duplicate.exit_code(), 2);
        assert!(
            duplicate
                .stderr()
                .contains("--check may be specified only once")
        );
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
