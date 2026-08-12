#![forbid(unsafe_code)]

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;
use ymp_application::Application;
use ymp_domain::{Budget, Command, EventEnvelope, EventKind, RunState, RunStatus};
use ymp_runtime_api::{ProbeReport, Readiness, RuntimeDriver, RuntimeKind};
use ymp_runtime_claude::ClaudeRuntime;
use ymp_runtime_codex::CodexRuntime;
use ymp_runtime_fake::{FakeRuntime, ScriptStep};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, ManagedRunHandle,
    start_managed_candidate,
};
use ymp_verifier::ExactDigestVerifier;

const AMBER: Color = Color::Indexed(179);
const MUTED: Color = Color::Indexed(245);

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> anyhow::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum View {
    Runtimes,
    Candidates,
    Run,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewStateKind {
    Empty,
    Ready,
    Degraded,
    Terminal,
}

impl ViewStateKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Empty => "[ ] empty",
            Self::Ready => "[*] ready",
            Self::Degraded => "[~] degraded",
            Self::Terminal => "[!] terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenFocus {
    Body,
    Actions,
    Filter,
    Detail,
    Help,
}

impl ScreenFocus {
    const fn label(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Actions => "actions",
            Self::Filter => "filter",
            Self::Detail => "detail",
            Self::Help => "help",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenKeyAction {
    None,
    ProbeRuntime,
    UseRuntime,
    StartAttempt,
    SubmitCandidate,
    VerifyCandidate,
    ExportEvidence,
    CancelRun,
    Quit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CandidateRow {
    id: String,
    verification: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RunProjection {
    view_state: ViewStateKind,
    reason: String,
}

impl RunProjection {
    fn from_state(state: &RunState, last_event: Option<&EventEnvelope>) -> Self {
        let view_state = if state.status.is_terminal() {
            ViewStateKind::Terminal
        } else if matches!(
            last_event.map(|event| &event.event),
            Some(EventKind::VerificationRecorded {
                accepted: false,
                ..
            })
        ) {
            ViewStateKind::Degraded
        } else if state.active_attempts.is_empty() && state.candidate_digest.is_none() {
            ViewStateKind::Empty
        } else {
            ViewStateKind::Ready
        };

        let reason = match last_event.map(|event| &event.event) {
            Some(EventKind::RunStarted { .. }) | None => {
                "first launch · no attempt started".to_owned()
            }
            Some(EventKind::AttemptStarted { .. }) => "run in progress".to_owned(),
            Some(EventKind::CandidateSubmitted { .. }) => {
                "candidate captured · verification pending".to_owned()
            }
            Some(EventKind::VerificationRecorded { accepted: true, .. }) => {
                "exact candidate accepted by verifier evidence".to_owned()
            }
            Some(EventKind::VerificationRecorded {
                accepted: false, ..
            }) => "candidate failure · run remains active".to_owned(),
            Some(EventKind::RunExhausted { reason })
            | Some(EventKind::RunAbstained { reason })
            | Some(EventKind::RunCancelled { reason }) => reason.clone(),
            Some(EventKind::RunFailed { reason }) => {
                format!("{reason} · no candidate judged")
            }
        };

        Self { view_state, reason }
    }
}

struct UiModel {
    view: View,
    focus: ScreenFocus,
    data_root: PathBuf,
    probes: Vec<ProbeReport>,
    selected_runtime: usize,
    candidates: Vec<CandidateRow>,
    selected_candidate: usize,
    candidate_scroll: usize,
    contracts: Vec<ManagedContract>,
    selected_contract: usize,
    active_contract: Option<ManagedContract>,
    notice: String,
}

impl UiModel {
    fn new(data_root: &Path, contracts: Vec<ManagedContract>) -> Self {
        Self {
            view: View::Runtimes,
            focus: ScreenFocus::Body,
            data_root: data_root.to_path_buf(),
            probes: probe_runtimes(),
            selected_runtime: 0,
            candidates: Vec::new(),
            selected_candidate: 0,
            candidate_scroll: 0,
            contracts,
            selected_contract: 0,
            active_contract: None,
            notice: "Ready. No model request has been made.".to_owned(),
        }
    }

    fn select_previous_runtime(&mut self) {
        if self.probes.is_empty() {
            return;
        }
        self.selected_runtime = self
            .selected_runtime
            .checked_sub(1)
            .unwrap_or(self.probes.len() - 1);
    }

    fn select_next_runtime(&mut self) {
        if !self.probes.is_empty() {
            self.selected_runtime = (self.selected_runtime + 1) % self.probes.len();
        }
    }

    fn select_previous_runtime_page(&mut self) {
        self.selected_runtime = self.selected_runtime.saturating_sub(10);
    }

    fn select_next_runtime_page(&mut self) {
        if !self.probes.is_empty() {
            self.selected_runtime = (self.selected_runtime + 10).min(self.probes.len() - 1);
        }
    }

    fn select_previous_contract(&mut self) {
        if self.contracts.is_empty() {
            return;
        }
        self.selected_contract = self
            .selected_contract
            .checked_sub(1)
            .unwrap_or(self.contracts.len() - 1);
    }

    fn select_next_contract(&mut self) {
        if !self.contracts.is_empty() {
            self.selected_contract = (self.selected_contract + 1) % self.contracts.len();
        }
    }

    fn selected_contract(&self) -> Option<&ManagedContract> {
        self.contracts.get(self.selected_contract)
    }
}

fn handle_screen_key(model: &mut UiModel, state: &RunState, key: KeyCode) -> ScreenKeyAction {
    match key {
        KeyCode::Char('q') => ScreenKeyAction::Quit,
        KeyCode::Esc if model.focus != ScreenFocus::Body => {
            model.focus = ScreenFocus::Body;
            ScreenKeyAction::None
        }
        KeyCode::Char('?') => {
            model.focus = ScreenFocus::Help;
            ScreenKeyAction::None
        }
        KeyCode::Char('/') => {
            model.focus = ScreenFocus::Filter;
            ScreenKeyAction::None
        }
        KeyCode::Char('d') => {
            model.focus = ScreenFocus::Detail;
            ScreenKeyAction::None
        }
        KeyCode::Tab => {
            model.focus = match model.focus {
                ScreenFocus::Actions => ScreenFocus::Body,
                _ => ScreenFocus::Actions,
            };
            ScreenKeyAction::None
        }
        KeyCode::Char('1') => {
            model.view = View::Runtimes;
            model.focus = ScreenFocus::Body;
            ScreenKeyAction::None
        }
        KeyCode::Char('3') => {
            model.view = View::Run;
            model.focus = ScreenFocus::Body;
            ScreenKeyAction::None
        }
        KeyCode::Char('5') => {
            model.view = View::Candidates;
            model.focus = ScreenFocus::Body;
            ScreenKeyAction::None
        }
        KeyCode::Up | KeyCode::Char('k') if model.view == View::Runtimes => {
            model.select_previous_runtime();
            ScreenKeyAction::None
        }
        KeyCode::Down | KeyCode::Char('j') if model.view == View::Runtimes => {
            model.select_next_runtime();
            ScreenKeyAction::None
        }
        KeyCode::PageUp if model.view == View::Runtimes => {
            model.select_previous_runtime_page();
            ScreenKeyAction::None
        }
        KeyCode::PageDown if model.view == View::Runtimes => {
            model.select_next_runtime_page();
            ScreenKeyAction::None
        }
        KeyCode::Up | KeyCode::Char('k') if model.view == View::Candidates => {
            if !model.candidates.is_empty() {
                model.selected_candidate = model.selected_candidate.saturating_sub(1);
            }
            ScreenKeyAction::None
        }
        KeyCode::Down | KeyCode::Char('j') if model.view == View::Candidates => {
            if !model.candidates.is_empty() {
                model.selected_candidate =
                    (model.selected_candidate + 1).min(model.candidates.len() - 1);
            }
            ScreenKeyAction::None
        }
        KeyCode::PageUp if model.view == View::Candidates => {
            model.candidate_scroll = model.candidate_scroll.saturating_sub(10);
            ScreenKeyAction::None
        }
        KeyCode::PageDown if model.view == View::Candidates => {
            model.candidate_scroll = model
                .candidate_scroll
                .saturating_add(10)
                .min(model.candidates.len().saturating_sub(1));
            ScreenKeyAction::None
        }
        KeyCode::Enter if model.view == View::Candidates => {
            model.focus = ScreenFocus::Detail;
            ScreenKeyAction::None
        }
        KeyCode::Char('c') if model.view == View::Runtimes => ScreenKeyAction::ProbeRuntime,
        KeyCode::Enter | KeyCode::Char('r') if model.view == View::Runtimes => {
            ScreenKeyAction::UseRuntime
        }
        KeyCode::Char('a') if !state.status.is_terminal() => ScreenKeyAction::StartAttempt,
        KeyCode::Char('s') if !state.status.is_terminal() && !state.active_attempts.is_empty() => {
            ScreenKeyAction::SubmitCandidate
        }
        KeyCode::Char('v') if !state.status.is_terminal() && state.candidate_digest.is_some() => {
            ScreenKeyAction::VerifyCandidate
        }
        KeyCode::Char('e') => ScreenKeyAction::ExportEvidence,
        KeyCode::Char('x') if !state.status.is_terminal() => ScreenKeyAction::CancelRun,
        _ => ScreenKeyAction::None,
    }
}

pub fn run(data_root: impl AsRef<Path>) -> anyhow::Result<()> {
    run_with_contracts(data_root, Vec::new())
}

pub fn run_with_contracts(
    data_root: impl AsRef<Path>,
    contracts: Vec<ManagedContract>,
) -> anyhow::Result<()> {
    let data_root = data_root.as_ref();
    let event_log = data_root.join("events.jsonl");
    let app = if event_log.is_file() && event_log.metadata()?.len() > 0 {
        Application::open(data_root).context("open existing ymp run")?
    } else {
        Application::create(
            data_root,
            format!("run-{}", Uuid::new_v4()),
            Budget::new(8, 4),
        )
        .context("create ymp run")?
    };
    let app = Arc::new(Mutex::new(app));
    let mut model = UiModel::new(data_root, contracts);
    let mut managed_run: Option<ManagedRunHandle> = None;
    let mut terminal = TerminalGuard::enter()?;

    loop {
        poll_managed_run(&mut managed_run, &mut model)?;
        let (state, last_event) = {
            let app = app
                .lock()
                .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
            let state = app.state().clone();
            let last_event = app
                .events_after(state.last_sequence.saturating_sub(1))?
                .pop();
            (state, last_event)
        };
        let projection = RunProjection::from_state(&state, last_event.as_ref());
        terminal
            .terminal
            .draw(|frame| render(frame, &state, &projection, &model))?;

        if event::poll(Duration::from_millis(150))?
            && let Event::Key(key) = event::read()?
        {
            if model.view == View::Runtimes {
                match key.code {
                    KeyCode::Char('[') => model.select_previous_contract(),
                    KeyCode::Char(']') => model.select_next_contract(),
                    _ => {}
                }
            }
            match handle_screen_key(&mut model, &state, key.code) {
                ScreenKeyAction::Quit => {
                    if let Some(handle) = &managed_run
                        && !handle.is_finished()
                    {
                        handle.cancel("operator quit")?;
                    } else if state.status == RunStatus::Running
                        && !state.active_attempts.is_empty()
                    {
                        app.lock()
                            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?
                            .execute(
                                format!("tui.quit.{}", Uuid::new_v4()),
                                Command::Cancel {
                                    reason: "operator quit before terminal outcome".to_owned(),
                                },
                            )?;
                    }
                    break;
                }
                ScreenKeyAction::ProbeRuntime => {
                    model.probes = probe_runtimes();
                    model.selected_runtime = model
                        .selected_runtime
                        .min(model.probes.len().saturating_sub(1));
                    model.notice =
                        "Local executable probes completed; no model was invoked.".to_owned();
                }
                ScreenKeyAction::UseRuntime => {
                    model.notice =
                        start_selected_runtime(Arc::clone(&app), &mut model, &mut managed_run);
                    model.view = View::Run;
                }
                ScreenKeyAction::StartAttempt => {
                    let attempt_id = format!("attempt-{}", Uuid::new_v4());
                    model.notice = match app
                        .lock()
                        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?
                        .execute(
                            format!("tui.start.{attempt_id}"),
                            Command::StartAttempt { attempt_id },
                        ) {
                        Ok(_) => "Deterministic fake attempt started and committed.".to_owned(),
                        Err(error) => error.to_string(),
                    };
                    model.view = View::Run;
                }
                ScreenKeyAction::SubmitCandidate => {
                    model.notice = match app.lock() {
                        Ok(mut app) => submit_fixture_candidate(&mut app, &model.data_root),
                        Err(_) => "Application lock was poisoned.".to_owned(),
                    };
                    model.view = View::Run;
                }
                ScreenKeyAction::VerifyCandidate => {
                    model.notice = match app.lock() {
                        Ok(mut app) => verify_selected_candidate(&mut app, &model),
                        Err(_) => "Application lock was poisoned.".to_owned(),
                    };
                    model.view = View::Run;
                }
                ScreenKeyAction::ExportEvidence => {
                    let destination = model.data_root.join("exports").join(state.run_id.as_str());
                    model.notice = match app
                        .lock()
                        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?
                        .export_evidence(&destination)
                    {
                        Ok(_) => format!("Evidence exported to {}.", destination.display()),
                        Err(error) => error.to_string(),
                    };
                    model.view = View::Run;
                }
                ScreenKeyAction::CancelRun => {
                    let result: anyhow::Result<String> = match &managed_run {
                        Some(handle) if !handle.is_finished() => {
                            handle.cancel("operator cancellation").map(|_| {
                                "Run cancelled; managed runtime termination requested.".to_owned()
                            })
                        }
                        _ => app
                            .lock()
                            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?
                            .execute(
                                format!("tui.cancel.{}", Uuid::new_v4()),
                                Command::Cancel {
                                    reason: "operator cancellation".to_owned(),
                                },
                            )
                            .map(|_| {
                                "Run cancelled; active attempt authority was cleared.".to_owned()
                            })
                            .map_err(anyhow::Error::from),
                    };
                    model.notice = match result {
                        Ok(notice) => notice,
                        Err(error) => error.to_string(),
                    };
                    model.view = View::Run;
                }
                ScreenKeyAction::None => {}
            }
        }
    }
    Ok(())
}

fn start_selected_runtime(
    application: Arc<Mutex<Application>>,
    model: &mut UiModel,
    managed_run: &mut Option<ManagedRunHandle>,
) -> String {
    if managed_run.is_some() {
        return "A managed runtime is already active.".to_owned();
    }
    let Some(probe) = model.probes.get(model.selected_runtime) else {
        return "No runtime profile is selected.".to_owned();
    };
    if probe.readiness != Readiness::Ready {
        return format!(
            "Selected {} profile is not ready: {}",
            runtime_name(probe.kind),
            probe.detail
        );
    }
    let runtime_kind = probe.kind;
    let Some(contract) = model.selected_contract().cloned() else {
        return "No managed contract is configured; pass --contract <file>.".to_owned();
    };
    let driver: Box<dyn RuntimeDriver> = match runtime_kind {
        RuntimeKind::Fake => Box::new(FakeRuntime::with_script(vec![
            ScriptStep::Output("managed fake runtime completed".to_owned()),
            ScriptStep::Complete(Default::default()),
        ])),
        RuntimeKind::Codex => Box::new(CodexRuntime::default()),
        RuntimeKind::ClaudeCode => Box::new(ClaudeRuntime::default()),
    };
    let bridge_executable = match std::env::current_exe()
        .context("resolve current ymp executable")
        .and_then(|path| {
            path.canonicalize()
                .context("canonicalize current ymp executable")
        }) {
        Ok(path) => path,
        Err(error) => return error.to_string(),
    };
    match start_managed_candidate(
        application,
        driver,
        ManagedCandidateRequest {
            contract: contract.clone(),
            bridge_executable,
        },
    ) {
        Ok(handle) => {
            let attempt_id = handle.attempt_id().to_owned();
            model.active_contract = Some(contract.clone());
            *managed_run = Some(handle);
            format!(
                "Started {} for contract {} as {}.",
                runtime_name(runtime_kind),
                contract.contract_id,
                attempt_id
            )
        }
        Err(error) => error.to_string(),
    }
}

fn poll_managed_run(
    managed_run: &mut Option<ManagedRunHandle>,
    model: &mut UiModel,
) -> anyhow::Result<()> {
    let Some(handle) = managed_run.as_ref() else {
        return Ok(());
    };
    while let Some(event) = handle.try_next() {
        match event {
            ManagedRunEvent::Runtime(event) => {
                model.notice = runtime_event_notice(event.event);
            }
            ManagedRunEvent::CandidateAvailable {
                candidate_digest,
                change_count,
            } => {
                model.notice = match change_count {
                    Some(count) => format!(
                        "Managed candidate {} captured with {} changes.",
                        &candidate_digest[..12],
                        count
                    ),
                    None => format!(
                        "Agent-submitted candidate {} is available.",
                        &candidate_digest[..12]
                    ),
                };
            }
            ManagedRunEvent::Failed { detail } => {
                model.notice = format!("Managed runtime failed: {detail}");
            }
            ManagedRunEvent::Finished => {}
        }
    }
    if handle.is_finished() {
        let handle = managed_run.take().expect("managed handle is present");
        handle.join()?;
    }
    Ok(())
}

fn runtime_event_notice(event: ymp_runtime_api::RuntimeEventKind) -> String {
    match event {
        ymp_runtime_api::RuntimeEventKind::Started { .. } => "Runtime session started.".to_owned(),
        ymp_runtime_api::RuntimeEventKind::Output { text } => {
            let first_line = text.lines().next().unwrap_or_default();
            format!("Runtime output: {}", bounded_notice(first_line, 240))
        }
        ymp_runtime_api::RuntimeEventKind::McpToolCall {
            tool,
            status,
            error,
            ..
        } => {
            if let Some(error) = error {
                format!(
                    "MCP tool {tool} {status}: {}",
                    bounded_notice(&error.to_string(), 180)
                )
            } else {
                format!("MCP tool {tool} completed with status {status}.")
            }
        }
        ymp_runtime_api::RuntimeEventKind::Yielded { cursor } => {
            format!("Runtime yielded at cursor {cursor}.")
        }
        ymp_runtime_api::RuntimeEventKind::Completed { usage } => format!(
            "Runtime completed: input={} cached={} output={} reasoning={}.",
            usage.input_tokens,
            usage.cached_input_tokens,
            usage.output_tokens,
            usage.reasoning_output_tokens
        ),
        ymp_runtime_api::RuntimeEventKind::Interrupted => {
            "Managed runtime process tree was interrupted.".to_owned()
        }
    }
}

fn bounded_notice(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_owned();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

fn submit_fixture_candidate(app: &mut Application, data_root: &Path) -> String {
    let Some(attempt_id) = app.state().active_attempts.last().cloned() else {
        return "Start an attempt before submitting a candidate.".to_owned();
    };
    let result = (|| -> anyhow::Result<()> {
        let fixture = data_root.join("tui-fixtures").join(&attempt_id);
        let source = fixture.join("source");
        let workspace = fixture.join("workspace");
        std::fs::create_dir_all(&source)?;
        std::fs::write(source.join("result.txt"), b"before\n")?;
        let artifacts = app.artifact_store();
        let base = artifacts.capture_source(&source)?;
        artifacts.materialize(&base.manifest_digest, &workspace)?;
        std::fs::write(workspace.join("result.txt"), b"accepted fixture\n")?;
        app.submit_workspace_candidate(
            format!("tui.submit.{}", Uuid::new_v4()),
            attempt_id,
            base.manifest_digest,
            workspace,
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => "Immutable fixture candidate committed.".to_owned(),
        Err(error) => error.to_string(),
    }
}

fn verify_fixture_candidate(app: &mut Application) -> String {
    let Some(candidate_digest) = app.state().candidate_digest.clone() else {
        return "Submit a candidate before verification.".to_owned();
    };
    let evidence = app
        .object_store()
        .path_for(&candidate_digest)
        .map_err(anyhow::Error::from)
        .and_then(|candidate_path| {
            ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &candidate_digest)
                .map_err(anyhow::Error::from)?
                .verify_candidate(candidate_path, &candidate_digest)
                .map_err(anyhow::Error::from)
        });
    match evidence.and_then(|evidence| {
        app.record_verification(format!("tui.verify.{}", Uuid::new_v4()), &evidence)
            .map_err(anyhow::Error::from)
    }) {
        Ok(_) => "Candidate accepted by the configured exact-digest fixture contract.".to_owned(),
        Err(error) => error.to_string(),
    }
}

fn verify_selected_candidate(app: &mut Application, model: &UiModel) -> String {
    let Some(contract) = &model.active_contract else {
        return verify_fixture_candidate(app);
    };
    let Some(verifier_config) = &contract.verifier else {
        return "The active managed contract has no verifier configuration.".to_owned();
    };
    let Some(candidate_digest) = app.state().candidate_digest.clone() else {
        return "Submit a candidate before verification.".to_owned();
    };
    let result = (|| -> anyhow::Result<RunStatus> {
        let candidate_directory = model.data_root.join("verification-inputs").join(format!(
            "{}-{}",
            candidate_digest,
            Uuid::new_v4()
        ));
        app.artifact_store()
            .materialize(&candidate_digest, &candidate_directory)?;
        let verifier = ymp_verifier::CommandVerifier::new(
            &contract.contract_digest,
            &verifier_config.oracle_digest,
            &verifier_config.program,
            verifier_config.arguments.clone(),
            Duration::from_millis(verifier_config.wall_time_ms),
            verifier_config.output_limit_bytes,
        )?;
        let evidence = verifier.verify_candidate(
            &candidate_directory,
            &verifier_config.negative_control,
            &candidate_digest,
        )?;
        let outcome =
            app.record_verification(format!("tui.verify.{}", Uuid::new_v4()), &evidence)?;
        Ok(outcome.status)
    })();
    match result {
        Ok(status) => format!(
            "Candidate verification completed and evidence was committed: {}.",
            status_text(status)
        ),
        Err(error) => error.to_string(),
    }
}

fn probe_runtimes() -> Vec<ProbeReport> {
    let drivers: [&dyn RuntimeDriver; 3] = [
        &FakeRuntime::default(),
        &CodexRuntime::default(),
        &ClaudeRuntime::default(),
    ];
    drivers
        .into_iter()
        .map(|driver| {
            driver.probe().unwrap_or_else(|error| ProbeReport {
                kind: driver.kind(),
                executable: driver.executable().display().to_string(),
                version: None,
                readiness: Readiness::Unavailable,
                detail: format!("probe failed: {error}"),
            })
        })
        .collect()
}

fn render(
    frame: &mut ratatui::Frame<'_>,
    state: &RunState,
    projection: &RunProjection,
    model: &UiModel,
) {
    let area = frame.area();
    if area.width < 80 || area.height < 24 {
        render_too_small(frame, area);
        return;
    }

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(area);
    render_header(frame, areas[0], state, model);
    match model.view {
        View::Runtimes => render_runtimes(frame, areas[1], model),
        View::Candidates => render_candidates(frame, areas[1], state, projection, model),
        View::Run => render_run(frame, areas[1], state, projection),
    }
    render_footer(frame, areas[2], model);
}

fn render_header(frame: &mut ratatui::Frame<'_>, area: Rect, state: &RunState, model: &UiModel) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(area);
    let ready = model
        .probes
        .iter()
        .filter(|probe| probe.readiness == Readiness::Ready)
        .count();
    let summary = Paragraph::new(vec![
        key_value("Store", model.data_root.display().to_string()),
        key_value("Version", env!("CARGO_PKG_VERSION")),
        key_value(
            "Assurance",
            "poc_process_isolation — no hostile-code containment",
        ),
        key_value(
            "Runtimes",
            format!("{ready} ready · {} total", model.probes.len()),
        ),
        key_value(
            "Contract",
            model
                .selected_contract()
                .map(|contract| contract.contract_id.as_str())
                .unwrap_or("none"),
        ),
        key_value(
            "Run",
            format!("{} · {}", state.run_id, status_label(state.status)),
        ),
        key_value("Focus", format!("{} · keyboard", model.focus.label())),
    ]);
    frame.render_widget(summary, columns[0]);

    let shortcut_lines = match (model.view, area.width < 100, state.status.is_terminal()) {
        (View::Runtimes, true, _) => vec![
            Line::from("<1> runtimes · <3> run · <5> candidates"),
            Line::from("<↑↓/jk> move · <PgUp/Dn> page"),
            Line::from("<Enter> use · <Tab> focus · </> filter"),
            Line::from("<c> probe · <d> describe · <?> help"),
            Line::from("<Esc> back · <q> quit"),
        ],
        (View::Runtimes, false, _) => vec![
            shortcut_line("<1>", "runtimes", "<3>", "run", "<5>", "candidates"),
            shortcut_line("<↑↓/jk>", "runtime", "<[ ]>", "contract", "<Enter>", "use"),
            shortcut_line("<PgUp/Dn>", "page", "<c>", "probe", "</>", "filter"),
            shortcut_line("<d>", "describe", "<?>", "help", "<Tab>", "focus"),
            shortcut_line("<Esc>", "back", "<q>", "quit", "", ""),
        ],
        (View::Candidates, true, _) => vec![
            Line::from("<1> runtimes · <3> run · <5> candidates"),
            Line::from("<↑↓/jk> move · <PgUp/Dn> page"),
            Line::from("<Enter/d> detail · </> filter"),
            Line::from("<Tab> focus · <?> help"),
            Line::from("<Esc> back · <q> quit"),
        ],
        (View::Candidates, false, _) => vec![
            shortcut_line("<1>", "runtimes", "<3>", "run", "<5>", "candidates"),
            shortcut_line("<↑↓/jk>", "candidate", "<PgUp/Dn>", "page", "</>", "filter"),
            shortcut_line("<Enter/d>", "detail", "<?>", "help", "<Tab>", "focus"),
            shortcut_line("<Esc>", "back", "<q>", "quit", "", ""),
        ],
        (View::Run, true, true) => vec![
            Line::from("<1> runtimes · <3> run · <5> candidates"),
            Line::from("<e> export · <d> describe"),
            Line::from("</> filter · <?> help · <Tab> focus"),
            Line::from("<Esc> back · <q> quit"),
        ],
        (View::Run, false, true) => vec![
            shortcut_line("<1>", "runtimes", "<3>", "run", "<5>", "candidates"),
            shortcut_line("<e>", "export", "<d>", "describe", "</>", "filter"),
            shortcut_line("<?>", "help", "<Tab>", "focus", "<Esc>", "back"),
            shortcut_line("<q>", "quit", "", "", "", ""),
        ],
        (View::Run, true, false) => run_action_lines(state, true),
        (View::Run, false, false) => run_action_lines(state, false),
    };
    let shortcuts = Paragraph::new(shortcut_lines);
    frame.render_widget(shortcuts, columns[1]);
}

fn run_action_lines(state: &RunState, compact: bool) -> Vec<Line<'static>> {
    let mut actions = vec!["<a> start"];
    if !state.active_attempts.is_empty() {
        actions.push("<s> submit");
    }
    if state.candidate_digest.is_some() {
        actions.push("<v> verify");
    }
    let action_line = actions.join(" · ");
    if compact {
        vec![
            Line::from("<1> runtimes · <3> run · <5> candidates"),
            Line::from(action_line),
            Line::from("<e> export · <x> cancel · <d> describe"),
            Line::from("</> filter · <?> help · <Tab> focus"),
            Line::from("<Esc> back · <q> quit"),
        ]
    } else {
        vec![
            shortcut_line("<1>", "runtimes", "<3>", "run", "<5>", "candidates"),
            Line::from(action_line),
            shortcut_line("<e>", "export", "<x>", "cancel", "<d>", "describe"),
            shortcut_line("</>", "filter", "<?>", "help", "<Tab>", "focus"),
            shortcut_line("<Esc>", "back", "<q>", "quit", "", ""),
        ]
    }
}

fn key_value(key: &str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<11}"), Style::default().fg(AMBER)),
        Span::raw(value.into()),
    ])
}

fn shortcut_line(
    first_key: &str,
    first_value: &str,
    second_key: &str,
    second_value: &str,
    third_key: &str,
    third_value: &str,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{first_key:<7}"), Style::default().fg(AMBER)),
        Span::raw(format!("{first_value:<13}")),
        Span::styled(format!("{second_key:<7}"), Style::default().fg(AMBER)),
        Span::raw(format!("{second_value:<13}")),
        Span::styled(format!("{third_key:<7}"), Style::default().fg(AMBER)),
        Span::raw(third_value.to_owned()),
    ])
}

