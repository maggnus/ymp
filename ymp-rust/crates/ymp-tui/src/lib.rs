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
use ymp_domain::{Budget, Command, RunState, RunStatus};
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
    Run,
}

struct UiModel {
    view: View,
    data_root: PathBuf,
    probes: Vec<ProbeReport>,
    selected_runtime: usize,
    contracts: Vec<ManagedContract>,
    selected_contract: usize,
    active_contract: Option<ManagedContract>,
    notice: String,
}

impl UiModel {
    fn new(data_root: &Path, contracts: Vec<ManagedContract>) -> Self {
        Self {
            view: View::Runtimes,
            data_root: data_root.to_path_buf(),
            probes: probe_runtimes(),
            selected_runtime: 0,
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
        let state = app
            .lock()
            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?
            .state()
            .clone();
        terminal
            .terminal
            .draw(|frame| render(frame, &state, &model))?;

        if event::poll(Duration::from_millis(150))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') => {
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
                KeyCode::Char('1') => model.view = View::Runtimes,
                KeyCode::Char('3') => model.view = View::Run,
                KeyCode::Tab => {
                    model.view = match model.view {
                        View::Runtimes => View::Run,
                        View::Run => View::Runtimes,
                    };
                }
                KeyCode::Char('c') if model.view == View::Runtimes => {
                    model.probes = probe_runtimes();
                    model.selected_runtime = model
                        .selected_runtime
                        .min(model.probes.len().saturating_sub(1));
                    model.notice =
                        "Local executable probes completed; no model was invoked.".to_owned();
                }
                KeyCode::Up | KeyCode::Char('k') if model.view == View::Runtimes => {
                    model.select_previous_runtime();
                }
                KeyCode::Down | KeyCode::Char('j') if model.view == View::Runtimes => {
                    model.select_next_runtime();
                }
                KeyCode::Char('[') if model.view == View::Runtimes => {
                    model.select_previous_contract();
                }
                KeyCode::Char(']') if model.view == View::Runtimes => {
                    model.select_next_contract();
                }
                KeyCode::Enter | KeyCode::Char('r') if model.view == View::Runtimes => {
                    model.notice =
                        start_selected_runtime(Arc::clone(&app), &mut model, &mut managed_run);
                    model.view = View::Run;
                }
                KeyCode::Char('a') => {
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
                KeyCode::Char('s') => {
                    model.notice = match app.lock() {
                        Ok(mut app) => submit_fixture_candidate(&mut app, &model.data_root),
                        Err(_) => "Application lock was poisoned.".to_owned(),
                    };
                    model.view = View::Run;
                }
                KeyCode::Char('v') => {
                    model.notice = match app.lock() {
                        Ok(mut app) => verify_selected_candidate(&mut app, &model),
                        Err(_) => "Application lock was poisoned.".to_owned(),
                    };
                    model.view = View::Run;
                }
                KeyCode::Char('e') => {
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
                KeyCode::Char('x') if state.status == RunStatus::Running => {
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
                _ => {}
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

fn render(frame: &mut ratatui::Frame<'_>, state: &RunState, model: &UiModel) {
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
        View::Run => render_run(frame, areas[1], state),
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
            format!("{} · {}", state.run_id, status_text(state.status)),
        ),
    ]);
    frame.render_widget(summary, columns[0]);

    let shortcuts = Paragraph::new(vec![
        shortcut_line("<1>", "runtimes", "<3>", "run", "<Tab>", "switch"),
        shortcut_line("<↑↓>", "runtime", "<[ ]>", "contract", "<Enter>", "use"),
        shortcut_line("<c>", "probe", "<a>", "fake", "<s>", "submit"),
        shortcut_line("<v>", "verify", "<e>", "export", "<x>", "cancel"),
        shortcut_line("<q>", "quit", "", "", "", ""),
    ]);
    frame.render_widget(shortcuts, columns[1]);
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
    let rows = model.probes.iter().enumerate().map(|(index, probe)| {
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
                " runtimes(all)[{}] · contracts[{}] ",
                model.probes.len(),
                model.contracts.len()
            ))
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    )
    .column_spacing(1);
    frame.render_widget(table, area);
}

fn render_run(frame: &mut ratatui::Frame<'_>, area: Rect, state: &RunState) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(5)])
        .split(area);
    let overview = Paragraph::new(vec![
        key_value("Status", status_text(state.status)),
        key_value("Events", state.last_sequence.to_string()),
        key_value(
            "Budget",
            format!(
                "attempts={} · verification={}",
                state.budget.attempts_remaining, state.budget.verification_queries_remaining
            ),
        ),
        key_value(
            "Candidate",
            state.candidate_digest.as_deref().unwrap_or("—"),
        ),
        key_value("Authority", authority_text(state)),
    ])
    .block(
        Block::default()
            .title(format!(" run({}) ", state.run_id))
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(overview, sections[0]);

    let attempt_rows: Vec<Row<'_>> = if state.active_attempts.is_empty() {
        vec![Row::new(["—", "no active authority", "—"])]
    } else {
        state
            .active_attempts
            .iter()
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
            .title(format!(" attempts[{}] ", state.active_attempts.len()))
            .title_style(Style::default().fg(AMBER))
            .borders(Borders::ALL),
    );
    frame.render_widget(attempts, sections[1]);
}

fn render_footer(frame: &mut ratatui::Frame<'_>, area: Rect, model: &UiModel) {
    let view = match model.view {
        View::Runtimes => "runtimes",
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
        Line::styled(model.notice.as_str(), Style::default().fg(MUTED)),
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
        Readiness::Unauthenticated => "not authenticated",
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

fn authority_text(state: &RunState) -> &'static str {
    if state.status.is_terminal() {
        "cleared"
    } else if state.active_attempts.is_empty() {
        "none"
    } else {
        "active"
    }
}

#[cfg(test)]
mod tests {
    use super::{UiModel, View, render, verify_selected_candidate};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;
    use ymp_application::Application;
    use ymp_domain::{Budget, Command, RunState, RunStatus};
    use ymp_runtime_api::{ProbeReport, Readiness, RuntimeKind};
    use ymp_runtime_supervisor::{ManagedContract, ManagedVerifier};

    fn state(status: RunStatus) -> RunState {
        RunState {
            run_id: "run-test".to_owned(),
            status,
            budget: Budget::new(3, 2),
            active_attempts: if status == RunStatus::Running {
                vec!["attempt-1".to_owned()]
            } else {
                Vec::new()
            },
            candidate_digest: Some("b".repeat(64)),
            last_sequence: 7,
            last_event_digest: "a".repeat(64),
        }
    }

    fn model(view: View, notice: &str) -> UiModel {
        UiModel {
            view,
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
                    readiness: Readiness::Unavailable,
                    detail: "authentication unavailable".to_owned(),
                },
            ],
            selected_runtime: 0,
            contracts: Vec::new(),
            selected_contract: 0,
            active_contract: None,
            notice: notice.to_owned(),
        }
    }

    fn screen(width: u16, height: u16, state: &RunState, model: &UiModel) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, state, model))
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

    #[test]
    fn runtime_view_covers_mixed_readiness_at_standard_sizes() {
        for (width, height) in [(80, 24), (120, 40)] {
            let output = screen(
                width,
                height,
                &state(RunStatus::Running),
                &model(View::Runtimes, "fixture notice"),
            );
            assert!(output.contains("runtimes(all)[3]"));
            assert!(output.contains("fake"));
            assert!(output.contains("codex"));
            assert!(output.contains("ready"));
            assert!(output.contains("not installed"));
            assert!(output.contains("fixture notice"));
        }
    }

    #[test]
    fn run_view_covers_running_and_every_terminal_outcome_without_color() {
        for (width, height) in [(80, 24), (120, 40)] {
            for status in [
                RunStatus::Running,
                RunStatus::Accepted,
                RunStatus::Exhausted,
                RunStatus::Abstained,
                RunStatus::Cancelled,
                RunStatus::InfrastructureError,
            ] {
                let output = screen(
                    width,
                    height,
                    &state(status),
                    &model(View::Run, "Candidate verification failed."),
                );
                assert!(output.contains(super::status_text(status)));
                assert!(output.contains("run(run-test)"));
                assert!(output.contains("Candidate verification failed."));
                if status.is_terminal() {
                    assert!(output.contains("cleared"));
                } else {
                    assert!(output.contains("attempt-1"));
                }
            }
        }
    }

    #[test]
    fn undersized_terminal_has_stable_safe_exit() {
        let output = screen(
            60,
            20,
            &state(RunStatus::Running),
            &model(View::Run, "fixture notice"),
        );
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
