//! Kernel ports of the bounded native execution slice.
//!
//! [`Registry`], [`Gatekeeper`], [`Treasury`] and [`WorkspaceGuard`] are
//! kernel ports; [`ExecutionBackend`] is the provider-boundary port. Every
//! value an [`ExecutionBackend`] returns is an observation, never authority:
//! a receipt cannot admit another assignment, satisfy a criterion or settle
//! acceptance. Only the kernel decides those.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::types::{
    AdmissionDenial, AgentId, AgentIneligibility, AssignmentRequest, ErrorClass, Grant,
    IndependenceConflict, InvocationId, InvocationLimits, ModelOffering, ObservedUsage, Pool,
    PoolEligibility, ResourceAmount, Role, SettingKey, Settings, Termination,
    WorkspaceAccessRefusal, WorkspaceScope,
};
use crate::Revision;

/// A registry failure that is not a typed pool fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryFailure {
    message: String,
}

impl RegistryFailure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RegistryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "registry failed: {}", self.message)
    }
}

impl Error for RegistryFailure {}

/// Resolves native identities through explicit discovery.
///
/// A scan supplies the actual agent identities, [`ModelOffering`]s and
/// supported controls as the native environment reports them. The registry
/// keeps snapshots of what a scan returned; it maintains no hand-curated
/// model table and invents no offerings. Authentication stays in the native
/// environment: no credential is read, copied or stored. Reading the pool
/// never triggers discovery; only an explicit scan does.
pub trait Registry: Send {
    /// Runs discovery now and returns the resulting pool.
    fn scan(&mut self) -> Result<Pool, RegistryFailure>;

    /// The snapshot of the last scan; never scans.
    fn pool(&self) -> Pool;
}

/// The treasury refused a reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReserveRefused {
    requested: ResourceAmount,
    available: ResourceAmount,
}

impl ReserveRefused {
    pub const fn new(requested: ResourceAmount, available: ResourceAmount) -> Self {
        Self {
            requested,
            available,
        }
    }

    pub const fn requested(self) -> ResourceAmount {
        self.requested
    }

    pub const fn available(self) -> ResourceAmount {
        self.available
    }
}

impl fmt::Display for ReserveRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "reservation of {} refused: {} available",
            self.requested.value(),
            self.available.value()
        )
    }
}

impl Error for ReserveRefused {}

/// The treasury refused a settlement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettleRefused {
    invocation: InvocationId,
    message: String,
}

impl SettleRefused {
    pub fn new(invocation: InvocationId, message: impl Into<String>) -> Self {
        Self {
            invocation,
            message: message.into(),
        }
    }

    pub fn invocation(&self) -> &InvocationId {
        &self.invocation
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SettleRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "settlement of invocation '{}' refused: {}",
            self.invocation, self.message
        )
    }
}

impl Error for SettleRefused {}

/// Accounts for every invocation, including failed, cancelled and
/// coordination work.
///
/// A reservation is held while the invocation is in flight and settled at
/// the termination observation. The journal remains the only durable record:
/// ledger state is an in-memory projection of committed events and can be
/// rebuilt by replay.
pub trait Treasury: Send {
    /// Total configured capacity.
    fn capacity(&self) -> ResourceAmount;

    /// The sum of reservations currently held.
    fn held(&self) -> ResourceAmount;

    /// Whether a reservation of this amount could be held now.
    fn can_hold(&self, amount: ResourceAmount) -> bool;

    /// Holds the grant's reservation for an in-flight invocation.
    fn reserve(&mut self, grant: &Grant) -> Result<(), ReserveRefused>;

    /// Settles one invocation: releases its reservation and returns the
    /// released amount. The observed usage is the settlement's record.
    fn settle(
        &mut self,
        invocation: &InvocationId,
        usage: &ObservedUsage,
    ) -> Result<ResourceAmount, SettleRefused>;
}

/// The workspace guard refused a hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceHoldConflict {
    scope: WorkspaceScope,
    held_by: InvocationId,
}

impl WorkspaceHoldConflict {
    pub const fn new(scope: WorkspaceScope, held_by: InvocationId) -> Self {
        Self { scope, held_by }
    }

    pub fn scope(&self) -> &WorkspaceScope {
        &self.scope
    }

    pub fn held_by(&self) -> &InvocationId {
        &self.held_by
    }
}

impl fmt::Display for WorkspaceHoldConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "workspace scope '{}' is held by invocation '{}'",
            self.scope, self.held_by
        )
    }
}

impl Error for WorkspaceHoldConflict {}