fn render_runtimes(frame: &mut ratatui::Frame<'_>, area: Rect, model: &UiModel) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);
    let capacity = usize::from(sections[0].height.saturating_sub(3)).max(1);
    let (start, end) = visible_range(model.probes.len(), model.selected_runtime, capacity);
    let rows = model
        .probes
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(index, probe)| {
            let marker = if index == model.selected_runtime {
                ">"
            } else {
                " "
            };
            let row = Row::new(vec![
                Cell::from(format!("{marker} {}", runtime_name(probe.kind))),
                Cell::from(probe.executable.clone()),
                Cell::from(probe.version.as_deref().unwrap_or("—").to_owned()),
                Cell::from(readiness_text(probe.readiness)),
                Cell::from(probe.detail.clone()),
            ]);
            if index == model.selected_runtime {
                row.style(Style::default().fg(AMBER))
            } else {
                row
            }
        });
    let shown = if start == end {
        "rows 0/0".to_owned()
    } else {
        format!("rows {}–{}/{}", start + 1, end, model.probes.len())
    };
    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(16),
            Constraint::Length(22),
            Constraint::Length(15),
            Constraint::Min(12),
        ],
    )
    .header(
        Row::new(["RUNTIME", "EXECUTABLE", "VERSION", "STATE", "DETAIL"]).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!(
                " runtimes(all)[{}] · {shown} · contracts[{}] · PgUp/PgDn ",
                model.probes.len(),
                model.contracts.len()
            ))
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    )
    .column_spacing(1);
    frame.render_widget(table, sections[0]);

    let selection = model.probes.get(model.selected_runtime).map_or_else(
        || "selection: none".to_owned(),
        |probe| {
            format!(
                "selection: {} · {} · stable executable={} · <Enter> use",
                runtime_name(probe.kind),
                readiness_text(probe.readiness),
                probe.executable
            )
        },
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(selection),
            Line::from("focus=body · ↑↓/jk/PgUp/PgDn"),
        ]),
        sections[1],
    );
}

