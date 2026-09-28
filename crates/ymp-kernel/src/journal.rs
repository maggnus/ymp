//! Journal adapter contract and shared validation of complete, atomic appends.

use crate::{events::Event, view::SessionView};
use std::collections::BTreeMap;
use ymp_domain::{
    Denial, Digest, Id, Result,
    journal::{Envelope, MethodParameters, PolicySelection, encode},
    require_text,
};

type ParameterValidator = fn(&PolicySelection) -> Result<()>;

/// Explicit, immutable-at-runtime bindings of implementation/version to parameter schema.
/// This validates configuration data only; validators cannot authorize kernel operations.
#[derive(Clone)]
pub struct ParameterSchemas {
    validators: BTreeMap<(String, String, String), ParameterValidator>,
}

impl Default for ParameterSchemas {
    fn default() -> Self {
        Self {
            validators: BTreeMap::from([
                (
                    ("NarrativeComposer".into(), "Narrator".into(), "1".into()),
                    validate_narrative as ParameterValidator,
                ),
                (
                    (
                        "NarrativeComposer".into(),
                        "DeterministicReport".into(),
                        "1".into(),
                    ),
                    validate_narrative as ParameterValidator,
                ),
                (
                    (
                        "ClaimAuditor".into(),
                        "EvidenceClassRules".into(),
                        "1".into(),
                    ),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    (
                        "ClaimAuditor".into(),
                        "ConservativeAudit".into(),
                        "1".into(),
                    ),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    (
                        "ContextComposer".into(),
                        "CriteriaProjection".into(),
                        "1".into(),
                    ),
                    validate_context_parameters as ParameterValidator,
                ),
                (
                    (
                        "ContextComposer".into(),
                        "CompactContext".into(),
                        "1".into(),
                    ),
                    validate_context_parameters as ParameterValidator,
                ),
                (
                    ("ReviewerPolicy".into(), "AnyNonProducer".into(), "1".into()),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    (
                        "ReviewerPolicy".into(),
                        "LeastUsedReviewer".into(),
                        "1".into(),
                    ),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    ("ProgressMonitor".into(), "EvidenceDelta".into(), "1".into()),
                    validate_monitor as ParameterValidator,
                ),
                (
                    (
                        "ProgressMonitor".into(),
                        "AcceptedOnlyProgress".into(),
                        "1".into(),
                    ),
                    validate_monitor as ParameterValidator,
                ),
                (
                    (
                        "FailureDiagnoser".into(),
                        "RuleBasedDiagnoser".into(),
                        "1".into(),
                    ),
                    validate_diagnoser as ParameterValidator,
                ),
                (
                    (
                        "FailureDiagnoser".into(),
                        "DirectFailuresOnly".into(),
                        "1".into(),
                    ),
                    validate_diagnoser as ParameterValidator,
                ),
                (
                    (
                        "EscalationPolicy".into(),
                        "DiagnosisFirstLadder".into(),
                        "1".into(),
                    ),
                    validate_escalation as ParameterValidator,
                ),
                (
                    (
                        "EscalationPolicy".into(),
                        "StopOnUncertainty".into(),
                        "1".into(),
                    ),
                    validate_escalation as ParameterValidator,
                ),
                (
                    ("IntakePolicy".into(), "VoiClarification".into(), "1".into()),
                    validate_intake as ParameterValidator,
                ),
                (
                    ("IntakePolicy".into(), "NoQuestions".into(), "1".into()),
                    validate_intake as ParameterValidator,
                ),
                (
                    ("Planner".into(), "AsNeededDecomposition".into(), "1".into()),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    ("Planner".into(), "ExplicitPlan".into(), "1".into()),
                    validate_explicit_plan as ParameterValidator,
                ),
                (
                    (
                        "ContributionPolicy".into(),
                        "OrdinalValue".into(),
                        "1".into(),
                    ),
                    validate_contribution as ParameterValidator,
                ),
                (
                    (
                        "ContributionPolicy".into(),
                        "FixedWorkflow".into(),
                        "1".into(),
                    ),
                    validate_contribution as ParameterValidator,
                ),
                (
                    (
                        "BeliefModel".into(),
                        "LikelihoodRatioTable".into(),
                        "1".into(),
                    ),
                    validate_likelihood as ParameterValidator,
                ),
                (
                    ("BeliefModel".into(), "StrongestSupport".into(), "1".into()),
                    validate_strongest as ParameterValidator,
                ),
                (
                    ("CreditPolicy".into(), "ConfirmedOnly".into(), "1".into()),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    (
                        "CreditPolicy".into(),
                        "IncludeDiscriminated".into(),
                        "1".into(),
                    ),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    (
                        "ExecutionBackend".into(),
                        "CodexAppServer".into(),
                        "1".into(),
                    ),
                    validate_codex as ParameterValidator,
                ),
                (
                    (
                        "ExecutionBackend".into(),
                        "CodexAppServer".into(),
                        "2".into(),
                    ),
                    validate_codex as ParameterValidator,
                ),
                (
                    ("ExecutionBackend".into(), "Scripted".into(), "1".into()),
                    validate_scripted as ParameterValidator,
                ),
                (
                    (
                        "VerificationDesigner".into(),
                        "ExplicitVisible".into(),
                        "1".into(),
                    ),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    ("CheckRunner".into(), "RetainedBytes".into(), "1".into()),
                    validate_no_parameters as ParameterValidator,
                ),
                (
                    ("CheckRunner".into(), "ProcessRunner".into(), "1".into()),
                    validate_check_limits as ParameterValidator,
                ),
                (
                    ("AwardPolicy".into(), "FirstOffer".into(), "2".into()),
                    validate_commitment_terms as ParameterValidator,
                ),
                (
                    ("AwardPolicy".into(), "FirstOffer".into(), "1".into()),
                    validate_first_offer as ParameterValidator,
                ),
                (
                    ("WorkspaceProvider".into(), "Direct".into(), "1".into()),
                    validate_direct as ParameterValidator,
                ),
                (
                    ("WorkspaceProvider".into(), "ReadOnly".into(), "1".into()),
                    validate_direct as ParameterValidator,
                ),
                (
                    ("CostModel".into(), "PriceWeighted".into(), "1".into()),
                    validate_price_weighted as ParameterValidator,
                ),
                (
                    ("ResourcePolicy".into(), "PurposeBounded".into(), "1".into()),
                    validate_purpose_bounded as ParameterValidator,
                ),
                (
                    ("MethodRouter".into(), "FixedMethod".into(), "1".into()),
                    validate_fixed as ParameterValidator,
                ),
                (
                    (
                        "ReadinessProbe".into(),
                        "StaticDependencyProbe".into(),
                        "1".into(),
                    ),
                    validate_static as ParameterValidator,
                ),
            ]),
        }
    }
}
fn validate_likelihood(selection: &PolicySelection) -> Result<()> {
    let parameters: ymp_domain::verification::LikelihoodRatioParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    parameters.validate()
}
fn validate_strongest(selection: &PolicySelection) -> Result<()> {
    let parameters: ymp_domain::verification::StrongestSupportParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    parameters.validate()
}
fn validate_scripted(selection: &PolicySelection) -> Result<()> {
    if !selection.parameters.as_object().is_some_and(|value| {
        value.len() == 1
            && value
                .get("steps")
                .and_then(|steps| steps.as_array())
                .is_some_and(|steps| !steps.is_empty() && steps.len() <= 4096)
    }) || encode(&selection.parameters)?.len() > 1_048_576
    {
        return Err(Denial::new(
            "backend_parameters",
            "Scripted requires a bounded explicit step program",
        ));
    }
    Ok(())
}
fn validate_check_limits(selection: &PolicySelection) -> Result<()> {
    let limits: ymp_domain::verification::CheckLimits =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    limits.validate()
}
fn validate_no_parameters(selection: &PolicySelection) -> Result<()> {
    if selection.parameters != serde_json::json!({}) {
        return Err(Denial::new(
            "policy_parameters",
            "This implementation has no parameters",
        ));
    }
    Ok(())
}
fn validate_commitment_terms(selection: &PolicySelection) -> Result<()> {
    let terms: ymp_domain::coordination::CommitmentTerms =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    terms.validate()
}
fn validate_first_offer(selection: &PolicySelection) -> Result<()> {
    if selection.parameters != serde_json::json!({}) {
        return Err(Denial::new(
            "policy_parameters",
            "FirstOffer version 1 has no parameters",
        ));
    }
    Ok(())
}
fn validate_direct(selection: &PolicySelection) -> Result<()> {
    let limits: ymp_domain::workspace::CaptureLimits =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    limits.validate()
}
fn validate_price_weighted(selection: &PolicySelection) -> Result<()> {
    let parameters: ymp_domain::resources::PriceWeightedParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    parameters.validate()
}
fn validate_purpose_bounded(selection: &PolicySelection) -> Result<()> {
    let parameters: ymp_domain::resources::PurposeBoundedParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    parameters.validate()
}
fn validate_static(selection: &PolicySelection) -> Result<()> {
    if selection.parameters != serde_json::json!({}) {
        return Err(Denial::new(
            "policy_parameters",
            "StaticDependencyProbe version 1 has no parameters",
        ));
    }
    Ok(())
}
fn validate_fixed(selection: &PolicySelection) -> Result<()> {
    MethodParameters::from_selection(selection)?;
    Ok(())
}
impl ParameterSchemas {
    pub fn register(
        &mut self,
        port: &str,
        implementation: &str,
        version: &str,
        validator: ParameterValidator,
    ) -> Result<()> {
        for text in [port, implementation, version] {
            require_text(text, 256)?;
        }
        let key = (port.into(), implementation.into(), version.into());
        if self.validators.contains_key(&key) {
            return Err(Denial::new(
                "policy_schema",
                "An existing implementation/version schema cannot be replaced",
            ));
        }
        self.validators.insert(key, validator);
        Ok(())
    }
    pub fn validate(&self, selection: &PolicySelection) -> Result<()> {
        selection.validate()?;
        let policy = &selection.policy;
        let key = (
            policy.port.clone(),
            policy.implementation.clone(),
            policy.version.clone(),
        );
        let validator = self.validators.get(&key).ok_or_else(|| {
            Denial::new("policy_schema", "Unknown policy implementation or version")
        })?;
        validator(selection)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalRead {
    pub revision: u64,
    pub events: Vec<Envelope<Event>>,
}

impl JournalRead {
    pub fn view(&self, session: &Id, through: Option<u64>) -> Result<SessionView> {
        self.view_with_schemas(session, through, &ParameterSchemas::default())
    }
    pub fn view_with_schemas(
        &self,
        session: &Id,
        through: Option<u64>,
        schemas: &ParameterSchemas,
    ) -> Result<SessionView> {
        if self.revision != self.events.len() as u64 {
            return Err(Denial::new(
                "journal_head",
                "Journal head and retained events disagree",
            ));
        }
        let through = through.unwrap_or(self.revision);
        if through > self.revision {
            return Err(Denial::new(
                "revision_missing",
                "Requested journal revision is not available",
            ));
        }
        let count = usize::try_from(through)
            .map_err(|_| Denial::new("revision_overflow", "Revision is not addressable"))?;
        SessionView::replay_with_schemas(session.clone(), &self.events[..count], schemas)
    }
}

/// A trusted adapter is owned by kernel services. Strategies receive SessionView only.
/// `append` validates the whole batch while holding the same lock/transaction used
/// for commit. A denial must leave both the events and the revision unchanged.
pub trait Journal: Send + Sync {
    fn schemas(&self) -> &ParameterSchemas;
    /// Atomically order a trusted user control against the current state.
    fn session_control(
        &self,
        _session: &Id,
        _at: u64,
        _change: crate::session::SessionChange,
    ) -> Result<ymp_domain::Ref> {
        Err(Denial::new(
            "control_unavailable",
            "Journal does not support atomic owner controls",
        ))
    }

    fn binding_identity(&self) -> Result<ymp_domain::workspace::JournalIdentity> {
        Err(Denial::new(
            "journal_identity",
            "This journal does not provide a durable physical identity",
        ))
    }
    fn workspace_binding(
        &self,
        _root: &ymp_domain::workspace::WorkspaceLocation,
    ) -> Result<Option<ymp_domain::workspace::WorkspaceBinding>> {
        Err(Denial::new(
            "workspace_binding",
            "This journal does not provide durable root bindings",
        ))
    }
    fn read(&self, session: &Id) -> Result<JournalRead>;
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<u64>;
    /// Inventory for workspace ownership preflight. Adapters must revalidate it
    /// under the same aggregate write lock as any LockChanged append.
    fn workspace_inventory(&self, _session: &Id) -> Result<WorkspaceInventory> {
        Err(Denial::new(
            "workspace_inventory",
            "Journal does not support aggregate workspace ownership",
        ))
    }

    fn view(&self, session: &Id, through: Option<u64>) -> Result<SessionView> {
        self.read(session)?
            .view_with_schemas(session, through, self.schemas())
    }

    /// Read-only resolution of a possibly lost acknowledgement. Never resubmits.
    fn resolve_append(
        &self,
        session: &Id,
        expected: u64,
        events: &[Envelope<Event>],
    ) -> Result<AppendResolution> {
        resolve_append(
            &self.read(session)?,
            session,
            expected,
            events,
            self.schemas(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppendResolution {
    Committed(u64),
    Absent,
    Conflict { revision: u64 },
}

pub fn resolve_append(
    current: &JournalRead,
    session: &Id,
    expected: u64,
    events: &[Envelope<Event>],
    schemas: &ParameterSchemas,
) -> Result<AppendResolution> {
    current.view_with_schemas(session, None, schemas)?;
    if events.is_empty() {
        return Err(Denial::new("empty_append", "An append must contain events"));
    }
    let end = expected
        .checked_add(events.len() as u64)
        .ok_or_else(|| Denial::new("revision_overflow", "Journal sequence exhausted"))?;
    if current.revision < expected {
        return Ok(AppendResolution::Conflict {
            revision: current.revision,
        });
    }
    let offset = usize::try_from(expected)
        .map_err(|_| Denial::new("revision_overflow", "Revision is not addressable"))?;
    let prefix = JournalRead {
        revision: expected,
        events: current.events[..offset].to_vec(),
    };
    prefix.view_with_schemas(session, None, schemas)?;
    if current.revision >= end {
        let stored = &current.events[offset..offset + events.len()];
        if encode(&stored)? == encode(&events)? {
            if end < current.revision {
                JournalRead {
                    revision: end,
                    events: current.events[..offset + events.len()].to_vec(),
                }
                .view_with_schemas(session, None, schemas)?;
            }
            return Ok(AppendResolution::Committed(end));
        }
    }
    validate_append(&prefix, session, expected, events, schemas)?;
    if current.revision == expected {
        return Ok(AppendResolution::Absent);
    }
    Ok(AppendResolution::Conflict {
        revision: current.revision,
    })
}

/// Immutable bytes addressed by SHA-256. Readers provide an allocation bound.
pub trait ContentStore: Send + Sync {
    fn put(&self, bytes: &[u8]) -> Result<Digest>;
    fn get(&self, digest: &Digest, limit: usize) -> Result<Vec<u8>>;
}

/// Control capacity reserved across the services participating in an admission.
pub fn control_reserve(view: &SessionView) -> (usize, usize) {
    let workspace = crate::workspace_locks::control_reserve(view);
    let admission = crate::gatekeeper::control_reserve(view);
    let execution = crate::execution::control_reserve(view);
    let results = view.results().control_reserve();
    (
        workspace.0
            + admission.0
            + execution.0
            + results
            + usize::from(view.session_state().control.is_some() && !view.session_state().stopped),
        workspace.1
            + admission.1
            + execution.1
            + results * 65_536
            + if view.session_state().control.is_some() && !view.session_state().stopped {
                4096
            } else {
                0
            },
    )
}

pub fn validate_append(
    current: &JournalRead,
    session: &Id,
    expected: u64,
    events: &[Envelope<Event>],
    schemas: &ParameterSchemas,
) -> Result<SessionView> {
    if current.revision != expected {
        return Err(Denial::new(
            "stale_revision",
            "The journal changed since the proposal was prepared",
        ));
    }
    if events.is_empty() {
        return Err(Denial::new("empty_append", "An append must contain events"));
    }
    let mut view = current.view_with_schemas(session, None, schemas)?;
    for event in events {
        if let Event::ReservationChanged {
            change: crate::treasury::ReservationChange::Reserved(data),
            ..
        } = &event.payload
            && let Some(intent) = &data.admission
            && crate::arbiter::initial_lease(
                &view,
                &intent.assignment.commitment,
                event.at,
                intent.assignment.allowance.timeout,
            )?
            .is_none()
        {
            return Err(Denial::new(
                "commitment_terms",
                "New admission requires recorded award lifecycle terms",
            ));
        }
        if matches!(event.payload, Event::AcceptanceRecorded { version: 1, .. }) {
            return Err(Denial::new(
                "acceptance_version",
                "Version 1 acceptance is replay-only; new decisions require canonical existing check evidence",
            ));
        }
        if matches!(event.payload, Event::SnapshotTaken { version: 1, .. }) {
            return Err(Denial::new(
                "snapshot_version",
                "Version 1 snapshots are replay-only; new captures require a read hold",
            ));
        }
        if matches!(
            event.payload,
            Event::PlanCommitted { version: 1, .. } | Event::MethodChosen { version: 1, .. }
        ) {
            return Err(Denial::new(
                "planning_version",
                "Version 1 plan and method decisions are replay-only; new decisions require version 2",
            ));
        }

        view.apply(event, schemas)?;
    }
    view.validate_complete()?;
    Ok(view)
}

/// Only current-session history and active ownership from other sessions are
/// retained. Old unrelated events cannot exhaust an aggregate history allowance.
pub struct WorkspaceInventory {
    pub current: JournalRead,
    pub other: Vec<crate::workspace_locks::WorkspaceOwnership>,
}
/// Invoked while the adapter holds the same transaction used to append.
pub fn validate_workspace_append(
    inventory: &WorkspaceInventory,
    session: &Id,
    expected: u64,
    events: &[Envelope<Event>],
    schemas: &ParameterSchemas,
) -> Result<()> {
    validate_append(&inventory.current, session, expected, events, schemas)?;
    let mut view = inventory
        .current
        .view_with_schemas(session, None, schemas)?;
    if inventory.other.iter().any(|o| o.session() == session) {
        return Err(Denial::new(
            "workspace_inventory",
            "Current session is duplicated in inventory",
        ));
    }
    let mut owner = crate::workspace_locks::WorkspaceOwnership::from_view(&view);
    crate::workspace_locks::validate_ownership(
        inventory.other.iter().chain(std::iter::once(&owner)),
    )?;
    for event in events {
        view.apply(event, schemas)?;
        owner = crate::workspace_locks::WorkspaceOwnership::from_view(&view);
        crate::workspace_locks::validate_ownership(
            inventory.other.iter().chain(std::iter::once(&owner)),
        )?;
    }
    Ok(())
}

fn validate_intake(selection: &PolicySelection) -> Result<()> {
    let p: crate::ports::planning::IntakeParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    if p.cost_interrupt.get() < 0.0 {
        return Err(Denial::new(
            "intake_parameters",
            "Interruption cost must be nonnegative",
        ));
    }
    Ok(())
}
fn validate_explicit_plan(selection: &PolicySelection) -> Result<()> {
    if selection
        .parameters
        .as_object()
        .is_none_or(|m| m.len() != 1)
    {
        return Err(Denial::new(
            "planner_parameters",
            "ExplicitPlan requires one definition",
        ));
    }
    let d: crate::ports::planning::PlanDefinition =
        ymp_domain::journal::decode(&encode(&selection.parameters["definition"])?)?;
    d.plan.validate()?;
    for item in d.items {
        item.validate()?;
    }
    Ok(())
}
fn validate_contribution(selection: &PolicySelection) -> Result<()> {
    let p: crate::ports::planning::ContributionParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    if p.expected.get() <= 0.0
        || p.p90 < p.expected
        || p.delta_belief.get() < 0.0
        || p.delta_belief.get() > 1.0
    {
        return Err(Denial::new(
            "contribution_parameters",
            "Expected and p90 cost and benefit must be bounded",
        ));
    }
    Ok(())
}

fn validate_monitor(s: &PolicySelection) -> Result<()> {
    let p: crate::ports::progress::MonitorParameters =
        ymp_domain::journal::decode(&encode(&s.parameters)?)?;
    p.validate()
}
fn validate_diagnoser(s: &PolicySelection) -> Result<()> {
    let _: crate::ports::progress::DiagnosisParameters =
        ymp_domain::journal::decode(&encode(&s.parameters)?)?;
    Ok(())
}
fn validate_escalation(s: &PolicySelection) -> Result<()> {
    let p: crate::ports::progress::EscalationParameters =
        ymp_domain::journal::decode(&encode(&s.parameters)?)?;
    p.validate()
}

fn validate_context_parameters(selection: &PolicySelection) -> Result<()> {
    let p: crate::ports::reporting::ContextParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    if p.history_limit > 16 {
        return Err(Denial::new(
            "context_parameters",
            "Context history is bounded to sixteen references",
        ));
    }
    Ok(())
}

fn validate_narrative(selection: &PolicySelection) -> Result<()> {
    let p: crate::finalization::NarrativeParameters =
        ymp_domain::journal::decode(&encode(&selection.parameters)?)?;
    if p.max_claims == 0 || p.max_claims > 128 || p.max_text < 256 || p.max_text > 4096 {
        return Err(Denial::new(
            "narrative_parameters",
            "Narration needs finite claim and text limits",
        ));
    }
    Ok(())
}

fn validate_codex(selection: &PolicySelection) -> Result<()> {
    let value = &selection.parameters;
    let parameters: crate::ports::execution::CodexParameters =
        ymp_domain::journal::decode(&ymp_domain::journal::encode(value)?)?;
    parameters.validate()
}

/// Conservative path overlap used by durable and in-memory physical bindings.
pub fn root_paths_overlap(left: &str, right: &str) -> bool {
    let left = left.to_ascii_lowercase();
    let right = right.to_ascii_lowercase();
    left == "/"
        || right == "/"
        || left == right
        || left
            .strip_prefix(&right)
            .is_some_and(|tail| tail.starts_with('/'))
        || right
            .strip_prefix(&left)
            .is_some_and(|tail| tail.starts_with('/'))
}
