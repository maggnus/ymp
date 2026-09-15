//! Execution-domain value types for the bounded native execution slice.
//!
//! Every identifier, setting and reason is preserved exactly as reported:
//! unknown metadata stays unknown (an absent [`Option`]), values are never
//! defaulted or invented, and native identifiers are transported without
//! normalization. Distinguishing facts — such as the difference between an
//! agent that was never discovered and one that is excluded from serving —
//! are separate typed states, never collapsed into one flag.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use crate::Revision;

/// Failures raised while constructing execution-domain values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionTypeError {
    BlankText { field: &'static str },
    ZeroBound { field: &'static str },
    DuplicateKey { field: &'static str, key: String },
}

impl fmt::Display for ExecutionTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankText { field } => write!(formatter, "{field} must not be blank"),
            Self::ZeroBound { field } => write!(formatter, "{field} must be greater than zero"),
            Self::DuplicateKey { field, key } => {
                write!(formatter, "{field} '{key}' must be unique")
            }
        }
    }
}

impl Error for ExecutionTypeError {}

macro_rules! execution_identifier {
    ($(#[$doc:meta])* $name:ident, $field:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ExecutionTypeError> {
                let value = value.into();
                if value.trim().is_empty() {
                    Err(ExecutionTypeError::BlankText { field: $field })
                } else {
                    Ok(Self(value))
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

execution_identifier!(
    /// A native agent identity as the provider environment reports it.
    AgentId,
    "agent ID"
);
execution_identifier!(
    /// Host-minted identity of one admitted invocation.
    InvocationId,
    "invocation ID"
);
execution_identifier!(
    /// Host-minted identity of one grant issued by an admission.
    GrantId,
    "grant ID"
);
execution_identifier!(
    /// A role that exists only inside one assignment.
    Role,
    "role"
);
execution_identifier!(
    /// A workspace scope the backend can be asked to enforce access to.
    WorkspaceScope,
    "workspace scope"
);
execution_identifier!(
    /// A provider-reported error class; it states a class, not a cause.
    ErrorClass,
    "error class"
);
execution_identifier!(
    /// One execution-setting key, as the native environment names it.
    SettingKey,
    "setting key"
);
execution_identifier!(
    /// One execution-setting value, preserved exactly as reported.
    SettingValue,
    "setting value"
);
execution_identifier!(
    /// The native offering identity a scan reported.
    OfferingId,
    "offering ID"
);

/// A set of execution settings keyed by native names.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Settings(BTreeMap<SettingKey, SettingValue>);

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds settings from key/value pairs; duplicate keys are rejected.
    pub fn from_pairs(
        pairs: impl IntoIterator<Item = (SettingKey, SettingValue)>,
    ) -> Result<Self, ExecutionTypeError> {
        let mut settings = Self::new();
        for (key, value) in pairs {
            if settings.0.contains_key(&key) {
                return Err(ExecutionTypeError::DuplicateKey {
                    field: "setting key",
                    key: key.to_string(),
                });
            }
            settings.0.insert(key, value);
        }
        Ok(settings)
    }

    pub fn insert(&mut self, key: SettingKey, value: SettingValue) {
        self.0.insert(key, value);
    }

    pub fn get(&self, key: &SettingKey) -> Option<&SettingValue> {
        self.0.get(key)
    }

    pub fn contains_key(&self, key: &SettingKey) -> bool {
        self.0.contains_key(key)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, SettingKey, SettingValue> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Settings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (key, value) in self.iter() {
            if !first {
                formatter.write_str(", ")?;
            }
            first = false;
            write!(formatter, "{key}={value}")?;
        }
        Ok(())
    }
}

/// One control an offering reported as supported.
///
/// An empty value list means the key is supported without the native
/// environment enumerating its accepted values; the host invents no values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportedControl {
    key: SettingKey,
    values: Vec<SettingValue>,
}

impl SupportedControl {
    pub fn new(key: SettingKey, values: Vec<SettingValue>) -> Self {
        Self { key, values }
    }

    pub fn key(&self) -> &SettingKey {
        &self.key
    }

    pub fn values(&self) -> &[SettingValue] {
        &self.values
    }
}

/// An offering a scan returned: its native identity and supported controls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelOffering {
    id: OfferingId,
    controls: BTreeMap<SettingKey, Vec<SettingValue>>,
}

impl ModelOffering {
    /// Builds an offering; duplicate control keys are rejected.
    pub fn new(
        id: OfferingId,
        controls: Vec<SupportedControl>,
    ) -> Result<Self, ExecutionTypeError> {
        let mut map = BTreeMap::new();
        for control in controls {
            if map.contains_key(control.key()) {
                return Err(ExecutionTypeError::DuplicateKey {
                    field: "control key",
                    key: control.key().to_string(),
                });
            }
            map.insert(control.key().clone(), control.values().to_vec());
        }
        Ok(Self { id, controls: map })
    }

    pub fn id(&self) -> &OfferingId {
        &self.id
    }

    pub fn controls(&self) -> impl Iterator<Item = (&SettingKey, &[SettingValue])> {
        self.controls
            .iter()
            .map(|(key, values)| (key, values.as_slice()))
    }

    /// Whether the offering reports a control for this setting key.
    pub fn has_control(&self, key: &SettingKey) -> bool {
        self.controls.contains_key(key)
    }

    /// Whether the offering's reported controls support this exact setting.
    pub fn supports(&self, key: &SettingKey, value: &SettingValue) -> bool {
        match self.controls.get(key) {
            None => false,
            Some(values) if values.is_empty() => true,
            Some(values) => values.contains(value),
        }
    }
}

/// Why a discovered adapter is excluded from serving right now.
///
/// Exclusion is a fact about serving, not a denial of identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExclusionReason {
    /// The readiness probe reported the adapter cannot serve invocations now.
    NotReady { detail: String },
    /// The scan reported the agent but withheld it from serving.
    WithheldByScan { detail: String },
}