fn render_candidates(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &RunState,
    projection: &RunProjection,
    model: &UiModel,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(4)])
        .split(area);
    let capacity = usize::from(sections[0].height.saturating_sub(3)).max(1);
    let total = model.candidates.len();
    let selected = model.selected_candidate.min(total.saturating_sub(1));
    let max_start = total.saturating_sub(capacity);
    let mut start = model.candidate_scroll.min(max_start);
    if total > 0 && selected < start {
        start = selected;
    } else if total > 0 && selected >= start.saturating_add(capacity) {
        start = selected.saturating_add(1).saturating_sub(capacity);
    }
    let end = start.saturating_add(capacity).min(total);
    let rows = model
        .candidates
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(index, candidate)| {
            let marker = if index == selected { ">" } else { " " };
            let row = Row::new([
                format!("{marker} {}", candidate.id),
                candidate.verification.clone(),
                "preserved".to_owned(),
            ]);
            if index == selected {
                row.style(Style::default().fg(AMBER))
            } else {
                row
            }
        });
    let shown = if start == end {
        "rows 0/0".to_owned()
    } else {
        format!("rows {}–{}/{total}", start + 1, end)
    };
    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Min(28),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(["CANDIDATE", "VERIFICATION", "ARTIFACT"]).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!(
                " candidates({})[{total}] · {shown} · PgUp/PgDn ",
                state.run_id
            ))
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    );
    frame.render_widget(table, sections[0]);

    let selection = model.candidates.get(selected).map_or_else(
        || "selection: none".to_owned(),
        |candidate| format!("selection: {} · stable_id={}", candidate.id, candidate.id),
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(selection),
            Line::from(format!(
                "state={} · status={}",
                projection.view_state.label(),
                status_label(state.status)
            )),
            Line::from(format!("reason: {}", projection.reason)),
            Line::from("primary=<Enter/d> detail · selection and reason remain visible"),
        ]),
        sections[1],
    );
}

