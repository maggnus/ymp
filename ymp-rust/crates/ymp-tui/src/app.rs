//! The session, the event loop and key handling.
//!
//! [`Session`] owns the application handle and the read model; the event loop owns the terminal
//! and the view state. Key handling is a pure function over the view state that returns the
//! [`Action`] the loop must execute, so every keyboard transition can be driven in a test
//! without a terminal or a store.
//!
//! Input is read by a dedicated thread; journal notifications and the runtime probe arrive on
//! the same channel. The main thread blocks until something happens and never polls.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ymp_application::{Application, PreparedContract, RunRequest, prepare_contract};
use ymp_domain::Command as DomainCommand;
use ymp_runtime_supervisor::ManagedContract;

use crate::decisions;
use crate::draft::{self, Draft, Step};
use crate::journal::Model;
use crate::pages::Page;
use crate::projection::{ContractFacts, Environment, Projection};
use crate::runtimes::Report;
use crate::state::{App, Command, ConfirmAction, Modal, PageKind, Surface};
use crate::terminal::TerminalGuard;
use crate::theme::Markers;
use crate::ui;

/// How long the input thread waits before checking whether it should stop.
const INPUT_TICK: Duration = Duration::from_millis(50);
/// Upper bound on how long the main thread sleeps with nothing to do.
const IDLE: Duration = Duration::from_millis(500);
/// Journal notifications buffered before the reader is considered lagging; it then recovers
/// from its event cursor rather than assuming every notification arrived (INV-6).
const NOTIFICATION_CAPACITY: usize = 64;

/// Everything durable the interface reads and the commands it can commit.
pub struct Session {
    application: Option<Application>,
    data_root: PathBuf,
    model: Model,
    runtimes: Option<Report>,
    /// The request being assembled from what the operator typed, while one is.
    draft: Option<Draft>,
    /// The contract the application prepared from that request, once it validated.
    prepared: Option<PreparedContract>,
}

impl Session {
    /// Open a data root. A root with no committed run is not an error: the interface states
    /// the absence and offers what is possible from there.
    pub fn open(data_root: &Path, contracts: &[ManagedContract]) -> Self {
        let environment = Environment::detect(data_root);
        let facts: Vec<ContractFacts> = contracts.iter().map(configured_contract).collect();
        let mut model = Model::cold(environment, facts);

        match Application::open(data_root) {
            Err(_) => Self {
                application: None,
                data_root: data_root.to_path_buf(),
                model,
                runtimes: None,
                draft: None,
                prepared: None,
            },
            Ok(application) => {
                let state = application.state().clone();
                match application.events_after(0) {
                    Ok(events) => model.absorb(&state, &events),
                    Err(error) => model.reply(format!("journal unreadable: {error}")),
                }
                Self {
                    application: Some(application),
                    data_root: data_root.to_path_buf(),
                    model,
                    runtimes: None,
                    draft: None,
                    prepared: None,
                }
            }
        }
    }

    /// A session over an already-open application, used by the preview and by tests.
    pub fn from_application(
        application: Application,
        environment: Environment,
        contracts: Vec<ContractFacts>,
    ) -> Self {
        let mut model = Model::cold(environment, contracts);
        let state = application.state().clone();
        if let Ok(events) = application.events_after(0) {
            model.absorb(&state, &events);
        }
        let data_root = application.data_root().to_path_buf();
        Self {
            application: Some(application),
            data_root,
            model,
            runtimes: None,
            draft: None,
            prepared: None,
        }
    }

    /// Catch up with the journal from the model's cursor.
    pub fn refresh(&mut self) {
        let Some(application) = &self.application else {
            return;
        };
        let state = application.state().clone();
        match application.events_after(self.model.cursor) {
            Ok(events) => self.model.absorb(&state, &events),
            Err(error) => self.model.reply(format!("journal unreadable: {error}")),
        }
    }

    pub fn set_runtimes(&mut self, report: Report) {
        self.runtimes = Some(report);
    }

