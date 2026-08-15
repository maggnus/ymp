//! The session, the event loop and key handling.
//!
//! [`Session`] owns the application handle and the read model; the event loop owns the terminal
//! and the view state. Key handling is a pure function over the view state that returns the
//! [`Action`] the loop must execute, so every keyboard transition can be driven in a test
//! without a terminal or a store.
//!
//! Input is read by a dedicated thread; journal notifications, the runtime probe and the outcome
//! of a verification arrive on the same channel. The main thread blocks until something happens.
//! The one exception is a working managed attempt, which reports through a channel of its own that
//! wakes nothing: while one is working the loop looks for its output on a short tick, and with no
//! attempt working it goes back to waiting.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ymp_application::root::{StoreIntent, store_under};
use ymp_application::{
    Application, ApplicationError, PreparedContract, VerificationJob, VerificationOutcome,
    prepare_contract,
};
use ymp_domain::Command as DomainCommand;
use ymp_domain::commitment::Verdict;
use ymp_domain::contract::ContractDocument;
use ymp_domain::{RunStatus, VerificationDecision};
use ymp_runtime_api::{RuntimeEventKind, RuntimeKind};
use ymp_runtime_supervisor::{ManagedRunEvent, ManagedRunHandle};

use crate::attempt::{self, Route, Routing};
use crate::decisions;
use crate::draft::{Amendment, Assembly, Draft, DraftJob};
use crate::journal::Model;
use crate::pages::Page;
use crate::projection::{ContractFacts, Environment, Projection};
use crate::runtimes::Report;
use crate::state::{App, COMMAND_PREFIX, Command, ConfirmAction, Modal, PageKind, Surface};
use crate::terminal::TerminalGuard;
use crate::theme::Markers;
use crate::ui;
use ymp_runtime_registry::{Engine, RegistryAddress};

/// How long the input thread waits before checking whether it should stop.
const INPUT_TICK: Duration = Duration::from_millis(50);
/// Upper bound on how long the main thread sleeps with nothing to do.
const IDLE: Duration = Duration::from_millis(500);
/// Journal notifications buffered before the reader is considered lagging; it then recovers
/// from its event cursor rather than assuming every notification arrived (INV-6).
const NOTIFICATION_CAPACITY: usize = 64;
/// How long the main thread sleeps while a managed attempt is producing events. The runtime
/// writes on its own schedule, so the loop looks often enough for its output to read as output.
const ATTEMPT_TICK: Duration = Duration::from_millis(50);
/// How long a cancellation waits for the attempt's worker to end the process tree and close its
/// slice in the kernel. A cancellation is that wait; past this the interface says what it could
/// not establish rather than claiming an ending it never read.
const TERMINATION_WAIT: Duration = Duration::from_secs(30);
/// Where under the data root the product writes what it supplies for a draft: the verifier it
/// proposes, the copy it takes as the negative control, and the sample that verifier must accept.
const DRAFT_DIRECTORY: &str = "draft";
/// What an export directory is called when the operator names none. The run identifier follows it.
const EXPORT_PREFIX: &str = "ymp-evidence-";

/// Everything durable the interface reads and the commands it can commit.
pub struct Session {
    /// The store, shared because the managed attempt commits through the same writer: the
    /// candidate the runtime submits reaches the journal through this handle and no other.
    application: Option<Arc<Mutex<Application>>>,
    /// The root the invocation addressed, when it addressed one. A store holds one run, so the
    /// second run of a project is addressed here rather than refused.
    root: Option<PathBuf>,
    data_root: PathBuf,
    /// How this session addresses the engine registry. A root an invocation named is taken as
    /// stated; a store it named resolves to the root that store stands under, so naming the store
    /// instead of the root never reaches a different decision about an engine.
    registry: RegistryAddress,
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
    /// What this session has authorized, keyed by everything an authorization grants: the
    /// contract that decides the work and the budget the run would spend. Re-authorizing exactly
    /// that is one confirmation; anything else is authorized by typing the contract id. The
    /// record is this session's own, so a new process authorizes from the beginning.
    authorized: BTreeSet<String>,
    /// The contract the current draft last produced, so an amended draft replaces it instead of
    /// leaving the version it replaced on the screen.
    drafted: Option<String>,
    /// The runtime profile the operator named for this run, when one was named.
    route: Option<Route>,
    /// The managed attempt, from its launch until the verdict on its candidate is recorded.
    /// Holding it is what makes the kernel record of this run's process slice reachable: a
    /// cancellation and a verdict are written through it, not around it.
    attempt: Option<ManagedRunHandle>,
    /// Which verification this session is waiting for, under the same rule as a draft check: an
    /// outcome from work that has been superseded decides nothing.
    verifying: u64,
}

/// A demonstration this session asked for, ready to be run wherever the caller decides.
///
/// It holds no reference to the session, so the interface can move it to another thread and hand
/// the outcome back through [`Session::finish_check`].
#[derive(Clone, Debug)]
pub struct PendingCheck {
    generation: u64,
    check: DraftJob,
}

impl PendingCheck {
    /// What the interface says it is waiting for while this runs.
    pub fn waiting_for(&self) -> String {
        self.check.waiting_for()
    }

    /// Assemble and demonstrate the draft, and label the outcome with the work it belongs to.
    pub fn run(self) -> CheckOutcome {
        let outcome = self.check.run();
        CheckOutcome {
            generation: self.generation,
            outcome,
        }
    }
}

/// The outcome of one assembly, with the work it belongs to.
#[derive(Clone, Debug)]
pub struct CheckOutcome {
    generation: u64,
    outcome: Result<Box<Assembly>, String>,
}

