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
use ymp_application::{
    Application, ApplicationError, PreparedContract, RunRequest, prepare_contract,
};
use ymp_domain::Command as DomainCommand;
use ymp_domain::contract::ContractDocument;

use crate::decisions;
use crate::draft::{self, Draft, Step};
use crate::journal::Model;
use crate::pages::Page;
use crate::projection::{ContractFacts, Environment, Projection};
use crate::runtimes::Report;
use crate::state::{App, COMMAND_PREFIX, Command, ConfirmAction, Modal, PageKind, Surface};
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
    /// Every contract this session can start a run from, whatever supplied it: a package the
    /// command line named, or a request typed here. Both arrive as the same prepared contract,
    /// so both start through one call.
    contracts: Vec<PreparedContract>,
    /// Which demonstration this session is waiting for. Every check carries the number it was
    /// asked under, so an outcome that arrives after a cancellation or a later answer is
    /// recognised as deciding nothing.
    checking: u64,
}

/// A demonstration this session asked for, ready to be run wherever the caller decides.
///
/// It holds no reference to the session, so the interface can move it to another thread and hand
/// the outcome back through [`Session::finish_check`].
#[derive(Clone, Debug)]
pub struct PendingCheck {
    generation: u64,
    check: draft::EntryCheck,
}

impl PendingCheck {
    /// What the interface says it is waiting for while this runs.
    pub fn waiting_for(&self) -> String {
        self.check.waiting_for()
    }

    /// Run the demonstration and label its outcome with the check it belongs to.
    pub fn run(self) -> CheckOutcome {
        let outcome = self.check.run();
        CheckOutcome {
            generation: self.generation,
            outcome,
        }
    }
}

/// The outcome of one demonstration, with the check it belongs to.
#[derive(Clone, Debug)]
pub struct CheckOutcome {
    generation: u64,
    outcome: Result<(), String>,
}