    pub fn runtimes(&self) -> Option<&Report> {
        self.runtimes.as_ref()
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Commit the operator's cancellation. The domain decides the transition; the interface
    /// only names who asked for it.
    pub fn cancel_run(&mut self) {
        let Some(application) = &mut self.application else {
            self.model
                .reply("no run is open in this store — nothing to cancel");
            return;
        };
        let command_id = format!(
            "ymp.tui.cancel.{}",
            application.state().last_sequence.saturating_add(1)
        );
        let reason = decisions::cancellation_reason();
        match application.execute(command_id, DomainCommand::Cancel { reason }) {
            Ok(_) => {
                self.refresh();
                self.model
                    .reply("cancel recorded — the run ended with the terminal outcome cancelled");
            }
            Err(error) => self
                .model
                .reply(format!("cancel was not recorded: {error}")),
        }
    }

    /// Whether this store holds a run the session has opened.
    pub fn has_application(&self) -> bool {
        self.application.is_some()
    }

    /// Subscribe to journal notifications, when there is a journal to follow.
    fn subscribe(&mut self) -> Option<mpsc::Receiver<u64>> {
        self.application
            .as_mut()
            .and_then(|application| application.subscribe(NOTIFICATION_CAPACITY).ok())
    }

    /// The full projection for the given describe selection.
    pub fn projection(&self, describe: Option<usize>) -> Projection {
        let mut projection = self.model.projection(self.runtimes.as_ref());
        if let Some(index) = describe
            && let Some(page) = self.model.describe_candidate(index)
        {
            projection.pages.push((PageKind::Describe, page));
        }
        projection
    }

    pub fn describe_page(&self, index: usize) -> Option<Page> {
        self.model.describe_candidate(index)
    }

    /// Take what the operator typed.
    ///
    /// With no run in this store the line is a request: it opens a draft, and the answers that
    /// follow complete it. Once a run exists there is nothing for prose to become — the domain
    /// carries no messages — so the turn is answered honestly and recorded nowhere.
    pub fn local_turn(&mut self, text: String) {
        // An empty line is not something the operator said; it accepts what the question
        // offered, and the reply below states what that was.
        if !text.trim().is_empty() {
            self.model.human(text.clone());
        }
        if self.application.is_some() {
            self.model.reply(
                "local turn — not recorded in the journal. This domain carries no messages, so no \
                 participant can receive it. Commands work: press : for the list, ? for the keys.",
            );
            return;
        }
        let project = self.model.environment().project_path.clone();
        match self.draft.as_mut() {
            None => {
                if text.trim().is_empty() {
                    return;
                }
                self.draft = Some(Draft::new(text));
                self.model.reply(format!(
                    "request recorded locally — nothing has started and nothing is spent. {}",
                    draft::question_text(draft::Question::Source, &project)
                ));
                self.model
                    .await_answer(Some(draft::question_hint(draft::Question::Source)));
            }
            Some(current) => {
                let step = current.answer(&text, &project);
                let taken = current.taken();
                match step {
                    Step::Ask(question) => {
                        self.model.reply(format!(
                            "{taken} · {}",
                            draft::question_text(question, &project)
                        ));
                        self.model
                            .await_answer(Some(draft::question_hint(question)));
                    }
                    Step::Ready(request) => {
                        self.model.await_answer(None);
                        self.prepare(*request);
                    }
                }
            }
        }
    }

    /// Hand the assembled request to the application, which decides whether it is a contract.
    fn prepare(&mut self, request: RunRequest) {
        self.draft = None;
        match prepare_contract(&request) {
            Ok(prepared) => {
                let facts = ContractFacts::from_prepared(&prepared);
                self.model.reply(format!(
                    "contract {} drafted · digest {} · verifier {} · negative control {} · \
                     nothing has started and nothing is spent. :authorize {} reviews what would \
                     be checked and starts run {}",
                    facts.contract_id,
                    crate::projection::short_digest(&facts.contract_digest),
                    prepared.verifier().program.display(),
                    prepared.verifier().negative_control.display(),
                    facts.contract_id,
                    prepared.run_id()
                ));
                self.model.record_contract(facts);
                self.prepared = Some(prepared);
            }
            Err(error) => {
                self.prepared = None;
                self.model.error(format!(
                    "{error}. Nothing was recorded. State the request again to draft another \
                     contract."
                ));
            }
        }
    }

    /// Commit the operator's authorization: store the contract and start the run against it.
    pub fn start_run(&mut self) {
        if self.application.is_some() {
            self.model
                .error("this store already holds a run — a second run needs its own store");
            return;
        }
        let Some(prepared) = self.prepared.clone() else {
            self.model
                .error("no contract is drafted — state the request first");
            return;
        };
        match Application::create_with_contract(&self.data_root, &prepared) {
            Ok((application, outcome)) => {
                self.application = Some(application);
                self.model.reply(format!(
                    "run {} started against contract {} · the journal records the approved \
                     contract at event #{:04}",
                    prepared.run_id(),
                    prepared.contract_id(),
                    outcome.event.sequence
                ));
                self.refresh();
            }
            Err(error) => self
                .model
                .error(format!("the run was not started: {error}")),
        }
    }
}

/// A contract configured on the command line, taken through the same application path a typed
/// request takes. A package the application refuses carries its refusal onto the screen instead
/// of appearing as a contract that could be authorized.
fn configured_contract(contract: &ManagedContract) -> ContractFacts {
    let request = RunRequest {
        prompt: contract.prompt.clone(),
        source: contract.source.clone(),
        acceptance: contract.verifier.as_ref().map(|verifier| {
            let mut acceptance = ymp_application::AcceptanceCondition::new(
                verifier.program.clone(),
                verifier.negative_control.clone(),
            );
            acceptance.arguments = verifier.arguments.clone();
            acceptance.oracle_digest = Some(verifier.oracle_digest.clone());
            acceptance.wall_time_ms = verifier.wall_time_ms;
            acceptance.output_limit_bytes = verifier.output_limit_bytes;
            acceptance
        }),
        capture_exclusions: contract.capture_exclusions.clone(),
        contract_id: Some(contract.contract_id.clone()),
        budget: None,
    };
    match prepare_contract(&request) {
        Ok(prepared) => ContractFacts::from_prepared(&prepared),
        Err(error) => ContractFacts::refused(
            contract.contract_id.clone(),
            contract.source.clone(),
            contract.prompt.clone(),
            error.to_string(),
        ),
    }
}

/// What a keystroke asks the loop to do beyond changing the view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    CancelRun,
    /// Store the drafted contract and start the run it names.
    StartRun,
    /// The operator typed prose. It is shown as a local turn and answered honestly: no
    /// participant can receive it until the domain carries messages.
    LocalTurn(String),
    Rebuild,
}

