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
    PoolEligibility, ReservationPurpose, ResourceAmount, Role, SettingKey, Settings, Termination,
    WorkspaceAccess, WorkspaceAccessRefusal, WorkspaceOperation, WorkspaceScope,
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

    /// Whether a reservation of this amount and purpose could be held now.
    fn can_hold(&self, amount: ResourceAmount, purpose: ReservationPurpose) -> bool;

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
    /// Whether the backend can enforce these exact operations in this
    /// scope, stated honestly by the adapter in this slice.
    fn enforceable(
        &self,
        scope: &WorkspaceScope,
        operations: &[WorkspaceOperation],
    ) -> Result<(), WorkspaceAccessRefusal>;

    /// Takes the exclusive holds of every requested scope for one
    /// invocation. No hold is taken if any scope conflicts.
    fn hold(
        &mut self,
        accesses: &[WorkspaceAccess],
        invocation: &InvocationId,
    ) -> Result<(), WorkspaceHoldConflict>;

    /// Releases all holds one invocation owns.
    fn release(&mut self, invocation: &InvocationId) -> Vec<WorkspaceAccess>;

    /// The per-scope access requirements one invocation currently holds.
    fn accesses_of(&self, invocation: &InvocationId) -> Vec<WorkspaceAccess>;

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
    enforceable: BTreeMap<WorkspaceScope, BTreeSet<WorkspaceOperation>>,
    holds_by_invocation: BTreeMap<InvocationId, Vec<WorkspaceAccess>>,
    holders_by_scope: BTreeMap<WorkspaceScope, InvocationId>,
}

impl TrackedWorkspaceGuard {
    /// A guard over the per-scope operations the backend states it can
    /// enforce.
    pub fn new(enforceable: impl IntoIterator<Item = WorkspaceAccess>) -> Self {
        Self {
            enforceable: enforceable
                .into_iter()
                .map(|access| {
                    (
                        access.scope().clone(),
                        access.operations().iter().copied().collect(),
                    )
                })
                .collect(),
            holds_by_invocation: BTreeMap::new(),
            holders_by_scope: BTreeMap::new(),
        }
    }

    /// Rebuilds the holds from replayed history; existing holds are cleared.
    pub fn rebuild_holds(
        &mut self,
        holds: impl IntoIterator<Item = (InvocationId, Vec<WorkspaceAccess>)>,
    ) {
        self.holds_by_invocation.clear();
        self.holders_by_scope.clear();
        for (invocation, accesses) in holds {
            self.holds_by_invocation
                .insert(invocation.clone(), accesses.clone());
            for access in accesses {
                self.holders_by_scope
                    .insert(access.scope().clone(), invocation.clone());
            }
        }
    }
}

impl WorkspaceGuard for TrackedWorkspaceGuard {
    fn enforceable(
        &self,
        scope: &WorkspaceScope,
        operations: &[WorkspaceOperation],
    ) -> Result<(), WorkspaceAccessRefusal> {
        if self.enforceable.get(scope).is_some_and(|available| {
            operations
                .iter()
                .all(|operation| available.contains(operation))
        }) {
            Ok(())
        } else {
            Err(WorkspaceAccessRefusal::NotEnforceable {
                detail: format!(
                    "the backend does not state effective access for operations [{}] in this \
                     scope",
                    operations
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            })
        }
    }

    fn hold(
        &mut self,
        accesses: &[WorkspaceAccess],
        invocation: &InvocationId,
    ) -> Result<(), WorkspaceHoldConflict> {
        for access in accesses {
            if let Some(held_by) = self.holders_by_scope.get(access.scope())
                && held_by != invocation
            {
                return Err(WorkspaceHoldConflict::new(
                    access.scope().clone(),
                    held_by.clone(),
                ));
            }
        }
        self.holds_by_invocation
            .insert(invocation.clone(), accesses.to_vec());
        for access in accesses {
            self.holders_by_scope
                .insert(access.scope().clone(), invocation.clone());
        }
        Ok(())
    }

    fn release(&mut self, invocation: &InvocationId) -> Vec<WorkspaceAccess> {
        let Some(accesses) = self.holds_by_invocation.remove(invocation) else {
            return Vec::new();
        };
        for access in &accesses {
            self.holders_by_scope.remove(access.scope());
        }
        accesses
    }

    fn accesses_of(&self, invocation: &InvocationId) -> Vec<WorkspaceAccess> {
        self.holds_by_invocation
            .get(invocation)
            .cloned()
            .unwrap_or_default()
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

/// The kernel's reference gatekeeper: the revision check and the five
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
        if !snapshot
            .treasury()
            .can_hold(requested, request.allowance().reservation_purpose())
        {
            return Err(AdmissionDenial::ResourcesUnavailable {
                requested,
                available: Self::available(snapshot.treasury()),
            });
        }

        for access in request.workspace_accesses() {
            if let Err(refusal) = snapshot
                .workspace()
                .enforceable(access.scope(), access.operations())
            {
                return Err(AdmissionDenial::WorkspaceNotEnforceable {
                    scope: access.scope().clone(),
                    refusal,
                });
            }
            if let Some(held_by) = snapshot.workspace().holder_of(access.scope()) {
                return Err(AdmissionDenial::WorkspaceNotEnforceable {
                    scope: access.scope().clone(),
                    refusal: WorkspaceAccessRefusal::HeldByPredecessor { held_by },
                });
            }
        }

        Ok(())
    }
}

/// An admitted invocation as handed to the backend: its sent settings,
/// per-scope workspace operations and per-invocation limits. Every value is
/// copied from the committed assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendInvocation {
    invocation: InvocationId,
    agent: AgentId,
    sent_settings: Settings,
    workspace_accesses: Vec<WorkspaceAccess>,
    limits: InvocationLimits,
}

