//! The session, the event loop and key handling.
//!
//! [`Session`] owns the application handle and the read model; the event loop owns the terminal
//! and the view state. Key handling is a pure function over the view state that returns the
//! [`Action`] the loop must execute, so every keyboard transition can be driven in a test
//! without a terminal or a store.
//!
//! Input is read by a dedicated thread; journal notifications, the runtime probe, the outcome of a
//! verification and the outcome of a provider measurement arrive on the same channel. The main
//! thread blocks until something happens.
//! The one exception is a working managed attempt, which reports through a channel of its own that
//! wakes nothing: while one is working the loop looks for its output on a short tick, and with no
//! attempt working it goes back to waiting.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ymp_application::root::{StoreIntent, store_under};
use ymp_application::{
    Application, ApplicationError, OriginStartRequest, ParticipantHost, ParticipantRuntimes,
    PreparedContract, VerificationJob, VerificationOutcome, freeze_under,
    ignite_origin_participant, prepare_contract,
};
use ymp_domain::Command as DomainCommand;
use ymp_domain::commitment::Verdict;
use ymp_domain::contract::ContractDocument;
use ymp_domain::participant::ParticipantState;
use ymp_domain::{RunStatus, VerificationDecision};
use ymp_runtime_api::{CancellationToken, RuntimeEventKind, RuntimeKind};
use ymp_runtime_supervisor::{
    CONTROLLER_SHUTDOWN_LIMIT, ManagedRunEvent, ManagedRunHandle, ManagedShutdown,
};

use crate::attempt::{self, Route, Routing};
use crate::decisions;
use crate::draft::{Amendment, Assembly, Draft, DraftJob};
use crate::engines;
use crate::journal::Model;
use crate::origin::{ManagedRuntimes, PrivateGit};
use crate::pages::Page;
use crate::pools::{self, PoolCapacity, PoolEntry, PoolName};
use crate::projection::{ContractFacts, Environment, Projection};
use crate::providers;
use crate::runtimes::Report;
use crate::state::{
    App, COMMAND_PREFIX, Command, ConfirmAction, Modal, PageKind, PaletteItem, Surface,
};
use crate::terminal::TerminalGuard;
use crate::theme::Markers;
use crate::ui;
use ymp_runtime_registry::{Engine, ProviderFamily, RegistryAddress};

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
/// How long a quit waits for the measurement worker to come back out of the engines it started,
/// once those engines have been ended. The driver returns as soon as the process it is reading is
/// gone, so this is the time the worker needs to notice, not the time an engine needs to answer.
const WORKER_RETURN_LIMIT: Duration = Duration::from_secs(1);
/// What a quit during a provider measurement is bounded by: the wait for the worker to finish of
/// its own accord, the request to the engines to end, the ending of what did not, and the wait for
/// the worker to return from them. Every part is enforced by a deadline of its own, so the sum is a
/// bound and not an expectation.
pub const MEASUREMENT_SHUTDOWN_LIMIT: Duration = CONTROLLER_SHUTDOWN_LIMIT
    .saturating_add(engines::REQUEST_LIMIT)
    .saturating_add(engines::ENFORCEMENT_LIMIT)
    .saturating_add(WORKER_RETURN_LIMIT);
/// Where under the data root the product writes what it supplies for a draft: the verifier it
/// proposes, the copy it takes as the negative control, and the sample that verifier must accept.
const DRAFT_DIRECTORY: &str = "draft";
/// The state a goal is answered with while no provider is enabled.
///
/// It is the plain-words state of the product brief (Part A §6) and not a technical refusal: the
/// goal is taken and held, the one thing that is missing is named, and the command that supplies it
/// is named with it. The sentence `no providers configured` is what this replaces.
const A_GOAL_NEEDS_A_PROVIDER: &str = "your goal is held · running it needs at least one enabled AI provider — /providers is where \
     one is enabled · nothing has started and nothing has left this host";
/// The state a goal is answered with where this host offers models and holds no pool to draw them
/// from.
///
/// It should not occur: the pools are resolved wherever a provider is observed, so the pool that
/// permits the catalog exists from the first measurement onwards. It is stated all the same,
/// because a state nothing explains is worse than a state nobody expected, and it names the act
/// that resolves the pools rather than asking the operator to create one — nothing but the
/// controller creates a pool.
const A_GOAL_NEEDS_A_POOL: &str = "your goal is held · this host offers models and holds no pool to draw them from, which is a \
     state it should never reach — /providers · r measures the account again and resolves the \
     pools · nothing has started and nothing has left this host";
/// What an export directory is called when the operator names none. The run identifier follows it.
const EXPORT_PREFIX: &str = "ymp-evidence-";
/// Where default evidence exports live inside the run store. A path outside the product root is
/// reached only when the operator names it explicitly.
const EXPORT_DIRECTORY: &str = "exports";

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
    /// The provider level as this root holds it. It is read from the records — never probed — so
    /// holding it costs nothing and opening the table measures nothing.
    providers: providers::Report,
    /// The pool level as this root holds it, read from the records for the same reason. It is
    /// re-read wherever the records are written, so what the surfaces state is what the controller
    /// last resolved rather than a resolution taken behind the operator while they looked at it.
    pools: pools::Report,
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
    /// The measurement running away from the thread that draws, while one is. Every surface of
    /// the provider level reads it, so a row being measured says so.
    measuring: Option<providers::Measuring>,
    /// Which measurement this session is waiting for, under the same rule as a draft check.
    measured: u64,
    /// The worker that measurement runs on. Holding it is what makes the engines it started
    /// reachable when this session goes away: quitting waits for it rather than leaving the
    /// processes it is waiting on behind.
    probe: Option<RunningMeasurement>,
    /// The runtime profiles a frozen route is served with. It is held rather than built where it
    /// is used so that a check can put the in-process fixture runtime here and change nothing
    /// else: what the route is stays the run's own record either way.
    participants: Option<Arc<dyn ParticipantRuntimes + Send + Sync>>,
    /// The participant this run ignited on, while its worker is still running it. Holding it is
    /// what makes that participant reachable when this session goes away.
    origin: Option<RunningOrigin>,
}

/// The origin participant of this run, running on a worker of its own.
///
/// The worker holds the store the same way the managed attempt's worker does — through the shared
/// handle, taking the lock for each thing it records — so the interface goes on drawing the run
/// while the participant works in it. Nothing can hold a worker this session cannot end: it is
/// reached only through [`Session::await_origin`] and [`Session::end_origin`].
struct RunningOrigin {
    cancellation: CancellationToken,
    worker: thread::JoinHandle<()>,
}

/// What ending the origin participant left behind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginShutdown {
    /// This session was running none.
    Nothing,
    /// The participant ended and its worker left, so nothing of it outlived this.
    Ended,
    /// The worker did not finish within the limit and keeps the session it owns.
    LeftRunning,
}

/// A session that goes away takes the participant it started with it.
///
/// The store is held by that participant's worker as well as by this session, so a session dropped
/// while one is working would leave a writer nothing could reach and a process nobody would end.
/// Waiting here is what makes the end of a session the end of the work it authorized.
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.end_origin();
    }
}

/// The rows the operator is standing on, for the surfaces whose views exist only while one is.
///
/// A properties view is not a value the caller supplies but a position it reports: naming the
/// three positions together is what keeps a caller from handing the pool's row to the provider's
/// view, which three bare positions in a row would invite.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selected {
    /// Which candidate the describe view is showing.
    pub candidate: Option<usize>,
    /// Which row of the provider table its properties view is showing.
    pub provider: Option<usize>,
    /// Which row of the pool table its properties view is showing.
    pub pool: Option<usize>,
}