impl Session {
    /// Open a data root. A root with no committed run is not an error: the interface states
    /// the absence and offers what is possible from there.
    pub fn open(data_root: &Path, contracts: &[PreparedContract]) -> Self {
        let environment = Environment::detect(data_root);
        let facts: Vec<ContractFacts> = contracts
            .iter()
            .map(|contract| ContractFacts::from_prepared(contract, data_root))
            .collect();
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
                Self::over(None, data_root, model, contracts)
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
                Self::over(Some(application), data_root, model, contracts)
            }
        }
    }

    /// Open a data root addressed under a root the invocation named.
    ///
    /// The root is what lets a second run of the same project be addressed rather than refused: a
    /// store holds one run, and the store of the next one is chosen here instead of by the
    /// operator.
    pub fn open_under_root(root: &Path, data_root: &Path, contracts: &[PreparedContract]) -> Self {
        let mut session = Self::open(data_root, contracts);
        session.root = Some(root.to_path_buf());
        session.registry = RegistryAddress::Root(root.to_path_buf());
        session
    }

    fn over(
        application: Option<Application>,
        data_root: &Path,
        model: Model,
        contracts: &[PreparedContract],
    ) -> Self {
        Self {
            application: application.map(|application| Arc::new(Mutex::new(application))),
            root: None,
            data_root: data_root.to_path_buf(),
            registry: RegistryAddress::Store(data_root.to_path_buf()),
            model,
            runtimes: None,
            draft: None,
            contracts: contracts.to_vec(),
            checking: 0,
            authorized: BTreeSet::new(),
            drafted: None,
            route: None,
            attempt: None,
            verifying: 0,
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
        Self::over(Some(application), &data_root, model, &[])
    }

    /// Take the writer for the length of one call.
    ///
    /// A lock another thread panicked under is taken all the same: every durable fact is committed
    /// whole, so what a panic can leave behind is an unfinished call and never a half-written
    /// record, and refusing here would leave a store nobody could read or end.
    fn writer(application: &Arc<Mutex<Application>>) -> MutexGuard<'_, Application> {
        application
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Catch up with the journal from the model's cursor.
    pub fn refresh(&mut self) {
        let Some(application) = self.application.clone() else {
            return;
        };
        let (state, events) = {
            let application = Self::writer(&application);
            (
                application.state().clone(),
                application.events_after(self.model.cursor),
            )
        };
        match events {
            Ok(events) => self.model.absorb(&state, &events),
            Err(error) => self.model.reply(format!("journal unreadable: {error}")),
        }
    }

    pub fn set_runtimes(&mut self, report: Report) {
        self.runtimes = Some(report);
    }

    /// Address the engine registry from a root this session does not otherwise act under.
    ///
    /// A command is handed one store and acts on that store. The engines are not part of a store:
    /// which of them this host admits is one decision, and every run under a root reads it. This
    /// states the root an invocation named, which is taken as stated; an invocation that named
    /// none keeps the store it was given and resolves the root from it.
    pub fn with_registry_root(mut self, root: &Path) -> Self {
        self.registry = RegistryAddress::Root(root.to_path_buf());
        self
    }

    /// How this session addresses the engine registry.
    pub fn registry_address(&self) -> &RegistryAddress {
        &self.registry
    }

    /// Enable or disable one engine, and say what changed.
    ///
    /// The decision is durable: it is written to the registry, so the next invocation — and every
    /// store under this root — reads it. What this call does not do is re-probe: the held reading
    /// carries the new flag, and readiness is measured again when the page next asks for it.
    pub fn set_engine_enabled(&mut self, engine: Engine, enabled: bool, reason: Option<String>) {
        let registry = self.registry.registry();
        match registry.set_enabled(engine, enabled, reason.as_deref()) {
            Err(error) => self.model.error(format!(
                "the {} engine was not changed — {error}",
                engine.name()
            )),
            Ok(record) => {
                let recorded_at = registry.path_of(engine).display().to_string();
                if let Some(report) = self.runtimes.as_mut()
                    && let Some(profile) = report
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.name == engine.name())
                    && let Some(state) = profile.registry.as_mut()
                {
                    state.enabled = record.enabled;
                    state.disabled_reason = record.disabled_reason.clone();
                    profile.detail = match record.enabled {
                        true => "enabled in the registry — readiness is measured again when the \
                                 runtimes page next asks for it"
                            .to_owned(),
                        false => format!(
                            "disabled in the registry — {}. Nothing was probed and nothing is \
                             offered.",
                            record.refusal_reason()
                        ),
                    };
                }
                if self.route.is_some_and(|route| route.engine() == engine) && !record.enabled {
                    self.route = None;
                }
                // The record is named, because which file holds a decision is the difference
                // between a decision that governs the next run and one written where nothing
                // reads it.
                self.model.reply(match record.enabled {
                    true => format!(
                        "the {} engine is enabled · recorded in {recorded_at} · it can be routed \
                         to once its probe reports it ready",
                        engine.name()
                    ),
                    false => format!(
                        "the {} engine is disabled — {} · recorded in {recorded_at} · it is not \
                         probed, not offered and not routed to",
                        engine.name(),
                        record.refusal_reason()
                    ),
                });
            }
        }
    }

    pub fn runtimes(&self) -> Option<&Report> {
        self.runtimes.as_ref()
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Commit the operator's cancellation.
    ///
    /// A run whose managed attempt this session started is cancelled through that attempt, because
    /// the attempt is where both records of how a run ended live: the journal, and the kernel
    /// record of the process slice. Writing the journal alone would leave the kernel holding the
    /// slice open on an unjudged candidate, and a verdict arriving afterwards could drive the same
    /// run to acceptance. The domain still decides the transition; the interface only names who
    /// asked for it.
    pub fn cancel_run(&mut self) {
        let reason = decisions::cancellation_reason();
        if self.attempt.is_some() {
            let cancelled = self
                .attempt
                .as_ref()
                .map(|handle| handle.cancel(reason.clone()));
            match cancelled {
                Some(Ok(())) => {
                    // The slice is closed by the worker on its way out, so the kernel terminal is
                    // read once that worker reports itself finished — not at the moment the
                    // cancellation was asked for, when the slice is still open. Waiting here is
                    // waiting for the process tree to end, which is what a cancellation is.
                    let kernel = self.attempt.as_ref().map_or_else(String::new, |handle| {
                        let deadline = Instant::now() + TERMINATION_WAIT;
                        while !handle.is_finished() && Instant::now() < deadline {
                            while handle.try_next().is_some() {}
                            thread::sleep(ATTEMPT_TICK);
                        }
                        match handle.kernel().root_terminal() {
                            Ok(Some(terminal)) => {
                                format!("the kernel record of the attempt reads {terminal:?}")
                            }
                            Ok(None) => "the kernel record of the attempt still holds its slice \
                                         open"
                                .to_owned(),
                            Err(error) => format!(
                                "the kernel record of the attempt could not be read: {error}"
                            ),
                        }
                    });
                    // Dropping the handle ends the runtime's process tree and waits for the
                    // worker, so nothing this run started outlives the cancellation.
                    self.attempt = None;
                    self.verifying = self.verifying.wrapping_add(1);
                    self.model.working(None);
                    self.refresh();
                    // What the run ended as is read from the journal after it was caught up, not
                    // assumed from the command that was issued. A cancellation asked for while the
                    // attempt was already failing finds the run terminal for another reason, and
                    // the domain refuses to rename it; saying `cancelled` here would be the
                    // interface asserting an outcome the record does not hold.
                    self.model.reply(format!(
                        "cancel recorded — {}, {kernel}, and the managed process tree was ended",
                        self.recorded_terminal()
                    ));
                }
                Some(Err(error)) => self
                    .model
                    .reply(format!("cancel was not recorded: {error}")),
                None => {}
            }
            return;
        }
        let Some(application) = self.application.clone() else {
            self.model
                .reply("no run is open in this store — nothing to cancel");
            return;
        };
        let outcome = {
            let mut application = Self::writer(&application);
            let command_id = format!(
                "ymp.tui.cancel.{}",
                application.state().last_sequence.saturating_add(1)
            );
            application.execute(command_id, DomainCommand::Cancel { reason })
        };
        match outcome {
            Ok(_) => {
                self.refresh();
                self.model
                    .reply(format!("cancel recorded — {}", self.recorded_terminal()));
            }
            Err(error) => self
                .model
                .reply(format!("cancel was not recorded: {error}")),
        }
    }

    /// How the run stands in the journal, in the domain's own vocabulary.
    ///
    /// It is read after the model has caught up, so what the interface says about an ending is
    /// what the record holds rather than what the command asked for.
    fn recorded_terminal(&self) -> String {
        match self.model.run() {
            None => "this store holds no run".to_owned(),
            Some(run) if run.is_live() => {
                "the run is still live — nothing terminal was recorded".to_owned()
            }
            Some(run) => format!(
                "the run ended with the terminal outcome {}",
                crate::projection::outcome(run.status)
            ),
        }
    }

    /// Whether this store holds a run the session has opened.
    pub fn has_application(&self) -> bool {
        self.application.is_some()
    }

    /// Whether a managed attempt of this session is still running or still owes a verdict.
    pub fn attempt_is_live(&self) -> bool {
        self.attempt.is_some()
    }

    /// Subscribe to journal notifications, when there is a journal to follow.
    fn subscribe(&mut self) -> Option<mpsc::Receiver<u64>> {
        let application = self.application.clone()?;
        let mut application = Self::writer(&application);
        application.subscribe(NOTIFICATION_CAPACITY).ok()
    }

    /// The full projection for the given describe selection.
    pub fn projection(&self, describe: Option<usize>) -> Projection {
        let mut projection = self.model.projection(self.runtimes.as_ref());
        let (route, note) = attempt::routing_facts(self.route, self.runtimes.as_ref());
        projection.route = route;
        projection.route_note = note;
        projection.addresses_a_store_of_its_own = self.addresses_a_store_of_its_own();
        // The action that starts the agent is offered where a run is live, no attempt of it is
        // working, and one profile is settled to do the work.
        if let Some(run) = projection.run.as_ref().filter(|run| run.is_live())
            && self.attempt.is_none()
            && projection.route.is_some()
        {
            projection.commands.push(crate::state::PaletteItem {
                name: format!("attempt {}", run.run_id),
                description: "start the agent on this run — asks for typed confirmation".to_owned(),
                command: Command::StartAttempt,
            });
        }
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
    /// With no run in this store the line is a request: the first one opens a draft, and every
    /// line after it amends that draft while it is still unauthorized. Once a run exists there is
    /// nothing for prose to become — the domain carries no messages — so the turn is answered
    /// honestly and recorded nowhere.
    ///
    /// Assembling the draft copies the project and runs the verifier over it, so that work is
    /// returned to the caller instead of being done here. Nothing about the draft is settled
    /// until its outcome comes back, and a line typed while an assembly is running supersedes
    /// it: the amended draft is what the operator asked about, so the older outcome decides
    /// nothing when it arrives.
    pub fn begin_turn(&mut self, text: String) -> Option<PendingCheck> {
        if !text.trim().is_empty() {
            self.model.human(text.clone());
        }
        // A line opening with `runtime` states one of two things about the engines: which of them
        // does the work of this run, or whether one of them is admitted at all. Neither is part of
        // the contract — the same contract can be done by either engine — so neither amends the
        // draft nor starts an assembly. Both are read before anything else a line can be, because
        // the run they speak about may already exist: an attempt is launched into a run this
        // session did not start as readily as into one it did. Only the word and something this
        // product manages is that choice; anything else opening with the word is a line like any
        // other.
        match runtime_line(&text) {
            Some(RuntimeLine::Route(route)) => {
                self.route = Some(route);
                self.model.reply(format!(
                    "the work would be done by the {} profile · nothing has started and nothing is \
                     spent",
                    route.name()
                ));
                return None;
            }
            Some(RuntimeLine::Enabled {
                engine,
                enabled,
                reason,
            }) => {
                self.set_engine_enabled(engine, enabled, reason);
                return None;
            }
            Some(RuntimeLine::UnknownEngine(refusal)) => {
                self.model.error(format!("nothing was changed — {refusal}"));
                return None;
            }
            None => {}
        }
        // A store that holds a finished run, under a root that can address the next one, is
        // ready for the next request: the line is a request again, and authorizing what it drafts
        // records that run in a store of its own. While the run being read is still live there is
        // nothing for prose to become — the domain carries no messages — so the turn is answered
        // honestly and recorded nowhere.
        if self.application.is_some() && !self.addresses_a_store_of_its_own() {
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
        if text.trim().is_empty() {
            return None;
        }
        match self.draft.as_mut() {
            None => {
                self.draft = Some(Draft::new(text));
                self.model.reply(
                    "request recorded locally — nothing has started and nothing is spent. \
                     Assembling a contract from this project: a copy of it as the negative \
                     control, and a verifier proposed from the way it runs its tests or, when \
                     it runs none, derived from the request itself.",
                );
            }
            // While a draft is unauthorized the next line amends it. A line that names something
            // this host cannot take leaves the draft exactly as it was.
            Some(current) => match current.amend(&text) {
                Amendment::Took(stated) => {
                    self.model
                        .reply(format!("{stated} · assembling the amended draft"));
                }
                Amendment::Refused(reason) => {
                    self.model.error(format!(
                        "{reason}. The draft is unchanged and nothing was recorded."
                    ));
                    return None;
                }
            },
        }

        let project = self.model.environment().project_path.clone();
        let drafts = self.data_root.join(DRAFT_DIRECTORY);
        self.checking = self.checking.wrapping_add(1);
        let job = self.draft.as_mut()?.job(&project, &drafts, self.checking);
        // What an earlier assembly of this project left behind is of no use to anyone: its copy
        // is of a state the project has left, and the assembly that took it has been superseded,
        // so its outcome decides nothing whether it finishes or not.
        discard_earlier_workspaces(&drafts, &job);
        // The contract the draft last produced was assembled against what has just been removed,
        // so it is withdrawn with it. Until this assembly decides, there is nothing on offer.
        if let Some(withdrawn) = self.drafted.take() {
            self.model.forget_contract(&withdrawn);
            self.contracts
                .retain(|contract| contract.contract_id() != withdrawn);
        }
        self.model.await_answer(None);
        self.model.working(Some(job.waiting_for()));
        Some(PendingCheck {
            generation: self.checking,
            check: job,
        })
    }

    /// Take the outcome of an assembly this session asked for.
    ///
    /// An outcome from work the operator has since cancelled, or from work superseded by a later
    /// amendment, decides nothing: the draft it was assembling is no longer waiting for it.
    pub fn finish_check(&mut self, outcome: CheckOutcome) {
        if outcome.generation != self.checking {
            return;
        }
        self.model.working(None);
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        draft.settled();
        match outcome.outcome {
            Ok(assembly) => self.prepare(*assembly),
            // The draft stays: it is unauthorized, so the next line can amend whatever the
            // refusal named. Nothing was recorded and nothing was spent.
            Err(reason) => self.model.error(format!(
                "{reason}. Nothing was recorded. Amend the draft — for example `verifier \
                 <path>` — or state the work again."
            )),
        }
    }

    /// Abandon a running assembly at the operator's word.
    ///
    /// The draft stays as it was, so the operator can amend it or state the work again. The
    /// outcome of the abandoned work is ignored when it arrives; the program it started ends on
    /// its own, bounded by the limit the assembly carries.
    pub fn cancel_check(&mut self) {
        if !self.is_assembling() {
            return;
        }
        self.checking = self.checking.wrapping_add(1);
        self.model.working(None);
        if let Some(draft) = self.draft.as_mut() {
            draft.settled();
        }
        self.model.reply(
            "the check was cancelled — nothing was recorded and the draft was left as it was",
        );
    }

    /// Whether work this session asked for is still assembling a draft.
    pub fn is_assembling(&self) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| draft.is_assembling())
    }

    /// Hand the assembled request to the application, which decides whether it is a contract,
    /// and state the whole draft at once.
    ///
    /// What the operator has to judge is one statement — what will be run, what will decide it,
    /// what it was shown deciding — followed by one authorization. The draft stays here while it
    /// is unauthorized, so the next line can amend it.
    fn prepare(&mut self, assembly: Assembly) {
        match prepare_contract(&assembly.request) {
            Ok(prepared) => {
                let mut facts = ContractFacts::from_prepared(&prepared, &self.data_root);
                facts.previously_authorized =
                    self.authorized.contains(&authorization_key(&prepared));
                let ceremony = if facts.previously_authorized {
                    "one confirmation, because you authorized this exact contract in this session \
                     and nothing about it changed"
                } else {
                    "the contract id typed in full"
                };
                let mut statement = vec![format!(
                    "contract {} drafted · digest {} · nothing has started and nothing is spent",
                    facts.contract_id,
                    crate::projection::short_digest(&facts.contract_digest)
                )];
                statement.push(format!("work {}", first_line(&prepared.document.prompt)));
                statement.push(format!("source {}", prepared.document.source.display()));
                statement.extend(assembly.stated.iter().cloned());
                statement.push(format!(
                    "budget attempts {} · verification queries {}",
                    prepared.budget.attempts_remaining,
                    prepared.budget.verification_queries_remaining
                ));
                statement.push(match self.run_a_start_would_carry(&prepared) {
                    Some(run_id) => format!(
                        "/authorize {} starts run {run_id} — {ceremony}. Anything else you type \
                         amends this draft first.",
                        facts.contract_id
                    ),
                    None => format!(
                        "/authorize {} starts a run in a store of its own, and that run is \
                         identified once its store is addressed — {ceremony}. Anything else you \
                         type amends this draft first.",
                        facts.contract_id
                    ),
                });
                self.model.reply(statement.join(" · "));
                // The draft is judged as one contract, so the version an amendment replaced
                // leaves the screen with it.
                if let Some(replaced) = self
                    .drafted
                    .replace(prepared.contract_id().to_owned())
                    .filter(|replaced| replaced != prepared.contract_id())
                {
                    self.model.forget_contract(&replaced);
                    self.contracts
                        .retain(|contract| contract.contract_id() != replaced);
                }
                self.model.record_contract(facts);
                self.contracts
                    .retain(|contract| contract.contract_id() != prepared.contract_id());
                self.contracts.push(prepared);
            }
            Err(error) => {
                self.model.error(format!(
                    "{error}. Nothing was recorded. Amend the draft or state the work again."
                ));
            }
        }
    }

    /// Commit the operator's authorization: store the named contract and start its run.
    ///
    /// The contract is looked up by the identifier the decision surface required the operator to
    /// type, so a package named on the command line and a request typed here start identically.
    pub fn start_run(&mut self, contract_id: &str) {
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
        // A profile the operator named is the profile this run uses. One that is not ready stops
        // the run here, before anything is stored, rather than being replaced by whichever other
        // profile happens to be installed.
        if let Some(named) = self.route
            && let Routing::Refused(reason) = attempt::resolve(Some(named), self.runtimes.as_ref())
        {
            self.model.error(format!("no run was started — {reason}"));
            return;
        }
        if self.application.is_some() && !self.move_to_a_store_of_its_own() {
            return;
        }
        // The operator has authorized exactly this here: they read the coverage map and typed
        // the contract id. Whether the run then starts is the store's business, so the
        // authorization is recorded before the attempt and a second one asks for one
        // confirmation rather than for the same identifier again.
        self.authorized.insert(authorization_key(&prepared));
        match Application::create_with_contract(&self.data_root, &prepared) {
            Ok((application, outcome)) => {
                self.application = Some(Arc::new(Mutex::new(application)));
                self.draft = None;
                self.drafted = None;
                self.model.reply(format!(
                    "run {} started against contract {} · the journal records the approved \
                     contract at event #{:04}",
                    outcome.event.run_id,
                    prepared.contract_id(),
                    outcome.event.sequence
                ));
                self.refresh();
                // Starting the run spends the store; starting the agent spends the operator's own
                // account with a runtime profile. They are separate authorizations because they
                // grant separate things, so this states which profile would do the work and stops
                // there.
                let run_id = outcome.event.run_id.clone();
                match attempt::resolve(self.route, self.runtimes.as_ref()) {
                    Routing::Ready(route) => self.model.reply(format!(
                        "nothing is being done yet · /attempt {run_id} starts the {} profile on \
                         this run, which is where spending against your own account begins",
                        route.name()
                    )),
                    Routing::Refused(reason) => self.model.reply(format!(
                        "nothing is being done yet, and no profile could start — {reason}"
                    )),
                    Routing::Probing => self.model.reply(format!(
                        "nothing is being done yet · /attempt {run_id} starts the work once this \
                         host has been probed"
                    )),
                }
            }
            Err(error) => {
                // The contract is still there and still unauthorized in the store's eyes, so it
                // is restated with the weight its second authorization carries.
                let mut facts = ContractFacts::from_prepared(&prepared, &self.data_root);
                facts.previously_authorized =
                    self.authorized.contains(&authorization_key(&prepared));
                self.model.record_contract(facts);
                self.model
                    .error(format!("the run was not started: {error}"));
            }
        }
    }

    /// The identifier a run authorized now would carry, when this session can already name the
    /// store that would hold it.
    ///
    /// A run is identified by its contract and by its store together. Where the store this
    /// session reads is free to take the run, that store names it and the identifier can be shown
    /// before the authorization. Where the run would go to a store of its own instead, the layout
    /// addresses that store only when the run is started, and until then there is no run to name:
    /// the surface states the contract and says when the identifier is fixed.
    fn run_a_start_would_carry(&self, prepared: &PreparedContract) -> Option<String> {
        (!self.addresses_a_store_of_its_own()).then(|| prepared.run_id_in(&self.data_root))
    }

    /// Whether a run authorized now would be recorded in a store of its own.
    ///
    /// It would, exactly when the run being read has ended, this invocation addressed a root
    /// rather than one exact store, and no attempt is still working — because the layout can then
    /// name the next store, the run left behind is finished, and nothing this session holds is
    /// abandoned by moving on. A run that is still live is one to watch, not one to start another
    /// beside.
    fn addresses_a_store_of_its_own(&self) -> bool {
        self.application.is_some()
            && self.root.is_some()
            && self.attempt.is_none()
            && self.model.run().is_some_and(|run| !run.is_live())
    }

    /// Move this session to a store of its own for the run it is about to start.
    ///
    /// A store holds one run. Under a root the product addresses the next store itself, so a
    /// second run is a store away rather than a refusal; the run this session was reading is left
    /// exactly as it stands. An invocation that named one exact store instead of a root has said
    /// which store it acts on, and is told so.
    fn move_to_a_store_of_its_own(&mut self) -> bool {
        let Some(root) = self.root.clone() else {
            self.model.error(
                "this store already holds a run, and this invocation names one exact store rather \
                 than a root — a second run needs its own store",
            );
            return false;
        };
        if self.attempt.is_some() {
            self.model.error(
                "this store holds a run whose attempt is still working — end it before starting \
                 another run",
            );
            return false;
        }
        match store_under(&root, StoreIntent::New) {
            Ok(store) => {
                // The store is what tells this run from the one left behind, so the contracts are
                // restated for the store they can now start into rather than for the one read.
                let carried: Vec<ContractFacts> = self
                    .contracts
                    .iter()
                    .map(|contract| ContractFacts::from_prepared(contract, &store))
                    .collect();
                self.application = None;
                self.data_root = store;
                self.model = Model::cold(Environment::detect(&self.data_root), carried);
                self.model.reply(format!(
                    "a store holds one run · the run you authorized is started in {}, and the run \
                     this session was reading is left exactly as it stands",
                    self.data_root.display()
                ));
                true
            }
            Err(error) => {
                self.model.error(format!(
                    "no store could be addressed for a second run: {error}"
                ));
                false
            }
        }
    }

    /// Launch the managed attempt of the open run, at the operator's word.
    ///
    /// This is the action that spends: it starts the agent, gives it a private copy of the source
    /// and the contract's prompt, and records everything it does. Where no profile can do the
    /// work, nothing is launched and the reason names every profile and what the probe reported.
    pub fn start_attempt(&mut self) {
        if self.attempt.is_some() {
            self.model
                .error("this run already has an attempt working — nothing was launched");
            return;
        }
        match attempt::resolve(self.route, self.runtimes.as_ref()) {
            Routing::Ready(route) => self.launch_attempt(route),
            Routing::Refused(reason) => self
                .model
                .error(format!("no attempt was launched — {reason}")),
            Routing::Probing => self.model.error(
                "no attempt was launched — the runtime profiles of this host have not been probed \
                 yet",
            ),
        }
    }

    fn launch_attempt(&mut self, route: Route) {
        let Some(application) = self.application.clone() else {
            self.model
                .error("no attempt was launched — this store holds no run to attempt");
            return;
        };
        match attempt::start(application, route, &self.registry) {
            Ok(handle) => {
                self.model.reply(format!(
                    "attempt {} launched on the {} profile · the workspace is a private copy of \
                     the source the contract names · every launch, output and tool call is \
                     recorded under runtime-evidence",
                    handle.attempt_id(),
                    route.name()
                ));
                self.model
                    .working(Some(format!("{} is doing the work", route.name())));
                self.attempt = Some(handle);
                self.refresh();
            }
            Err(error) => {
                self.model
                    .error(format!("no attempt was launched — {error:#}"));
                self.refresh();
            }
        }
    }

    /// Take whatever the managed attempt has produced since the last look.
    ///
    /// The verification a committed candidate makes possible is handed back rather than run here:
    /// it runs a program of the operator's choosing and waits for it, which on the drawing thread
    /// would stop every redraw for as long as that program runs.
    pub fn poll_attempt(&mut self) -> AttemptProgress {
        let Some(handle) = self.attempt.as_ref() else {
            return AttemptProgress::Idle;
        };
        let profile = profile_label(handle.runtime_kind());
        let mut notes = Vec::new();
        let mut candidate = None;
        let mut failure = None;
        let mut finished = handle.is_finished();
        while let Some(event) = handle.try_next() {
            match event {
                ManagedRunEvent::Runtime(event) => {
                    if let Some(note) = runtime_note(&event.event) {
                        notes.push(note);
                    }
                }
                ManagedRunEvent::CandidateAvailable {
                    candidate_digest, ..
                } => candidate = Some(candidate_digest),
                ManagedRunEvent::Failed { detail } => failure = Some(detail),
                ManagedRunEvent::Finished => finished = true,
            }
        }

        let advanced = !notes.is_empty() || candidate.is_some() || failure.is_some();
        for note in notes {
            self.model.runtime(profile, note);
        }
        if let Some(detail) = failure {
            // The machinery failed. The journal already carries the terminal the controller
            // recorded; what is added here is the detail, so the condition reads as its own kind
            // rather than as anything about the candidate.
            self.model.error(format!(
                "the managed attempt did not complete — {detail}. This is an infrastructure \
                 condition: nothing was established about any candidate."
            ));
        }
        if advanced || finished {
            self.refresh();
        }
        if let Some(candidate_digest) = candidate {
            return match self.begin_verification(&candidate_digest) {
                Some(pending) => AttemptProgress::Verify(Box::new(pending)),
                None => AttemptProgress::Advanced,
            };
        }
        if finished {
            // The attempt is over and committed no candidate, so it owes the kernel no verdict.
            self.attempt = None;
            self.model.working(None);
            self.report_terminal();
            return AttemptProgress::Advanced;
        }
        if advanced {
            AttemptProgress::Advanced
        } else {
            AttemptProgress::Idle
        }
    }

    /// Everything the committed candidate has to be judged by, read from the approved contract.
    fn begin_verification(&mut self, candidate_digest: &str) -> Option<PendingVerification> {
        let application = self.application.clone()?;
        let staging = attempt::verification_inputs(&self.data_root);
        let job = Self::writer(&application).verification_job(&staging);
        match job {
            Ok(job) => {
                self.verifying = self.verifying.wrapping_add(1);
                self.model.reply(format!(
                    "candidate {} published — asking the verifier the contract names to decide it",
                    crate::projection::short_digest(candidate_digest)
                ));
                self.model.working(Some(job.waiting_for()));
                Some(PendingVerification {
                    generation: self.verifying,
                    job,
                })
            }
            Err(error) => {
                self.fail_infrastructure(format!(
                    "the candidate could not be prepared for verification: {error}"
                ));
                None
            }
        }
    }

    /// Take what one verification decided.
    pub fn finish_verification(&mut self, report: VerificationReport) {
        if report.generation != self.verifying {
            return;
        }
        self.model.working(None);
        let Some(application) = self.application.clone() else {
            return;
        };
        let verdict = match &report.outcome {
            VerificationOutcome::Judged(evidence) => {
                if evidence.decision() == VerificationDecision::Accept {
                    Verdict::Passed
                } else {
                    Verdict::Failed
                }
            }
            VerificationOutcome::Undecided { .. } => Verdict::InfrastructureError,
        };
        if let VerificationOutcome::Undecided { reason } = &report.outcome {
            self.model.error(format!(
                "{reason}. This is an infrastructure condition: the candidate was neither \
                 accepted nor rejected."
            ));
        }
        let command_id = format!(
            "ymp.tui.verify.{}",
            crate::projection::short_digest(&report.candidate_digest)
        );
        let recorded =
            Self::writer(&application).record_verification_outcome(command_id, report.outcome);
        if let Err(error) = recorded {
            self.model
                .error(format!("the verdict was not recorded: {error}"));
        }
        // The kernel record of this run's process slice carries what the protected query decided,
        // so the work obligation closes where it was opened rather than staying open on a
        // candidate the journal has already judged.
        if let Some(handle) = self.attempt.as_ref()
            && let Err(error) = handle.verified(&report.candidate_digest, verdict)
        {
            self.model.error(format!(
                "the kernel record of this attempt did not take the verdict: {error}"
            ));
        }
        self.attempt = None;
        self.refresh();
        self.exhaust_when_nothing_is_left();
        self.report_terminal();
    }

    /// Drive the managed attempt to its terminal outcome without returning.
    ///
    /// A caller with nothing else to do — a command, whose process is the wait — settles the
    /// attempt on its own thread. The interface has a screen to keep drawing, so it uses
    /// [`Self::poll_attempt`] and [`Self::finish_verification`] instead: the same two steps,
    /// scheduled rather than run inline.
    pub fn settle_attempt(&mut self) {
        while self.attempt.is_some() {
            match self.poll_attempt() {
                AttemptProgress::Verify(pending) => {
                    let report = pending.run();
                    self.finish_verification(report);
                }
                AttemptProgress::Advanced => {}
                AttemptProgress::Idle => thread::sleep(ATTEMPT_TICK),
            }
        }
    }

    /// A rejected candidate leaves the run live only while its budget could fund another attempt.
    ///
    /// Where it could not, the next attempt is asked for and the domain answers with the
    /// exhaustion it actually reached; nothing is started by asking. Where it could, the run stays
    /// live: this release does one attempt per run, and says so rather than ending a run whose
    /// budget still holds.
    fn exhaust_when_nothing_is_left(&mut self) {
        let Some(application) = self.application.clone() else {
            return;
        };
        let (running, attempts, sequence) = {
            let application = Self::writer(&application);
            (
                application.state().status == RunStatus::Running,
                application.state().budget.attempts_remaining,
                application.state().last_sequence,
            )
        };
        if !running {
            return;
        }
        if attempts > 0 {
            self.model.reply(format!(
                "the candidate was not accepted · {attempts} attempt(s) of this run's budget are \
                 unspent, and this release does one attempt per run — cancel the run, or start \
                 another run from an amended request"
            ));
            return;
        }
        let outcome = {
            let mut application = Self::writer(&application);
            application.execute(
                format!("ymp.tui.attempt.{}", sequence.saturating_add(1)),
                DomainCommand::StartAttempt {
                    attempt_id: format!("attempt-after-{}", sequence.saturating_add(1)),
                },
            )
        };
        if let Err(error) = outcome {
            self.model.error(format!(
                "the run's remaining budget could not be read: {error}"
            ));
        }
        self.refresh();
    }

    /// Record an infrastructure condition against the open run.
    fn fail_infrastructure(&mut self, reason: String) {
        let Some(application) = self.application.clone() else {
            return;
        };
        let outcome = {
            let mut application = Self::writer(&application);
            let sequence = application.state().last_sequence.saturating_add(1);
            application.execute(
                format!("ymp.tui.infrastructure.{sequence}"),
                DomainCommand::FailInfrastructure {
                    reason: reason.chars().take(ymp_domain::MAX_REASON_BYTES).collect(),
                },
            )
        };
        if let Err(error) = outcome {
            self.model
                .error(format!("{reason} · and it was not recorded: {error}"));
        } else {
            self.model.error(format!(
                "{reason}. The run ended with the terminal outcome infrastructure_error."
            ));
        }
        self.attempt = None;
        self.model.working(None);
        self.refresh();
    }

    /// State the terminal the run reached, and what can still be taken out of it.
    fn report_terminal(&mut self) {
        let Some(run) = self.model.run().cloned() else {
            return;
        };
        if run.is_live() {
            return;
        }
        let mut line = format!(
            "run {} ended · {}",
            run.run_id,
            crate::projection::outcome(run.status)
        );
        if let Some(reason) = &run.terminal_reason {
            line.push_str(&format!(" · {reason}"));
        }
        if run.candidate_digest.is_some() {
            line.push_str(
                " · /export writes the exact candidate and the verifier evidence out of this store",
            );
        }
        self.model.reply(line);
    }

    /// Write the run's candidate and the evidence that judged it out of the store.
    ///
    /// What is written is what the store holds: the journal, the run state, the candidate as an
    /// exact tree and its manifest, every verifier evidence object with the environment each was
    /// bound to, and the runtime evidence of the attempt. Nothing is summarised and nothing is
    /// recomputed.
    pub fn export_evidence(&mut self, destination: Option<PathBuf>) {
        let Some(application) = self.application.clone() else {
            self.model
                .error("no run is open in this store — there is nothing to export");
            return;
        };
        let destination = destination.unwrap_or_else(|| self.export_destination());
        let report = Self::writer(&application).export_evidence(&destination);
        match report {
            Ok(report) => self.model.reply(format!(
                "evidence exported to {} · candidate {} · {} verifier evidence object(s) · {} \
                 environment object(s) · {} journal events",
                report.destination.display(),
                crate::projection::short_digest(&report.candidate_digest),
                report.evidence_digests.len(),
                report.environment_digests.len(),
                report.event_count
            )),
            Err(error) => self.model.error(format!("nothing was exported — {error}")),
        }
    }

    /// Where an export of this run is written when the operator names no directory.
    ///
    /// It is beside the project rather than under the root, because an export exists to leave the
    /// root: what it carries has to be readable without this product and without this store. One
    /// directory per run, so an export names the run it came from and never lands on another's.
    pub fn export_destination(&self) -> PathBuf {
        let run = self
            .model
            .run()
            .map_or_else(|| "run".to_owned(), |run| run.run_id.clone());
        self.model
            .environment()
            .project_path
            .join(format!("{EXPORT_PREFIX}{run}"))
    }
}