/// Coordinates effective access to workspace scopes.
///
/// What governs is the access the backend can actually enforce, not paths a
/// provider declares; a declared path list is not an isolation guarantee.
/// Access to one scope is exclusive: the hold belongs to one invocation at a
/// time, and a conflicting successor waits until the predecessor's
/// termination or effect evidence exists.
pub trait WorkspaceGuard: Send {
    /// Whether the backend can enforce access to this scope, stated
    /// honestly by the adapter in this slice.
    fn enforceable(&self, scope: &WorkspaceScope) -> Result<(), WorkspaceAccessRefusal>;

    /// Takes the exclusive hold of one scope for one invocation.
    fn hold(
        &mut self,
        scope: &WorkspaceScope,
        invocation: &InvocationId,
    ) -> Result<(), WorkspaceHoldConflict>;

    /// Releases the hold one invocation holds, if any.
    fn release(&mut self, invocation: &InvocationId) -> Option<WorkspaceScope>;

    /// The scope one invocation holds, if any.
    fn hold_of(&self, invocation: &InvocationId) -> Option<WorkspaceScope>;

    /// The invocation currently holding one scope, if any.
    fn holder_of(&self, scope: &WorkspaceScope) -> Option<InvocationId>;
}

/// A reference workspace guard over the backend's stated effective scopes.
///
/// In this slice the enforceable set is the scripted backend's honest
/// statement of effective access: test fidelity, not evidence about real
/// providers.
#[derive(Clone, Debug, Default)]
pub struct TrackedWorkspaceGuard {
    enforceable: BTreeSet<WorkspaceScope>,
    holds_by_invocation: BTreeMap<InvocationId, WorkspaceScope>,
    holders_by_scope: BTreeMap<WorkspaceScope, InvocationId>,
}

impl TrackedWorkspaceGuard {
    /// A guard over the scopes the backend states it can enforce.
    pub fn new(enforceable: impl IntoIterator<Item = WorkspaceScope>) -> Self {
        Self {
            enforceable: enforceable.into_iter().collect(),
            holds_by_invocation: BTreeMap::new(),
            holders_by_scope: BTreeMap::new(),
        }
    }

    /// Rebuilds the holds from replayed history; existing holds are cleared.
    pub fn rebuild_holds(
        &mut self,
        holds: impl IntoIterator<Item = (InvocationId, WorkspaceScope)>,
    ) {
        self.holds_by_invocation.clear();
        self.holders_by_scope.clear();
        for (invocation, scope) in holds {
            self.holds_by_invocation
                .insert(invocation.clone(), scope.clone());
            self.holders_by_scope.insert(scope, invocation);
        }
    }
}

impl WorkspaceGuard for TrackedWorkspaceGuard {
    fn enforceable(&self, scope: &WorkspaceScope) -> Result<(), WorkspaceAccessRefusal> {
        if self.enforceable.contains(scope) {
            Ok(())
        } else {
            Err(WorkspaceAccessRefusal::NotEnforceable {
                detail: "the backend does not state effective access to this scope".to_owned(),
            })
        }
    }

    fn hold(
        &mut self,
        scope: &WorkspaceScope,
        invocation: &InvocationId,
    ) -> Result<(), WorkspaceHoldConflict> {
        if let Some(held_by) = self.holders_by_scope.get(scope)
            && held_by != invocation
        {
            return Err(WorkspaceHoldConflict::new(scope.clone(), held_by.clone()));
        }
        self.holds_by_invocation
            .insert(invocation.clone(), scope.clone());
        self.holders_by_scope
            .insert(scope.clone(), invocation.clone());
        Ok(())
    }

    fn release(&mut self, invocation: &InvocationId) -> Option<WorkspaceScope> {
        let scope = self.holds_by_invocation.remove(invocation)?;
        self.holders_by_scope.remove(&scope);
        Some(scope)
    }

    fn hold_of(&self, invocation: &InvocationId) -> Option<WorkspaceScope> {
        self.holds_by_invocation.get(invocation).cloned()
    }

    fn holder_of(&self, scope: &WorkspaceScope) -> Option<InvocationId> {
        self.holders_by_scope.get(scope).cloned()
    }
}

/// One live (in-flight) assignment of the session, for the independence
/// requirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveAssignment {
    invocation: InvocationId,
    agent: AgentId,
    role: Role,
}

impl LiveAssignment {
    pub fn new(invocation: InvocationId, agent: AgentId, role: Role) -> Self {
        Self {
            invocation,
            agent,
            role,
        }
    }

    pub fn invocation(&self) -> &InvocationId {
        &self.invocation
    }

    pub fn agent(&self) -> &AgentId {
        &self.agent
    }

    pub fn role(&self) -> &Role {
        &self.role
    }
}

/// The current-state facts one admission is reviewed against.
pub struct AdmissionSnapshot<'a> {
    pool: &'a Pool,
    live_assignments: &'a [LiveAssignment],
    treasury: &'a dyn Treasury,
    workspace: &'a dyn WorkspaceGuard,
    current_revision: Revision,
}

