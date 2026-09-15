//! Scripted provider-boundary adapters for the bounded execution slice.
//!
//! [`ScriptedRegistry`] and [`ScriptedBackend`] are deterministic,
//! configuration-driven test doubles: no process is spawned, no network is
//! touched, no credential is read and no user data is mutated. The
//! effective-workspace statement is the scripted backend's honest statement
//! of the access it can enforce — test fidelity, not evidence about real
//! providers.

use std::collections::{HashMap, VecDeque};

use ymp_kernel::execution::{
    AgentId, BackendCancelRefused, BackendInvocation, BackendStartFailure, ErrorClass,
    ExclusionReason, ExecutionBackend, ExecutionObservation, InvocationId, ModelOffering,
    ObservedUsage, Pool, PoolEntry, Receipt, Registry, RegistryFailure, Settings, Termination,
    WorkspaceScope,
};

/// The scripted provider as one scan reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptedProvider {
    agent: AgentId,
    offering: ModelOffering,
    ready: bool,
    readiness_detail: String,
    effective_workspaces: Vec<WorkspaceScope>,
}

impl ScriptedProvider {
    /// A ready provider with one agent, one offering and no effective
    /// workspaces (add them with [`ScriptedProvider::with_effective_workspaces`]).
    pub fn new(agent: AgentId, offering: ModelOffering) -> Self {
        Self {
            agent,
            offering,
            ready: true,
            readiness_detail: "the scripted provider is ready".to_owned(),
            effective_workspaces: Vec::new(),
        }
    }

    /// States whether the readiness probe admits the adapter, with the
    /// probe detail used when it does not.
    pub fn with_readiness(mut self, ready: bool, detail: impl Into<String>) -> Self {
        self.ready = ready;
        self.readiness_detail = detail.into();
        self
    }

    /// States the workspaces the backend can actually enforce access to.
    pub fn with_effective_workspaces(
        mut self,
        scopes: impl IntoIterator<Item = WorkspaceScope>,
    ) -> Self {
        self.effective_workspaces = scopes.into_iter().collect();
        self
    }

    pub fn agent(&self) -> &AgentId {
        &self.agent
    }

    pub fn offering(&self) -> &ModelOffering {
        &self.offering
    }

    pub fn ready(&self) -> bool {
        self.ready
    }

    pub fn readiness_detail(&self) -> &str {
        &self.readiness_detail
    }

    pub fn effective_workspaces(&self) -> &[WorkspaceScope] {
        &self.effective_workspaces
    }
}

/// A registry whose scans return the configured providers, each with its
/// readiness. Scanning with no provider configured yields the empty pool
/// with its typed reason.
#[derive(Clone, Debug, Default)]
pub struct ScriptedRegistry {
    providers: Vec<ScriptedProvider>,
    last_scan: Option<Pool>,
}

impl ScriptedRegistry {
    pub fn new(providers: impl IntoIterator<Item = ScriptedProvider>) -> Self {
        Self {
            providers: providers.into_iter().collect(),
            last_scan: None,
        }
    }
}

impl Registry for ScriptedRegistry {
    fn scan(&mut self) -> Result<Pool, RegistryFailure> {
        let pool = Pool::from_scan(
            self.providers
                .iter()
                .map(|provider| {
                    let exclusion = (!provider.ready()).then(|| ExclusionReason::NotReady {
                        detail: provider.readiness_detail().to_owned(),
                    });
                    PoolEntry::new(
                        provider.agent().clone(),
                        provider.offering().clone(),
                        exclusion,
                    )
                })
                .collect(),
        );
        self.last_scan = Some(pool.clone());
        Ok(pool)
    }

    fn pool(&self) -> Pool {
        self.last_scan.clone().unwrap_or_else(Pool::unscanned)
    }
}

/// The scripted receipt spec: what the backend's final report says, and
/// whether it is withheld until a cancellation was requested.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptedReceiptSpec {
    termination: Termination,
    reported_settings: Option<Settings>,
    usage: ObservedUsage,
    only_after_cancel: bool,
}

impl ScriptedReceiptSpec {
    /// The backend reports a completion.
    pub fn completes(reported_settings: Option<Settings>, usage: ObservedUsage) -> Self {
        Self {
            termination: Termination::Completed,
            reported_settings,
            usage,
            only_after_cancel: false,
        }
    }