fn visible_range(total: usize, selected: usize, capacity: usize) -> (usize, usize) {
    if total == 0 {
        return (0, 0);
    }
    let capacity = capacity.min(total);
    let selected = selected.min(total - 1);
    let start = selected.saturating_sub(capacity / 2).min(total - capacity);
    (start, start + capacity)
}

fn render_run(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &RunState,
    projection: &RunProjection,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(5)])
        .split(area);
    let overview = Paragraph::new(vec![
        key_value("State", projection.view_state.label()),
        key_value("Status", status_label(state.status)),
        key_value(
            "Progress",
            format!(
                "event #{} · attempts {} · verification {}",
                state.last_sequence,
                state.budget.attempts_remaining,
                state.budget.verification_queries_remaining
            ),
        ),
        key_value(
            "Candidate",
            state.candidate_digest.as_deref().unwrap_or("—"),
        ),
        key_value("Authority", authority_text(state)),
        key_value("Reason", projection.reason.clone()),
    ])
    .block(
        Block::default()
            .title(format!(" run({}) ", state.run_id))
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(overview, sections[0]);

    let capacity = usize::from(sections[1].height.saturating_sub(3)).max(1);
    let shown_attempts = state.active_attempts.len().min(capacity);
    let attempt_rows: Vec<Row<'_>> = if state.active_attempts.is_empty() {
        vec![Row::new(["—", "no active authority", "—"])]
    } else {
        state
            .active_attempts
            .iter()
            .take(shown_attempts)
            .map(|attempt| {
                Row::new([
                    attempt.clone(),
                    "active".to_owned(),
                    state.candidate_digest.as_deref().unwrap_or("—").to_owned(),
                ])
            })
            .collect()
    };
    let attempts = Table::new(
        attempt_rows,
        [
            Constraint::Length(28),
            Constraint::Length(22),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(["ATTEMPT", "AUTHORITY", "CANDIDATE"]).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!(
                " attempts[{}] · rows {}/{} ",
                state.active_attempts.len(),
                shown_attempts,
                state.active_attempts.len()
            ))
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    );
    frame.render_widget(attempts, sections[1]);
}

