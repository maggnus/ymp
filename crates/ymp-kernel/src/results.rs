//! Immutable candidates and their recorded production responsibility.
use crate::{
    events::Event,
    gatekeeper::{Gatekeeper, GrantToken},
    journal::{AppendResolution, ContentStore, Journal},
    view::SessionView,
    workspace_locks::LockChange,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    assignment::*,
    journal::{Actor, Capability, Envelope},
    plan::*,
    result::{Artifact, ResultVersion},
    workspace::{LockMode, Snapshot, Workspace, WorkspacePath},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRecord {
    pub plan: Plan,
    pub item: WorkItem,
    pub contract: Ref,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptRecord {
    pub attempt: Attempt,
    pub before: Id<Snapshot>,
    pub after: Id<Snapshot>,
    pub owner: Digest,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ResultsView {
    plans: BTreeMap<Id<Plan>, PlanRecord>,
    items: BTreeMap<Id<WorkItem>, WorkItem>,
    attempts: BTreeMap<Id<Attempt>, AttemptRecord>,
    results: BTreeMap<Id<ResultVersion>, ResultVersion>,
    captures: BTreeMap<Id<Attempt>, Ref>,
    pending_capture: Option<(Id<Attempt>, Id<Workspace>, Id<Snapshot>)>,
}
impl ResultsView {
    pub fn plans(&self) -> &BTreeMap<Id<Plan>, PlanRecord> {
        &self.plans
    }
    pub fn items(&self) -> &BTreeMap<Id<WorkItem>, WorkItem> {
        &self.items
    }
    pub fn attempts(&self) -> &BTreeMap<Id<Attempt>, AttemptRecord> {
        &self.attempts
    }
    pub fn results(&self) -> &BTreeMap<Id<ResultVersion>, ResultVersion> {
        &self.results
    }
    pub(crate) fn complete(&self) -> Result<()> {
        if self.pending_capture.is_some() {
            return Err(Denial::new(
                "result_capture_packet",
                "Releasing production ownership must acquire its result capture in the same packet",
            ));
        }
        Ok(())
    }
    pub fn pending_for(&self, assignment: &Id<Assignment>) -> Option<&AttemptRecord> {
        self.attempts.values().find(|r| {
            &r.attempt.assignment == assignment && r.attempt.outcome == AttemptOutcome::Pending
        })
    }
    pub(crate) fn control_reserve(&self) -> usize {
        self.attempts
            .values()
            .map(|r| match r.attempt.outcome {
                AttemptOutcome::Pending => {
                    if self.captures.contains_key(&r.attempt.id) {
                        2
                    } else {
                        4
                    }
                }
                AttemptOutcome::Submitted => 1,
                _ => 0,
            })
            .sum()
    }
}
pub(crate) fn subject_item<'a>(
    view: &'a SessionView,
    contribution: &Contribution,
) -> Result<Option<&'a WorkItem>> {
    let Some(subject) = &contribution.subject else {
        return Ok(None);
    };
    if matches!(subject, ContributionSubject::ResultVersion(_)) {
        review_subject(view, contribution)?;
        return Ok(None);
    }
    let ContributionSubject::WorkItem(reference) = subject else {
        return Err(Denial::new(
            "subject_unsupported",
            "This subject needs its owning consumer",
        )
        .with_ref(subject.reference().clone()));
    };
    let item = view
        .results()
        .items
        .values()
        .find(|item| item.reference().is_ok_and(|r| r == *reference))
        .ok_or_else(|| Denial::new("work_item_missing", "No exact committed work definition"))?;
    if !matches!(
        contribution.kind,
        ContributionKind::Produce | ContributionKind::Alternative
    ) || contribution.targets != item.targets
        || contribution.needs != item.needs
        || view.results().plans[&item.plan].contract != view.contract().unwrap().reference()
    {
        return Err(Denial::new(
            "work_item_subject",
            "Production differs from the recorded work definition or contract",
        ));
    }
    Ok(Some(item))
}
pub(crate) fn review_subject<'a>(
    view: &'a SessionView,
    contribution: &Contribution,
) -> Result<Option<&'a ResultVersion>> {
    let Some(ContributionSubject::ResultVersion(reference)) = &contribution.subject else {
        return Ok(None);
    };
    if contribution.kind != ContributionKind::Review {
        return Err(Denial::new(
            "subject_unsupported",
            "Only Review supports a ResultVersion subject here",
        )
        .with_ref(reference.clone()));
    }
    let result = view
        .results()
        .results
        .values()
        .find(|r| r.reference().is_ok_and(|r| r == *reference))
        .ok_or_else(|| Denial::new("review_subject", "No exact retained result for review"))?;
    let item = &view.results().items[&result.item];
    if contribution.targets != item.targets {
        return Err(Denial::new(
            "review_subject",
            "Only review of the exact result and all its targets is available",
        ));
    }
    Ok(Some(result))
}
pub(crate) fn validate_admission(view: &SessionView, contribution: &Contribution) -> Result<()> {
    if let Some(item) = subject_item(view, contribution)?
        && (item.state != WorkState::Open || item.accepted.is_some())
    {
        return Err(Denial::new(
            "work_item_state",
            "This work is already committed, under review or accepted",
        ));
    }
    Ok(())
}
pub(crate) fn validate_paths(
    view: &SessionView,
    assignment: &Assignment,
    lock: &crate::workspace_locks::LockAcquisition,
) -> Result<()> {
    let contribution = &view.coordination().contributions()[&assignment.contribution].value;
    if let Some(item) = subject_item(view, contribution)? {
        let writes: BTreeSet<_> = lock
            .requested
            .iter()
            .filter(|p| p.mode == LockMode::Write)
            .map(|p| p.path.clone())
            .collect();
        if writes != item.writes {
            return Err(Denial::new(
                "work_item_paths",
                "Admission must preserve the work item's declared write scope",
            ));
        }
    }
    Ok(())
}
pub(crate) fn validate_dispatch(view: &SessionView, assignment: &Assignment) -> Result<()> {
    let contribution = &view
        .coordination()
        .contributions()
        .get(&assignment.contribution)
        .ok_or_else(|| Denial::new("contribution_missing", "No production contribution"))?
        .value;
    if subject_item(view, contribution)?.is_some()
        && view.results().pending_for(&assignment.id).is_none()
    {
        return Err(Denial::new(
            "attempt_missing",
            "Production requires its recorded before snapshot and Pending Attempt",
        ));
    }
    Ok(())
}
pub(crate) fn attribution(view: &SessionView, event: &Event) -> Result<Vec<Ref>> {
    let mut refs = match event {
        Event::PlanCommitted { data, .. } => vec![
            data.contract.clone(),
            view.admission()
                .assignments()
                .get(&data.plan.author)
                .ok_or_else(|| Denial::new("plan_author", "No admitted planner"))?
                .references
                .last()
                .unwrap()
                .clone(),
        ],
        Event::AttemptStarted { data, .. } => vec![
            view.results()
                .items
                .get(&data.attempt.item)
                .ok_or_else(|| Denial::new("work_item_missing", "No work item"))?
                .reference()?,
            view.snapshots()
                .get(&data.before)
                .ok_or_else(|| Denial::new("snapshot_missing", "No before snapshot"))?
                .reference()?,
            view.admission()
                .assignments()
                .get(&data.attempt.assignment)
                .ok_or_else(|| Denial::new("assignment_missing", "No producer"))?
                .references
                .last()
                .unwrap()
                .clone(),
        ],
        Event::ResultSubmitted {
            attempt, result, ..
        } => {
            let record = view
                .results()
                .attempts
                .get(attempt)
                .ok_or_else(|| Denial::new("attempt_missing", "No production attempt"))?;
            let invocation = view
                .execution()
                .invocations()
                .values()
                .find(|r| r.dispatch.assignment.id == record.attempt.assignment)
                .ok_or_else(|| Denial::new("invocation_missing", "No producer invocation"))?;
            vec![
                view.results().items[&record.attempt.item].reference()?,
                view.snapshots()
                    .get(&result.before)
                    .ok_or_else(|| Denial::new("snapshot_missing", "No before snapshot"))?
                    .reference()?,
                view.snapshots()
                    .get(&result.after)
                    .ok_or_else(|| Denial::new("snapshot_missing", "No after snapshot"))?
                    .reference()?,
                invocation
                    .end
                    .clone()
                    .ok_or_else(|| Denial::new("result_unfinished", "No completed invocation"))?,
            ]
        }
        Event::AttemptAbandoned { attempt, .. } => {
            let record = view
                .results()
                .attempts
                .get(attempt)
                .ok_or_else(|| Denial::new("attempt_missing", "No production attempt"))?;
            let mut refs = vec![view.results().items[&record.attempt.item].reference()?];
            if let Some(result) = &record.attempt.result {
                refs.push(view.results().results[result].reference()?);
            }
            refs
        }
        _ => return Err(Denial::new("result_event", "Not a result event")),
    };
    refs.sort();
    refs.dedup();
    Ok(refs)
}
fn validate_plan(view: &SessionView, data: &PlanRecord, at: u64) -> Result<()> {
    data.plan.validate()?;
    data.item.validate()?;
    let assignment = view
        .admission()
        .assignments()
        .get(&data.plan.author)
        .ok_or_else(|| Denial::new("plan_author", "A real admitted planner is required"))?;
    let author = &assignment.intent.assignment;
    let contribution = &view.coordination().contributions()[&author.contribution].value;
    crate::gatekeeper::validate_active_grant(view, assignment, None, at)?;
    if author.role != RoleKind::Planner
        || contribution.kind != ContributionKind::Plan
        || data.plan.session != *view.session()
        || data.plan.version != 1
        || !view.results().plans.is_empty()
        || data.plan.items != BTreeSet::from([data.item.id.clone()])
        || data.item.plan != data.plan.id
        || data.item.state != WorkState::Open
        || !data.item.attempts.is_empty()
        || data.item.accepted.is_some()
        || !data.item.deps.is_empty()
        || data.item.parent.is_some()
        || data.contract != view.contract().unwrap().reference()
        || !data.item.targets.is_subset(&contribution.targets)
        || !data
            .item
            .needs
            .is_subset(&view.task().unwrap().constraints.allowed)
        || data.item.writes.is_empty()
        || !data.item.needs.contains(&Capability::WriteFiles)
    {
        return Err(Denial::new(
            "plan_definition",
            "The initial one-item production plan must match its planner and current contract",
        ));
    }
    Ok(())
}
fn validate_attempt(view: &SessionView, data: &AttemptRecord, at: u64) -> Result<()> {
    data.attempt.validate()?;
    let source = view
        .admission()
        .assignments()
        .get(&data.attempt.assignment)
        .ok_or_else(|| Denial::new("assignment_missing", "Attempt needs its admitted producer"))?;
    crate::gatekeeper::validate_active_grant(view, source, None, at)?;
    let assignment = &source.intent.assignment;
    let contribution = &view.coordination().contributions()[&assignment.contribution].value;
    let item = subject_item(view, contribution)?
        .ok_or_else(|| Denial::new("work_item_missing", "Attempt needs a typed work subject"))?;
    let before = view
        .snapshots()
        .get(&data.before)
        .ok_or_else(|| Denial::new("snapshot_missing", "Before state must already be retained"))?;
    let lock = view
        .path_locks()
        .get(&assignment.id.erased())
        .ok_or_else(|| Denial::new("locks_missing", "No production path ownership"))?;
    let writes: BTreeSet<_> = lock
        .acquired
        .requested
        .iter()
        .filter(|p| p.mode == LockMode::Write)
        .map(|p| p.path.clone())
        .collect();
    if data.attempt.item != item.id
        || assignment.role != RoleKind::Producer
        || data.attempt.workspace != assignment.workspace
        || before.workspace != assignment.workspace
        || data.attempt.outcome != AttemptOutcome::Pending
        || data.attempt.result.is_some()
        || assignment.state != AssignmentState::Admitted
        || lock.invocation.is_some()
        || lock.released.is_some()
        || writes != item.writes
        || item.state != WorkState::Committed
        || view.results().attempts.contains_key(&data.attempt.id)
        || view
            .results()
            .attempts
            .values()
            .any(|r| r.attempt.assignment == assignment.id || r.after == data.after)
        || view.snapshots().contains_key(&data.after)
        || view.capture_reads().contains_key(&data.after)
        || data.before == data.after
        || view.results().attempts.len() >= 4096
        || view
            .capture_reads()
            .get(&data.before)
            .is_none_or(|capture| {
                capture.ended.is_none()
                    || capture.failure.is_some()
                    || capture.protected_by.as_ref() != Some(&assignment.id.erased())
            })
    {
        return Err(Denial::new(
            "attempt_binding",
            "Attempt differs from its unused producer, paths or retained baseline",
        ));
    }
    Ok(())
}
fn validate_result(
    view: &SessionView,
    attempt: &Id<Attempt>,
    result: &ResultVersion,
) -> Result<()> {
    result.validate()?;
    let record = view
        .results()
        .attempts
        .get(attempt)
        .ok_or_else(|| Denial::new("attempt_missing", "No attempt"))?;
    let assignment = &view.admission().assignments()[&record.attempt.assignment]
        .intent
        .assignment;
    let invocation = view
        .execution()
        .invocations()
        .values()
        .find(|r| r.dispatch.assignment.id == assignment.id)
        .ok_or_else(|| Denial::new("invocation_missing", "No actual producer execution"))?;
    let after = view
        .snapshots()
        .get(&result.after)
        .ok_or_else(|| Denial::new("snapshot_missing", "No retained after state"))?;
    let capture = view
        .capture_reads()
        .get(&result.after)
        .ok_or_else(|| Denial::new("capture_missing", "Result has no scoped capture"))?;
    let item = &view.results().items[&record.attempt.item];
    if record.attempt.outcome != AttemptOutcome::Pending
        || record.attempt.result.is_some()
        || result.item != record.attempt.item
        || result.producer != assignment.agent
        || result.profile != assignment.profile
        || result.before != record.before
        || result.after != record.after
        || after.workspace != assignment.workspace
        || invocation.terminal != Some(InvocationTerminal::Completed)
        || !invocation.confirmed_terminal
        || !crate::execution::closed(view, &assignment.id)
        || view.results().captures.get(attempt) != Some(&capture.started)
        || capture.ended.is_none()
        || capture.failure.is_some()
        || item.accepted.is_some()
        || view.results().results.contains_key(&result.id)
    {
        return Err(Denial::new(
            "result_binding",
            "Candidate lacks attributable completed production and its exact immutable snapshots",
        ));
    }
    for artifact in &result.artifacts {
        if after
            .tree
            .files
            .get(&artifact.path)
            .is_none_or(|file| file.digest != artifact.digest)
            || !item.writes.iter().any(|path| path.contains(&artifact.path))
        {
            return Err(Denial::new(
                "artifact_binding",
                "Artifact digest or path differs from the captured production scope",
            ));
        }
    }
    Ok(())
}
pub(crate) fn apply(view: &SessionView, event: &Envelope<Event>) -> Result<ResultsView> {
    if matches!(
        event.payload,
        Event::PlanCommitted { .. }
            | Event::AttemptStarted { .. }
            | Event::ResultSubmitted { .. }
            | Event::AttemptAbandoned { .. }
    ) && event.at < view.latest_at()
    {
        return Err(Denial::new(
            "result_time",
            "Result transition predates its recorded inputs",
        ));
    }
    let mut next = view.results().clone();
    if let Some((attempt, workspace, snapshot)) = &next.pending_capture {
        let Event::LockChanged {
            change:
                LockChange::CaptureStarted {
                    workspace: actual,
                    snapshot: id,
                    ..
                },
            ..
        } = &event.payload
        else {
            return Err(Denial::new(
                "result_capture_packet",
                "Production release must transfer directly to its capture",
            ));
        };
        if actual != workspace || id != snapshot {
            return Err(Denial::new(
                "result_capture_packet",
                "Foreign production capture",
            ));
        }
        next.captures.insert(attempt.clone(), event.reference()?);
        next.pending_capture = None;
        return Ok(next);
    }
    match &event.payload {
        Event::PlanCommitted { data, .. } => {
            validate_plan(view, data, event.at)?;
            next.plans.insert(data.plan.id.clone(), (**data).clone());
            next.items.insert(data.item.id.clone(), data.item.clone());
        }
        Event::AttemptStarted { data, .. } => {
            validate_attempt(view, data, event.at)?;
            next.items
                .get_mut(&data.attempt.item)
                .unwrap()
                .attempts
                .push(data.attempt.id.clone());
            next.attempts
                .insert(data.attempt.id.clone(), (**data).clone());
        }
        Event::ResultSubmitted {
            attempt, result, ..
        } => {
            validate_result(view, attempt, result)?;
            let record = next.attempts.get_mut(attempt).unwrap();
            record.attempt.result = Some(result.id.clone());
            record.attempt.outcome = AttemptOutcome::Submitted;
            next.items.get_mut(&record.attempt.item).unwrap().state = WorkState::InReview;
            next.results.insert(result.id.clone(), (**result).clone());
        }
        Event::AttemptAbandoned {
            attempt, reason, ..
        } => {
            ymp_domain::require_text(reason, 1024)?;
            let record = next
                .attempts
                .get_mut(attempt)
                .ok_or_else(|| Denial::new("attempt_missing", "No attempt"))?;
            if !matches!(
                record.attempt.outcome,
                AttemptOutcome::Pending | AttemptOutcome::Submitted
            ) || next.items[&record.attempt.item].accepted.is_some()
                || view.admission().assignments()[&record.attempt.assignment]
                    .intent
                    .assignment
                    .state
                    != AssignmentState::Revoked
            {
                return Err(Denial::new(
                    "attempt_state",
                    "Abandoning unaccepted work first revokes production authority and retains unresolved holds",
                ));
            }
            record.attempt.outcome = AttemptOutcome::Abandoned;
            next.items.get_mut(&record.attempt.item).unwrap().state = WorkState::Open;
        }
        Event::AssignmentAdmitted { assignment, .. } => {
            // The final admission event sees the pending intent through its proposed commitment.
            if let Some(source) = crate::gatekeeper::pending_assignment(view, assignment) {
                let contribution = &view.coordination().contributions()[&source.contribution].value;
                if let Some(item) = subject_item(view, contribution)? {
                    next.items.get_mut(&item.id).unwrap().state = WorkState::Committed;
                }
            }
        }
        Event::InvocationStarted { invocation, .. } => {
            if let Some(record) = next.pending_for(&invocation.assignment) {
                let item = record.attempt.item.clone();
                next.items.get_mut(&item).unwrap().state = WorkState::Running;
            }
        }
        Event::LockChanged {
            change: LockChange::Released(proof),
            ..
        } => {
            if let Some(record) = next.attempts.values().find(|r| {
                r.attempt.assignment.erased() == proof.assignment
                    && r.attempt.outcome == AttemptOutcome::Pending
            }) {
                next.pending_capture = Some((
                    record.attempt.id.clone(),
                    record.attempt.workspace.clone(),
                    record.after.clone(),
                ));
            }
        }
        _ => {}
    }
    Ok(next)
}