    /// The backend reports a failure with an error class.
    pub fn fails(class: ErrorClass, usage: ObservedUsage) -> Self {
        Self {
            termination: Termination::Failed { class },
            reported_settings: None,
            usage,
            only_after_cancel: false,
        }
    }

    /// The backend reports a cancellation, but only once cancellation was
    /// requested: the confirmation path.
    pub fn cancel_confirmed(usage: ObservedUsage) -> Self {
        Self {
            termination: Termination::Cancelled,
            reported_settings: None,
            usage,
            only_after_cancel: true,
        }
    }

    /// The backend reports a timed-out termination: the observation that
    /// confirms the expiry.
    pub fn times_out(usage: ObservedUsage) -> Self {
        Self {
            termination: Termination::TimedOut,
            reported_settings: None,
            usage,
            only_after_cancel: false,
        }
    }
}

/// One configured invocation outcome: an optional typed start failure (with
/// whether it confirms the invocation never started), the observations the
/// stream delivers, and an optional final receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptedOutcome {
    start_failure: Option<ErrorClass>,
    start_confirmed_never_started: bool,
    stream: Vec<ExecutionObservation>,
    receipt: Option<ScriptedReceiptSpec>,
}

impl ScriptedOutcome {
    /// A clean completion with reported settings and usage.
    pub fn completes(reported_settings: Option<Settings>, usage: ObservedUsage) -> Self {
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: Vec::new(),
            receipt: Some(ScriptedReceiptSpec::completes(reported_settings, usage)),
        }
    }

    /// A completion preceded by in-flight stream observations.
    pub fn completes_with_observations(
        observations: Vec<ExecutionObservation>,
        reported_settings: Option<Settings>,
        usage: ObservedUsage,
    ) -> Self {
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: observations,
            receipt: Some(ScriptedReceiptSpec::completes(reported_settings, usage)),
        }
    }

    /// A failure reported through the receipt, with its error class.
    pub fn fails(class: impl Into<String>, usage: ObservedUsage) -> Self {
        let class = ErrorClass::new(class).expect("valid error class");
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: Vec::new(),
            receipt: Some(ScriptedReceiptSpec::fails(class, usage)),
        }
    }

    /// A timed-out termination reported through the receipt: the
    /// termination observation that confirms the expiry.
    pub fn times_out(usage: ObservedUsage) -> Self {
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: Vec::new(),
            receipt: Some(ScriptedReceiptSpec::times_out(usage)),
        }
    }

    /// A typed start failure that confirms the invocation never started: no
    /// stream, no receipt.
    pub fn fails_at_start(class: impl Into<String>) -> Self {
        Self {
            start_failure: Some(ErrorClass::new(class).expect("valid error class")),
            start_confirmed_never_started: true,
            stream: Vec::new(),
            receipt: None,
        }
    }

    /// A start error whose outcome is unknown: not a confirmed failure. The
    /// invocation stands uncertain with its reservation and hold intact.
    pub fn start_outcome_unknown(class: impl Into<String>) -> Self {
        Self {
            start_failure: Some(ErrorClass::new(class).expect("valid error class")),
            start_confirmed_never_started: false,
            stream: Vec::new(),
            receipt: None,
        }
    }

    /// Cancellation with confirmation: the receipt arrives only after the
    /// cancellation request.
    pub fn cancelled_with_confirmation(usage: ObservedUsage) -> Self {
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: Vec::new(),
            receipt: Some(ScriptedReceiptSpec::cancel_confirmed(usage)),
        }
    }

    /// Cancellation without confirmation: the backend acknowledges the
    /// request but never reports termination.
    pub fn cancelled_without_confirmation() -> Self {
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: Vec::new(),
            receipt: None,
        }
    }

    /// No termination report at all: a lost stream or lost receipt.
    pub fn never_reports() -> Self {
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: Vec::new(),
            receipt: None,
        }
    }

    /// No termination report, with in-flight stream observations first: a
    /// lost stream or lost receipt that still observes partial output and
    /// usage.
    pub fn never_reports_with(observations: Vec<ExecutionObservation>) -> Self {
        Self {
            start_failure: None,
            start_confirmed_never_started: false,
            stream: observations,
            receipt: None,
        }
    }

    /// Whether a configured start failure confirms the invocation never
    /// started.
    pub fn start_confirmed_never_started(&self) -> bool {
        self.start_confirmed_never_started
    }
}