fn render_footer(frame: &mut ratatui::Frame<'_>, area: Rect, model: &UiModel) {
    let view = match model.view {
        View::Runtimes => "runtimes",
        View::Candidates => "candidates",
        View::Run => "run",
    };
    let footer = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" ymp ", Style::default().bg(MUTED).fg(Color::Black)),
            Span::styled(
                format!(" {view} "),
                Style::default().bg(AMBER).fg(Color::Black),
            ),
        ]),
        Line::styled(
            format!("{} · focus={}", model.notice, model.focus.label()),
            Style::default().fg(MUTED),
        ),
    ]);
    frame.render_widget(footer, area);
}

fn render_too_small(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let message = Paragraph::new(vec![
        Line::from("terminal too small: need ≥80×24"),
        Line::from("q quit safely"),
    ])
    .block(
        Block::default()
            .title(" ymp ")
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(message, area);
}

const fn runtime_name(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Fake => "fake",
        RuntimeKind::Codex => "codex",
        RuntimeKind::ClaudeCode => "claude-code",
    }
}

const fn readiness_text(readiness: Readiness) -> &'static str {
    match readiness {
        Readiness::Ready => "ready",
        Readiness::NotInstalled => "not installed",
        Readiness::Unauthenticated => "unauthenticated",
        Readiness::Incompatible => "incompatible",
        Readiness::Unavailable => "unavailable",
    }
}

const fn status_text(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "running",
        RunStatus::Accepted => "accepted",
        RunStatus::Exhausted => "exhausted",
        RunStatus::Abstained => "abstained",
        RunStatus::Cancelled => "cancelled",
        RunStatus::InfrastructureError => "infrastructure_error",
    }
}

const fn status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "[*] running",
        RunStatus::Accepted => "[+] accepted",
        RunStatus::Exhausted => "[-] exhausted",
        RunStatus::Abstained => "[?] abstained",
        RunStatus::Cancelled => "[x] cancelled",
        RunStatus::InfrastructureError => "[!] infrastructure_error",
    }
}

fn authority_text(state: &RunState) -> &'static str {
    if state.status.is_terminal() {
        "cleared"
    } else if state.active_attempts.is_empty() {
        "none"
    } else {
        "active"
    }
}