impl BackendInvocation {
    pub fn new(
        invocation: InvocationId,
        agent: AgentId,
        sent_settings: Settings,
        workspace_accesses: Vec<WorkspaceAccess>,
        limits: InvocationLimits,
    ) -> Self {
        Self {
            invocation,
            agent,
            sent_settings,
            workspace_accesses,
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

    pub fn workspace_accesses(&self) -> &[WorkspaceAccess] {
        &self.workspace_accesses
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
    writes_ended: bool,
}

impl Receipt {
    pub fn new(
        termination: Termination,
        reported_settings: Option<Settings>,
        usage: ObservedUsage,
        writes_ended: bool,
    ) -> Self {
        Self {
            termination,
            reported_settings,
            usage,
            writes_ended,
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

    /// Whether the receipt reports that this invocation's writes to every
    /// held scope have ended.
    pub const fn writes_ended(&self) -> bool {
        self.writes_ended
    }
}

/// The provider-boundary port: four operations whose results are
/// observations, never authority.
///
/// The event stream is read through a two-phase scan so no observation is
/// lost to a failed journal append: [`ExecutionBackend::next_event`] reads
/// the next pending observation one at a time without making it consumed;
/// [`ExecutionBackend::unread_last`] un-reads the observation the current
/// scan read last, so a scan can stop at an observation and leave it
/// pending for the step that owns it;
/// [`ExecutionBackend::reset_scan`] rewinds everything the current scan
/// read, and [`ExecutionBackend::commit_scan`] makes it consumed only after
/// the journal append that records the scan's facts has resolved. A start
/// is idempotent per invocation: starting an invocation whose start already
/// resolved returns the same outcome again instead of consuming a new one.
pub trait ExecutionBackend: Send {
    /// Starts an admitted invocation with its sent settings and
    /// per-invocation limits. Idempotent per invocation: a repeated start
    /// of the same invocation returns its first outcome again.
    fn start(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure>;

    /// Cancels a started invocation. Cancellation does not prove
    /// termination.
    fn cancel(&mut self, invocation: &InvocationId) -> Result<(), BackendCancelRefused>;

    /// Reads the next pending observation from one in-flight invocation's
    /// event stream, one at a time, without making it consumed.
    fn next_event(&mut self, invocation: &InvocationId) -> Option<ExecutionObservation>;

    /// Un-reads the observation the current scan read last, leaving it
    /// pending on the stream.
    fn unread_last(&mut self, invocation: &InvocationId);

    /// Rewinds every observation the current scan read since the last
    /// commit, so a failed append can be retried without losing them.
    fn reset_scan(&mut self, invocation: &InvocationId);

    /// Makes every observation the current scan read consumed, after the
    /// journal append that records their facts has resolved.
    fn commit_scan(&mut self, invocation: &InvocationId);

    /// The backend's final report for one invocation, if it has one.
    fn receipt(&mut self, invocation: &InvocationId) -> Option<Receipt>;
}