impl Session {
    /// Open a data root. A root with no committed run is not an error: the interface states
    /// the absence and offers what is possible from there.
    pub fn open(data_root: &Path, contracts: &[PreparedContract]) -> Self {
        let environment = Environment::detect(data_root);
        let facts: Vec<ContractFacts> =
            contracts.iter().map(ContractFacts::from_prepared).collect();
        let mut model = Model::cold(environment, facts);

        match Application::open(data_root) {
            Err(error) => {
                // A store this binary cannot read is reported as what it is. It was left
                // untouched, so the interface neither claims the run never existed nor offers
                // to start one over it.
                if let ApplicationError::IncompatibleStore { .. } = &error {
                    model.error(format!(
                        "{error}. Nothing in it was changed. Point ymp at another store, or keep \
                         this one for a binary that reads its version."
                    ));
                    model.refuse_store();
                }
                Self {
                    application: None,
                    data_root: data_root.to_path_buf(),
                    model,
                    runtimes: None,
                    draft: None,
                    contracts: contracts.to_vec(),
                    checking: 0,
                }
            }
            Ok(application) => {
                let state = application.state().clone();
                match application.events_after(0) {
                    Ok(events) => model.absorb(&state, &events),
                    Err(error) => model.reply(format!("journal unreadable: {error}")),
                }
                if let Some(facts) = bound_contract(&application) {
                    model.record_contract(facts);
                }
                Self {
                    application: Some(application),
                    data_root: data_root.to_path_buf(),
                    model,
                    runtimes: None,
                    draft: None,
                    contracts: contracts.to_vec(),
                    checking: 0,
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
            contracts: Vec::new(),
            checking: 0,
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

    /// Take what the operator typed and settle it here, including any demonstration it needs.
    ///
    /// A caller with nothing else to do — a command, whose process is the wait — settles the turn
    /// on its own thread. The interface has a screen to keep drawing, so it uses [`Self::begin_turn`]
    /// and [`Self::finish_check`] instead: the same two steps, scheduled rather than run inline.
    pub fn local_turn(&mut self, text: String) {
        if let Some(pending) = self.begin_turn(text) {
            let outcome = pending.run();
            self.finish_check(outcome);
        }
    }

    /// Take what the operator typed, up to the point where work would begin.
    ///
    /// With no run in this store the line is a request: it opens a draft, and the answers that
    /// follow complete it. Once a run exists there is nothing for prose to become — the domain
    /// carries no messages — so the turn is answered honestly and recorded nowhere.
    ///
    /// An answer whose decision needs the verifier run returns that work to the caller instead of
    /// doing it here. Nothing about the draft is settled until the outcome comes back.
    pub fn begin_turn(&mut self, text: String) -> Option<PendingCheck> {
        // An empty line is not something the operator said; it accepts what the question
        // offered, and the reply below states what that was.
        if !text.trim().is_empty() {
            self.model.human(text.clone());
        }
        if self.application.is_some() {
            self.model.reply(
                "local turn — not recorded in the journal. This domain carries no messages, so no \
                 participant can receive it. Commands work: press / for the list, ? for the keys.",
            );
            return None;
        }
        if self.model.store_refused() {
            self.model.error(
                "this store cannot be read by this binary, so no request can be drafted over it",
            );
            return None;
        }
        let project = self.model.environment().project_path.clone();
        match self.draft.as_mut() {
            None => {
                if text.trim().is_empty() {
                    return None;
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
                return self.take_step(step, taken, &project);
            }
        }
        None
    }

    /// State what one step of the draft means, and return the work it needs, if any.
    fn take_step(&mut self, step: Step, taken: String, project: &Path) -> Option<PendingCheck> {
        match step {
            Step::Ask(question) => {
                self.model.reply(format!(
                    "{taken} · {}",
                    draft::question_text(question, project)
                ));
                self.model
                    .await_answer(Some(draft::question_hint(question)));
                None
            }
            // An answer this host cannot resolve ends the draft where it was typed. The
            // remaining questions are not asked, because they would collect answers for a
            // contract that already cannot be assembled.
            Step::Refused(reason) => {
                self.draft = None;
                self.model.await_answer(None);
                self.model.working(None);
                self.model.error(format!(
                    "{reason}. Nothing was recorded. State the request again to draft another \
                     contract."
                ));
                None
            }
            // The answer resolved; what decides it is the verifier's own decision about it. The
            // interface states what it is waiting for and accepts no answer meanwhile, because
            // the answer being decided is the one that was just typed.
            Step::Check(check) => {
                self.checking = self.checking.wrapping_add(1);
                self.model.await_answer(None);
                self.model.working(Some(check.waiting_for()));
                self.model.reply(format!(
                    "{taken} · {} · nothing has started and nothing is spent · Esc cancels the \
                     check",
                    check.waiting_for()
                ));
                Some(PendingCheck {
                    generation: self.checking,
                    check,
                })
            }
            Step::Ready(request) => {
                self.model.await_answer(None);
                self.model.working(None);
                self.prepare(*request);
                None
            }
        }
    }

    /// Take the outcome of a demonstration this session asked for.
    ///
    /// An outcome from a check the operator has since cancelled, or from one superseded by a
    /// later answer, decides nothing: the draft it was deciding is no longer waiting for it.
    pub fn finish_check(&mut self, outcome: CheckOutcome) {
        if outcome.generation != self.checking {
            return;
        }
        let project = self.model.environment().project_path.clone();
        let Some(draft) = self.draft.as_mut() else {
            self.model.working(None);
            return;
        };
        let step = draft.checked(outcome.outcome);
        let taken = draft.taken();
        self.model.working(None);
        self.take_step(step, taken, &project);
    }

    /// Abandon a running demonstration at the operator's word.
    ///
    /// The draft returns to the answer the check was deciding, so the operator can state another
    /// one. The outcome of the abandoned run is ignored when it arrives; the program it started
    /// ends on its own, bounded by the limit the check carries.
    pub fn cancel_check(&mut self) {
        if !self.is_checking() {
            return;
        }
        self.checking = self.checking.wrapping_add(1);
        self.model.working(None);
        let question = self
            .draft
            .as_mut()
            .map(|draft| draft.abandon_check())
            .unwrap_or(draft::Question::NegativeControl);
        self.model.reply(
            "the check was cancelled — nothing was recorded and the answer it was deciding was \
             not taken",
        );
        self.model
            .await_answer(Some(draft::question_hint(question)));
    }

    /// Whether a demonstration this session asked for is still deciding an answer.
    pub fn is_checking(&self) -> bool {
        self.draft.as_ref().is_some_and(|draft| draft.is_checking())
    }

    /// Hand the assembled request to the application, which decides whether it is a contract.
    fn prepare(&mut self, request: RunRequest) {
        self.draft = None;
        match prepare_contract(&request) {
            Ok(prepared) => {
                let facts = ContractFacts::from_prepared(&prepared);
                self.model.reply(format!(
                    "contract {} drafted · digest {} · verifier {} · negative control {} · \
                     nothing has started and nothing is spent. /authorize {} reviews what would \
                     be checked and starts run {}",
                    facts.contract_id,
                    crate::projection::short_digest(&facts.contract_digest),
                    prepared.verifier().program.display(),
                    prepared.verifier().negative_control.display(),
                    facts.contract_id,
                    prepared.run_id()
                ));
                self.model.record_contract(facts);
                self.contracts
                    .retain(|contract| contract.contract_id() != prepared.contract_id());
                self.contracts.push(prepared);
            }
            Err(error) => {
                self.model.error(format!(
                    "{error}. Nothing was recorded. State the request again to draft another \
                     contract."
                ));
            }
        }
    }

    /// Commit the operator's authorization: store the named contract and start its run.
    ///
    /// The contract is looked up by the identifier the decision surface required the operator to
    /// type, so a package named on the command line and a request typed here start identically.
    pub fn start_run(&mut self, contract_id: &str) {
        if self.application.is_some() {
            self.model
                .error("this store already holds a run — a second run needs its own store");
            return;
        }
        let Some(prepared) = self
            .contracts
            .iter()
            .find(|contract| contract.contract_id() == contract_id)
            .cloned()
        else {
            self.model.error(format!(
                "no contract named {contract_id} is available to this session"
            ));
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

/// The contract a started run is bound to, read back from the store it was approved into.
///
/// A store opened later carries no drafted contract, so the coverage map would otherwise have
/// nothing to show for a run that has one. The stored object is the source; when it cannot be
/// read the binding is shown with what the journal alone records and says so.
fn bound_contract(application: &Application) -> Option<ContractFacts> {
    let binding = application.contract()?;
    match application.contract_bytes() {
        Ok(Some(bytes)) => match ContractDocument::parse(&bytes) {
            Ok(parsed) => Some(ContractFacts::from_document(
                &parsed.document,
                parsed.digest,
            )),
            Err(error) => Some(ContractFacts::refused(
                binding.contract_id.clone(),
                PathBuf::new(),
                String::new(),
                format!("the stored contract object could not be read: {error}"),
            )),
        },
        _ => Some(ContractFacts::refused(
            binding.contract_id.clone(),
            PathBuf::new(),
            String::new(),
            "the stored contract object is not in this store".to_owned(),
        )),
    }
}

/// What a keystroke asks the loop to do beyond changing the view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    CancelRun,
    /// Store the named contract and start the run it names.
    StartRun(String),
    /// The operator typed prose. It is shown as a local turn and answered honestly: no
    /// participant can receive it until the domain carries messages.
    LocalTurn(String),
    /// Abandon the demonstration a typed answer started. It acts on this interface's own
    /// scheduling: nothing durable is written and nothing spent, so there is nothing for a
    /// command to mirror — a command's process is its own check.
    CancelCheck,
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
    /// A demonstration this session asked for has decided.
    Checked(Box<CheckOutcome>),
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
                        AppEvent::Checked(outcome) => {
                            session.finish_check(*outcome);
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
            // Nothing arrived. While work runs away from this thread the row that states it
            // advances, so the interface is visibly drawing rather than held.
            Err(RecvTimeoutError::Timeout) => {
                if app.data.working.is_some() {
                    app.working_ticks = app.working_ticks.wrapping_add(1);
                    dirty = true;
                }
            }
        }
    }
}

fn perform(session: &mut Session, app: &mut App, action: Action, tx: &Sender<AppEvent>) {
    match action {
        Action::CancelRun => {
            session.cancel_run();
            adopt(session, app);
        }
        Action::StartRun(contract_id) => {
            // The store had no journal to follow until now, so the loop starts following the
            // one this action created rather than waiting for the next keystroke to notice it.
            let follow = !session.has_application();
            session.start_run(&contract_id);
            if follow && let Some(receiver) = session.subscribe() {
                spawn_journal_thread(tx.clone(), receiver);
            }
            adopt(session, app);
        }
        // The turn is taken here; the work it needs is not. A demonstration runs a program and
        // waits for it, which on this thread would stop every redraw for as long as the program
        // runs, so it goes to a thread of its own and reports back as an event.
        Action::LocalTurn(text) => {
            let pending = session.begin_turn(text);
            adopt(session, app);
            if let Some(pending) = pending {
                app.working_ticks = 0;
                spawn_check_thread(tx.clone(), pending);
            }
        }
        Action::CancelCheck => {
            session.cancel_check();
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
        KeyCode::Esc if app.data.working.is_some() => return Some(Action::CancelCheck),
        KeyCode::Char('?') if app.prompt.buffer.is_empty() => app.modal = Modal::Keys,
        KeyCode::Char('q') if app.prompt.buffer.is_empty() => app.should_quit = true,
        KeyCode::Char(COMMAND_PREFIX) if app.prompt.buffer.is_empty() => open_palette(app),
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
        KeyCode::Char(COMMAND_PREFIX) => open_palette(app),
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
            let matches = palette.matches();
            // A line that names no command is a line, not a failed command: it leaves as the
            // request or answer it was, prefix included, exactly as it was typed. Without this
            // an absolute path could never be typed, because it opens with the command prefix.
            if matches.is_empty() {
                let typed = palette.input.clone();
                close_modal(app);
                app.prompt.buffer.clear();
                app.resume_live();
                return Some(Action::LocalTurn(typed));
            }
            let command = matches.get(palette.selected).map(|item| item.command);
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
                ConfirmAction::StartRun { contract_id, .. } => Action::StartRun(contract_id),
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

/// A demonstration runs a program of the operator's choosing and waits for it. It therefore
/// never runs on the thread that draws; its outcome returns as an event like any other.
fn spawn_check_thread(tx: Sender<AppEvent>, pending: PendingCheck) {
    thread::spawn(move || {
        let outcome = pending.run();
        let _ = tx.send(AppEvent::Checked(Box::new(outcome)));
    });
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