impl<'a> AdmissionSnapshot<'a> {
    pub fn new(
        pool: &'a Pool,
        live_assignments: &'a [LiveAssignment],
        treasury: &'a dyn Treasury,
        workspace: &'a dyn WorkspaceGuard,
        current_revision: Revision,
    ) -> Self {
        Self {
            pool,
            live_assignments,
            treasury,
            workspace,
            current_revision,
        }
    }

    pub fn pool(&self) -> &Pool {
        self.pool
    }

    /// The session's live assignments, for the independence requirement.
    pub fn live_assignments(&self) -> &[LiveAssignment] {
        self.live_assignments
    }

    pub fn treasury(&self) -> &dyn Treasury {
        self.treasury
    }

    pub fn workspace(&self) -> &dyn WorkspaceGuard {
        self.workspace
    }

    pub fn current_revision(&self) -> Revision {
        self.current_revision
    }
}

/// Validates and admits assignments.
///
/// Admission requires, checked atomically against current state at commit:
/// eligibility, supported settings, resources and enforceable workspace
/// access. Denials are typed and leave state, grants and reservations
/// unchanged.
pub trait Gatekeeper: Send {
    /// Reviews one request presented at `expected_revision` against the
    /// snapshot; `Err` is the typed denial.
    fn review(
        &self,
        request: &AssignmentRequest,
        expected_revision: Revision,
        snapshot: &AdmissionSnapshot<'_>,
    ) -> Result<(), AdmissionDenial>;
}

/// The kernel's reference gatekeeper: the revision check and the four
/// admission requirements in the contract's order.
#[derive(Clone, Copy, Debug, Default)]
pub struct PolicyGatekeeper;

impl PolicyGatekeeper {
    fn unsupported_settings(offering: &ModelOffering, settings: &Settings) -> Vec<SettingKey> {
        settings
            .iter()
            .filter(|(key, value)| !offering.supports(key, value))
            .map(|(key, _)| key.clone())
            .collect()
    }

    fn available(treasury: &dyn Treasury) -> ResourceAmount {
        ResourceAmount::new(
            treasury
                .capacity()
                .value()
                .saturating_sub(treasury.held().value()),
        )
    }
}

impl Gatekeeper for PolicyGatekeeper {
    fn review(
        &self,
        request: &AssignmentRequest,
        expected_revision: Revision,
        snapshot: &AdmissionSnapshot<'_>,
    ) -> Result<(), AdmissionDenial> {
        // The revision check runs early here and finally at the journal
        // commit; both produce the same typed stale denial.
        if snapshot.current_revision() != expected_revision {
            return Err(AdmissionDenial::StaleRevision {
                expected: expected_revision,
                actual: snapshot.current_revision(),
            });
        }

        let offering = match snapshot.pool().eligibility(request.agent()) {
            PoolEligibility::Eligible { offering } => offering,
            PoolEligibility::NotDiscovered => {
                return Err(AdmissionDenial::IneligibleAgent {
                    agent: request.agent().clone(),
                    ineligibility: AgentIneligibility::NotDiscovered,
                });
            }
            PoolEligibility::Excluded(reason) => {
                return Err(AdmissionDenial::IneligibleAgent {
                    agent: request.agent().clone(),
                    ineligibility: AgentIneligibility::Excluded(reason),
                });
            }
        };

        let unsupported = Self::unsupported_settings(&offering, request.requested_settings());
        if !unsupported.is_empty() {
            return Err(AdmissionDenial::UnsupportedSettings {
                agent: request.agent().clone(),
                unsupported,
            });
        }

        for live in snapshot.live_assignments() {
            if live.agent() == request.agent() {
                return Err(AdmissionDenial::AssignmentNotIndependent {
                    agent: request.agent().clone(),
                    role: request.role().clone(),
                    conflict: IndependenceConflict::AgentHasLiveAssignment {
                        invocation: live.invocation().clone(),
                    },
                });
            }
            if live.role() == request.role() {
                return Err(AdmissionDenial::AssignmentNotIndependent {
                    agent: request.agent().clone(),
                    role: request.role().clone(),
                    conflict: IndependenceConflict::RoleAlreadyActive {
                        invocation: live.invocation().clone(),
                    },
                });
            }
        }

        let requested = request.allowance().reservation();
        if !snapshot.treasury().can_hold(requested) {
            return Err(AdmissionDenial::ResourcesUnavailable {
                requested,
                available: Self::available(snapshot.treasury()),
            });
        }

        if let Err(refusal) = snapshot.workspace().enforceable(request.workspace()) {
            return Err(AdmissionDenial::WorkspaceNotEnforceable {
                scope: request.workspace().clone(),
                refusal,
            });
        }
        if let Some(held_by) = snapshot.workspace().holder_of(request.workspace()) {
            return Err(AdmissionDenial::WorkspaceNotEnforceable {
                scope: request.workspace().clone(),
                refusal: WorkspaceAccessRefusal::HeldByPredecessor { held_by },
            });
        }

        Ok(())
    }
}