/// Start the interface over a data root.
pub fn run(mut session: Session, markers: Markers) -> anyhow::Result<()> {
    let mut app = App::new(session.projection(None));
    let mut guard = TerminalGuard::enter()?;
    let result = event_loop(&mut guard, &mut session, &mut app, markers);
    drop(guard);
    result
}

enum AppEvent {
    Terminal(Event),
    Journal,
    Runtimes(Box<Report>),
    InputEnded,
}

fn event_loop(
    guard: &mut TerminalGuard,
    session: &mut Session,
    app: &mut App,
    markers: Markers,
) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let input = spawn_input_thread(tx.clone(), Arc::clone(&stop));
    spawn_probe_thread(tx.clone());
    if let Some(receiver) = session.subscribe() {
        spawn_journal_thread(tx.clone(), receiver);
    }

    let mut dirty = true;
    loop {
        if dirty {
            guard
                .terminal()
                .draw(|frame| ui::render(frame, app, &markers))?;
            dirty = false;
        }

        match rx.recv_timeout(IDLE) {
            Ok(event) => {
                let mut batch = vec![event];
                // Collapse whatever arrived while the frame was drawn: holding a scroll key
                // must not queue one redraw per repeat.
                while let Ok(next) = rx.try_recv() {
                    batch.push(next);
                }
                for event in batch {
                    match event {
                        AppEvent::Terminal(event) => {
                            let height = guard.terminal().size().map_or(24, |size| size.height);
                            if let Some(action) = handle_event(app, event, height) {
                                perform(session, app, action, &tx);
                            }
                        }
                        AppEvent::Journal => {
                            session.refresh();
                            adopt(session, app);
                        }
                        AppEvent::Runtimes(report) => {
                            session.set_runtimes(*report);
                            adopt(session, app);
                        }
                        AppEvent::InputEnded => {
                            app.should_quit = true;
                        }
                    }
                }
                dirty = true;
                if app.should_quit {
                    stop.store(true, Ordering::SeqCst);
                    let _ = input.join();
                    return Ok(());
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                stop.store(true, Ordering::SeqCst);
                let _ = input.join();
                return Ok(());
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn perform(session: &mut Session, app: &mut App, action: Action, tx: &Sender<AppEvent>) {
    match action {
        Action::CancelRun => {
            session.cancel_run();
            adopt(session, app);
        }
        Action::StartRun => {
            // The store had no journal to follow until now, so the loop starts following the
            // one this action created rather than waiting for the next keystroke to notice it.
            let follow = !session.has_application();
            session.start_run();
            if follow && let Some(receiver) = session.subscribe() {
                spawn_journal_thread(tx.clone(), receiver);
            }
            adopt(session, app);
        }
        Action::LocalTurn(text) => {
            session.local_turn(text);
            adopt(session, app);
        }
        Action::Rebuild => adopt(session, app),
    }
}

/// Replace the projection and re-apply the operator's position within it.
fn adopt(session: &Session, app: &mut App) {
    app.adopt(session.projection(app.describe_index));
}

// ---------------------------------------------------------------------------
// Key handling — pure over the view state
// ---------------------------------------------------------------------------

/// Apply one terminal event. `height` is the terminal height, used for page-sized scrolling.
pub fn handle_event(app: &mut App, event: Event, height: u16) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key, height),
        Event::Paste(text) => {
            if matches!(app.modal, Modal::None) && app.surface == Surface::Transcript {
                app.prompt.buffer.push_str(text.trim_end_matches('\n'));
            }
            None
        }
        _ => None,
    }
}

/// Apply one key press.
pub fn handle_key(app: &mut App, key: KeyEvent, height: u16) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return None;
    }

    // A modal owns the keyboard while it is open.
    match &app.modal {
        Modal::Palette(_) => return palette_key(app, key),
        Modal::Confirm(_) => return confirm_key(app, key),
        Modal::Authorize(_) => return authorize_key(app, key),
        Modal::Keys => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                close_modal(app);
            }
            return None;
        }
        Modal::None => {}
    }

    match app.surface {
        Surface::Transcript => transcript_key(app, key, height),
        Surface::Page(kind) => page_key(app, kind, key),
    }
}

