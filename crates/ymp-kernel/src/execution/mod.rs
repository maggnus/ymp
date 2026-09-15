//! Types, ports and orchestration for bounded native execution.
//!
//! This module implements the first executable execution slice fixed by the
//! bounded native execution contract: one admitted invocation runs from native
//! discovery through admission, execution, termination observation and
//! accounting against a scripted [`ExecutionBackend`](ports::ExecutionBackend).
//! It never calls a real provider, runs a real agent or delivers a checked
//! result.
//!
//! # Ports
//!
//! [`Registry`](ports::Registry), [`Gatekeeper`](ports::Gatekeeper),
//! [`Treasury`](ports::Treasury) and [`WorkspaceGuard`](ports::WorkspaceGuard)
//! are kernel ports; [`ExecutionBackend`](ports::ExecutionBackend) is the
//! provider-boundary port whose results are observations, never authority. A
//! receipt cannot admit another assignment, satisfy a criterion or settle
//! acceptance; only the kernel decides those.
//!
//! # Authority and durability
//!
//! The [`Dispatcher`](crate::Dispatcher) remains the session entry point and
//! the [`Journal`](crate::Journal) remains the only durable record: execution
//! facts are session events appended under the existing revision rules, and
//! every in-memory port state (reservations, workspace holds, accounting) is a
//! projection of those events that can be rebuilt by replay. The kernel alone
//! admits work, issues grants and records outcomes.
//!
//! # Honest limits
//!
//! Wall-clock timeout enforcement compares a caller-supplied elapsed time
//! against the allowance limit. Turn and output-size bounds are enforced by
//! the host while it scans the observation stream, one observation at a
//! time, against the totals accumulated across every observation attempt of
//! the invocation; that accumulation is host-side projection state, so
//! after a restart it begins empty and the durable usage record of an
//! invocation is its termination observation. Workspace rules rest on the
//! backend's honest statement of its effective access, which is test
//! fidelity, not evidence about real providers.

mod accounting;
mod orchestration;
mod ports;
mod types;
mod view;

pub use accounting::{LedgerTreasury, SessionAccounting, UsageAggregate};
pub use orchestration::{
    AdmissionContext, AdmissionFailure, CancellationContext, CancellationOutcome, EvidenceContext,
    EvidenceOutcome, ExecutionError, HostEnforcement, ObservationAccumulation, ObservationContext,
    ObservationOutcome, SettlementContext, StartContext, StartOutcome, admit_assignment,
    observe_invocation, read_execution, record_effect_evidence, request_invocation_cancellation,
    resolve_sent_settings, scan_registry, settle_invocation, start_invocation,
};
pub use ports::{
    AdmissionSnapshot, BackendCancelRefused, BackendInvocation, BackendStartFailure,
    ExecutionBackend, ExecutionObservation, Gatekeeper, LiveAssignment, PolicyGatekeeper, Receipt,
    Registry, RegistryFailure, ReserveRefused, SettleRefused, TrackedWorkspaceGuard, Treasury,
    WorkspaceGuard, WorkspaceHoldConflict,
};
pub use types::{
    AdmissionDenial, AgentId, AgentIneligibility, Allowance, Assignment, AssignmentRequest,
    EmptyPoolReason, ErrorClass, ExclusionReason, ExecutionProfile, ExecutionTypeError, Grant,
    GrantId, IndependenceConflict, InvocationId, InvocationLimits, InvocationStatus, ModelOffering,
    ObservedUsage, OfferingId, Pool, PoolEligibility, PoolEntry, ReservationPurpose,
    ResourceAmount, Role, SettingKey, SettingValue, Settings, SupportedControl, Termination,
    UncertaintyCause, WorkspaceAccess, WorkspaceAccessRefusal, WorkspaceOperation, WorkspaceScope,
};
pub use view::{InvocationView, SessionExecutionView, replay_execution};