/// Why the pool holds no entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmptyPoolReason {
    /// No scan has run; reading the pool never triggers discovery.
    NotScanned,
    /// The last explicit scan returned nothing.
    ScanFoundNothing,
}

/// One agent as the last scan reported it, with its exclusion if any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolEntry {
    agent: AgentId,
    offering: ModelOffering,
    exclusion: Option<ExclusionReason>,
}

impl PoolEntry {
    pub fn new(
        agent: AgentId,
        offering: ModelOffering,
        exclusion: Option<ExclusionReason>,
    ) -> Self {
        Self {
            agent,
            offering,
            exclusion,
        }
    }

    pub fn agent(&self) -> &AgentId {
        &self.agent
    }

    pub fn offering(&self) -> &ModelOffering {
        &self.offering
    }

    pub fn exclusion(&self) -> Option<&ExclusionReason> {
        self.exclusion.as_ref()
    }
}

/// The result of asking the pool whether one agent can serve.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolEligibility {
    Eligible { offering: ModelOffering },
    NotDiscovered,
    Excluded(ExclusionReason),
}

/// A snapshot of what one scan returned.
///
/// The pool lists eligible agents with their exclusion reasons; it maintains
/// no hand-curated model table and invents no offerings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    entries: Vec<PoolEntry>,
    empty_reason: Option<EmptyPoolReason>,
}

impl Pool {
    /// A pool built from one explicit scan; an empty scan keeps its typed
    /// reason.
    pub fn from_scan(entries: Vec<PoolEntry>) -> Self {
        let empty_reason = if entries.is_empty() {
            Some(EmptyPoolReason::ScanFoundNothing)
        } else {
            None
        };
        Self {
            entries,
            empty_reason,
        }
    }

    /// The pool before any scan has run.
    pub fn unscanned() -> Self {
        Self {
            entries: Vec::new(),
            empty_reason: Some(EmptyPoolReason::NotScanned),
        }
    }

    pub fn entries(&self) -> &[PoolEntry] {
        &self.entries
    }

    pub fn empty_reason(&self) -> Option<EmptyPoolReason> {
        self.empty_reason
    }

    /// Resolves one agent's eligibility from this snapshot alone.
    pub fn eligibility(&self, agent: &AgentId) -> PoolEligibility {
        match self.entries.iter().find(|entry| entry.agent() == agent) {
            None => PoolEligibility::NotDiscovered,
            Some(entry) => match entry.exclusion() {
                None => PoolEligibility::Eligible {
                    offering: entry.offering().clone(),
                },
                Some(reason) => PoolEligibility::Excluded(reason.clone()),
            },
        }
    }
}

/// An amount of abstract reservation capacity.
///
/// The slice does not normalize cost across providers; amounts are opaque
/// units compared against treasury capacity. An amount of zero is a fact
/// (for example, an empty remaining capacity), while a reservation must
/// always be greater than zero and is validated by [`Allowance::new`].
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceAmount(u64);