fn close_modal(app: &mut App) {
    app.modal = Modal::None;
    app.prompt.suspended = None;
}

fn transcript_key(app: &mut App, key: KeyEvent, height: u16) -> Option<Action> {
    let page = usize::from(height.saturating_sub(8)).max(1);
    match key.code {
        KeyCode::Up => app.scroll_up(1),
        KeyCode::Down => app.scroll_down(1),
        KeyCode::PageUp => app.scroll_up(page),
        KeyCode::PageDown => app.scroll_down(page),
        KeyCode::End => app.resume_live(),
        KeyCode::Char('?') if app.prompt.buffer.is_empty() => app.modal = Modal::Keys,
        KeyCode::Char('q') if app.prompt.buffer.is_empty() => app.should_quit = true,
        KeyCode::Char(':') if app.prompt.buffer.is_empty() => open_palette(app),
        KeyCode::Esc => {
            if app.prompt.buffer.is_empty() {
                app.resume_live();
            } else {
                app.prompt.buffer.clear();
            }
        }
        KeyCode::Backspace => {
            app.prompt.buffer.pop();
        }
        KeyCode::Enter => {
            // A typed line is a request when the store holds no run, and free prose otherwise;
            // the session decides, because only it knows what the store holds. An empty line is
            // sent only while an answer is awaited, where it accepts what the question offers.
            if !app.prompt.buffer.trim().is_empty() || app.data.awaiting.is_some() {
                let text = std::mem::take(&mut app.prompt.buffer);
                app.resume_live();
                return Some(Action::LocalTurn(text));
            }
        }
        KeyCode::Char(character) => app.prompt.buffer.push(character),
        _ => {}
    }
    None
}

fn page_key(app: &mut App, kind: PageKind, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => {
            app.surface = if kind == PageKind::Describe {
                Surface::Page(PageKind::Candidates)
            } else {
                Surface::Transcript
            };
            app.prompt.suspended = None;
        }
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(kind),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(kind),
        KeyCode::PageUp => {
            for _ in 0..10 {
                app.select_prev(kind);
            }
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                app.select_next(kind);
            }
        }
        KeyCode::Enter if kind == PageKind::Candidates => {
            app.describe_index = Some(app.selection_of(kind));
            app.surface = Surface::Page(PageKind::Describe);
            return Some(Action::Rebuild);
        }
        KeyCode::Char('?') => app.modal = Modal::Keys,
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char(':') => open_palette(app),
        _ => {}
    }
    None
}