/// What one look at the managed attempt found.
pub enum AttemptProgress {
    /// Nothing arrived since the last look.
    Idle,
    /// Something arrived and the projection changed.
    Advanced,
    /// The attempt committed a candidate, and this is the verification it made possible.
    Verify(Box<PendingVerification>),
}

/// A verification this session asked for, ready to run wherever the caller decides.
pub struct PendingVerification {
    generation: u64,
    job: VerificationJob,
}

impl PendingVerification {
    /// What the interface says it is waiting for while this runs.
    pub fn waiting_for(&self) -> String {
        self.job.waiting_for()
    }

    /// Run the verifier and label the outcome with the work it belongs to.
    pub fn run(self) -> VerificationReport {
        VerificationReport {
            generation: self.generation,
            candidate_digest: self.job.candidate_digest().to_owned(),
            outcome: self.job.run(),
        }
    }
}

/// The outcome of one verification, with the work it belongs to.
pub struct VerificationReport {
    generation: u64,
    candidate_digest: String,
    outcome: VerificationOutcome,
}

/// The profile name a runtime event is attributed to.
fn profile_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Codex => "codex",
        RuntimeKind::ClaudeCode => "claude-code",
        RuntimeKind::Fake => "fake",
    }
}

/// What one runtime event is worth saying in the conversation.
///
/// Only what the operator is watching for is stated: what the agent said, that it started, that it
/// asked the product for something, and how the invocation ended. The full record of every event
/// is written to the run's runtime evidence, which the export carries out.
fn runtime_note(event: &RuntimeEventKind) -> Option<String> {
    match event {
        RuntimeEventKind::Launch { .. } => None,
        RuntimeEventKind::Started { .. } => Some("started".to_owned()),
        RuntimeEventKind::Output { text } => Some(text.clone()),
        RuntimeEventKind::McpToolCall {
            tool,
            status,
            error,
            ..
        } => Some(match error {
            Some(_) => format!("asked ymp to {tool} — refused ({status})"),
            None => format!("asked ymp to {tool} — {status}"),
        }),
        RuntimeEventKind::Yielded { .. } => Some("yielded — waiting to be woken".to_owned()),
        RuntimeEventKind::Completed { usage } => Some(format!(
            "completed · input {} · output {} tokens",
            usage.input_tokens, usage.output_tokens
        )),
        RuntimeEventKind::Failed { kind, .. } => Some(format!("failed · {kind:?}")),
        RuntimeEventKind::TimedOut { limit_ms, .. } => {
            Some(format!("stopped at its {limit_ms} ms wall limit"))
        }
        RuntimeEventKind::Cancelled { .. } => Some("cancelled".to_owned()),
        RuntimeEventKind::Interrupted => Some("interrupted".to_owned()),
    }
}