#[derive(Clone, Debug)]
struct RunningInvocation {
    outcome: ScriptedOutcome,
    delivered: usize,
    cancel_requested: bool,
}

/// A deterministic scripted backend: outcomes are consumed in the order
/// they were configured, one per started invocation; an unconfigured start
/// never reports. Every return value is an observation.
#[derive(Clone, Debug, Default)]
pub struct ScriptedBackend {
    effective_workspaces: Vec<WorkspaceScope>,
    outcomes: VecDeque<ScriptedOutcome>,
    running: HashMap<InvocationId, RunningInvocation>,
    last_started: Option<BackendInvocation>,
}

impl ScriptedBackend {
    /// A backend whose effective access is the provider's honest statement.
    pub fn for_provider(provider: &ScriptedProvider) -> Self {
        Self {
            effective_workspaces: provider.effective_workspaces().to_vec(),
            outcomes: VecDeque::new(),
            running: HashMap::new(),
            last_started: None,
        }
    }

    /// Queues one outcome for the next started invocation.
    pub fn with_outcome(mut self, outcome: ScriptedOutcome) -> Self {
        self.push_outcome(outcome);
        self
    }

    /// Queues one outcome in place.
    pub fn push_outcome(&mut self, outcome: ScriptedOutcome) {
        self.outcomes.push_back(outcome);
    }

    pub fn effective_workspaces(&self) -> &[WorkspaceScope] {
        &self.effective_workspaces
    }

    /// The last accepted start invocation, for asserting that `start`
    /// passed exactly the recorded sent settings.
    pub fn last_started(&self) -> Option<&BackendInvocation> {
        self.last_started.as_ref()
    }

    /// How many outcomes are still queued.
    pub fn pending_outcomes(&self) -> usize {
        self.outcomes.len()
    }

    /// Delivers one deferred observation onto a running invocation's
    /// stream, as an in-flight backend would report later.
    pub fn deliver_observation(
        &mut self,
        invocation: &InvocationId,
        observation: ExecutionObservation,
    ) {
        if let Some(running) = self.running.get_mut(invocation) {
            running.outcome.stream.push(observation);
        }
    }
}

impl ExecutionBackend for ScriptedBackend {
    fn start(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure> {
        let outcome = self
            .outcomes
            .pop_front()
            .unwrap_or_else(ScriptedOutcome::never_reports);
        if let Some(class) = outcome.start_failure.clone() {
            let confirmed = outcome.start_confirmed_never_started();
            return Err(BackendStartFailure::new(
                class,
                confirmed,
                if confirmed {
                    "the scripted backend confirms this start never happened"
                } else {
                    "the scripted backend reports a start error with an unknown outcome"
                },
            ));
        }
        self.last_started = Some(invocation.clone());
        self.running.insert(
            invocation.invocation().clone(),
            RunningInvocation {
                outcome,
                delivered: 0,
                cancel_requested: false,
            },
        );
        Ok(())
    }

    fn cancel(&mut self, invocation: &InvocationId) -> Result<(), BackendCancelRefused> {
        match self.running.get_mut(invocation) {
            None => Err(BackendCancelRefused::new(
                "the scripted backend does not know this invocation",
            )),
            Some(running) => {
                running.cancel_requested = true;
                Ok(())
            }
        }
    }

    fn events(&mut self, invocation: &InvocationId) -> Vec<ExecutionObservation> {
        match self.running.get_mut(invocation) {
            None => Vec::new(),
            Some(running) => {
                let pending = running.outcome.stream[running.delivered..].to_vec();
                running.delivered = running.outcome.stream.len();
                pending
            }
        }
    }

    fn receipt(&mut self, invocation: &InvocationId) -> Option<Receipt> {
        let running = self.running.get(invocation)?;
        let spec = running.outcome.receipt.as_ref()?;
        if spec.only_after_cancel && !running.cancel_requested {
            return None;
        }
        Some(Receipt::new(
            spec.termination.clone(),
            spec.reported_settings.clone(),
            spec.usage,
        ))
    }
}