impl Selected {
    /// Standing on no row of any of them, which is where a caller that only wants the pages is.
    pub fn none() -> Self {
        Self::default()
    }
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

/// A measurement this session asked for, ready to be run wherever the caller decides.
///
/// It holds no reference to the session, so the interface can move it to another thread and hand
/// the outcome back through [`Session::finish_measurement`]. What it does is start the engines of
/// the accounts it names and read what they serve. Recording the observation is not part of it:
/// that is a write against this root's records, and it belongs where the outcome is taken.
#[derive(Clone, Debug)]
pub struct PendingMeasurement {
    generation: u64,
    address: RegistryAddress,
    /// The engines this measurement starts: those of the accounts that were enabled when it was
    /// asked for, and no others.
    engines: Vec<Engine>,
    /// The accounts the observation is recorded for, which are the accounts those engines reach.
    families: Vec<ProviderFamily>,
}

impl PendingMeasurement {
    /// Start the engines and read what they serve, labelling the outcome with the work it belongs
    /// to.
    pub fn run(self) -> MeasurementOutcome {
        let report = crate::runtimes::probe_engines(
            &self.address,
            crate::runtimes::Measure::Catalog,
            &self.engines,
        );
        MeasurementOutcome {
            generation: self.generation,
            report,
            // The moment the engines answered, which is the moment the observation records: an
            // observation dated when the outcome was drawn would date the measurement by how
            // busy the interface was.
            taken_at: SystemTime::now(),
            families: self.families,
        }
    }
}

/// The outcome of one measurement, with the work it belongs to.
#[derive(Clone, Debug)]
pub struct MeasurementOutcome {
    generation: u64,
    report: Report,
    taken_at: SystemTime,
    families: Vec<ProviderFamily>,
}

/// A measurement running on a worker of its own, from its launch until that worker ends.
///
/// It is reached only through the session that started it: [`Session::start_measurement`] launches
/// one and [`Session::end_measurement`] ends it, so nothing can hold a worker this session cannot
/// wait for on its way out.
struct RunningMeasurement {
    /// Closed by the worker as it leaves. Nothing is ever sent through it, so a wait on it ends
    /// exactly when the worker does and at no other moment.
    ended: mpsc::Receiver<std::convert::Infallible>,
    worker: thread::JoinHandle<()>,
}

impl RunningMeasurement {
    /// Start one measurement on a worker, and hand its outcome to `deliver` from that worker.
    fn start(
        pending: PendingMeasurement,
        deliver: impl FnOnce(MeasurementOutcome) + Send + 'static,
    ) -> Self {
        let (open, ended) = mpsc::channel();
        // Which processes belong to this measurement is recorded from here on, while the process
        // table still says so. A process handed to another parent when the one that started it ends
        // carries nothing a later reading could attribute, and that is the process a quit most needs
        // to reach.
        engines::observe();
        let worker = thread::spawn(move || {
            let outcome = pending.run();
            deliver(outcome);
            // Closing this is what says the engines have answered and the worker is leaving.
            drop(open);
        });
        Self { ended, worker }
    }

    /// Wait for the worker, and end what it is inside where that wait runs out.
    ///
    /// The worker is inside the engines it started, so waiting for it is waiting for those
    /// processes. An engine that answers inside the limit a managed run's shutdown carries is
    /// waited out and the worker is joined. One that answers later than that cannot be waited out
    /// without leaving the shutdown unbounded, and returning at the limit would end the interface
    /// while a process it started kept running — so past the limit the engines are ended and their
    /// absence is read from the operating system. Nothing is claimed that was not established:
    /// where the ending could not be observed, the report says what is still running.
    fn end(self) -> MeasurementShutdown {
        match self.ended.recv_timeout(CONTROLLER_SHUTDOWN_LIMIT) {
            Ok(nothing) => match nothing {},
            Err(RecvTimeoutError::Disconnected) => {
                let _ = self.worker.join();
                // The measurement is over, so what was recorded about its processes describes
                // nothing this interface still holds.
                engines::forget();
                MeasurementShutdown::Ended
            }
            Err(RecvTimeoutError::Timeout) => self.terminate(),
        }
    }

    /// End the engines the worker is still inside, then wait for that worker to come back out.
    ///
    /// The worker returns as soon as the engines it was reading are gone, and joining it is what
    /// says nothing of this measurement is still moving. A worker that does not return leaves this
    /// interface unable to state what it is inside, so that is reported rather than passed over,
    /// even where the processes themselves were established to be gone.
    fn terminate(self) -> MeasurementShutdown {
        let Self { ended, worker } = self;
        let termination = engines::end_measurement_processes();
        let returned = matches!(
            ended.recv_timeout(WORKER_RETURN_LIMIT),
            Err(RecvTimeoutError::Disconnected)
        );
        if returned {
            let _ = worker.join();
        }
        engines::forget();
        let waited = CONTROLLER_SHUTDOWN_LIMIT.as_millis();
        match (termination, returned) {
            (engines::Termination::NothingRunning, true) => MeasurementShutdown::Ended,
            (engines::Termination::Ended(processes), true) => {
                MeasurementShutdown::Terminated(format!(
                    "the provider measurement did not end within {waited} ms; the {} engine \
                     process(es) it started were ended and the process table was read back with \
                     none of them left",
                    processes.len()
                ))
            }
            (engines::Termination::Unestablished(reason), _) => {
                MeasurementShutdown::LeftRunning(format!(
                    "the provider measurement did not end within {waited} ms and the engines it \
                     started were not established to be gone: {reason}"
                ))
            }
            (_, false) => MeasurementShutdown::LeftRunning(format!(
                "the provider measurement did not end within {waited} ms; the engines it started \
                 were ended, and its worker had still not returned from them {} ms later, so what \
                 it is inside is not established",
                WORKER_RETURN_LIMIT.as_millis()
            )),
        }
    }
}

/// What ending a running measurement left behind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MeasurementShutdown {
    /// Nothing was running.
    Nothing,
    /// The worker ended and was joined, so no engine this session started is still running.
    Ended,
    /// The worker had not ended when the wait for it ran out, so the engines it was inside were
    /// ended and read back as gone. Nothing this session started is still running; the report says
    /// what the quit had to end, because the measurement it cut short produced no observation.
    Terminated(String),
    /// The worker had not ended when the bound ran out, and what it started was not established to
    /// be gone.
    LeftRunning(String),
}

impl Session {
    /// Open a data root. A root with no committed run is not an error: the interface states
    /// the absence and offers what is possible from there.
    pub fn open(data_root: &Path, contracts: &[PreparedContract]) -> Self {
        Self::open_with_diagnostics(data_root, contracts, false)
    }

    /// Open a store explicitly named by a diagnostic invocation, preserving its path and schema
    /// details when it cannot be read.
    pub fn open_diagnostic(data_root: &Path, contracts: &[PreparedContract]) -> Self {
        Self::open_with_diagnostics(data_root, contracts, true)
    }

    fn open_with_diagnostics(
        data_root: &Path,
        contracts: &[PreparedContract],
        diagnostic: bool,
    ) -> Self {
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
                    match diagnostic {
                        true => model.error(format!(
                            "saved state at {} cannot be read: {error}. Nothing in it was changed.",
                            data_root.display()
                        )),
                        false => model.error(
                            "ymp cannot read its saved state. Nothing in it was changed. Update ymp \
                             or restore compatible saved data before continuing.",
                        ),
                    }
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
        session.read_providers();
        session.read_pools();
        session
    }

    /// Open a store under an explicitly named diagnostic root.
    pub fn open_under_root_diagnostic(
        root: &Path,
        data_root: &Path,
        contracts: &[PreparedContract],
    ) -> Self {
        let mut session = Self::open_diagnostic(data_root, contracts);
        session.root = Some(root.to_path_buf());
        session.registry = RegistryAddress::Root(root.to_path_buf());
        session.read_providers();
        session.read_pools();
        session
    }

