//! Scoped finalization and report authority; no fabricated candidate or dispatcher.
mod context;
mod narration;
mod report;
pub use report::render_report;
pub(crate) mod review;
use crate::{
    decision::{DecisionConsumer, SessionControl},
    events::Event,
    journal::{ContentStore, Journal, ParameterSchemas, validate_append},
    ports::execution::WorkspaceProvider,
    view::SessionView,
    workspace_guard::WorkspaceGuard,
};
pub use context::{ContextRecorded, context_prompt};
pub use narration::NarrativeWork;
pub use report::{
    AccountingSummary, AuditRecorded, NarrativeParameters, NarrativeRecorded, ReportDelivered,
    ReportPrepared, audit_claim, canonical_claim, deterministic,
};
pub use review::{FinalAcceptance, FinalReview, FinalVerdict, ReviewerRecorded};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    journal::{Actor, Envelope},
    report::*,
    task::SessionStatus,
    verification::{AcceptanceDecision, AcceptanceSubject},
    workspace::*,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Continuation {
    Continue,
    NoAuthority,
    Stopped,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalFence {
    pub workspace: Id<Workspace>,
    pub observation: PathObservation,
    pub holder: Id,
    pub snapshot: Id<Snapshot>,
    pub contract: Ref,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum FinalizationRecorded {
    ReportPrepared(ReportPrepared),
    NarrationWork(Box<NarrativeWork>),
    Narrative(Box<NarrativeRecorded>),
    Audit(Box<AuditRecorded>),
    Delivered(Box<ReportDelivered>),
    Reviewer(Box<ReviewerRecorded>),
    CandidateReviewer(Box<ReviewerRecorded>),
    Reviewed(Box<FinalReview>),
    Accepted(Box<FinalAcceptance>),
    Policy {
        effective: ymp_domain::journal::PolicySelection,
        change: ymp_domain::journal::SelectionChange,
    },
    Context(Box<ContextRecorded>),
    Control(Continuation),
    Blocked(String),
    Started(FinalFence),
    Captured(FinalAggregate),
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FinalizationState {
    pub control: Option<Continuation>,
    pub stopped: bool,
    pub fence: Option<(Ref, FinalFence)>,
    pub aggregate: Option<FinalAggregate>,
    pub outcome: Option<SessionStatus>,
    pub reviews: Vec<FinalReview>,
    pub acceptance: Option<FinalAcceptance>,
    pub delivered: Option<ReportDelivered>,
    pub pending_work: Option<ymp_domain::assignment::Contribution>,
    pub history: Vec<(Ref, FinalizationRecorded)>,
}
pub struct Finalization<J: Journal, C: ContentStore> {
    journal: Arc<J>,
    content: Arc<C>,
    owner: DecisionConsumer<J>,
    workspace: WorkspaceGuard<J, C>,
}
pub(crate) fn refs(data: &FinalizationRecorded) -> Vec<Ref> {
    let mut refs = match data {
        FinalizationRecorded::NarrationWork(data) => data.contribution.basis.clone(),
        FinalizationRecorded::Narrative(data) => data.decision.proposal.basis.clone(),
        FinalizationRecorded::Audit(data) => {
            let mut refs = vec![data.draft.clone()];
            for decision in &data.decisions {
                refs.extend(decision.proposal.basis.iter().cloned());
            }
            refs
        }
        FinalizationRecorded::Delivered(data) => {
            let mut refs = vec![data.draft.clone(), data.audit.clone()];
            refs.extend(data.accounting.receipts.clone());
            refs.extend(data.aggregate.clone());
            refs.extend(data.acceptance.clone());
            refs.extend(data.retained.clone());
            for source in &data.accepted_sources {
                refs.push(source.acceptance.clone());
            }
            refs
        }
        FinalizationRecorded::Reviewer(data) | FinalizationRecorded::CandidateReviewer(data) => {
            data.decision.proposal.basis.clone()
        }
        FinalizationRecorded::Reviewed(data) => {
            let mut refs = vec![
                data.verdict.aggregate.clone(),
                data.admission.clone(),
                data.completion.clone(),
                data.receipt.clone(),
                data.prompt.clone(),
            ];
            refs.extend(data.criteria.clone());
            refs
        }
        FinalizationRecorded::Accepted(data) => {
            let mut refs = data.acceptance.basis.clone();
            refs.extend(data.credit.proposal.basis.iter().cloned());
            refs
        }
        FinalizationRecorded::Context(data) => data.input.basis.clone(),
        FinalizationRecorded::Started(data) => vec![data.contract.clone()],
        FinalizationRecorded::Captured(data) => {
            let mut refs = vec![data.snapshot.clone(), data.contract.clone()];
            refs.extend(data.criteria.iter().cloned());
            refs.extend(data.checks.iter().map(|c| c.check.clone()));
            for source in &data.sources {
                refs.extend([source.result.clone(), source.acceptance.clone()]);
            }
            if let Some(base) = &data.baseline {
                refs.push(base.clone());
            }
            refs
        }
        _ => vec![],
    };
    refs.sort();
    refs.dedup();
    refs
}
pub(crate) fn writable_effects(view: &SessionView, observation: &PathObservation) -> Result<()> {
    crate::workspace_locks::WorkspaceOwnership::from_view(view).finalization_effects(observation)
}
pub fn aggregate_current(view: &SessionView, aggregate: &FinalAggregate) -> Result<()> {
    if aggregate.session != *view.session()
        || view.contract().map(|c| c.reference()) != Some(aggregate.contract.clone())
        || aggregate.criteria
            != view
                .criteria()
                .iter()
                .map(|c| c.reference())
                .collect::<Result<BTreeSet<_>>>()?
        || view.finalization().aggregate.as_ref() != Some(aggregate)
    {
        return Err(Denial::new(
            "final_scope_stale",
            "The final snapshot, contract or criterion scope changed",
        ));
    }
    Ok(())
}
fn validate_aggregate(view: &SessionView, data: &FinalAggregate) -> Result<()> {
    let (_, fence) = view.finalization().fence.as_ref().ok_or_else(|| {
        Denial::new(
            "final_fence",
            "Capture requires a retained finalization fence",
        )
    })?;
    let snapshot = view
        .snapshots()
        .values()
        .find(|s| s.reference().is_ok_and(|r| r == data.snapshot))
        .ok_or_else(|| Denial::new("final_snapshot", "No exact integrated snapshot"))?;
    let criteria: BTreeSet<_> = view
        .criteria()
        .iter()
        .map(|c| c.reference())
        .collect::<Result<_>>()?;
    let check_refs: BTreeSet<_> = view
        .contract()
        .unwrap()
        .checks
        .iter()
        .map(|id| {
            view.checks()
                .values()
                .find(|c| c.id.erased() == *id)
                .unwrap()
                .reference()
        })
        .collect();
    if snapshot.id != fence.snapshot
        || snapshot.workspace != fence.workspace
        || data.session != *view.session()
        || data.contract != fence.contract
        || view.contract().unwrap().reference() != data.contract
        || criteria != data.criteria
        || data
            .checks
            .iter()
            .map(|c| c.check.clone())
            .collect::<BTreeSet<_>>()
            != check_refs
        || data.checks.len() != check_refs.len()
    {
        return Err(Denial::new(
            "final_scope",
            "Final scope must preserve the complete captured contract and check versions",
        ));
    }
    let mut expected = vec![];
    let mut producers = BTreeSet::new();
    let mut baselines = BTreeSet::new();
    let mut known_tree = false;
    for item in view.results().items().values() {
        if let Some(id) = &item.accepted {
            let result = &view.results().results()[id];
            let after = &view.snapshots()[&result.after];
            if after.workspace != fence.workspace {
                return Err(Denial::new(
                    "integration_unavailable",
                    "This finalizer supports accepted sources in one Direct target",
                ));
            }
            let acceptance = view
                .acceptances()
                .values()
                .find(|a| {
                    a.acceptance.subject == AcceptanceSubject::ResultVersion(id.clone())
                        && a.acceptance.decision == AcceptanceDecision::Accepted
                        && a.result == result.reference().unwrap()
                })
                .ok_or_else(|| {
                    Denial::new(
                        "final_source",
                        "Accepted source lost its committed acceptance",
                    )
                })?;
            for artifact in &result.artifacts {
                if snapshot
                    .tree
                    .files
                    .get(&artifact.path)
                    .is_none_or(|file| file.digest != artifact.digest)
                {
                    return Err(Denial::new(
                        "integration_changed",
                        "An accepted source artifact differs on the integrated snapshot",
                    ));
                }
            }
            known_tree |= snapshot.tree == after.tree;
            baselines.insert(view.snapshots()[&result.before].reference()?);
            producers.insert(result.producer.clone());
            expected.push(FinalSource {
                result: result.reference()?,
                acceptance: acceptance.acceptance.reference()?,
            });
        }
    }
    expected.sort_by(|a, b| a.result.cmp(&b.result));
    if expected.is_empty()
        || !known_tree
        || data.sources != expected
        || data.producers != producers
        || data.baseline
            != if baselines.len() == 1 {
                baselines.into_iter().next()
            } else {
                None
            }
    {
        return Err(Denial::new(
            "integration_unavailable",
            "Final bytes and producer union must come from the complete accepted source set; no merge is fabricated",
        ));
    }
    writable_effects(view, &fence.observation)
}
pub(crate) fn apply(
    view: &SessionView,
    event: &Envelope<Event>,
    data: &FinalizationRecorded,
    schemas: &ParameterSchemas,
) -> Result<FinalizationState> {
    if event.at < view.latest_at()
        || event.input != Some(view.digest()?)
        || event.policy != data.selection().map(|s| s.policy.clone())
        || event.refs != refs(data)
    {
        return Err(Denial::new(
            "finalization_attribution",
            "Finalization differs from its recorded boundary",
        ));
    }
    let mut next = view.finalization().clone();
    if let Some(selection) = data.selection() {
        schemas.validate(selection)?;
    }
    match data {
        FinalizationRecorded::Policy { effective, change } => {
            if !matches!(
                effective.policy.port.as_str(),
                "ContextComposer" | "ReviewerPolicy" | "NarrativeComposer" | "ClaimAuditor"
            ) {
                return Err(Denial::new("policy_port", "Not an A11 strategy"));
            }
            if view.admission().assignments().values().any(|a| {
                matches!(
                    a.intent.assignment.role,
                    ymp_domain::assignment::RoleKind::FinalReviewer
                        | ymp_domain::assignment::RoleKind::Narrator
                ) && crate::gatekeeper::unresolved(view, a)
            }) {
                return Err(Denial::new(
                    "reporting_boundary",
                    "Finish the owned final review or narration before changing its strategy",
                ));
            }
            crate::ledger::selection(view, effective, Some(change), &effective.policy.port)?;
        }
        FinalizationRecorded::Context(data) => context::validate(view, data)?,
        FinalizationRecorded::ReportPrepared(data) => {
            ymp_domain::require_text(&data.reason, 4096)?;
            if report::prepared(view).is_ok()
                || view.treasury().is_none_or(|t| t.reporting_mode.is_none())
                || data.narrated
                    && (view.finalization().control != Some(Continuation::Continue)
                        || view.owner_stopped()
                        || view.treasury().unwrap().reporting_mode
                            != Some(ymp_domain::resources::ReportingMode::Narrated))
            {
                return Err(Denial::new(
                    "report_phase",
                    "Report preparation differs from its actual reporting mode and continuation",
                ));
            }
            if next.outcome.is_none() && next.acceptance.is_none() {
                next.outcome = Some(SessionStatus::Blocked(
                    "final_acceptance_unavailable".into(),
                ));
            }
        }
        FinalizationRecorded::NarrationWork(data) => {
            narration::validate_work(view, data)?;
            next.pending_work = Some(data.contribution.clone());
        }
        FinalizationRecorded::Narrative(data) => report::validate_narrative(view, data)?,
        FinalizationRecorded::Audit(data) => report::validate_audit(view, data)?,
        FinalizationRecorded::Delivered(data) => {
            if **data != report::delivered(view)? {
                return Err(Denial::new(
                    "report_delivery",
                    "Delivered facts, audit or accounting differ from recorded state",
                ));
            }
            next.delivered = Some((**data).clone());
            next.fence = None;
        }
        FinalizationRecorded::CandidateReviewer(data) => {
            review::validate_candidate_reviewer(view, data)?
        }
        FinalizationRecorded::Reviewer(data) => {
            review::validate_reviewer(view, data)?;
            if data.decision.outcome.is_none() {
                next.outcome = Some(SessionStatus::Blocked("final_review_pending".into()));
            } else if matches!(&next.outcome, Some(SessionStatus::Blocked(reason)) if reason == "final_review_pending")
            {
                next.outcome = Some(SessionStatus::Finalizing);
            }
        }
        FinalizationRecorded::Reviewed(data) => {
            if **data != review::paid_review(view, &data.invocation)? {
                return Err(Denial::new(
                    "final_review_attribution",
                    "Final review differs from its actual paid source",
                ));
            }
            next.reviews.push((**data).clone());
        }
        FinalizationRecorded::Accepted(data) => {
            let aggregate = review::final_phase(view)?;
            if data.aggregate != aggregate.reference()?
                || data.context != context(aggregate)?
                || data.acceptance
                    != review::acceptance_value(
                        view,
                        data.acceptance.id.clone(),
                        &data.rules,
                        event.at,
                    )?
                || data.credit.selection_change.is_some()
                || view.policies().get("CreditPolicy") != Some(&data.credit.effective)
            {
                return Err(Denial::new(
                    "final_acceptance",
                    "Final acceptance differs from its exact aggregate, independent review or grade",
                ));
            }
            crate::acceptance::decisions::validate_credit(&data.acceptance, &data.credit)?;
            next.acceptance = Some((**data).clone());
            next.outcome = Some(
                if data.acceptance.decision
                    == ymp_domain::verification::AcceptanceDecision::Accepted
                {
                    SessionStatus::Finalizing
                } else {
                    SessionStatus::Blocked("final_acceptance_rejected".into())
                },
            );
        }
        FinalizationRecorded::Control(control) => {
            if view.owner_stopped() && *control == Continuation::Continue {
                return Err(Denial::new(
                    "continuation_stopped",
                    "A stopped finalization cannot silently resume paid work",
                ));
            }
            next.control = Some(*control);
            if *control == Continuation::Stopped {
                next.stopped = true;
                next.outcome = Some(SessionStatus::Cancelled);
            }
        }
        FinalizationRecorded::Blocked(reason) => {
            ymp_domain::require_text(reason, 4096)?;
            if next.outcome != Some(SessionStatus::Cancelled) {
                next.outcome = Some(SessionStatus::Blocked(reason.clone()));
            }
        }
        FinalizationRecorded::Started(fence) => {
            if view.owner_stopped()
                || next.control != Some(Continuation::Continue)
                || next.fence.is_some()
                || view.treasury().is_some_and(|t| t.reporting_mode.is_some())
            {
                return Err(Denial::new(
                    "finalization_phase",
                    "Final checks and review precede reporting and require continuation authority",
                ));
            }
            let workspace = view
                .workspaces()
                .get(&fence.workspace)
                .ok_or_else(|| Denial::new("workspace_missing", "No final target"))?;
            fence.observation.validate(&workspace.location)?;
            if fence.holder
                != Id::new(format!(
                    "final-fence-{}",
                    Digest::of_value(&(view.session(), &fence.snapshot))?
                ))?
                || view
                    .admission()
                    .assignments()
                    .keys()
                    .any(|id| id.erased() == fence.holder)
                || view
                    .capture_reads()
                    .keys()
                    .any(|id| id.erased() == fence.holder)
            {
                return Err(Denial::new(
                    "final_fence",
                    "Final fence identity is not canonical or collides with existing authority",
                ));
            }
            if workspace.kind != WorkspaceKind::Direct
                || workspace.provider.policy.implementation != "Direct"
                || fence.observation.path != WorkspacePath::root()
                || view.contract().unwrap().reference() != fence.contract
                || view.snapshots().contains_key(&fence.snapshot)
            {
                return Err(Denial::new(
                    "final_target",
                    "Use one exact Direct target and a fresh snapshot identifier",
                ));
            }
            writable_effects(view, &fence.observation)?;
            next.fence = Some((event.reference()?, fence.clone()));
            if matches!(&next.outcome, Some(SessionStatus::Blocked(reason)) if reason == "effects_uncertain")
            {
                next.outcome = Some(SessionStatus::Finalizing);
            }
        }
        FinalizationRecorded::Captured(data) => {
            if next.aggregate.is_some()
                || next.control != Some(Continuation::Continue)
                || view.owner_stopped()
                || view.treasury().is_some_and(|t| t.reporting_mode.is_some())
            {
                return Err(Denial::new(
                    "final_scope_exists",
                    "The final aggregate is immutable and capture precedes reporting",
                ));
            }
            for c in &data.checks {
                c.environment.validate()?;
                schemas.validate(&c.environment.runner)?;
            }
            validate_aggregate(view, data)?;
            next.aggregate = Some(data.clone());
        }
    }
    if next.history.len() >= 4096 {
        return Err(Denial::new(
            "finalization_limit",
            "Finalization history capacity exhausted",
        ));
    }
    next.history.push((event.reference()?, data.clone()));
    Ok(next)
}
impl<J: Journal, C: ContentStore> Finalization<J, C> {
    pub(crate) fn new(journal: Arc<J>, content: Arc<C>, owner: DecisionConsumer<J>) -> Self {
        Self {
            workspace: WorkspaceGuard::new(journal.clone(), content.clone()),
            journal,
            content,
            owner,
        }
    }
    fn append(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        data: FinalizationRecorded,
    ) -> Result<Ref> {
        let session = self.owner.authorize(owner)?;
        let current = self.journal.read(&session)?;
        let view = current.view_with_schemas(&session, None, self.journal.schemas())?;
        let event = Envelope {
            seq: expected
                .checked_add(1)
                .ok_or_else(|| Denial::new("revision_overflow", "Journal exhausted"))?,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: data.selection().map(|s| s.policy.clone()),
            input: Some(view.digest()?),
            refs: refs(&data),
            payload: Event::FinalizationRecorded {
                version: 1,
                data: Box::new(data),
            },
        };
        validate_append(
            &current,
            &session,
            expected,
            std::slice::from_ref(&event),
            self.journal.schemas(),
        )?;
        self.journal
            .append(&session, expected, std::slice::from_ref(&event))?;
        event.reference()
    }
    pub fn control(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        control: Continuation,
    ) -> Result<Ref> {
        self.append(owner, expected, at, FinalizationRecorded::Control(control))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn capture(
        &self,
        owner: &SessionControl,
        expected: u64,
        at: u64,
        workspace: Id<Workspace>,
        snapshot: Id<Snapshot>,
        checks: Vec<FinalCheck>,
        provider: &dyn WorkspaceProvider,
    ) -> Result<Option<FinalAggregate>> {
        let session = self.owner.authorize(owner)?;
        let inventory = self.journal.workspace_inventory(&session)?;
        let view = inventory
            .current
            .view_with_schemas(&session, None, self.journal.schemas())?;
        if view.revision() != expected {
            return Err(Denial::new("stale_revision", "Final target changed"));
        }
        if view.finalization().control != Some(Continuation::Continue)
            || view.treasury().is_some_and(|t| t.reporting_mode.is_some())
        {
            self.append(
                owner,
                expected,
                at,
                FinalizationRecorded::Blocked(
                    "final_acceptance_unavailable: no continuation or reporting already started"
                        .into(),
                ),
            )?;
            return Ok(None);
        }
        let observation = provider
            .observe_paths(&[WorkspacePath::root()])?
            .into_iter()
            .next()
            .ok_or_else(|| {
                Denial::new("final_target", "Provider did not observe the target root")
            })?;
        let effects = writable_effects(&view, &observation).and_then(|()| {
            inventory
                .other
                .iter()
                .try_for_each(|o| o.finalization_effects(&observation))
        });
        if effects.is_err() {
            self.append(
                owner,
                expected,
                at,
                FinalizationRecorded::Blocked("effects_uncertain".into()),
            )?;
            return Ok(None);
        }
        let fence = FinalFence {
            workspace: workspace.clone(),
            observation,
            holder: Id::new(format!(
                "final-fence-{}",
                Digest::of_value(&(&session, &snapshot))?
            ))?,
            snapshot: snapshot.clone(),
            contract: view
                .contract()
                .ok_or_else(|| Denial::new("contract_missing", "Finalization requires a task"))?
                .reference(),
        };
        self.append(owner, expected, at, FinalizationRecorded::Started(fence))?;
        let view = self.journal.view(&session, None)?;
        self.workspace.snapshot(
            &session,
            view.revision(),
            at,
            &workspace,
            snapshot.clone(),
            provider,
        )?;
        let view = self.journal.view(&session, None)?;
        let mut sources = vec![];
        let mut producers = BTreeSet::new();
        let mut baselines = BTreeSet::new();
        for item in view.results().items().values() {
            if let Some(id) = &item.accepted {
                let r = &view.results().results()[id];
                let a = view
                    .acceptances()
                    .values()
                    .find(|a| {
                        a.acceptance.subject == AcceptanceSubject::ResultVersion(id.clone())
                            && a.acceptance.decision == AcceptanceDecision::Accepted
                    })
                    .ok_or_else(|| Denial::new("final_source", "No source acceptance"))?;
                sources.push(FinalSource {
                    result: r.reference()?,
                    acceptance: a.acceptance.reference()?,
                });
                producers.insert(r.producer.clone());
                baselines.insert(view.snapshots()[&r.before].reference()?);
            }
        }
        sources.sort_by(|a, b| a.result.cmp(&b.result));
        let aggregate = FinalAggregate {
            session: session.clone(),
            snapshot: view.snapshots()[&snapshot].reference()?,
            contract: view.contract().unwrap().reference(),
            criteria: view
                .criteria()
                .iter()
                .map(|c| c.reference())
                .collect::<Result<_>>()?,
            checks,
            sources,
            producers,
            baseline: if baselines.len() == 1 {
                baselines.into_iter().next()
            } else {
                None
            },
        };
        if let Err(error) = validate_aggregate(&view, &aggregate) {
            self.append(
                owner,
                view.revision(),
                at,
                FinalizationRecorded::Blocked(error.code),
            )?;
            return Ok(None);
        }
        self.append(
            owner,
            view.revision(),
            at,
            FinalizationRecorded::Captured(aggregate.clone()),
        )?;
        Ok(Some(aggregate))
    }
}

pub fn context(aggregate: &FinalAggregate) -> Result<crate::acceptance::ApplicabilityContext> {
    Ok(crate::acceptance::ApplicabilityContext {
        result: aggregate.reference()?,
        criteria: aggregate.criteria.clone(),
        environments: aggregate
            .checks
            .iter()
            .map(|c| {
                Ok((
                    c.check.clone(),
                    BTreeSet::from([Digest::of_value(&c.environment)?]),
                ))
            })
            .collect::<Result<_>>()?,
    })
}

impl FinalizationRecorded {
    pub fn selection(&self) -> Option<&ymp_domain::journal::PolicySelection> {
        match self {
            Self::Policy { effective, .. } => Some(effective),
            Self::Context(data) => Some(&data.decision.effective),
            Self::Reviewer(data) | Self::CandidateReviewer(data) => Some(&data.decision.effective),
            Self::Narrative(data) => Some(&data.decision.effective),
            Self::Audit(data) => data.decisions.first().map(|d| &d.effective),
            Self::Accepted(data) => Some(&data.credit.effective),
            _ => None,
        }
    }
}

pub(crate) fn validate_admission(
    view: &SessionView,
    intent: &crate::gatekeeper::AdmissionIntent,
) -> Result<()> {
    if view.finalization().control.is_some()
        && view.finalization().control != Some(Continuation::Continue)
    {
        return Err(Denial::new(
            "continuation_required",
            "No new model assignment after stop or without continuation authority",
        ));
    }
    if view
        .finalization()
        .fence
        .as_ref()
        .is_some_and(|(_, f)| f.holder == intent.assignment.id.erased())
    {
        return Err(Denial::new(
            "final_fence",
            "Finalization fence identity cannot be reused as an assignment",
        ));
    }
    review::validate_admission(view, intent)?;
    narration::validate_admission(view, intent)
}
pub(crate) fn execution_allowed(
    view: &SessionView,
    assignment: &ymp_domain::assignment::Assignment,
) -> Result<()> {
    if view.owner_stopped() {
        return Err(Denial::new(
            "session_stopped",
            "Owner stop forbids a new model call",
        ));
    }

    if assignment.role == ymp_domain::assignment::RoleKind::Narrator
        && view.policies().contains_key("NarrativeComposer")
    {
        let work = narration::work(view, &assignment.contribution)
            .ok_or_else(|| Denial::new("narration_work", "No semantic narration authorization"))?;
        narration::continuing_work(view, work)?;
    }
    if view.finalization().delivered.is_some()
        && assignment.role != ymp_domain::assignment::RoleKind::Curator
    {
        return Err(Denial::new(
            "report_delivered",
            "No further work calls after report delivery",
        ));
    }
    if view.finalization().control.is_some()
        && view.finalization().control != Some(Continuation::Continue)
    {
        return Err(Denial::new(
            "continuation_required",
            "Owner stop or missing continuation forbids a new model call",
        ));
    }
    let latest_context =
        view.finalization()
            .history
            .iter()
            .rev()
            .find_map(|(_, event)| match event {
                FinalizationRecorded::Context(context)
                    if context.input.assignment.id == assignment.id =>
                {
                    Some(context)
                }
                _ => None,
            });
    if latest_context.is_some_and(|context| context.input.assignment.profile != assignment.profile)
    {
        return Err(Denial::new(
            "context_identity",
            "Prompt belongs to another native profile",
        ));
    }
    Ok(())
}
pub(crate) fn validate_dispatch(
    view: &SessionView,
    data: &crate::execution::InvocationDispatch,
) -> Result<()> {
    execution_allowed(view, &data.assignment)?;
    let context = view.finalization().history.iter().rev().find_map(|(_, e)| {
        if let FinalizationRecorded::Context(c) = e {
            (c.input.assignment.id == data.assignment.id).then_some(c)
        } else {
            None
        }
    });
    if let Some(context) = context {
        if context.decision.outcome != data.prompt {
            return Err(Denial::new(
                "context_prompt",
                "Dispatch must preserve its attributed context",
            ));
        }
    } else if matches!(
        data.assignment.role,
        ymp_domain::assignment::RoleKind::FinalReviewer
            | ymp_domain::assignment::RoleKind::Narrator
    ) && view.policies().contains_key("ContextComposer")
    {
        return Err(Denial::new(
            "final_review_context",
            "Final review requires a committed role context",
        ));
    }
    Ok(())
}

impl FinalizationState {
    pub(crate) fn check_next(&self, event: &Event) -> Result<()> {
        if let Some(work) = &self.pending_work
            && !matches!(event,Event::ContributionProposed{contribution,..} if contribution.as_ref()==work)
        {
            return Err(Denial::new(
                "narration_incomplete",
                "Narration authorization and contribution commit together",
            ));
        }
        Ok(())
    }
    pub(crate) fn complete(&self) -> Result<()> {
        if self.pending_work.is_some() {
            return Err(Denial::new(
                "narration_incomplete",
                "Missing reporting contribution",
            ));
        }
        Ok(())
    }
}