impl ResourceAmount {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Per-invocation limits carried by the bounded allowance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvocationLimits {
    max_turns: u32,
    max_output_chars: u64,
    max_wall_clock: Duration,
}

impl InvocationLimits {
    pub fn new(
        max_turns: u32,
        max_output_chars: u64,
        max_wall_clock: Duration,
    ) -> Result<Self, ExecutionTypeError> {
        if max_turns == 0 {
            return Err(ExecutionTypeError::ZeroBound {
                field: "maximum turns",
            });
        }
        if max_output_chars == 0 {
            return Err(ExecutionTypeError::ZeroBound {
                field: "maximum output characters",
            });
        }
        if max_wall_clock.is_zero() {
            return Err(ExecutionTypeError::ZeroBound {
                field: "maximum wall-clock duration",
            });
        }
        Ok(Self {
            max_turns,
            max_output_chars,
            max_wall_clock,
        })
    }

    pub const fn max_turns(&self) -> u32 {
        self.max_turns
    }

    pub const fn max_output_chars(&self) -> u64 {
        self.max_output_chars
    }

    pub const fn max_wall_clock(&self) -> Duration {
        self.max_wall_clock
    }
}

/// The bounded allowance one admission grants: a reservation and the
/// per-invocation limits bound to it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allowance {
    reservation: ResourceAmount,
    limits: InvocationLimits,
}

impl Allowance {
    /// A reservation of zero is not a bounded allowance.
    pub fn new(
        reservation: ResourceAmount,
        limits: InvocationLimits,
    ) -> Result<Self, ExecutionTypeError> {
        if reservation.value() == 0 {
            return Err(ExecutionTypeError::ZeroBound {
                field: "allowance reservation",
            });
        }
        Ok(Self {
            reservation,
            limits,
        })
    }

    pub fn reservation(&self) -> ResourceAmount {
        self.reservation
    }

    pub fn limits(&self) -> &InvocationLimits {
        &self.limits
    }
}

/// The grant one committed admission issues: evidence that the reservation
/// is held for exactly this invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grant {
    id: GrantId,
    invocation: InvocationId,
    reservation: ResourceAmount,
}

impl Grant {
    pub fn new(id: GrantId, invocation: InvocationId, reservation: ResourceAmount) -> Self {
        Self {
            id,
            invocation,
            reservation,
        }
    }

    pub fn id(&self) -> &GrantId {
        &self.id
    }

    pub fn invocation(&self) -> &InvocationId {
        &self.invocation
    }

    pub fn reservation(&self) -> ResourceAmount {
        self.reservation
    }
}

/// An assignment request as the session caller states it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignmentRequest {
    agent: AgentId,
    role: Role,
    requested_settings: Settings,
    allowance: Allowance,
    workspace: WorkspaceScope,
}

impl AssignmentRequest {
    pub fn new(
        agent: AgentId,
        role: Role,
        requested_settings: Settings,
        allowance: Allowance,
        workspace: WorkspaceScope,
    ) -> Self {
        Self {
            agent,
            role,
            requested_settings,
            allowance,
            workspace,
        }
    }

    pub fn agent(&self) -> &AgentId {
        &self.agent
    }

    pub fn role(&self) -> &Role {
        &self.role
    }

    pub fn requested_settings(&self) -> &Settings {
        &self.requested_settings
    }

    pub fn allowance(&self) -> &Allowance {
        &self.allowance
    }

    pub fn workspace(&self) -> &WorkspaceScope {
        &self.workspace
    }
}

/// A committed admission: one assignment carrying its assignment-scoped
/// role, bounded allowance, grant, and the sent settings resolved during
/// admission from the requested settings and the offering's reported
/// controls. `start` passes exactly these recorded settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assignment {
    invocation: InvocationId,
    agent: AgentId,
    role: Role,
    requested_settings: Settings,
    sent_settings: Settings,
    allowance: Allowance,
    grant: Grant,
    workspace: WorkspaceScope,
}