pub struct PreparedAttempt {
    issuer: Arc<()>,
    session: Id,
    expected: u64,
    events: Vec<Envelope<Event>>,
    data: AttemptRecord,
}
impl PreparedAttempt {
    pub fn id(&self) -> &Id<Attempt> {
        &self.data.attempt.id
    }
}
pub struct Results<J: Journal, C: ContentStore> {
    journal: Arc<J>,
    gate: Arc<Gatekeeper<J, C>>,
    issuer: Arc<()>,
}
impl<J: Journal, C: ContentStore> Results<J, C> {
    pub fn new(journal: Arc<J>, gate: Arc<Gatekeeper<J, C>>) -> Result<Self> {
        gate.require_journal(&journal)?;
        Ok(Self {
            journal,
            gate,
            issuer: Arc::new(()),
        })
    }
    fn events(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        payload: Event,
    ) -> Result<Vec<Envelope<Event>>> {
        let view = self.journal.view(session, None)?;
        if view.revision() != expected {
            return Err(Denial::new("stale_revision", "Result inputs changed"));
        }
        Ok(vec![Envelope {
            seq: expected + 1,
            session: session.clone(),
            at,
            actor: Actor::Runtime,
            policy: None,
            input: None,
            refs: attribution(&view, &payload)?,
            payload,
        }])
    }
    fn append(&self, session: &Id, expected: u64, events: &[Envelope<Event>]) -> Result<()> {
        if matches!(
            self.journal.resolve_append(session, expected, events)?,
            AppendResolution::Committed(_)
        ) {
            return Ok(());
        }
        let result = self.journal.append(session, expected, events);
        match self.journal.resolve_append(session, expected, events) {
            Ok(AppendResolution::Committed(_)) => Ok(()),
            _ => Err(result.err().unwrap_or_else(|| {
                Denial::new("result_uncertain", "Result commit is unconfirmed")
            })),
        }
    }
    pub fn commit_plan(
        &self,
        token: &GrantToken,
        expected: u64,
        at: u64,
        mut plan: Plan,
        item: WorkItem,
    ) -> Result<Ref> {
        let assignment = self.gate.authorize_revision(token, None, at, expected)?;
        plan.author = assignment.id;
        plan.session = assignment.session;
        let view = self.journal.view(token.session(), None)?;
        let data = PlanRecord {
            plan: plan.clone(),
            item,
            contract: view.contract().unwrap().reference(),
        };
        let events = self.events(
            token.session(),
            expected,
            at,
            Event::PlanCommitted {
                version: 1,
                data: Box::new(data),
            },
        )?;
        self.append(token.session(), expected, &events)?;
        plan.reference()
    }
    pub fn prepare_attempt(
        &self,
        token: &GrantToken,
        expected: u64,
        at: u64,
        id: Id<Attempt>,
        before: Id<Snapshot>,
        after: Id<Snapshot>,
    ) -> Result<PreparedAttempt> {
        let assignment = self.gate.authorize_revision(token, None, at, expected)?;
        let view = self.journal.view(token.session(), None)?;
        let contribution = &view.coordination().contributions()[&assignment.contribution].value;
        let item = subject_item(&view, contribution)?.ok_or_else(|| {
            Denial::new(
                "work_item_missing",
                "A production attempt needs a work item",
            )
        })?;
        let mut random = [0; 32];
        getrandom::fill(&mut random)
            .map_err(|_| Denial::new("attempt_entropy", "Cannot create an attempt owner"))?;
        let data = AttemptRecord {
            attempt: Attempt {
                id,
                item: item.id.clone(),
                assignment: assignment.id,
                workspace: assignment.workspace,
                result: None,
                outcome: AttemptOutcome::Pending,
            },
            before,
            after,
            owner: Digest::of(random),
        };
        self.gate
            .workspace()
            .retained(token.session(), &data.before)?;
        validate_attempt(&view, &data, at)?;
        let events = self.events(
            token.session(),
            expected,
            at,
            Event::AttemptStarted {
                version: 1,
                data: Box::new(data.clone()),
            },
        )?;
        Ok(PreparedAttempt {
            issuer: self.issuer.clone(),
            session: token.session().clone(),
            expected,
            events,
            data,
        })
    }
    fn own(&self, attempt: &PreparedAttempt) -> Result<()> {
        if !Arc::ptr_eq(&self.issuer, &attempt.issuer) {
            return Err(Denial::new(
                "attempt_owner",
                "Attempt belongs to another result consumer",
            ));
        }
        Ok(())
    }
    pub fn begin(&self, attempt: &PreparedAttempt) -> Result<()> {
        self.own(attempt)?;
        self.append(&attempt.session, attempt.expected, &attempt.events)
    }
    fn bound(&self, attempt: &PreparedAttempt) -> Result<SessionView> {
        self.own(attempt)?;
        let view = self.journal.view(&attempt.session, None)?;
        let recorded = view
            .results()
            .attempts
            .get(&attempt.data.attempt.id)
            .ok_or_else(|| Denial::new("attempt_missing", "Attempt is not committed"))?;
        if recorded.owner != attempt.data.owner
            || recorded.attempt.assignment != attempt.data.attempt.assignment
        {
            return Err(Denial::new(
                "attempt_owner",
                "Attempt differs from its live preparation",
            ));
        }
        Ok(view)
    }
    pub fn submit(
        &self,
        attempt: &PreparedAttempt,
        at: u64,
        id: Id<ResultVersion>,
        artifacts: BTreeSet<Artifact>,
        summary: String,
    ) -> Result<ResultVersion> {
        let view = self.bound(attempt)?;
        let assignment = &view.admission().assignments()[&attempt.data.attempt.assignment]
            .intent
            .assignment;
        self.gate
            .workspace()
            .retained(&attempt.session, &attempt.data.before)?;
        self.gate
            .workspace()
            .retained(&attempt.session, &attempt.data.after)?;
        let result = ResultVersion {
            id,
            item: attempt.data.attempt.item.clone(),
            producer: assignment.agent.clone(),
            profile: assignment.profile.clone(),
            before: attempt.data.before.clone(),
            after: attempt.data.after.clone(),
            artifacts,
            summary,
        };
        if let Some(prior) = view.results().results.get(&result.id) {
            if prior == &result
                && view.results().attempts[attempt.id()]
                    .attempt
                    .result
                    .as_ref()
                    == Some(&result.id)
            {
                return Ok(prior.clone());
            }
            return Err(Denial::new(
                "result_conflict",
                "A result identity cannot change its candidate",
            ));
        }
        let events = self.events(
            &attempt.session,
            view.revision(),
            at,
            Event::ResultSubmitted {
                version: 1,
                attempt: attempt.data.attempt.id.clone(),
                result: Box::new(result.clone()),
            },
        )?;
        self.append(&attempt.session, view.revision(), &events)?;
        Ok(result)
    }
    pub fn abandon(&self, attempt: &PreparedAttempt, at: u64, reason: String) -> Result<()> {
        let view = self.bound(attempt)?;
        if view.results().attempts[attempt.id()].attempt.outcome == AttemptOutcome::Abandoned {
            return Ok(());
        }
        let mut payloads = crate::gatekeeper::revocation_payloads(
            &view,
            &attempt.data.attempt.assignment,
            reason.clone(),
        )?;
        payloads.push(Event::AttemptAbandoned {
            version: 1,
            attempt: attempt.id().clone(),
            reason,
        });
        let current = self.journal.read(&attempt.session)?;
        if current.revision != view.revision() {
            return Err(Denial::new(
                "stale_revision",
                "Attempt changed before abandonment",
            ));
        }
        let events = self.gate.events(&current, &attempt.session, at, payloads)?;
        self.append(&attempt.session, view.revision(), &events)
    }
    pub fn read_artifact(
        &self,
        session: &Id,
        result: &Id<ResultVersion>,
        path: &WorkspacePath,
    ) -> Result<Vec<u8>> {
        let view = self.journal.view(session, None)?;
        let result = view
            .results()
            .results
            .get(result)
            .ok_or_else(|| Denial::new("result_missing", "No retained candidate"))?;
        if !result.artifacts.iter().any(|a| &a.path == path) {
            return Err(Denial::new(
                "artifact_missing",
                "Path is not a submitted artifact",
            ));
        }
        self.gate
            .workspace()
            .read_artifact(session, &result.after, path)
    }
}