/// What a line opening with `runtime` states about the engines.
#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeLine {
    /// Which engine does the work of this run.
    Route(Route),
    /// Whether an engine is admitted at all, and why it is not.
    Enabled {
        engine: Engine,
        enabled: bool,
        reason: Option<String>,
    },
    /// A line that speaks about admitting an engine but names none this build manages. It is a
    /// refusal rather than prose: the operator asked for a decision about an engine, so the name
    /// that selects none is named back instead of being answered as a sentence.
    UnknownEngine(String),
}

/// A line that names the runtime profile this run would use, or changes what the registry admits.
///
/// A choice of profile is the word `runtime` and the name of a profile this product ships, and
/// nothing else. A line that merely opens with the word is a line: `runtime overhead in the parser
/// must be reduced` is work somebody is asking for, and taking it as a failed choice of agent
/// would lose the request and answer a question nobody asked.
///
/// A decision about admission is read more firmly, because `runtime enable` and `runtime disable`
/// name an action on an engine rather than a profile. A name after either verb that selects no
/// engine is refused with the engines this build manages, instead of falling back to prose. The
/// cost is stated: a request whose first three words are `runtime disable <word>` is refused where
/// it would once have been drafted, and the operator is told exactly which names exist. The gain
/// is that a command mirroring these lines exits non-zero on a name that changes nothing, which a
/// line silently taken as prose could never do.
fn runtime_line(text: &str) -> Option<RuntimeLine> {
    let rest = text.trim().strip_prefix("runtime")?;
    if !rest.starts_with([':', '=', ' ']) {
        return None;
    }
    let rest = rest.trim_start_matches([':', '=', ' ']).trim();
    for (word, enabled) in [("enable", true), ("disable", false)] {
        let Some(tail) = rest.strip_prefix(word).filter(|tail| tail.starts_with(' ')) else {
            continue;
        };
        let tail = tail.trim();
        let (name, reason) = tail.split_once(char::is_whitespace).unwrap_or((tail, ""));
        let engine = match Engine::parse(name) {
            Ok(engine) => engine,
            Err(error) => return Some(RuntimeLine::UnknownEngine(error.to_string())),
        };
        let reason = reason.trim();
        return Some(RuntimeLine::Enabled {
            engine,
            enabled,
            reason: (!reason.is_empty()).then(|| reason.to_owned()),
        });
    }
    Route::parse(rest).map(RuntimeLine::Route)
}