/// Minimal public surface used by a package outside `ymp-tui` to exercise the
/// accepted screen contract without owning application or domain transitions.
pub mod screen_contract {
    use super::{
        CandidateRow, RunProjection, ScreenFocus, ScreenKeyAction, UiModel, View, ViewStateKind,
        handle_screen_key, render,
    };
    use crossterm::event::KeyCode;
    use ratatui::Frame;
    use std::path::PathBuf;
    use ymp_domain::{Budget, EventEnvelope, EventKind, RunState, RunStatus};
    use ymp_runtime_api::{ProbeReport, Readiness, RuntimeKind};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Screen {
        Runtimes,
        Candidates,
        Run,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RuntimeProfileKind {
        Fake,
        Codex,
        ClaudeCode,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RuntimeReadiness {
        Ready,
        NotInstalled,
        Unauthenticated,
        Incompatible,
        Unavailable,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct RuntimeProjection {
        id: String,
        kind: RuntimeProfileKind,
        version: Option<String>,
        readiness: RuntimeReadiness,
        detail: String,
    }

    impl RuntimeProjection {
        pub fn new(
            id: impl Into<String>,
            kind: RuntimeProfileKind,
            version: Option<String>,
            readiness: RuntimeReadiness,
            detail: impl Into<String>,
        ) -> Self {
            Self {
                id: id.into(),
                kind,
                version,
                readiness,
                detail: detail.into(),
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct CandidateProjection {
        id: String,
        verification: String,
    }

    impl CandidateProjection {
        pub fn new(id: impl Into<String>, verification: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                verification: verification.into(),
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RunScenario {
        FirstLaunch,
        Running,
        Cancelled { reason: String },
        Accepted { candidate_id: String },
        CandidateFailure { candidate_id: String },
        BudgetExhausted { reason: String },
        InfrastructureFailure { reason: String },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RootStatus {
        Running,
        Accepted,
        Exhausted,
        Cancelled,
        InfrastructureError,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ScreenProjection {
        screen: Screen,
        scenario: RunScenario,
        runtimes: Vec<RuntimeProjection>,
        candidates: Vec<CandidateProjection>,
        selected_runtime: Option<String>,
        selected_candidate: Option<String>,
        candidate_scroll: usize,
        notice: String,
    }

    impl ScreenProjection {
        pub fn new(screen: Screen, scenario: RunScenario) -> Self {
            Self {
                screen,
                scenario,
                runtimes: Vec::new(),
                candidates: Vec::new(),
                selected_runtime: None,
                selected_candidate: None,
                candidate_scroll: 0,
                notice: "Deterministic external contract fixture.".to_owned(),
            }
        }

        pub fn with_runtimes(
            mut self,
            runtimes: Vec<RuntimeProjection>,
            selected_id: impl Into<String>,
        ) -> Self {
            self.runtimes = runtimes;
            self.selected_runtime = Some(selected_id.into());
            self
        }

        pub fn with_candidates(
            mut self,
            candidates: Vec<CandidateProjection>,
            selected_id: impl Into<String>,
            scroll: usize,
        ) -> Self {
            self.candidates = candidates;
            self.selected_candidate = Some(selected_id.into());
            self.candidate_scroll = scroll;
            self
        }
    }

    pub struct ScreenHarness {
        state: RunState,
        run_projection: RunProjection,
        model: UiModel,
    }

    impl ScreenHarness {
        pub fn new(projection: ScreenProjection) -> Self {
            let (state, event) = state_and_event(&projection.scenario);
            let run_projection = RunProjection::from_state(&state, Some(&event));
            let probes = projection
                .runtimes
                .into_iter()
                .map(|runtime| ProbeReport {
                    kind: match runtime.kind {
                        RuntimeProfileKind::Fake => RuntimeKind::Fake,
                        RuntimeProfileKind::Codex => RuntimeKind::Codex,
                        RuntimeProfileKind::ClaudeCode => RuntimeKind::ClaudeCode,
                    },
                    executable: runtime.id,
                    version: runtime.version,
                    readiness: match runtime.readiness {
                        RuntimeReadiness::Ready => Readiness::Ready,
                        RuntimeReadiness::NotInstalled => Readiness::NotInstalled,
                        RuntimeReadiness::Unauthenticated => Readiness::Unauthenticated,
                        RuntimeReadiness::Incompatible => Readiness::Incompatible,
                        RuntimeReadiness::Unavailable => Readiness::Unavailable,
                    },
                    detail: runtime.detail,
                })
                .collect::<Vec<_>>();
            let candidates = projection
                .candidates
                .into_iter()
                .map(|candidate| CandidateRow {
                    id: candidate.id,
                    verification: candidate.verification,
                })
                .collect::<Vec<_>>();
            let selected_runtime = projection
                .selected_runtime
                .as_deref()
                .map(|id| {
                    probes
                        .iter()
                        .position(|runtime| runtime.executable == id)
                        .unwrap_or_else(|| panic!("selected runtime {id:?} is absent"))
                })
                .unwrap_or(0);
            let selected_candidate = projection
                .selected_candidate
                .as_deref()
                .map(|id| {
                    candidates
                        .iter()
                        .position(|candidate| candidate.id == id)
                        .unwrap_or_else(|| panic!("selected candidate {id:?} is absent"))
                })
                .unwrap_or(0);
            let model = UiModel {
                view: match projection.screen {
                    Screen::Runtimes => View::Runtimes,
                    Screen::Candidates => View::Candidates,
                    Screen::Run => View::Run,
                },
                focus: ScreenFocus::Body,
                data_root: PathBuf::from(".ymp-contract"),
                probes,
                selected_runtime,
                candidates,
                selected_candidate,
                candidate_scroll: projection.candidate_scroll,
                contracts: Vec::new(),
                selected_contract: 0,
                active_contract: None,
                notice: projection.notice,
            };
            Self {
                state,
                run_projection,
                model,
            }
        }

        pub fn render_frame(&self, frame: &mut Frame<'_>) {
            render(frame, &self.state, &self.run_projection, &self.model);
        }

        pub fn handle_key(&mut self, key: KeyCode) -> ScreenKeyAction {
            handle_screen_key(&mut self.model, &self.state, key)
        }

        pub fn focus(&self) -> ScreenFocus {
            self.model.focus
        }

        pub fn screen(&self) -> Screen {
            match self.model.view {
                View::Runtimes => Screen::Runtimes,
                View::Candidates => Screen::Candidates,
                View::Run => Screen::Run,
            }
        }

        pub fn view_state(&self) -> ViewStateKind {
            self.run_projection.view_state
        }

        pub fn root_status(&self) -> RootStatus {
            match self.state.status {
                RunStatus::Running => RootStatus::Running,
                RunStatus::Accepted => RootStatus::Accepted,
                RunStatus::Exhausted => RootStatus::Exhausted,
                RunStatus::Cancelled => RootStatus::Cancelled,
                RunStatus::InfrastructureError => RootStatus::InfrastructureError,
                RunStatus::Abstained => unreachable!("contract fixtures do not use abstention"),
            }
        }

        pub fn selected_candidate_id(&self) -> Option<&str> {
            self.model
                .candidates
                .get(self.model.selected_candidate)
                .map(|candidate| candidate.id.as_str())
        }

        pub fn selected_runtime_id(&self) -> Option<&str> {
            self.model
                .probes
                .get(self.model.selected_runtime)
                .map(|runtime| runtime.executable.as_str())
        }

        pub fn available_actions(&self) -> Vec<ScreenKeyAction> {
            match (self.model.view, self.state.status.is_terminal()) {
                (View::Runtimes, _) => vec![
                    ScreenKeyAction::ProbeRuntime,
                    ScreenKeyAction::UseRuntime,
                    ScreenKeyAction::Quit,
                ],
                (View::Candidates, _) => vec![ScreenKeyAction::Quit],
                (View::Run, true) => vec![ScreenKeyAction::ExportEvidence, ScreenKeyAction::Quit],
                (View::Run, false) => vec![
                    ScreenKeyAction::StartAttempt,
                    ScreenKeyAction::ExportEvidence,
                    ScreenKeyAction::CancelRun,
                    ScreenKeyAction::Quit,
                ]
                .into_iter()
                .chain(
                    (!self.state.active_attempts.is_empty())
                        .then_some(ScreenKeyAction::SubmitCandidate),
                )
                .chain(
                    self.state
                        .candidate_digest
                        .is_some()
                        .then_some(ScreenKeyAction::VerifyCandidate),
                )
                .collect(),
            }
        }
    }

    fn state_and_event(scenario: &RunScenario) -> (RunState, EventEnvelope) {
        let candidate = match scenario {
            RunScenario::Accepted { candidate_id }
            | RunScenario::CandidateFailure { candidate_id } => Some(candidate_id.clone()),
            _ => None,
        };
        let (status, attempts, budget, event) = match scenario {
            RunScenario::FirstLaunch => (
                RunStatus::Running,
                Vec::new(),
                Budget::new(3, 2),
                EventKind::RunStarted {
                    budget: Budget::new(3, 2),
                },
            ),
            RunScenario::Running => (
                RunStatus::Running,
                vec!["attempt-1".to_owned()],
                Budget::new(2, 2),
                EventKind::AttemptStarted {
                    attempt_id: "attempt-1".to_owned(),
                },
            ),
            RunScenario::Cancelled { reason } => (
                RunStatus::Cancelled,
                Vec::new(),
                Budget::new(2, 2),
                EventKind::RunCancelled {
                    reason: reason.clone(),
                },
            ),
            RunScenario::Accepted { candidate_id } => (
                RunStatus::Accepted,
                Vec::new(),
                Budget::new(2, 1),
                verification_event(candidate_id, true),
            ),
            RunScenario::CandidateFailure { candidate_id } => (
                RunStatus::Running,
                vec!["attempt-1".to_owned()],
                Budget::new(2, 1),
                verification_event(candidate_id, false),
            ),
            RunScenario::BudgetExhausted { reason } => (
                RunStatus::Exhausted,
                Vec::new(),
                Budget::new(0, 2),
                EventKind::RunExhausted {
                    reason: reason.clone(),
                },
            ),
            RunScenario::InfrastructureFailure { reason } => (
                RunStatus::InfrastructureError,
                Vec::new(),
                Budget::new(2, 2),
                EventKind::RunFailed {
                    reason: reason.clone(),
                },
            ),
        };
        let state = RunState {
            run_id: "run-contract".to_owned(),
            status,
            budget,
            active_attempts: attempts,
            candidate_digest: candidate,
            last_sequence: 8842,
            last_event_digest: "a".repeat(64),
        };
        let event = EventEnvelope::new(
            "run-contract",
            8842,
            "command-contract",
            "c".repeat(64),
            Some("a".repeat(64)),
            event,
        )
        .expect("valid deterministic contract event");
        (state, event)
    }

    fn verification_event(candidate_id: &str, accepted: bool) -> EventKind {
        EventKind::VerificationRecorded {
            candidate_digest: candidate_id.to_owned(),
            contract_digest: "c".repeat(64),
            oracle_digest: "d".repeat(64),
            evidence_digest: "e".repeat(64),
            accepted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RunProjection, UiModel, View, ViewStateKind, render, verify_selected_candidate};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;
    use ymp_application::Application;
    use ymp_domain::{Budget, Command, EventEnvelope, EventKind, RunState, RunStatus};
    use ymp_runtime_api::{ProbeReport, Readiness, RuntimeKind};
    use ymp_runtime_supervisor::{ManagedContract, ManagedVerifier};

    fn state(
        status: RunStatus,
        active_attempts: Vec<String>,
        candidate_digest: Option<String>,
        budget: Budget,
    ) -> RunState {
        RunState {
            run_id: "run-test".to_owned(),
            status,
            budget,
            active_attempts,
            candidate_digest,
            last_sequence: 7,
            last_event_digest: "a".repeat(64),
        }
    }

    fn event(kind: EventKind) -> EventEnvelope {
        EventEnvelope::new(
            "run-test",
            7,
            "command-7",
            "c".repeat(64),
            Some("a".repeat(64)),
            kind,
        )
        .expect("fixture event")
    }

    fn model(view: View, notice: &str) -> UiModel {
        UiModel {
            view,
            focus: super::ScreenFocus::Body,
            data_root: PathBuf::from(".ymp-data"),
            probes: vec![
                ProbeReport {
                    kind: RuntimeKind::Fake,
                    executable: "ymp-internal-fake".to_owned(),
                    version: Some("0.1.0".to_owned()),
                    readiness: Readiness::Ready,
                    detail: "deterministic in-process runtime".to_owned(),
                },
                ProbeReport {
                    kind: RuntimeKind::Codex,
                    executable: "codex".to_owned(),
                    version: None,
                    readiness: Readiness::NotInstalled,
                    detail: "executable not found".to_owned(),
                },
                ProbeReport {
                    kind: RuntimeKind::ClaudeCode,
                    executable: "claude".to_owned(),
                    version: None,
                    readiness: Readiness::Unauthenticated,
                    detail: "authentication unavailable".to_owned(),
                },
            ],
            selected_runtime: 0,
            candidates: Vec::new(),
            selected_candidate: 0,
            candidate_scroll: 0,
            contracts: Vec::new(),
            selected_contract: 0,
            active_contract: None,
            notice: notice.to_owned(),
        }
    }

    fn screen(
        width: u16,
        height: u16,
        state: &RunState,
        last_event: Option<&EventEnvelope>,
        model: &UiModel,
    ) -> String {
        let projection = RunProjection::from_state(state, last_event);
        screen_with_projection(width, height, state, &projection, model)
    }

    fn screen_with_projection(
        width: u16,
        height: u16,
        state: &RunState,
        projection: &RunProjection,
        model: &UiModel,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, state, projection, model))
            .expect("render screen");
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in 0..height {
            for x in 0..width {
                output.push_str(buffer[(x, y)].symbol());
            }
            output.push('\n');
        }
        output
    }

    fn require(output: &str, needle: &str, region: &str) -> Result<(), String> {
        if output.contains(needle) {
            Ok(())
        } else {
            Err(format!("missing {region}: {needle:?}"))
        }
    }

    fn validate_common_regions(output: &str) -> Result<(), String> {
        for (needle, region) in [
            ("Store", "REG-CONTEXT"),
            ("Assurance", "REG-CONTEXT"),
            ("Focus", "REG-CONTEXT"),
            ("<1>", "REG-NAV"),
            ("<3>", "REG-NAV"),
            ("<q>", "REG-ACTIONS"),
            (" ymp ", "REG-BREADCRUMB"),
        ] {
            require(output, needle, region)?;
        }
        Ok(())
    }

    fn validate_run_contract(
        output: &str,
        view_state: ViewStateKind,
        status: RunStatus,
        reason: &str,
    ) -> Result<(), String> {
        validate_common_regions(output)?;
        let actions: &[&str] = if status.is_terminal() {
            &["<e>", "<q>"]
        } else {
            &["<a>", "<e>", "<x>", "<q>"]
        };
        for action in actions {
            require(output, action, "REG-ACTIONS")?;
        }
        if status.is_terminal() {
            for unavailable in ["<a>", "<s>", "<v>", "<x>"] {
                if output.contains(unavailable) {
                    return Err(format!("unavailable terminal action: {unavailable}"));
                }
            }
        }
        require(output, "run(run-test)", "REG-BODY")?;
        require(output, "attempts[", "REG-BODY")?;
        require(output, "State", "REG-STATE")?;
        require(output, view_state.label(), "REG-STATE")?;
        require(output, super::status_label(status), "REG-ACCESS")?;
        require(output, "Reason", "REG-TERMINAL-REASON")?;
        require(output, reason, "REG-TERMINAL-REASON")?;
        Ok(())
    }

    #[test]
    fn first_launch_and_mixed_runtime_readiness_keep_regions_focus_and_actions() {
        let initial = state(RunStatus::Running, Vec::new(), None, Budget::new(3, 2));
        let started = event(EventKind::RunStarted {
            budget: Budget::new(3, 2),
        });
        for (width, height) in [(80, 24), (120, 40)] {
            let output = screen(
                width,
                height,
                &initial,
                Some(&started),
                &model(View::Runtimes, "Ready. No model request has been made."),
            );
            validate_common_regions(&output)
                .unwrap_or_else(|error| panic!("{width}x{height}: {error}"));
            for action in ["<Enter>", "<↑↓/jk>", "<PgUp/Dn>", "<c>", "<q>"] {
                require(&output, action, "REG-ACTIONS")
                    .unwrap_or_else(|error| panic!("{width}x{height}: {error}"));
            }
            assert!(output.contains("runtimes(all)[3]"));
            assert!(output.contains("fake"));
            assert!(output.contains("codex"));
            assert!(output.contains("ready"));
            assert!(output.contains("not installed"));
            assert!(output.contains("unauthenticated"));
            assert!(output.contains("selection: fake"));
            assert!(output.contains("focus=body"));
            assert!(output.contains("Contract   none"));
            assert!(output.contains("No model request has been made."));
        }
    }

    #[test]
    fn run_projection_matrix_covers_required_states_at_standard_sizes() {
        let candidate = Some("b".repeat(64));
        let scenarios = [
            (
                "first launch",
                state(RunStatus::Running, Vec::new(), None, Budget::new(3, 2)),
                event(EventKind::RunStarted {
                    budget: Budget::new(3, 2),
                }),
                ViewStateKind::Empty,
                "first launch · no attempt started",
            ),
            (
                "running",
                state(
                    RunStatus::Running,
                    vec!["attempt-1".to_owned()],
                    None,
                    Budget::new(2, 2),
                ),
                event(EventKind::AttemptStarted {
                    attempt_id: "attempt-1".to_owned(),
                }),
                ViewStateKind::Ready,
                "run in progress",
            ),
            (
                "cancellation",
                state(
                    RunStatus::Cancelled,
                    Vec::new(),
                    candidate.clone(),
                    Budget::new(2, 2),
                ),
                event(EventKind::RunCancelled {
                    reason: "operator cancellation".to_owned(),
                }),
                ViewStateKind::Terminal,
                "operator cancellation",
            ),
            (
                "acceptance",
                state(
                    RunStatus::Accepted,
                    Vec::new(),
                    candidate.clone(),
                    Budget::new(2, 1),
                ),
                event(EventKind::VerificationRecorded {
                    candidate_digest: "b".repeat(64),
                    contract_digest: "c".repeat(64),
                    oracle_digest: "d".repeat(64),
                    evidence_digest: "e".repeat(64),
                    accepted: true,
                }),
                ViewStateKind::Terminal,
                "exact candidate accepted by verifier evidence",
            ),
            (
                "candidate failure",
                state(
                    RunStatus::Running,
                    vec!["attempt-1".to_owned()],
                    candidate.clone(),
                    Budget::new(2, 1),
                ),
                event(EventKind::VerificationRecorded {
                    candidate_digest: "b".repeat(64),
                    contract_digest: "c".repeat(64),
                    oracle_digest: "d".repeat(64),
                    evidence_digest: "e".repeat(64),
                    accepted: false,
                }),
                ViewStateKind::Degraded,
                "candidate failure · run remains active",
            ),
            (
                "budget exhaustion",
                state(
                    RunStatus::Exhausted,
                    Vec::new(),
                    candidate.clone(),
                    Budget::new(0, 2),
                ),
                event(EventKind::RunExhausted {
                    reason: "attempt budget exhausted".to_owned(),
                }),
                ViewStateKind::Terminal,
                "attempt budget exhausted",
            ),
            (
                "infrastructure failure",
                state(
                    RunStatus::InfrastructureError,
                    Vec::new(),
                    candidate,
                    Budget::new(2, 2),
                ),
                event(EventKind::RunFailed {
                    reason: "journal gap after #8840".to_owned(),
                }),
                ViewStateKind::Terminal,
                "journal gap after #8840 · no candidate judged",
            ),
        ];

        for (width, height) in [(80, 24), (120, 40)] {
            for (name, state, event, view_state, reason) in &scenarios {
                let output = screen(
                    width,
                    height,
                    state,
                    Some(event),
                    &model(View::Run, "Projection fixture."),
                );
                validate_run_contract(&output, *view_state, state.status, reason)
                    .unwrap_or_else(|error| panic!("{name} at {width}x{height}: {error}"));
                if state.status.is_terminal() {
                    assert!(output.contains("cleared"));
                } else if *view_state == ViewStateKind::Empty {
                    assert!(output.contains("no active authority"));
                } else {
                    assert!(output.contains("attempt-1"));
                }
            }
        }
    }

    #[test]
    fn high_volume_runtime_catalog_clips_around_selection_without_hiding_actions() {
        let run = state(RunStatus::Running, Vec::new(), None, Budget::new(3, 2));
        let mut high_volume = model(View::Runtimes, "High-volume fixture.");
        high_volume.probes = (0..257)
            .map(|index| ProbeReport {
                kind: RuntimeKind::Fake,
                executable: format!("runtime-{index:04}"),
                version: Some("0.1.0".to_owned()),
                readiness: Readiness::Ready,
                detail: "deterministic fixture".to_owned(),
            })
            .collect();
        high_volume.selected_runtime = 173;

        for (width, height) in [(80, 24), (120, 40)] {
            let output = screen(width, height, &run, None, &high_volume);
            validate_common_regions(&output)
                .unwrap_or_else(|error| panic!("{width}x{height}: {error}"));
            assert!(output.contains("runtimes(all)[257]"));
            assert!(output.contains("runtime-0173"));
            assert!(output.contains("selection: fake · ready"));
            assert!(output.contains("<Enter> use"));
            assert!(output.contains("PgUp/PgDn"));
            assert!(!output.contains("runtime-0000"));
        }
    }

    #[test]
    fn monochrome_markers_distinguish_all_root_states() {
        let cases = [
            (RunStatus::Running, "[*] running"),
            (RunStatus::Accepted, "[+] accepted"),
            (RunStatus::Exhausted, "[-] exhausted"),
            (RunStatus::Abstained, "[?] abstained"),
            (RunStatus::Cancelled, "[x] cancelled"),
            (RunStatus::InfrastructureError, "[!] infrastructure_error"),
        ];

        for (status, marker) in cases {
            let run = state(status, Vec::new(), None, Budget::new(0, 0));
            let output = screen(80, 24, &run, None, &model(View::Run, "Monochrome."));
            assert!(output.contains(marker), "missing {marker}");
        }
    }

    #[test]
    fn negative_control_rejects_wrong_mapping_and_missing_required_region() {
        let exhausted = state(RunStatus::Exhausted, Vec::new(), None, Budget::new(0, 2));
        let exhausted_event = event(EventKind::RunExhausted {
            reason: "attempt budget exhausted".to_owned(),
        });
        let correct = RunProjection::from_state(&exhausted, Some(&exhausted_event));
        let mut wrong = correct.clone();
        wrong.view_state = ViewStateKind::Ready;
        let wrong_output = screen_with_projection(
            80,
            24,
            &exhausted,
            &wrong,
            &model(View::Run, "Unrelated local notice."),
        );
        assert!(
            validate_run_contract(
                &wrong_output,
                ViewStateKind::Terminal,
                RunStatus::Exhausted,
                "attempt budget exhausted",
            )
            .is_err()
        );

        let correct_output = screen_with_projection(
            80,
            24,
            &exhausted,
            &correct,
            &model(View::Run, "Unrelated local notice."),
        );
        let missing_assurance = correct_output.replacen("Assurance", "", 1);
        assert!(
            validate_run_contract(
                &missing_assurance,
                ViewStateKind::Terminal,
                RunStatus::Exhausted,
                "attempt budget exhausted",
            )
            .is_err()
        );
    }

    #[test]
    fn undersized_terminal_has_stable_safe_exit() {
        let run = state(
            RunStatus::Running,
            vec!["attempt-1".to_owned()],
            None,
            Budget::new(3, 2),
        );
        let output = screen(60, 20, &run, None, &model(View::Run, "fixture notice"));
        assert!(output.contains("terminal too small: need ≥80×24"));
        assert!(output.contains("q quit safely"));
        assert!(!output.contains("attempt-1"));
    }

    #[cfg(unix)]
    #[test]
    fn configured_tui_verifier_commits_bounded_evidence() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let data_root = temporary.path().join("data");
        let source = temporary.path().join("source");
        let workspace = temporary.path().join("workspace");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("result.txt"), b"before\n").expect("source file");
        let mut application = Application::create(&data_root, "run-verifier", Budget::new(1, 1))
            .expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base snapshot");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        std::fs::write(workspace.join("result.txt"), b"accepted\n").expect("candidate file");
        application
            .execute(
                "start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("start attempt");
        application
            .submit_workspace_candidate("submit", "attempt-1", &base.manifest_digest, &workspace)
            .expect("submit candidate");

        let program = temporary.path().join("verify.sh");
        std::fs::write(
            &program,
            b"#!/bin/sh\ntest \"$(cat \"$1/result.txt\")\" = accepted\n",
        )
        .expect("verifier program");
        let mut permissions = std::fs::metadata(&program)
            .expect("verifier metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&program, permissions).expect("verifier permissions");
        let mut model = model(View::Run, "fixture");
        model.data_root = data_root;
        model.active_contract = Some(ManagedContract {
            contract_id: "contract-1".to_owned(),
            contract_digest: "c".repeat(64),
            source: source.clone(),
            prompt: "fixture".to_owned(),
            capture_exclusions: Vec::new(),
            verifier: Some(ManagedVerifier {
                program,
                arguments: Vec::new(),
                negative_control: source,
                oracle_digest: "d".repeat(64),
                wall_time_ms: 5_000,
                output_limit_bytes: 4096,
            }),
        });

        let notice = verify_selected_candidate(&mut application, &model);
        assert!(notice.contains("accepted"), "{notice}");
        assert_eq!(application.state().status, RunStatus::Accepted);
    }
}