/// An admitted invocation as handed to the backend: its sent settings and
/// per-invocation limits. Sent settings are exactly what the host resolved;
/// the host never invents a setting the native environment did not report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendInvocation {
    invocation: InvocationId,
    agent: AgentId,
    sent_settings: Settings,
    workspace: WorkspaceScope,
    limits: InvocationLimits,
}

impl BackendInvocation {
    pub fn new(
        invocation: InvocationId,
        agent: AgentId,
        sent_settings: Settings,
        workspace: WorkspaceScope,
        limits: InvocationLimits,
    ) -> Self {
        Self {
            invocation,
            agent,
            sent_settings,
            workspace,
            limits,
        }
    }

    pub fn invocation(&self) -> &InvocationId {
        &self.invocation
    }

    pub fn agent(&self) -> &AgentId {
        &self.agent
    }

    pub fn sent_settings(&self) -> &Settings {
        &self.sent_settings
    }

    pub fn workspace(&self) -> &WorkspaceScope {
        &self.workspace
    }

    pub fn limits(&self) -> &InvocationLimits {
        &self.limits
    }
}

/// A typed start failure with its error class and whether the backend
/// confirms the invocation never started. A start error with an unknown
/// outcome is not a confirmed failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendStartFailure {
    class: ErrorClass,
    confirmed_never_started: bool,
    detail: String,
}

impl BackendStartFailure {
    pub fn new(
        class: ErrorClass,
        confirmed_never_started: bool,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            class,
            confirmed_never_started,
            detail: detail.into(),
        }
    }

    pub fn class(&self) -> &ErrorClass {
        &self.class
    }

    /// Whether the failure confirms the invocation never started; only a
    /// confirmed never-started failure is accounted as `failed`.
    pub const fn confirmed_never_started(&self) -> bool {
        self.confirmed_never_started
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for BackendStartFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "backend start failed (class {}): {}",
            self.class, self.detail
        )
    }
}

impl Error for BackendStartFailure {}

/// The backend refused a cancellation request; the refusal is an
/// observation, and revoking a grant asserts nothing about whether a
/// process stopped writing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCancelRefused {
    detail: String,
}

impl BackendCancelRefused {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for BackendCancelRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "backend refused cancellation: {}", self.detail)
    }
}

impl Error for BackendCancelRefused {}

/// One execution observation from the event stream while in flight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionObservation {
    /// Output observed at the boundary the host controls.
    OutputObserved { chars: u64 },
    /// Settings the backend reports while in flight.
    SettingsReported { settings: Settings },
    /// Usage the backend reports while in flight.
    UsageObserved { usage: ObservedUsage },
    /// A termination observation delivered by the stream.
    Terminated { termination: Termination },
    /// The backend reports that this invocation's writes to the held scope
    /// have ended: effect evidence. It is the backend's observation, not
    /// kernel proof that a process stopped writing.
    WritesEnded,
}

/// The backend's final report for one invocation.
///
/// A receipt is an observation: it cannot admit another assignment, satisfy
/// a criterion or settle acceptance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    termination: Termination,
    reported_settings: Option<Settings>,
    usage: ObservedUsage,
}

impl Receipt {
    pub fn new(
        termination: Termination,
        reported_settings: Option<Settings>,
        usage: ObservedUsage,
    ) -> Self {
        Self {
            termination,
            reported_settings,
            usage,
        }
    }

    pub fn termination(&self) -> &Termination {
        &self.termination
    }

    pub fn reported_settings(&self) -> Option<&Settings> {
        self.reported_settings.as_ref()
    }

    pub fn usage(&self) -> &ObservedUsage {
        &self.usage
    }
}

/// The provider-boundary port: four operations whose results are
/// observations, never authority.
pub trait ExecutionBackend: Send {
    /// Starts an admitted invocation with its sent settings and
    /// per-invocation limits.
    fn start(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure>;

    /// Cancels a started invocation. Cancellation does not prove
    /// termination.
    fn cancel(&mut self, invocation: &InvocationId) -> Result<(), BackendCancelRefused>;

    /// Drains the event stream of execution observations for one in-flight
    /// invocation.
    fn events(&mut self, invocation: &InvocationId) -> Vec<ExecutionObservation>;

    /// The backend's final report for one invocation, if it has one.
    fn receipt(&mut self, invocation: &InvocationId) -> Option<Receipt>;
}