    fn over(
        application: Option<Application>,
        data_root: &Path,
        model: Model,
        contracts: &[PreparedContract],
    ) -> Self {
        let registry = RegistryAddress::Store(data_root.to_path_buf());
        let providers = providers::read(&registry, SystemTime::now());
        let pools = pools::read(&registry);
        Self {
            application: application.map(|application| Arc::new(Mutex::new(application))),
            root: None,
            data_root: data_root.to_path_buf(),
            registry,
            model,
            runtimes: None,
            providers,
            pools,
            draft: None,
            contracts: contracts.to_vec(),
            checking: 0,
            authorized: BTreeSet::new(),
            drafted: None,
            route: None,
            attempt: None,
            verifying: 0,
            measuring: None,
            measured: 0,
            probe: None,
            participants: None,
            origin: None,
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
        self.read_providers();
        self.read_pools();
        self
    }

    /// Re-read the provider level from the records. Starts nothing and writes nothing.
    pub fn read_providers(&mut self) {
        self.providers = providers::read(&self.registry, SystemTime::now());
    }

    /// Re-read the pool level from the records. Starts nothing, writes nothing and resolves
    /// nothing: what it states is what the controller last wrote.
    pub fn read_pools(&mut self) {
        self.pools = pools::read(&self.registry);
    }

    /// Resolve every pool of this root against the catalog as it now stands, and create the
    /// `default` pool where the catalog offers an entry and this root holds none.
    ///
    /// This is the pool controller, and it runs wherever a provider is observed: enabling an
    /// account, measuring one again and holding one back all change which entries the catalog
    /// offers, so all three leave the pools resolved against what the catalog now holds. That is
    /// what makes `default` exist from the first measurement onwards — the operator is never asked
    /// to create a pool — and what makes holding the last account back leave that pool empty and
    /// standing rather than deleted.
    ///
    /// It creates no participant, opens no process and spends nothing. A resolution that fails is
    /// stated rather than swallowed: a pool left resolved against an older catalog is a fact the
    /// operator can act on, and a silent failure is not.
    fn reconcile_pools(&mut self) {
        let pools = self.registry.pools();
        if let Err(error) = pools.reconcile(&self.registry.providers(), &self.registry.registry()) {
            self.model.error(format!(
                "the pools were not resolved against the catalog as it now stands — {error} · what \
                 they state is the resolution before this change",
            ));
        }
        self.read_pools();
    }

    /// The engines this session may measure without being asked to: those of the providers the
    /// operator has enabled, and no others.
    pub fn engines_of_enabled_providers(&self) -> Vec<Engine> {
        self.providers.engines_of_enabled_providers()
    }

    /// Take the operator's decision about one provider.
    ///
    /// Enabling is the act that permits this provider to be measured at all, and the consequence
    /// of it was stated on the properties view above the key. It is therefore also the act that
    /// measures: the engines that reach the provider are started, the catalog they serve is
    /// recorded, and the observation is stamped with the moment it was taken. Disabling starts
    /// nothing and erases nothing: the record and its measurements stay readable, and its models
    /// leave the offered catalog.
    ///
    /// The decision is committed here; the measurement it asks for is returned rather than run, so
    /// the caller decides which thread waits for the engines. The interface hands it to a worker;
    /// a command, whose process is the wait, settles it through [`Self::settle_measurement`].
    pub fn set_provider_enabled(
        &mut self,
        family: ProviderFamily,
        enabled: bool,
        reason: Option<String>,
    ) -> Option<PendingMeasurement> {
        let providers = self.registry.providers();
        let recorded_at = providers.path_of(family).display().to_string();
        match providers.set_enabled(family, enabled, reason.as_deref()) {
            Err(error) => {
                self.model.error(format!(
                    "the {} provider was not changed — {error}",
                    family.name()
                ));
                None
            }
            Ok(record) if record.enabled => {
                self.read_providers();
                let pending = self.begin_measurement(family, providers::Act::Enable);
                // What the engines found and what the pools then hold are stated when the
                // measurement lands, because neither is known yet: this reply is the decision and
                // the act it started, and `finish_measurement` states their outcome.
                self.model.reply(format!(
                    "the {} provider is enabled · recorded in {recorded_at} · repository content \
                     of any workspace may now be sent to it · {}",
                    family.name(),
                    self.measuring_clause()
                ));
                pending
            }
            Ok(record) => {
                self.read_providers();
                self.withdraw_routing_of(family);
                self.reconcile_pools();
                self.model.reply(format!(
                    "the {} provider is disabled — {} · recorded in {recorded_at} · its models are \
                     not offered and reach no later pool · nothing it measured was erased · {}",
                    family.name(),
                    record.display_reason(),
                    self.pools_after_the_change()
                ));
                None
            }
        }
    }

    /// Measure one provider again at the operator's word.
    ///
    /// A provider that is not enabled is not measured, and asking for it is answered with that
    /// rather than with a measurement: the enable transition is where a provider is first reached,
    /// and refusing here is what keeps it the only place.
    pub fn refresh_provider_models(
        &mut self,
        family: ProviderFamily,
    ) -> Option<PendingMeasurement> {
        let enabled = self
            .providers
            .provider(family)
            .is_some_and(|provider| provider.enabled());
        if !enabled {
            self.model.error(format!(
                "nothing was measured — the {} provider is not enabled, and nothing about a \
                 provider is measured before it is enabled",
                family.name()
            ));
            return None;
        }
        let pending = self.begin_measurement(family, providers::Act::Refresh);
        self.model.reply(self.measuring_clause());
        pending
    }

    /// What the pools of this root state after an observation, in one clause.
    ///
    /// It names the pool the operator never created, because an automatic record that appeared
    /// without being announced is a record nobody knows to look at. A root whose catalog offers
    /// nothing holds no pool, and that is stated as the state it is rather than as a failure.
    ///
    /// A pool that exists and cannot be read is neither of those, and it is stated as itself. The
    /// reading answers a failure with no pools, so a reply that read that emptiness as a state
    /// would tell an operator whose catalog offers two models that there is nothing to draw from —
    /// which is the very contradiction the `/pools` page avoids by naming the failure.
    fn pools_after_the_change(&self) -> String {
        if let Some(error) = &self.pools.error {
            return format!(
                "the pools could not be read in full, so what they now hold is not stated — {error}"
            );
        }
        match self.pools.pools.as_slice() {
            [] => "no pool stands under this root: the catalog offers no entry to draw from"
                .to_owned(),
            pools => format!(
                "pools · {}",
                pools
                    .iter()
                    .map(|pool| format!(
                        "{} {} · {} of {} offered",
                        pool.name(),
                        pool.state().label(),
                        pool.admissible(),
                        pool.record.resolved.entries.len()
                    ))
                    .collect::<Vec<_>>()
                    .join(" · ")
            ),
        }
    }

    /// Take back what this session measured about the engines of one account, without starting
    /// anything.
    ///
    /// Holding an account back is not a measurement, so nothing is started here. What must not
    /// survive it is the readiness this session measured while the account was enabled: a profile
    /// left `ready` after its account was held back would still be offered as the route of the
    /// next run, and the operator's decision would reach the records and not the session they took
    /// it in. The engines of the accounts they did not touch keep exactly what was measured about
    /// them, because nothing about those accounts changed.
    fn withdraw_routing_of(&mut self, family: ProviderFamily) {
        let withdrawn = family.engines();
        let read = crate::runtimes::read_all(&self.registry);
        match self.runtimes.as_mut() {
            None => self.runtimes = Some(read),
            Some(report) => {
                for profile in &mut report.profiles {
                    let Some(engine) = profile.registry.as_ref().map(|facts| facts.engine) else {
                        continue;
                    };
                    if !withdrawn.contains(&engine) {
                        continue;
                    }
                    if let Some(fresh) = read
                        .profiles
                        .iter()
                        .find(|fresh| fresh.registry.as_ref().is_some_and(|f| f.engine == engine))
                    {
                        *profile = fresh.clone();
                    }
                }
            }
        }
    }

    /// Ask for the engines of every enabled provider to be started, and for what they serve to be
    /// recorded.
    ///
    /// The engines of a provider nobody enabled are not started: they are read from their records,
    /// which is what makes the enable transition the only moment this host reaches an account.
    ///
    /// It is public because the engines page measures through it too: that page is the level
    /// beneath the providers, and measuring it on a root where nothing is enabled would start the
    /// very engines the provider level exists to hold back.
    ///
    /// A second ask arriving while one is running is folded into it rather than queued behind it:
    /// the running measurement starts the engines of every enabled account, so a second one would
    /// start the same engines again to learn the same thing. The surfaces state that, and how many
    /// asks the running measurement answered.
    pub fn measure_enabled_providers(&mut self) -> Option<PendingMeasurement> {
        if let Some(running) = self.measuring.as_mut() {
            running.folded = running.folded.saturating_add(1);
            return None;
        }
        // The reading is taken whether or not anything is measured by it: with no provider
        // enabled it holds one row per engine, read from the records alone, so a surface states
        // what this root knows instead of reporting a measurement that is not running.
        let families: Vec<ProviderFamily> = self
            .providers
            .providers
            .iter()
            .filter(|provider| provider.enabled())
            .map(|provider| provider.family)
            .collect();
        self.measured = self.measured.wrapping_add(1);
        Some(PendingMeasurement {
            generation: self.measured,
            address: self.registry.clone(),
            engines: self.engines_of_enabled_providers(),
            families,
        })
    }

    /// Take the measurement one act on one account asks for, and state it on every surface that
    /// draws that account while it runs.
    fn begin_measurement(
        &mut self,
        asked_by: ProviderFamily,
        act: providers::Act,
    ) -> Option<PendingMeasurement> {
        let pending = self.measure_enabled_providers()?;
        let measuring = providers::Measuring {
            asked_by,
            act,
            families: pending.families.clone(),
            folded: 0,
        };
        self.measuring = Some(measuring);
        Some(pending)
    }

    /// Take the outcome of a measurement this session asked for.
    ///
    /// This is where the observation is recorded and where every surface is re-read: the engines
    /// answered on a worker, and what this root now holds about the accounts they reach is written
    /// and read back here, on the one thread that draws.
    ///
    /// An outcome from a measurement that has been superseded decides nothing, under the same rule
    /// a draft check follows.
    pub fn finish_measurement(&mut self, outcome: MeasurementOutcome) {
        if outcome.generation != self.measured {
            return;
        }
        let measuring = self.measuring.take();
        self.runtimes = Some(outcome.report);
        let providers = self.registry.providers();
        let registry = self.registry.registry();
        for family in &outcome.families {
            if let Err(error) = providers.observe_family(*family, &registry, outcome.taken_at) {
                self.model.error(format!(
                    "the {} provider was measured and the observation was not recorded — {error}",
                    family.name()
                ));
            }
        }
        self.read_providers();
        // A decision the operator took while the engines were answering outranks what they
        // answered. This report was read before that decision, so applying it whole would put
        // back the readiness of an account that has since been held back — and the routing reads
        // that readiness, so the next run could be sent to the very account whose permission to
        // receive repository content was withdrawn. The engines of an account whose permission
        // moved under the measurement are therefore re-read from the records here, exactly as
        // holding one back does when nothing is running.
        for family in self.withdrawn_during(&outcome.families) {
            self.withdraw_routing_of(family);
        }
        // The observation is what the catalog is derived from, so the pools are resolved against
        // it here rather than at the next surface that happens to read them: a pool is what a run
        // may recruit from, and it must state the catalog this host has now. This is the one place
        // an observation lands, so it is the one place the pools are resolved for one — whether
        // the interface delivered it from a worker or a command settled it on its own thread.
        self.reconcile_pools();
        let Some(measuring) = measuring else {
            return;
        };
        let mut stated = format!(
            "the {} provider was measured · {} · {}",
            measuring.asked_by.name(),
            self.measurement_of(measuring.asked_by),
            // Stated after the resolution above, so what the pools hold is what this measurement
            // left them holding rather than what they held before it.
            self.pools_after_the_change()
        );
        if let Some(folded) = measuring.folded_note() {
            stated.push_str(&format!(" · {folded}"));
        }
        // An account enabled while the engines were already being started was never reached by
        // them, so nothing is claimed about it: it is named, with the key that measures it.
        for family in self.enabled_but_unmeasured(&outcome.families) {
            stated.push_str(&format!(
                " · the {} account was enabled while this measurement was running, so it was not \
                 part of it — r on its card measures it",
                family.name()
            ));
        }
        self.model.reply(stated);
    }

    /// Settle a measurement on the thread that asked for it.
    ///
    /// A caller with nothing else to do — a command, whose process is the wait — measures here.
    /// The interface has a screen to keep drawing, so it uses [`Self::start_measurement`] and
    /// [`Self::finish_measurement`] instead: the same two steps, scheduled rather than run inline.
    pub fn settle_measurement(&mut self, pending: Option<PendingMeasurement>) {
        if let Some(pending) = pending {
            let outcome = pending.run();
            self.finish_measurement(outcome);
        }
    }

    /// Start a measurement on a worker of its own and hold it until its outcome is taken.
    ///
    /// `deliver` hands the outcome back to whatever drives this session; the interface sends it
    /// through the same channel every other event arrives on, so the outcome is taken by the
    /// thread that draws and by no other.
    pub fn start_measurement(
        &mut self,
        pending: PendingMeasurement,
        deliver: impl FnOnce(MeasurementOutcome) + Send + 'static,
    ) {
        self.probe = Some(RunningMeasurement::start(pending, deliver));
    }

    /// Whether a measurement this session asked for is still running.
    pub fn is_measuring(&self) -> bool {
        self.measuring.is_some()
    }

    /// The measurement running away from the thread that draws, while one is.
    pub fn measuring(&self) -> Option<&providers::Measuring> {
        self.measuring.as_ref()
    }

    /// End a running measurement before this session goes away.
    ///
    /// The worker is waiting on the engines it started, so waiting for the worker is waiting for
    /// those processes to end — which is what keeps a quit from leaving one behind. That wait is
    /// bounded by the limit a managed run's shutdown carries; an engine that answers later than
    /// the limit is ended instead, so the whole act is bounded by [`MEASUREMENT_SHUTDOWN_LIMIT`]
    /// and leaves nothing running either way. What could not be established is stated rather than
    /// reported as an ending nobody observed.
    pub fn end_measurement(&mut self) -> MeasurementShutdown {
        self.measuring = None;
        match self.probe.take() {
            None => MeasurementShutdown::Nothing,
            Some(probe) => probe.end(),
        }
    }

    /// Which accounts a measurement covered and the operator has held back since it began.
    ///
    /// A measurement carries the accounts that were enabled when it was asked for. One that is no
    /// longer enabled was withdrawn while the engines were answering, and what those engines
    /// reported about it is a reading of an account this host is no longer permitted to reach.
    fn withdrawn_during(&self, measured: &[ProviderFamily]) -> Vec<ProviderFamily> {
        measured
            .iter()
            .copied()
            .filter(|family| {
                !self
                    .providers
                    .provider(*family)
                    .is_some_and(|provider| provider.enabled())
            })
            .collect()
    }

    /// Which accounts are enabled now and were not part of the measurement that has just landed.
    fn enabled_but_unmeasured(&self, measured: &[ProviderFamily]) -> Vec<ProviderFamily> {
        self.providers
            .providers
            .iter()
            .filter(|provider| provider.enabled())
            .map(|provider| provider.family)
            .filter(|family| !measured.contains(family))
            .collect()
    }

    /// What the reply to an act says about the measurement that act asked for.
    fn measuring_clause(&self) -> String {
        match self.measuring.as_ref() {
            Some(measuring) => match measuring.folded {
                0 => measuring.notice(),
                _ => format!(
                    "{} · {}",
                    measuring.notice(),
                    measuring
                        .folded_note()
                        .unwrap_or_else(|| "it answers this ask too".to_owned())
                ),
            },
            None => "nothing is being measured".to_owned(),
        }
    }

    /// What the last measurement of one provider found, in one clause.
    fn measurement_of(&self, family: ProviderFamily) -> String {
        match self.providers.provider(family) {
            None => "nothing was measured about it".to_owned(),
            Some(provider) => {
                let mut stated = format!(
                    "{} · {} model(s) · {} offered",
                    provider.record.display_state(),
                    provider.models,
                    provider.offered
                );
                for route in &provider.routes {
                    if let Some(reason) = &route.without_models {
                        stated.push_str(&format!(
                            " · the {} route serves none: {reason}",
                            route.engine.name()
                        ));
                    }
                }
                stated
            }
        }
    }

    /// How this session addresses the engine registry.
    pub fn registry_address(&self) -> &RegistryAddress {
        &self.registry
    }

    /// The pool level as this session last read it, which is what a key acts on.
    pub fn pools(&self) -> &pools::Report {
        &self.pools
    }

    /// Permit one entry in a pool, or take it out.
    ///
    /// The list this leaves is computed from the record and never from the screen, so an edit
    /// applies to the pool as it stands. It is one entry either way: the operator pressed a key on
    /// the row that names it, and the whole list is never restated.
    ///
    /// **The first edit of a pool that follows the catalog replaces the whole-catalog form.** The
    /// consequence was on the surface above the key, so the reply says what was done rather than
    /// asking whether it was meant: the pool now holds the list the operator left, and a model
    /// measured later joins the catalog and not this pool.
    pub fn set_pool_entry_permitted(
        &mut self,
        pool: PoolName,
        entry: PoolEntry,
        permitted: bool,
    ) -> bool {
        let records = self.registry.pools();
        let record = match records.read(&pool) {
            Err(error) => {
                self.model
                    .error(format!("the {pool} pool was not changed — {error}"));
                return false;
            }
            Ok(None) => {
                self.model.error(format!(
                    "the {pool} pool was not changed — this root holds no pool of that name, and \
                     nothing but the controller creates one"
                ));
                return false;
            }
            Ok(Some(record)) => record,
        };
        let was_tracking = record.resolved.tracking;
        let list = pools::permitted_after(&record, &entry, permitted);
        let outcome = records.edit(
            &pool,
            &self.registry.providers(),
            &self.registry.registry(),
            |declared| {
                declared.models = ymp_runtime_registry::PoolModels::explicit(list);
            },
        );
        match outcome {
            Err(error) => {
                self.model.error(format!(
                    "the {pool} pool was not changed — {error} · it stands exactly as it did"
                ));
                false
            }
            Ok(record) => {
                self.read_pools();
                self.model.reply(format!(
                    "{} · {} · {} of {} entries offered · recorded in {}{}",
                    match permitted {
                        true => format!(
                            "the {pool} pool permits {} through the {} engine of {}",
                            entry.model, entry.engine, entry.provider
                        ),
                        false => format!(
                            "the {pool} pool no longer permits {} through the {} engine of {}",
                            entry.model, entry.engine, entry.provider
                        ),
                    },
                    match was_tracking {
                        true =>
                            "it held every admissible entry and now holds the list you left, \
                                 so it no longer follows the catalog: a model measured later joins \
                                 the catalog and not this pool",
                        false => "it already held an explicit list and still does",
                    },
                    record.resolved.admissible,
                    record.resolved.entries.len(),
                    records.path_of(&pool).display(),
                    match self.attempt.is_some() {
                        true =>
                            " · the run that is working holds the snapshot it froze and is \
                                 not disturbed",
                        false => "",
                    }
                ));
                true
            }
        }
    }

    /// Raise or lower a pool's ceilings.
    ///
    /// Each ceiling is stated absolutely and each is optional, so a surface that moves one of them
    /// leaves the other exactly as the record holds it. Neither is a target: nothing creates a
    /// participant to reach one, and the digest a run freezes does not move when one changes,
    /// because the digest follows the entries and their order alone.
    pub fn set_pool_capacity(
        &mut self,
        pool: PoolName,
        max_agents: Option<u32>,
        max_concurrent_attempts: Option<u32>,
    ) -> bool {
        let records = self.registry.pools();
        let outcome = records.edit(
            &pool,
            &self.registry.providers(),
            &self.registry.registry(),
            |declared| {
                declared.capacity = PoolCapacity {
                    max_agents: max_agents.unwrap_or(declared.capacity.max_agents),
                    max_concurrent_attempts: max_concurrent_attempts
                        .unwrap_or(declared.capacity.max_concurrent_attempts),
                };
            },
        );
        match outcome {
            Err(error) => {
                self.model.error(format!(
                    "the {pool} pool was not changed — {error} · it stands exactly as it did"
                ));
                false
            }
            Ok(record) => {
                self.read_pools();
                self.model.reply(format!(
                    "the {pool} pool is held to up to {} participants and up to {} attempts at \
                     once · both are ceilings and neither is a target: nothing creates a \
                     participant to reach one · recorded in {}",
                    record.declared.capacity.max_agents,
                    record.declared.capacity.max_concurrent_attempts,
                    records.path_of(&pool).display()
                ));
                true
            }
        }
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
        // A cancelled run stops the participant it ignited on, first, so that the ending of that
        // participant is in the record before the run states its own. A run recorded cancelled
        // while its participant was still working would state an ending nothing had reached.
        if let OriginShutdown::LeftRunning = self.end_origin() {
            self.model.reply(
                "the participant this run ignited on did not stop within the limit and is still \
                 working · what it spends is still charged to your account",
            );
        }
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
                    // Closing the controller is what ends the runtime's process tree, and that
                    // close is bounded: a worker that outlasts the limit keeps its session and the
                    // processes under it. What the reply says about the process tree is therefore
                    // read from the close rather than assumed from having issued it.
                    let ending = self
                        .attempt
                        .take()
                        .map_or(ManagedShutdown::Ended, ManagedRunHandle::close);
                    self.verifying = self.verifying.wrapping_add(1);
                    self.model.working(None);
                    self.refresh();
                    // What the run ended as is read from the journal after it was caught up, not
                    // assumed from the command that was issued. A cancellation asked for while the
                    // attempt was already failing finds the run terminal for another reason, and
                    // the domain refuses to rename it; saying `cancelled` here would be the
                    // interface asserting an outcome the record does not hold.
                    self.model.reply(format!(
                        "cancel recorded — {}, {kernel}, and {}",
                        self.recorded_terminal(),
                        shutdown_clause(&ending)
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
            self.model.reply("no run is open — nothing to cancel");
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
            None => "no run is open".to_owned(),
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
        self.projection_for(Selected {
            candidate: describe,
            ..Selected::none()
        })
    }

    /// The full projection for the rows the operator is standing on.
    ///
    /// Each of them is a position rather than a value: the properties view of a provider or of a
    /// pool exists while a row of the table above it is selected and not otherwise, exactly as the
    /// describe view of a candidate does.
    pub fn projection_for(&self, selected: Selected) -> Projection {
        let Selected {
            candidate: describe,
            provider,
            pool,
        } = selected;
        let mut projection = self
            .model
            .projection_while(self.runtimes.as_ref(), self.measuring.as_ref());
        // The row that states what runs away from this thread carries the measurement only where
        // nothing else is already waiting there. A draft being assembled outranks it, because that
        // wait is the one Esc ends and losing the way out of it would be losing the way out: the
        // measurement is stated on every surface that draws the account either way.
        if projection.working.is_none()
            && let Some(measuring) = self.measuring.as_ref()
        {
            projection.working = Some(measuring.notice());
        }
        self.push_provider_pages(&mut projection, provider);
        self.push_pool_pages(&mut projection, pool);
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

    /// Add the three provider surfaces to a projection, with the palette entries that open them.
    ///
    /// They are added here rather than in the read model because the provider level is not in the
    /// journal: it is the product root's own configuration, read by this session from the records
    /// under the root it addresses.
    fn push_provider_pages(&self, projection: &mut Projection, provider: Option<usize>) {
        let status = self.model.status_line();
        projection.providers = Some(self.providers.clone());
        let measuring = self.measuring.as_ref();
        let mut pages = vec![
            (
                PageKind::Providers,
                providers::providers_page(&self.providers, status.clone(), measuring),
            ),
            (
                PageKind::Models,
                providers::models_page(&self.providers, status.clone()),
            ),
        ];
        if let Some(selected) = provider.and_then(|index| self.providers.providers.get(index)) {
            pages.push((
                PageKind::Provider,
                providers::provider_page(selected, status, measuring),
            ));
        }
        for (kind, page) in pages {
            projection.commands.push(PaletteItem {
                name: kind.command_name().to_owned(),
                description: crate::journal::page_description(kind).to_owned(),
                command: Command::OpenPage(kind),
            });
            projection.pages.push((kind, page));
        }
    }

    /// Add the pool surfaces to a projection, with the palette entries that open them.
    ///
    /// They are added beside the provider surfaces and for the same reason: which entries a run
    /// may recruit from is the product root's own configuration and is not in the journal. The
    /// list is always there, whether or not this root holds a pool — a page that vanished on a
    /// fresh root would answer *where do my models come from* with nothing at all.
    fn push_pool_pages(&self, projection: &mut Projection, pool: Option<usize>) {
        let status = self.model.status_line();
        projection.pools = Some(self.pools.clone());
        let mut pages = vec![(
            PageKind::Pools,
            pools::pools_page(&self.pools, status.clone()),
        )];
        if let Some(selected) = pool.and_then(|index| self.pools.pool_at(index)) {
            pages.push((
                PageKind::Pool,
                pools::pool_page(selected, &self.pools.entries, status),
            ));
        }
        for (kind, page) in pages {
            projection.commands.push(PaletteItem {
                name: kind.command_name().to_owned(),
                description: crate::journal::page_description(kind).to_owned(),
                command: Command::OpenPage(kind),
            });
            projection.pages.push((kind, page));
        }
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
            self.model
                .error("saved state is unavailable, so no request can be drafted here");
            return None;
        }
        if text.trim().is_empty() {
            return None;
        }
        // A goal stated while nothing is enabled is taken and held: the workspace is read locally
        // and nothing leaves this host, because the one thing that is missing is a provider. What
        // the operator is told is that state in plain words, with the command that supplies it —
        // never a technical refusal (product brief, Part A §6).
        if self.providers.enabled_count() == 0 {
            self.model.reply(A_GOAL_NEEDS_A_PROVIDER);
        } else if self.pools.is_empty()
            && self
                .providers
                .entries
                .iter()
                .any(|entry| entry.is_admissible())
        {
            // A catalog that offers an entry and a root that holds no pool cannot both be true
            // once the pools are resolved wherever a provider is observed. It is answered anyway,
            // and in the same plain words: a state nobody expected is still a state, and the reply
            // names the act that resolves the pools rather than leaving the operator to guess.
            self.model.reply(A_GOAL_NEEDS_A_POOL);
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
        // The one wait this interface offers a key out of: Esc abandons the assembly and leaves
        // the draft as it was. Every other wait states what is running and names no key, because
        // no key ends it.
        self.model.working_esc_ends(job.waiting_for());
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
                        "/authorize {} starts a new run and leaves the previous run unchanged — \
                         {ceremony}. Anything else you type amends this draft first.",
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
        // What this run may create participants from is fixed before it exists, and read from the
        // pool records of the root that governs this store. A root that can offer the run nothing
        // stops it here, before a store is addressed and before anything is written: the goal is
        // held, and what the operator reads is that state in plain words with the command that
        // supplies what is missing (product brief v2, Part A §6).
        let frozen = match freeze_under(&self.registry.root(), None) {
            Ok(frozen) => frozen,
            Err(refusal) => {
                self.model.reply(refusal.to_string());
                return;
            }
        };
        if self.application.is_some() && !self.move_to_a_store_of_its_own() {
            return;
        }
        // The operator has authorized exactly this here: they read the coverage map and typed
        // the contract id. Whether the run then starts is the store's business, so the
        // authorization is recorded before the attempt and a second one asks for one
        // confirmation rather than for the same identifier again.
        self.authorized.insert(authorization_key(&prepared));
        match Application::create_with_contract(&self.data_root, &prepared, &frozen) {
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
                // The run exists, and the entry it ignites on is already fixed in it. Starting
                // that participant is the other half of this authorization rather than a second
                // one: the operator authorized the work, and the run's own record — not this
                // session and not the pools of the root — decides what does it.
                let run_id = outcome.event.run_id.clone();
                self.ignite_origin(&run_id);
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

    /// Serve the frozen routes of this session's runs with the in-process fixture runtime, or with
    /// any other host a caller supplies.
    ///
    /// It replaces which programs a route is served by and nothing else: which route is asked for
    /// is read from the run's own frozen record either way, so a check that puts a fixture here
    /// measures the same start path the operator's host runs.
    pub fn set_participant_runtimes(
        &mut self,
        runtimes: Arc<dyn ParticipantRuntimes + Send + Sync>,
    ) {
        self.participants = Some(runtimes);
    }

    /// Start the participant this run ignites on, and put it to work on a worker of its own.
    ///
    /// A refusal is stated and nothing else happens: the run stands as it was created, its attempt
    /// is unspent, and the surface that starts an attempt by hand is still there. That is why the
    /// refusal names what the operator can still do rather than only what could not be done.
    fn ignite_origin(&mut self, run_id: &str) {
        let Some(application) = self.application.clone() else {
            return;
        };
        // A session under a root authorizes its second run in a store of its own, and the
        // participant of the first is still working when it does. It is ended here rather than
        // replaced: a handle overwritten would leave a process nothing could reach, spending the
        // operator's account for a run this session has stopped reading.
        if let OriginShutdown::LeftRunning = self.end_origin() {
            self.model.reply(
                "the participant of the run this session was reading did not stop within the \
                 limit and is still working · what it spends is still charged to your account",
            );
        }
        let runtimes: Arc<dyn ParticipantRuntimes + Send + Sync> = match &self.participants {
            Some(runtimes) => Arc::clone(runtimes),
            None => Arc::new(ManagedRuntimes::under(self.registry.clone())),
        };
        let cancellation = CancellationToken::default();
        let request = OriginStartRequest {
            root: self.registry.root().to_path_buf(),
            mcp: None,
            cancellation: cancellation.clone(),
        };
        let host = ParticipantHost::new(runtimes.as_ref(), &PrivateGit);
        let mut attempt = match ignite_origin_participant(&application, &request, &host) {
            Ok(attempt) => attempt,
            Err(refusal) => {
                self.model.reply(format!(
                    "{refusal} · /attempt {run_id} is where the work is started by hand"
                ));
                self.refresh();
                return;
            }
        };
        self.model.reply(format!(
            "run {run_id} ignited on {} · participant {} is working in a private copy of the \
             source, and every turn it takes reaches this transcript",
            attempt.route(),
            attempt.start().participant_id
        ));
        // The events the participant produces are journalled by the attempt itself, so nothing is
        // reported from here: the transcript follows the journal, which is where the participant's
        // own record already is by the time any of it is drawn.
        //
        // A participant that has stopped for an instruction produces no event and has not ended:
        // the worker holds its session and waits, because the session is the process, and a worker
        // that returned there would drop the process the record still calls running. What ends this
        // loop is the participant's own ending — reached by finishing, or by the cancellation the
        // runtime reads and answers with an interruption.
        let worker = thread::spawn(move || {
            loop {
                match attempt.next_event() {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        if attempt.state() == Some(ParticipantState::Finished) {
                            break;
                        }
                        thread::sleep(ATTEMPT_TICK);
                    }
                    Err(_) => break,
                }
            }
        });
        self.origin = Some(RunningOrigin {
            cancellation,
            worker,
        });
        self.refresh();
    }

    /// Wait for the origin participant's process slice to stop, and then leave nothing of it
    /// running.
    ///
    /// A command that authorized a run has no interface to watch the work in, so it waits for the
    /// slice the authorization started: the participant ends, or it stops for an instruction there
    /// is no one here to give. Either way the command leaves nothing behind — a participant that
    /// stopped is still holding a process, and the process is what would outlive the command.
    ///
    /// The wait for the slice is bounded by the same limit a cancellation waits for a process tree
    /// under; the ending that follows is bounded by [`Self::end_origin`].
    pub fn await_origin(&mut self) -> OriginShutdown {
        if self.origin.is_none() {
            return OriginShutdown::Nothing;
        }
        let deadline = Instant::now() + TERMINATION_WAIT;
        loop {
            if self
                .origin
                .as_ref()
                .is_none_or(|origin| origin.worker.is_finished())
                || self.origin_has_stopped()
                || Instant::now() >= deadline
            {
                break;
            }
            thread::sleep(ATTEMPT_TICK);
        }
        let ending = self.end_origin();
        self.refresh();
        ending
    }

    /// Whether the participant's process slice has stopped, as the run's own record states it.
    fn origin_has_stopped(&self) -> bool {
        self.application.as_ref().is_some_and(|application| {
            matches!(
                Self::writer(application)
                    .origin_participant()
                    .map(|participant| participant.state),
                Some(ParticipantState::Yielded | ParticipantState::Finished)
            )
        })
    }

    /// Stop the origin participant and wait for its worker, for as long as the controller's own
    /// shutdown limit allows.
    ///
    /// Every path that leaves a participant behind comes through here: closing the interface,
    /// cancelling the run, and authorizing a second run in a session that is still running the
    /// first. A participant left behind by any of them keeps a process spending the operator's
    /// account after the thing that authorized it has gone.
    ///
    /// The wait is bounded rather than instant, under the same rule the managed attempt is closed
    /// by. The cancellation reaches the runtime, which answers it with an interruption the attempt
    /// journals as the participant's ending; a runtime that has stopped answering holds its worker
    /// inside the call that reads its next event, where nothing this session sets is read. Such a
    /// worker is left running and stated as left running, because a session that stopped waiting
    /// has not thereby ended anything.
    pub fn end_origin(&mut self) -> OriginShutdown {
        let Some(origin) = self.origin.take() else {
            return OriginShutdown::Nothing;
        };
        origin.cancellation.cancel();
        let deadline = Instant::now() + CONTROLLER_SHUTDOWN_LIMIT;
        while !origin.worker.is_finished() {
            if Instant::now() >= deadline {
                return OriginShutdown::LeftRunning;
            }
            thread::sleep(ATTEMPT_TICK);
        }
        let _ = origin.worker.join();
        OriginShutdown::Ended
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
                self.model
                    .reply("continuing with a new run · the previous run remains unchanged");
                true
            }
            Err(_error) => {
                self.model.error(
                    "a new run could not be prepared · nothing changed · check that ymp can write \
                     its saved data and try again",
                );
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
            self.model.error("no attempt was launched — no run is open");
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
                " · /export writes a readable copy of the exact candidate and verifier evidence \
                 under this run's product data",
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
                .error("no run is open — there is nothing to export");
            return;
        };
        let destination_was_named = destination.is_some();
        let destination = destination.unwrap_or_else(|| self.export_destination());
        let report = Self::writer(&application).export_evidence(&destination);
        match report {
            Ok(report) => {
                let destination = match destination_was_named {
                    true => format!("evidence exported to {}", report.destination.display()),
                    false => "evidence export saved by ymp".to_owned(),
                };
                self.model.reply(format!(
                    "{destination} · candidate {} · {} verifier evidence object(s) · {} \
                     environment object(s) · {} journal events",
                    crate::projection::short_digest(&report.candidate_digest),
                    report.evidence_digests.len(),
                    report.environment_digests.len(),
                    report.event_count
                ));
            }
            Err(error) => self.model.error(format!("nothing was exported — {error}")),
        }
    }

    /// Put the accepted candidate's files into the project directory itself.
    ///
    /// This is the other form an export takes: the operator receives the work as files where they
    /// work, and nothing about the run is written beside them. The bundle stays the default and is
    /// kept under the product data root; applying files into a directory an operator already owns
    /// is chosen, never assumed.
    ///
    /// A file the project already holds is named and nothing is written, unless the operator
    /// states that replacing it is intended.
    pub fn apply_candidate(&mut self, destination: Option<PathBuf>, overwrite: bool) {
        let Some(application) = self.application.clone() else {
            self.model
                .error("no run is open — there is nothing to apply");
            return;
        };
        let destination =
            destination.unwrap_or_else(|| self.model.environment().project_path.clone());
        let report = Self::writer(&application).apply_candidate(&destination, overwrite);
        match report {
            Ok(report) => {
                let replaced = if report.replaced_paths.is_empty() {
                    String::new()
                } else {
                    format!(
                        " · {} replaced: {}",
                        report.replaced_paths.len(),
                        report.replaced_paths.join(", ")
                    )
                };
                self.model.reply(format!(
                    "candidate {} applied to {} · {} file(s): {}{replaced}",
                    crate::projection::short_digest(&report.candidate_digest),
                    report.destination.display(),
                    report.applied_paths.len(),
                    report.applied_paths.join(", ")
                ));
            }
            // An application that stopped part-way carries what it already wrote, and the
            // operator is told that rather than being told nothing happened.
            Err(error @ ApplicationError::ApplyInterrupted { .. }) => {
                self.model.error(error.to_string());
            }
            Err(error) => self.model.error(format!("nothing was applied — {error}")),
        }
    }

    /// Where an export of this run is written when the operator names no directory.
    ///
    /// It remains under the run store, so a default action never writes into the directory the
    /// product was launched from. The bundle is still directly readable, and an operator who
    /// wants it elsewhere names that destination explicitly. One directory per run means an
    /// export names the run it came from and never lands on another's.
    pub fn export_destination(&self) -> PathBuf {
        let run = self
            .model
            .run()
            .map_or_else(|| "run".to_owned(), |run| run.run_id.clone());
        self.data_root
            .join(EXPORT_DIRECTORY)
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

/// What a closed controller left behind, in the words the cancellation reply uses.
///
/// Closing a controller is bounded, so it can return while the worker still holds the runtime
/// session and the processes under it. Reporting the process tree as ended there would state as
/// measured the one thing supervision explicitly could not establish, and would tell an operator
/// that nothing of this run outlived the cancellation when something does. The two endings are
/// therefore named apart, and what the controller found the run owed is carried with the second.
fn shutdown_clause(shutdown: &ManagedShutdown) -> String {
    match shutdown {
        // A worker that died of a panic dropped its runtime session as the stack unwound, which
        // ends the process tree of that session exactly as an orderly ending does.
        ManagedShutdown::Ended | ManagedShutdown::WorkerPanicked => {
            "the managed process tree was ended".to_owned()
        }
        ManagedShutdown::WorkerLeftRunning(report) => format!(
            "the worker was left holding its runtime session and the processes under it — {report}"
        ),
    }
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
    /// Write a readable copy of the run's candidate and the evidence that judged it into the
    /// directory the operator named, or under the run store when they name none.
    ExportEvidence(Option<PathBuf>),
    /// Put the accepted candidate's files into the project directory the operator named, or the
    /// one this session stands in when they name none, and write nothing else there. `overwrite`
    /// states that replacing a file the project already holds is intended.
    ApplyCandidate {
        destination: Option<PathBuf>,
        overwrite: bool,
    },
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
    /// Enable a provider, or disable it with a stated reason. Enabling is the operator's consent to
    /// the disclosure stated above the key, and it is what measures the provider for the first
    /// time; disabling starts nothing and erases nothing.
    SetProviderEnabled {
        family: ProviderFamily,
        enabled: bool,
        reason: Option<String>,
    },
    /// Measure an enabled provider again and record what its engines serve.
    RefreshProviderModels {
        family: ProviderFamily,
    },
    /// Permit one catalog entry in a pool, or take it out. The first edit of a pool that follows
    /// the catalog replaces the whole-catalog form with the list the operator leaves, which is
    /// stated above the key that takes it and is not confirmed afterwards.
    SetPoolEntryPermitted {
        pool: PoolName,
        entry: PoolEntry,
        permitted: bool,
    },
    /// State a pool's ceilings. Each is optional and each is absolute, so moving one leaves the
    /// other exactly as the record holds it; neither is a target.
    SetPoolCapacity {
        pool: PoolName,
        max_agents: Option<u32>,
        max_concurrent_attempts: Option<u32>,
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
    // A measurement the quit could not wait out was cut short, and what that cost is said on the
    // error stream rather than dropped: the screen is gone by now, so the operator learns from the
    // product either that their measurement was ended or that something it started was left
    // running, and not from their process list.
    let ending = result?;
    match ending {
        MeasurementShutdown::Terminated(report) | MeasurementShutdown::LeftRunning(report) => {
            eprintln!("{report}");
        }
        MeasurementShutdown::Nothing | MeasurementShutdown::Ended => {}
    }
    Ok(())
}

enum AppEvent {
    Terminal(Event),
    Journal,
    /// A demonstration this session asked for has decided.
    Checked(Box<CheckOutcome>),
    /// A verification this session asked for has decided.
    Verified(Box<VerificationReport>),
    /// A measurement this session asked for has returned from its worker.
    Measured(Box<MeasurementOutcome>),
    InputEnded,
}

fn event_loop(
    guard: &mut TerminalGuard,
    session: &mut Session,
    app: &mut App,
    markers: Markers,
) -> anyhow::Result<MeasurementShutdown> {
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let input = spawn_input_thread(tx.clone(), Arc::clone(&stop));
    // **Starting the product measures nothing.** Launching ymp opens no process and reaches no
    // network: the surfaces are composed from the records under the root, and each one states how
    // old the measurement it shows is. Measuring happens where the operator asks for it — on the
    // enable transition and on an explicit refresh — and at no other moment, which is what makes
    // enabling the one act that lets this host reach an account.
    session.set_runtimes(crate::runtimes::read_all(session.registry_address()));
    adopt(session, app);
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
                        AppEvent::Checked(outcome) => {
                            session.finish_check(*outcome);
                            adopt(session, app);
                        }
                        AppEvent::Verified(report) => {
                            session.finish_verification(*report);
                            adopt(session, app);
                        }
                        // The engines answered. The observation is recorded and every surface is
                        // re-read here, on the thread that draws and on no other.
                        AppEvent::Measured(outcome) => {
                            session.finish_measurement(*outcome);
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
                    // The participant this run ignited on is stopped and waited for before the
                    // interface goes away, for the same reason a measurement is: a worker this
                    // session started is one this session ends.
                    session.end_origin();
                    return Ok(session.end_measurement());
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                stop.store(true, Ordering::SeqCst);
                let _ = input.join();
                session.end_origin();
                return Ok(session.end_measurement());
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
        // The decision is taken here; the measurement it asks for is not. Starting the engines of
        // an account waits for programs of this host, which on this thread would stop every
        // redraw for as long as they take, so it goes to a worker and returns as an event.
        Action::SetProviderEnabled {
            family,
            enabled,
            reason,
        } => {
            let pending = session.set_provider_enabled(family, enabled, reason);
            measure(session, app, pending, tx);
            adopt(session, app);
        }
        Action::RefreshProviderModels { family } => {
            let pending = session.refresh_provider_models(family);
            measure(session, app, pending, tx);
            adopt(session, app);
        }
        Action::SetPoolEntryPermitted {
            pool,
            entry,
            permitted,
        } => {
            session.set_pool_entry_permitted(pool.clone(), entry.clone(), permitted);
            adopt(session, app);
            follow_the_toggled_entry(app, &pool, &entry);
        }
        Action::SetPoolCapacity {
            pool,
            max_agents,
            max_concurrent_attempts,
        } => {
            session.set_pool_capacity(pool, max_agents, max_concurrent_attempts);
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
        Action::ApplyCandidate {
            destination,
            overwrite,
        } => {
            session.apply_candidate(destination, overwrite);
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

/// Hand a measurement to a worker, so the thread that draws keeps drawing while the engines
/// answer. An act that started none — a disable, a refusal, an ask folded into a running
/// measurement — starts nothing here either.
fn measure(
    session: &mut Session,
    app: &mut App,
    pending: Option<PendingMeasurement>,
    tx: &Sender<AppEvent>,
) {
    let Some(pending) = pending else {
        return;
    };
    app.working_ticks = 0;
    let tx = tx.clone();
    session.start_measurement(pending, move |outcome| {
        let _ = tx.send(AppEvent::Measured(Box::new(outcome)));
    });
}

/// Put the cursor back on the entry a toggle acted on.
///
/// Permitting an entry or taking one out moves its row: the entries a pool permits stand first, in
/// its declared order, and the rest of the catalog follows them. The cursor is kept by position, so
/// without this the operator would be left standing on whatever row took the place of the one they
/// acted on — and the next press would act on an entry they never chose. It is called after the
/// projection is rebuilt, because the row it looks for exists only in the rebuilt one.
pub fn follow_the_toggled_entry(app: &mut App, pool: &PoolName, entry: &PoolEntry) {
    let Some(report) = app.data.pools.as_ref() else {
        return;
    };
    let Some(facts) = report.named(pool) else {
        return;
    };
    let moved = pools::rows(facts, &report.entries)
        .iter()
        .position(|row| matches!(row, pools::PoolRow::Entry(row) if row.entry == *entry));
    if let Some(index) = moved {
        app.select_row(PageKind::Pool, index);
    }
}

/// Replace the projection and re-apply the operator's position within it.
fn adopt(session: &Session, app: &mut App) {
    app.adopt(session.projection_for(Selected {
        candidate: app.describe_index,
        provider: app.provider_index,
        pool: app.pool_index,
    }));
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
        // Esc ends the wait the row offers it for, and only that one. While a wait no key ends is
        // running — a measurement, an attempt, a verification — Esc is the key it always was, so
        // pressing it does what the surface says instead of nothing at all.
        KeyCode::Esc if app.data.working_ends_on_esc => return Some(Action::CancelCheck),
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
            app.surface = match kind {
                PageKind::Describe => Surface::Page(PageKind::Candidates),
                // A provider's properties were opened from the list, so leaving them returns to
                // the list rather than to the conversation. A pool's properties are the same.
                PageKind::Provider => Surface::Page(PageKind::Providers),
                PageKind::Pool => Surface::Page(PageKind::Pools),
                _ => Surface::Transcript,
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
        // The provider table is a list of accounts and nothing is decided on it: the row is
        // opened, and what each act would do is stated there, above the key that takes it.
        KeyCode::Enter if kind == PageKind::Providers => {
            app.provider_index = Some(app.selection_of(kind));
            app.surface = Surface::Page(PageKind::Provider);
            return Some(Action::Rebuild);
        }
        // The two acts an operator takes on one provider. Both are keys on the open row, both
        // state their consequence above themselves, and neither is confirmed afterwards.
        KeyCode::Char('e') if kind == PageKind::Provider => {
            let (family, enabled) = focused_provider(app)?;
            return Some(Action::SetProviderEnabled {
                family,
                enabled: !enabled,
                reason: enabled.then(|| "disabled from the provider view".to_owned()),
            });
        }
        KeyCode::Char('r') if kind == PageKind::Provider => {
            let (family, _) = focused_provider(app)?;
            return Some(Action::RefreshProviderModels { family });
        }
        // The pool table is a list and nothing is decided on it: the row is opened, and what each
        // act would do is stated there, above the key that takes it.
        KeyCode::Enter if kind == PageKind::Pools => {
            app.pool_index = Some(app.selection_of(kind));
            app.surface = Surface::Page(PageKind::Pool);
            return Some(Action::Rebuild);
        }
        // The acts an operator takes on one pool, each on the row it acts on: an entry is
        // permitted or taken out, and a ceiling is raised or lowered. No model name is typed and
        // nothing is confirmed afterwards, because the consequence stands above the key.
        KeyCode::Enter if kind == PageKind::Pool => {
            let (pool, row) = focused_pool_row(app)?;
            let pools::PoolRow::Entry(entry) = row else {
                return None;
            };
            return Some(Action::SetPoolEntryPermitted {
                pool,
                permitted: !entry.permitted,
                entry: entry.entry,
            });
        }
        KeyCode::Char(step @ ('+' | '-')) if kind == PageKind::Pool => {
            let (pool, row) = focused_pool_row(app)?;
            let pools::PoolRow::Capacity(ceiling) = row else {
                return None;
            };
            let held = app
                .data
                .pools
                .as_ref()?
                .named(&pool)?
                .record
                .declared
                .capacity;
            let moved = ceiling.stepped(held, step == '+');
            return Some(Action::SetPoolCapacity {
                pool,
                max_agents: Some(moved.max_agents),
                max_concurrent_attempts: Some(moved.max_concurrent_attempts),
            });
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

/// The provider whose properties are open, and whether it is enabled.
///
/// It is resolved from the view state alone, so a key press decides nothing the operator could not
/// read on the surface they are standing on.
fn focused_provider(app: &App) -> Option<(ProviderFamily, bool)> {
    let report = app.data.providers.as_ref()?;
    let provider = report.providers.get(app.provider_index?)?;
    Some((provider.family, provider.enabled()))
}

/// The pool whose properties are open, and the row the cursor stands on.
///
/// The rows are the ones the page drew, built by the same call, so a key acts on exactly what the
/// operator can see. A row that carries no act — the digest, the observation — resolves here and
/// is refused by the caller, which is why this returns the row rather than a decision.
fn focused_pool_row(app: &App) -> Option<(PoolName, pools::PoolRow)> {
    let report = app.data.pools.as_ref()?;
    let pool = report.pool_at(app.pool_index?)?;
    let rows = pools::rows(pool, &report.entries);
    let row = rows.get(app.selection_of(PageKind::Pool))?;
    Some((pool.pool(), row.clone()))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The cancellation reply says the managed process tree was ended only where the close
    /// established that it was.
    ///
    /// Closing a controller waits for the worker for a bounded time, and a worker that outlasts
    /// that limit is left running: it keeps the runtime session it owns and the processes under
    /// it. A reply that stated the process tree as ended in that case told the operator that
    /// nothing of the cancelled run outlived the cancellation, while supervision had recorded the
    /// opposite and named it in the report the close carries.
    ///
    /// The check that must fail: state the ending unconditionally, as the reply did before. The
    /// left-running close is then reported as an ended process tree, and the report supervision
    /// wrote about the worker it stopped waiting for reaches nobody.
    #[test]
    fn a_bounded_close_that_left_the_worker_running_is_not_reported_as_an_ended_process_tree() {
        let left = shutdown_clause(&ManagedShutdown::WorkerLeftRunning(
            "managed runtime supervision did not end within 5000 ms; the run was stopped in both \
             records and its worker was left running"
                .to_owned(),
        ));
        assert!(
            !left.contains("process tree was ended"),
            "a close that left the worker running was reported as an ended process tree: {left}"
        );
        assert!(
            left.contains("the worker was left holding its runtime session"),
            "the reply does not say what the close left behind: {left}"
        );
        assert!(
            left.contains("did not end within 5000 ms"),
            "the reply drops what supervision recorded about the run it gave up on: {left}"
        );
    }

    /// A close that ended the worker keeps the sentence it had, because there the process tree of
    /// the run is established to be gone. A panicking worker drops the same session as its stack
    /// unwinds, so it is the same ending.
    #[test]
    fn a_close_that_ended_the_worker_states_the_process_tree_was_ended() {
        for shutdown in [ManagedShutdown::Ended, ManagedShutdown::WorkerPanicked] {
            assert_eq!(
                shutdown_clause(&shutdown),
                "the managed process tree was ended",
                "an ended worker changed what the cancellation reply states: {shutdown:?}"
            );
        }
    }
}