impl Assignment {
    /// A record constructor: one field per journaled assignment fact.
    #[allow(clippy::too_many_arguments)] // one parameter per record field
    pub fn new(
        invocation: InvocationId,
        agent: AgentId,
        role: Role,
        requested_settings: Settings,
        sent_settings: Settings,
        allowance: Allowance,
        grant: Grant,
        workspace: WorkspaceScope,
    ) -> Self {
        Self {
            invocation,
            agent,
            role,
            requested_settings,
            sent_settings,
            allowance,
            grant,
            workspace,
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

    pub fn requested_settings(&self) -> &Settings {
        &self.requested_settings
    }

    /// The settings resolved during admission and recorded in the
    /// admission batch.
    pub fn sent_settings(&self) -> &Settings {
        &self.sent_settings
    }

    pub fn allowance(&self) -> &Allowance {
        &self.allowance
    }

    pub fn grant(&self) -> &Grant {
        &self.grant
    }

    pub fn workspace(&self) -> &WorkspaceScope {
        &self.workspace
    }
}

/// Why an agent in the request cannot serve this assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentIneligibility {
    /// The pool does not contain the agent at all.
    NotDiscovered,
    /// The pool contains the agent, excluded from serving with a typed
    /// reason.
    Excluded(ExclusionReason),
}

/// Why the workspace access an assignment needs is not available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceAccessRefusal {
    /// The backend cannot actually enforce access to this scope.
    NotEnforceable { detail: String },
    /// The scope is exclusively held; the successor waits until the
    /// predecessor's termination or effect evidence exists.
    HeldByPredecessor { held_by: InvocationId },
}

/// Why an assignment is not independent of the session's live assignments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndependenceConflict {
    /// The agent already holds a live assignment.
    AgentHasLiveAssignment { invocation: InvocationId },
    /// The role is already active in the session.
    RoleAlreadyActive { invocation: InvocationId },
}

/// The typed admission denials of the gatekeeper's requirements plus
/// revision staleness. Each is returned without a journal append and leaves
/// journal, revision, grants and reservations unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionDenial {
    IneligibleAgent {
        agent: AgentId,
        ineligibility: AgentIneligibility,
    },
    UnsupportedSettings {
        agent: AgentId,
        unsupported: Vec<SettingKey>,
    },
    AssignmentNotIndependent {
        agent: AgentId,
        role: Role,
        conflict: IndependenceConflict,
    },
    ResourcesUnavailable {
        requested: ResourceAmount,
        available: ResourceAmount,
    },
    WorkspaceNotEnforceable {
        scope: WorkspaceScope,
        refusal: WorkspaceAccessRefusal,
    },
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
}

impl fmt::Display for AdmissionDenial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IneligibleAgent {
                agent,
                ineligibility,
            } => match ineligibility {
                AgentIneligibility::NotDiscovered => {
                    write!(formatter, "agent '{agent}' is not in the pool")
                }
                AgentIneligibility::Excluded(reason) => match reason {
                    ExclusionReason::NotReady { detail } => write!(
                        formatter,
                        "agent '{agent}' is excluded from serving: not ready: {detail}"
                    ),
                    ExclusionReason::WithheldByScan { detail } => write!(
                        formatter,
                        "agent '{agent}' is excluded from serving: withheld by scan: {detail}"
                    ),
                },
            },
            Self::UnsupportedSettings { agent, unsupported } => {
                let keys: Vec<String> = unsupported.iter().map(SettingKey::to_string).collect();
                write!(
                    formatter,
                    "agent '{agent}' does not support settings: {}",
                    keys.join(", ")
                )
            }
            Self::AssignmentNotIndependent {
                agent,
                role,
                conflict,
            } => match conflict {
                IndependenceConflict::AgentHasLiveAssignment { invocation } => write!(
                    formatter,
                    "assignment for agent '{agent}' is not independent: agent holds live \
                     invocation '{invocation}'"
                ),
                IndependenceConflict::RoleAlreadyActive { invocation } => write!(
                    formatter,
                    "assignment for agent '{agent}' is not independent: role '{role}' is \
                     already active in invocation '{invocation}'"
                ),
            },
            Self::ResourcesUnavailable {
                requested,
                available,
            } => write!(
                formatter,
                "resources unavailable: requested {}, available {}",
                requested.value(),
                available.value()
            ),
            Self::WorkspaceNotEnforceable { scope, refusal } => match refusal {
                WorkspaceAccessRefusal::NotEnforceable { detail } => write!(
                    formatter,
                    "workspace access to '{scope}' is not enforceable: {detail}"
                ),
                WorkspaceAccessRefusal::HeldByPredecessor { held_by } => write!(
                    formatter,
                    "workspace access to '{scope}' is held by invocation '{held_by}'"
                ),
            },
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "admission revision is stale: expected {expected}, actual {actual}"
            ),
        }
    }
}

impl Error for AdmissionDenial {}

/// The typed termination states. A termination reports what was observed; it
/// does not assert a cause the error class does not state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Termination {
    Completed,
    Failed { class: ErrorClass },
    Cancelled,
    TimedOut,
}