fn open_palette(app: &mut App) {
    app.prompt.suspended = Some("command palette open".into());
    app.modal = Modal::Palette(app.open_palette());
}

fn palette_key(app: &mut App, key: KeyEvent) -> Option<Action> {
    let Modal::Palette(palette) = &mut app.modal else {
        return None;
    };
    match key.code {
        KeyCode::Esc => close_modal(app),
        KeyCode::Up => palette.selected = palette.selected.saturating_sub(1),
        KeyCode::Down => {
            let count = palette.matches().len();
            if count > 0 {
                palette.selected = (palette.selected + 1).min(count - 1);
            }
        }
        KeyCode::Backspace => {
            palette.input.pop();
            palette.selected = 0;
            if palette.input.is_empty() {
                close_modal(app);
            }
        }
        KeyCode::Char(character) => {
            palette.input.push(character);
            palette.selected = 0;
        }
        KeyCode::Enter => {
            let command = palette
                .matches()
                .get(palette.selected)
                .map(|item| item.command);
            close_modal(app);
            match command {
                Some(Command::OpenPage(kind)) => app.surface = Surface::Page(kind),
                Some(Command::Authorize(index)) => app.open_authorize_at(index),
                Some(Command::CancelRun) => app.open_cancel_confirm(),
                Some(Command::Quit) => app.should_quit = true,
                None => {}
            }
        }
        _ => {}
    }
    None
}

/// The coverage map. Esc always leaves it; Enter moves on to the typed confirmation only when
/// the projection says this contract can start a run.
fn authorize_key(app: &mut App, key: KeyEvent) -> Option<Action> {
    let Modal::Authorize(authorize) = &app.modal else {
        return None;
    };
    match key.code {
        KeyCode::Esc => close_modal(app),
        KeyCode::Enter => {
            if let Some(action) = authorize.action.clone() {
                app.modal = Modal::Confirm(decisions::start_run(&action));
            }
        }
        _ => {}
    }
    None
}

fn confirm_key(app: &mut App, key: KeyEvent) -> Option<Action> {
    let Modal::Confirm(confirm) = &mut app.modal else {
        return None;
    };
    match key.code {
        KeyCode::Esc => close_modal(app),
        KeyCode::Backspace => {
            confirm.typed.pop();
        }
        KeyCode::Char(character) => confirm.typed.push(character),
        KeyCode::Enter if confirm.is_exact() => {
            let action = confirm.action.clone();
            close_modal(app);
            app.resume_live();
            return Some(match action {
                ConfirmAction::CancelRun { .. } => Action::CancelRun,
                ConfirmAction::StartRun { .. } => Action::StartRun,
            });
        }
        _ => {}
    }
    None
}

// ---------------------------------------------------------------------------
// Threads
// ---------------------------------------------------------------------------

/// Read terminal events until asked to stop. `event::read` cannot be interrupted, so the thread
/// polls its own descriptor briefly and checks the stop flag between waits.
fn spawn_input_thread(tx: Sender<AppEvent>, stop: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            match event::poll(INPUT_TICK) {
                Ok(true) => match event::read() {
                    Ok(event) => {
                        if tx.send(AppEvent::Terminal(event)).is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        let _ = tx.send(AppEvent::InputEnded);
                        return;
                    }
                },
                Ok(false) => {}
                Err(_) => {
                    let _ = tx.send(AppEvent::InputEnded);
                    return;
                }
            }
        }
    })
}

/// Probing starts subprocesses, so it happens once, off the drawing thread.
fn spawn_probe_thread(tx: Sender<AppEvent>) {
    thread::spawn(move || {
        let report = crate::runtimes::probe_all();
        let _ = tx.send(AppEvent::Runtimes(Box::new(report)));
    });
}

/// Wake the loop when the journal advances. The payload is deliberately dropped: the reader
/// recovers from its own cursor rather than trusting a notification.
fn spawn_journal_thread(tx: Sender<AppEvent>, receiver: mpsc::Receiver<u64>) {
    thread::spawn(move || {
        while receiver.recv().is_ok() {
            if tx.send(AppEvent::Journal).is_err() {
                return;
            }
        }
    });
}