/// The contract a started run is bound to, read back from the store it was approved into.
///
/// A store opened later carries no drafted contract, so the coverage map would otherwise have
/// nothing to show for a run that has one. The stored object is the source; when it cannot be
/// read the binding is shown with what the journal alone records and says so.
/// Remove the workspaces of earlier assemblies of the same project, keeping this one.
///
/// Each assembly copies the project, so the copies would otherwise accumulate one per amendment.
/// Only workspaces of the same source are touched, by the name the assembly derived from it, and
/// only under the directory the product writes its own drafts into.
fn discard_earlier_workspaces(drafts: &Path, job: &DraftJob) {
    let prefix = format!("{}-", crate::draft::source_name(job.source()));
    let Ok(entries) = std::fs::read_dir(drafts) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == job.workspace() || !path.is_dir() {
            continue;
        }
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// What a second authorization has to match to be lighter than the first.
///
/// An authorization grants two things: the contract that decides the work, and the budget the
/// run may spend against it. The contract digest covers the first. The budget is carried beside
/// the contract rather than inside it, so a draft amended to spend more keeps the digest it had;
/// naming the budget here is what keeps that amendment from inheriting an authorization given
/// for a smaller one.
fn authorization_key(prepared: &PreparedContract) -> String {
    format!(
        "{}\u{0}attempts {}\u{0}verification queries {}",
        prepared.contract_digest,
        prepared.budget.attempts_remaining,
        prepared.budget.verification_queries_remaining
    )
}

fn first_line(prompt: &str) -> String {
    prompt.lines().next().unwrap_or_default().to_owned()
}

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
    /// Launch the managed attempt of the open run: start the agent and watch it work.
    StartAttempt,
    /// Write the run's candidate and the evidence that judged it out of the store, into the
    /// directory the operator named or the one the product states when they name none.
    ExportEvidence(Option<PathBuf>),
    /// The operator typed prose. It is shown as a local turn and answered honestly: no
    /// participant can receive it until the domain carries messages.
    LocalTurn(String),
    /// Admit an engine, or stop admitting it with a stated reason. The decision is the registry's
    /// and is durable; nothing about a run changes here.
    SetEngineEnabled {
        engine: Engine,
        enabled: bool,
        reason: Option<String>,
    },
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
    /// A verification this session asked for has decided.
    Verified(Box<VerificationReport>),
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
    spawn_probe_thread(tx.clone(), session.registry_address().clone());
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

        // A managed attempt reports on its own schedule and wakes nothing, so the loop looks for
        // its output rather than waiting to be told. With no attempt working there is nothing to
        // look for and the loop goes back to waiting.
        let wait = if session.attempt_is_live() {
            dirty |= advance_attempt(session, app, &tx);
            ATTEMPT_TICK
        } else {
            IDLE
        };
        if dirty {
            guard
                .terminal()
                .draw(|frame| ui::render(frame, app, &markers))?;
            dirty = false;
        }

        match rx.recv_timeout(wait) {
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
                        AppEvent::Verified(report) => {
                            session.finish_verification(*report);
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

/// Take whatever the managed attempt produced, and schedule the verification it made possible.
///
/// Returns whether anything changed, so a loop that finds nothing does not redraw.
fn advance_attempt(session: &mut Session, app: &mut App, tx: &Sender<AppEvent>) -> bool {
    match session.poll_attempt() {
        AttemptProgress::Idle => false,
        AttemptProgress::Advanced => {
            adopt(session, app);
            true
        }
        AttemptProgress::Verify(pending) => {
            app.working_ticks = 0;
            adopt(session, app);
            spawn_verification_thread(tx.clone(), *pending);
            true
        }
    }
}

fn perform(session: &mut Session, app: &mut App, action: Action, tx: &Sender<AppEvent>) {
    match action {
        Action::CancelRun => {
            session.cancel_run();
            adopt(session, app);
        }
        Action::SetEngineEnabled {
            engine,
            enabled,
            reason,
        } => {
            session.set_engine_enabled(engine, enabled, reason);
            adopt(session, app);
        }
        Action::StartAttempt => {
            session.start_attempt();
            adopt(session, app);
        }
        Action::ExportEvidence(destination) => {
            session.export_evidence(destination);
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
        // The runtimes page is where an engine is admitted or held back, so the decision is taken
        // on the row that states it. Disabling from here records no reason of its own; the reason
        // an operator wants recorded is stated on the `runtime disable <engine> <reason>` line.
        KeyCode::Enter if kind == PageKind::Runtimes => {
            let report = app.data.runtimes.as_ref()?;
            let row = app.selection_of(kind);
            let engine = report.engine_at(row)?;
            let enabled = report
                .profiles
                .get(row)
                .is_some_and(|profile| profile.admitted());
            return Some(Action::SetEngineEnabled {
                engine,
                enabled: !enabled,
                reason: enabled.then(|| "disabled from the runtimes page".to_owned()),
            });
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
                Some(Command::StartAttempt) => app.open_attempt_confirm(),
                Some(Command::CancelRun) => app.open_cancel_confirm(),
                // Writing what the store already holds into a directory of its own takes nothing
                // back and spends nothing, so it asks for no confirmation.
                Some(Command::Export) => {
                    app.resume_live();
                    return Some(Action::ExportEvidence(None));
                }
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
                ConfirmAction::StartAttempt { .. } => Action::StartAttempt,
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

/// A verification runs a program of the operator's choosing and waits for it, so it never runs on
/// the thread that draws; its outcome returns as an event like any other.
fn spawn_verification_thread(tx: Sender<AppEvent>, pending: PendingVerification) {
    thread::spawn(move || {
        let report = pending.run();
        let _ = tx.send(AppEvent::Verified(Box::new(report)));
    });
}

/// Reading the registry and probing the engines it admits starts subprocesses, so it happens once,
/// off the drawing thread. The interface is where an operator looks at the engines, so this pass
/// measures the model catalog of an engine whose recorded list is not the one its installed build
/// serves.
fn spawn_probe_thread(tx: Sender<AppEvent>, registry: RegistryAddress) {
    thread::spawn(move || {
        let report = crate::runtimes::probe_all(&registry, crate::runtimes::Measure::Catalog);
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