impl fmt::Display for Termination {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Completed => formatter.write_str("completed"),
            Self::Failed { class } => write!(formatter, "failed (class {class})"),
            Self::Cancelled => formatter.write_str("cancelled"),
            Self::TimedOut => formatter.write_str("timed out"),
        }
    }
}

/// Why the kernel recorded `uncertain` for one invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UncertaintyCause {
    /// The bounded wait deadline passed without a termination observation.
    BoundedWaitExpired,
    /// A start error whose outcome is unknown.
    StartOutcomeUnknown,
}

impl fmt::Display for UncertaintyCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundedWaitExpired => {
                formatter.write_str("the bounded wait deadline expired without a termination")
            }
            Self::StartOutcomeUnknown => {
                formatter.write_str("the start error's outcome is unknown")
            }
        }
    }
}

/// Usage one invocation actually observed. A `None` component is usage the
/// provider did not report: it stays unknown and is never read as zero.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObservedUsage {
    turns: Option<u64>,
    output_chars: Option<u64>,
    wall_clock: Option<Duration>,
}

impl ObservedUsage {
    /// Usage with every component unreported.
    pub const fn unknown() -> Self {
        Self {
            turns: None,
            output_chars: None,
            wall_clock: None,
        }
    }

    pub const fn with_turns(mut self, turns: u64) -> Self {
        self.turns = Some(turns);
        self
    }

    pub const fn with_output_chars(mut self, output_chars: u64) -> Self {
        self.output_chars = Some(output_chars);
        self
    }

    pub const fn with_wall_clock(mut self, wall_clock: Duration) -> Self {
        self.wall_clock = Some(wall_clock);
        self
    }

    pub const fn turns(&self) -> Option<u64> {
        self.turns
    }

    pub const fn output_chars(&self) -> Option<u64> {
        self.output_chars
    }

    pub const fn wall_clock(&self) -> Option<Duration> {
        self.wall_clock
    }

    /// Merges a later observation into this one: a component already known
    /// stays as observed; an unreported one may be filled by a later report.
    pub const fn merge_later(mut self, later: Self) -> Self {
        if self.turns.is_none() {
            self.turns = later.turns;
        }
        if self.output_chars.is_none() {
            self.output_chars = later.output_chars;
        }
        if self.wall_clock.is_none() {
            self.wall_clock = later.wall_clock;
        }
        self
    }
}

impl fmt::Display for ObservedUsage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let render = |value: Option<u64>| {
            value.map_or_else(|| "unknown".to_owned(), |number| number.to_string())
        };
        write!(
            formatter,
            "turns {}, output characters {}, wall clock {}",
            render(self.turns),
            render(self.output_chars),
            self.wall_clock.map_or_else(
                || "unknown".to_owned(),
                |duration| format!("{} ms", duration.as_millis())
            )
        )
    }
}

/// The three settings meanings kept separate: requested (what admission
/// asked for), sent (what the host passed to the backend) and reported (what
/// the backend says it used). None overwrites another; a backend that
/// reports nothing leaves `reported` unknown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionProfile {
    requested: Settings,
    sent: Settings,
    reported: Option<Settings>,
}

impl ExecutionProfile {
    pub fn new(requested: Settings, sent: Settings, reported: Option<Settings>) -> Self {
        Self {
            requested,
            sent,
            reported,
        }
    }

    pub fn requested(&self) -> &Settings {
        &self.requested
    }

    pub fn sent(&self) -> &Settings {
        &self.sent
    }

    /// `None` means the backend reported no settings; it is not an empty
    /// report.
    pub fn reported(&self) -> Option<&Settings> {
        self.reported.as_ref()
    }
}

/// The recorded lifecycle states of one invocation.
///
/// `Cancelling` is the in-flight phase after a cancellation request and
/// before the next observation attempt; a cancelled invocation without a
/// termination observation stands as `Uncertain`, holding its reservation
/// and workspace hold. `Terminated` requires a termination observation.
/// `Failed` is terminal without one: it records only a typed start failure
/// that confirms the invocation never started; a failure observed after a
/// successful start is a typed termination under `Terminated`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvocationStatus {
    Admitted,
    Started,
    Cancelling,
    Uncertain,
    Failed,
    Terminated,
}

impl fmt::Display for InvocationStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Admitted => "admitted",
            Self::Started => "started",
            Self::Cancelling => "cancelling",
            Self::Uncertain => "uncertain",
            Self::Failed => "failed",
            Self::Terminated => "terminated",
        };
        formatter.write_str(name)
    }
}
