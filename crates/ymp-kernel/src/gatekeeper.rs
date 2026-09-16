//! Attempt-bound atomic admission. Services validate their own resource transitions.
use crate::{
    arbiter::CommitmentChange,
    events::Event,
    treasury::{ReservationChange, ReserveDecision},
    view::SessionView,
    workspace_locks::{LockAcquisition, LockChange},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ymp_domain::{
    Denial, Digest, Id, Ref, Result,
    assignment::*,
    coordination::{CommitmentState, Lease, SolicitationState},
    identity::{DiscoverySource, ProviderKind, Readiness},
    journal::{Capability, Envelope},
    resources::{Reservation, ReservationState},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionIntent {
    pub nonce: Digest,
    pub award: Ref,
    pub assignment: Assignment,
    pub grant: Grant,
    pub lease: Lease,
    pub access_owner: Option<Digest>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AdmissionRecord {
    pub intent: AdmissionIntent,
    pub reservation: Id<Reservation>,
    pub references: Vec<Ref>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum Step {
    Lock,
    Grant,
    Commitment,
    Assignment,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct PendingAdmission {
    intent: AdmissionIntent,
    reservation: Id<Reservation>,
    references: Vec<Ref>,
    step: Step,
    at: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AdmissionView {
    assignments: BTreeMap<Id<Assignment>, AdmissionRecord>,
    pending: Option<PendingAdmission>,
}
impl AdmissionView {
    pub fn assignments(&self) -> &BTreeMap<Id<Assignment>, AdmissionRecord> {
        &self.assignments
    }
    pub(crate) fn complete(&self) -> Result<()> {
        if self.pending.is_some() {
            return Err(Denial::new(
                "admission_incomplete",
                "All admission components must be committed together",
            ));
        }
        Ok(())
    }
    pub(crate) fn check_next(&self, event: &Event) -> Result<()> {
        if let Some(pending) = &self.pending {
            let allowed = matches!(
                (pending.step, event),
                (
                    Step::Lock,
                    Event::LockChanged {
                        change: LockChange::Acquired(_),
                        ..
                    }
                ) | (Step::Grant, Event::GrantIssued { .. })
                    | (
                        Step::Commitment,
                        Event::CommitmentChanged {
                            change: CommitmentChange::Activated { .. },
                            ..
                        }
                    )
                    | (Step::Assignment, Event::AssignmentAdmitted { .. })
            );
            if !allowed {
                return Err(Denial::new(
                    "admission_incomplete",
                    "An event cannot interrupt this admission packet",
                ));
            }
        }
        Ok(())
    }
}
fn role_matches(kind: ContributionKind, role: RoleKind) -> bool {
    matches!(
        (kind, role),
        (
            ContributionKind::Plan | ContributionKind::Decompose | ContributionKind::Clarify,
            RoleKind::Planner
        ) | (ContributionKind::DesignChecks, RoleKind::CheckDesigner)
            | (
                ContributionKind::Produce
                    | ContributionKind::Alternative
                    | ContributionKind::Integrate,
                RoleKind::Producer
            )
            | (ContributionKind::Verify, RoleKind::Verifier)
            | (
                ContributionKind::Review,
                RoleKind::Reviewer | RoleKind::FinalReviewer | RoleKind::Advocate
            )
            | (
                ContributionKind::Research | ContributionKind::Diagnose,
                RoleKind::Researcher
            )
            | (ContributionKind::Curate, RoleKind::Curator)
            | (ContributionKind::Narrate, RoleKind::Narrator)
            | (ContributionKind::Judge, RoleKind::Judge)
    )
}
fn same_work(left: &Contribution, right: &Contribution) -> bool {
    match (&left.subject, &right.subject) {
        (Some(a), Some(b)) => {
            std::mem::discriminant(a) == std::mem::discriminant(b)
                && a.reference().id == b.reference().id
        }
        (None, None) => left.kind == right.kind && left.targets == right.targets,
        _ => false,
    }
}
fn unresolved(view: &SessionView, record: &AdmissionRecord) -> bool {
    match record.intent.assignment.state {
        AssignmentState::Finished => false,
        AssignmentState::Admitted | AssignmentState::Running => true,
        AssignmentState::Revoked => {
            let account = view
                .treasury()
                .and_then(|book| book.accounts.get(&record.reservation));
            let no_start = account.is_some_and(|account| {
                account.invocation.is_none()
                    && account.reservation.state == ReservationState::Released
            });
            let paths_ended = view
                .path_locks()
                .get(&record.intent.assignment.id.erased())
                .is_none_or(|lock| lock.released.is_some());
            !(no_start && paths_ended)
        }
    }
}
pub(crate) fn validate_intent(
    view: &SessionView,
    intent: &AdmissionIntent,
    funding: &ReserveDecision,
    at: u64,
) -> Result<()> {
    intent.assignment.validate()?;
    let assignment = &intent.assignment;
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "No task is open"))?;
    let award = view
        .coordination()
        .awards()
        .values()
        .find(|record| record.reference == intent.award)
        .ok_or_else(|| {
            Denial::new("award_missing", "No matching persisted award")
                .with_ref(intent.award.clone())
        })?;
    let solicitation =
        &view.coordination().solicitations()[&award.value.decision.outcome.solicitation].value;
    let offer = &view.coordination().offers()[&award.value.decision.outcome.offer].value;
    let contribution = view
        .coordination()
        .contributions()
        .get(&assignment.contribution)
        .ok_or_else(|| Denial::new("contribution_missing", "No proposed contribution"))?;
    let commitment = view
        .coordination()
        .commitments()
        .get(&assignment.commitment)
        .ok_or_else(|| Denial::new("commitment_missing", "No Proposed commitment"))?;
    if assignment.session != *view.session()
        || assignment.state != AssignmentState::Admitted
        || at < view.latest_at()
        || solicitation.state != SolicitationState::Awarded
        || solicitation.contribution != assignment.contribution
        || contribution.contract != view.contract().expect("task contract").reference()
        || offer.profile != assignment.profile
        || offer.agent != assignment.agent
        || award.value.commitment.id != assignment.commitment
        || commitment.state != CommitmentState::Proposed
        || commitment.debtor != assignment.agent
        || commitment.subject != assignment.contribution
        || !role_matches(contribution.value.kind, assignment.role)
        || !view.workspaces().contains_key(&assignment.workspace)
    {
        return Err(Denial::new(
            "admission_basis",
            "Assignment does not match its current award, contract, profile or commitment",
        )
        .with_ref(intent.award.clone()));
    }
    if contribution.value.subject.is_some()
        || matches!(
            assignment.role,
            RoleKind::Reviewer | RoleKind::FinalReviewer | RoleKind::Advocate
        )
    {
        return Err(Denial::new(
            "subject_unavailable",
            "Typed result/work-item provenance is required before admitting this subject or review",
        )
        .with_ref(contribution.reference.clone()));
    }
    let pool = view
        .registry()
        .ok_or_else(|| Denial::new("registry_missing", "No Registry observation"))?;
    let agent = pool
        .input
        .facts
        .agents
        .iter()
        .find(|agent| agent.id == assignment.agent)
        .ok_or_else(|| Denial::new("agent_missing", "No registered agent"))?;
    let discovery = pool
        .input
        .facts
        .discoveries
        .iter()
        .find(|d| d.provider.id == agent.provider)
        .ok_or_else(|| Denial::new("provider_missing", "No provider discovery"))?;
    let capabilities = discovery.provider.capabilities.as_ref().ok_or_else(|| {
        Denial::new(
            "capabilities_unknown",
            "Actual execution capabilities are unknown",
        )
    })?;
    if pool.input.constraints != task.constraints
        || !pool.decisions.iter().any(|decision| {
            decision.profile == assignment.profile && decision.outcome == Readiness::Ready
        })
        || !contribution.value.needs.is_subset(&assignment.access)
        || !assignment.access.is_subset(capabilities)
        || !assignment.access.is_subset(&task.constraints.allowed)
        || discovery.provider.kind != ProviderKind::Scripted
        || discovery.source != DiscoverySource::ScriptedFixture
        || capabilities
            .iter()
            .any(|cap| !matches!(cap, Capability::ReadFiles | Capability::WriteFiles))
        || (intent.access_owner.is_none()
            && (!capabilities.is_empty() || !assignment.access.is_empty()))
        || (intent.access_owner.is_some() && assignment.access.is_empty())
    {
        return Err(Denial::new(
            "execution_boundary",
            "Admission lacks a current enforceable execution boundary",
        ));
    }
    let end = at
        .checked_add(assignment.allowance.timeout)
        .ok_or_else(|| Denial::new("deadline", "Assignment lifetime overflows"))?;
    if intent.grant.id != assignment.grant
        || intent.grant.assignment != assignment.id
        || intent.grant.expires != end
        || intent.lease.expires <= at
        || intent.lease.expires > end
        || task
            .constraints
            .deadline
            .is_some_and(|deadline| end > deadline)
        || funding.reservation.assignment != assignment.id.erased()
        || funding.demand.contribution != assignment.contribution.erased()
        || funding.demand.kind != contribution.value.kind
        || funding.demand.difficulty != contribution.value.difficulty
        || funding.demand.provider != agent.provider
        || funding.demand.profile != assignment.profile
        || funding.allocation.outcome != assignment.allowance
    {
        return Err(Denial::new(
            "admission_resources",
            "Grant, lease or resource decisions disagree with the assignment",
        ));
    }
    let current = view.admission().assignments();
    if current.contains_key(&assignment.id)
        || current.len() >= 4096
        || current.values().any(|record| {
            record.intent.nonce == intent.nonce || record.intent.grant.id == intent.grant.id
        })
    {
        return Err(Denial::new(
            "admission_duplicate",
            "Assignment, grant or admission identity has already been used",
        ));
    }
    let active: Vec<_> = current
        .values()
        .filter(|record| unresolved(view, record))
        .collect();
    if active.len() >= task.constraints.parallel_limit as usize {
        return Err(Denial::new(
            "parallel_limit",
            "Outstanding assignments exhaust parallel capacity",
        ));
    }
    let mut members: std::collections::BTreeSet<_> = active
        .iter()
        .map(|record| &record.intent.assignment.agent)
        .collect();
    members.insert(&assignment.agent);
    if members.len() > task.constraints.max_members as usize
        || task
            .constraints
            .pins
            .team_size
            .is_some_and(|limit| members.len() > limit as usize)
    {
        return Err(Denial::new(
            "member_limit",
            "Admission exceeds the permitted team size",
        ));
    }
    let attempts = current
        .values()
        .filter(|record| {
            view.coordination()
                .contributions()
                .get(&record.intent.assignment.contribution)
                .is_some_and(|prior| same_work(&prior.value, &contribution.value))
        })
        .count();
    if attempts >= task.constraints.attempt_limit as usize {
        return Err(Denial::new(
            "attempt_limit",
            "This subject has exhausted its attempts",
        ));
    }
    if view.coordination().commitments().values().any(|held| {
        held.state == CommitmentState::Active
            && held.debtor == assignment.agent
            && view
                .coordination()
                .contributions()
                .get(&held.subject)
                .is_some_and(|prior| same_work(&prior.value, &contribution.value))
    }) {
        return Err(Denial::new(
            "duplicate_responsibility",
            "Agent already holds responsibility for this work",
        ));
    }
    Ok(())
}
fn matching_lock(intent: &AdmissionIntent, lock: &LockAcquisition) -> Result<()> {
    let assignment = &intent.assignment;
    if lock.assignment != assignment.id.erased()
        || lock.workspace != assignment.workspace
        || lock.profile != assignment.profile
        || lock.mediated_owner != intent.access_owner
        || lock.requested.iter().any(|lock| {
            !assignment.access.contains(&match lock.mode {
                ymp_domain::workspace::LockMode::Read => Capability::ReadFiles,
                ymp_domain::workspace::LockMode::Write => Capability::WriteFiles,
            })
        })
        || assignment.access.iter().any(|cap| {
            !lock.requested.iter().any(|lock| {
                matches!(
                    (cap, lock.mode),
                    (Capability::ReadFiles, ymp_domain::workspace::LockMode::Read)
                        | (
                            Capability::WriteFiles,
                            ymp_domain::workspace::LockMode::Write
                        )
                )
            })
        })
    {
        return Err(Denial::new(
            "admission_access",
            "New access ownership differs from this admission",
        ));
    }
    Ok(())
}
pub(crate) fn apply(view: &SessionView, event: &Envelope<Event>) -> Result<Option<AdmissionView>> {
    view.admission().check_next(&event.payload)?;
    if let Event::ReservationChanged {
        change: ReservationChange::Reserved(funding),
        ..
    } = &event.payload
        && let Some(intent) = &funding.admission
    {
        view.admission().complete()?;
        validate_intent(view, intent, funding, event.at)?;
        let mut book = view.admission().clone();
        book.pending = Some(PendingAdmission {
            intent: (**intent).clone(),
            reservation: funding.reservation.id.clone(),
            references: vec![event.reference()?],
            step: if intent.access_owner.is_some() {
                Step::Lock
            } else {
                Step::Grant
            },
            at: event.at,
        });
        return Ok(Some(book));
    }
    if let Event::AssignmentRevoked {
        assignment, reason, ..
    } = &event.payload
    {
        view.admission().complete()?;
        ymp_domain::require_text(reason, 4096)?;
        let source = view
            .admission()
            .assignments
            .get(assignment)
            .ok_or_else(|| Denial::new("assignment_missing", "No admitted assignment"))?;
        let account = view
            .treasury()
            .and_then(|book| book.accounts.get(&source.reservation))
            .ok_or_else(|| Denial::new("reservation_missing", "Assignment lost its reservation"))?;
        let locked = view.path_locks().get(&assignment.erased());
        if event.at < view.latest_at()
            || !matches!(
                source.intent.assignment.state,
                AssignmentState::Admitted | AssignmentState::Running
            )
            || (account.reservation.state == ReservationState::Held && !account.revoked)
            || locked.is_some_and(|lock| lock.released.is_none() && !lock.revoked)
        {
            return Err(Denial::new(
                "assignment_revocation",
                "Resource authority must be revoked with the assignment",
            ));
        }
        let mut book = view.admission().clone();
        let record = book.assignments.get_mut(assignment).unwrap();
        record.intent.assignment.state = AssignmentState::Revoked;
        record.references.push(event.reference()?);
        return Ok(Some(book));
    }
    if view.admission().pending.is_none() {
        if matches!(
            event.payload,
            Event::GrantIssued { .. }
                | Event::AssignmentAdmitted { .. }
                | Event::CommitmentChanged {
                    change: CommitmentChange::Activated { .. },
                    ..
                }
        ) {
            return Err(Denial::new(
                "admission_incomplete",
                "Admission component has no matching attempt",
            ));
        }
        return Ok(None);
    }
    let mut book = view.admission().clone();
    let pending = book.pending.as_mut().expect("pending admission");
    if event.at != pending.at {
        return Err(Denial::new(
            "admission_packet",
            "Admission timestamps differ within one decision",
        ));
    }
    match &event.payload {
        Event::LockChanged {
            change: LockChange::Acquired(lock),
            ..
        } => {
            matching_lock(&pending.intent, lock)?;
            pending.step = Step::Grant;
        }
        Event::GrantIssued { nonce, grant, .. } => {
            if nonce != &pending.intent.nonce || grant != &pending.intent.grant {
                return Err(Denial::new(
                    "admission_grant",
                    "Grant differs from its admission attempt",
                ));
            }
            pending.step = Step::Commitment;
        }
        Event::CommitmentChanged {
            change:
                CommitmentChange::Activated {
                    nonce,
                    commitment,
                    lease,
                },
            ..
        } => {
            if nonce != &pending.intent.nonce
                || commitment != &pending.intent.assignment.commitment
                || lease != &pending.intent.lease
            {
                return Err(Denial::new(
                    "admission_commitment",
                    "Active commitment differs from its admission attempt",
                ));
            }
            pending.step = Step::Assignment;
        }
        Event::AssignmentAdmitted {
            nonce, assignment, ..
        } => {
            if nonce != &pending.intent.nonce || assignment != &pending.intent.assignment.id {
                return Err(Denial::new(
                    "admission_assignment",
                    "Final assignment differs from its admission attempt",
                ));
            }
            pending.references.push(event.reference()?);
            let pending = book.pending.take().unwrap();
            book.assignments.insert(
                pending.intent.assignment.id.clone(),
                AdmissionRecord {
                    intent: pending.intent,
                    reservation: pending.reservation,
                    references: pending.references,
                },
            );
            return Ok(Some(book));
        }
        _ => {
            return Err(Denial::new(
                "admission_incomplete",
                "Unexpected admission event",
            ));
        }
    }
    pending.references.push(event.reference()?);
    Ok(Some(book))
}
pub(crate) fn attribution(view: &SessionView) -> Result<Vec<Ref>> {
    let pending = view
        .admission()
        .pending
        .as_ref()
        .ok_or_else(|| Denial::new("admission_incomplete", "No admission packet"))?;
    let mut refs = vec![
        pending.intent.award.clone(),
        pending.references.last().unwrap().clone(),
    ];
    refs.sort();
    refs.dedup();
    Ok(refs)
}

/// Additional control capacity for admitted assignments and their financial closure.
/// Workspace authorization/revocation/release is reserved by WorkspaceGuard separately.
pub fn control_reserve(view: &SessionView) -> (usize, usize) {
    let mut events = view
        .coordination()
        .commitments()
        .values()
        .filter(|value| value.state == CommitmentState::Proposed)
        .count();
    for record in view.admission().assignments().values() {
        if matches!(
            record.intent.assignment.state,
            AssignmentState::Admitted | AssignmentState::Running
        ) {
            events += 1;
        }
        if let Some(account) = view
            .treasury()
            .and_then(|book| book.accounts.get(&record.reservation))
            && account.reservation.state == ReservationState::Held
        {
            if !account.revoked {
                events += 1;
            }
            if account.invocation.is_none() {
                // Authorization, first receipt, one final clarification and settlement;
                // revocation before authorization instead needs only a no-start release.
                events += if account.revoked { 1 } else { 4 };
            } else if account.complete_cost.is_some()
                || account.receipt.as_ref().is_some_and(|receipt| {
                    receipt.coverage == ymp_domain::resources::Coverage::Complete
                })
            {
                events += 1;
            } else if account.receipt.is_some() {
                events += 2;
            } else {
                events += 3;
            }
        }
    }
    (events, events * 64 * 1024)
}
pub(crate) fn revocation_refs(view: &SessionView, assignment: &Id<Assignment>) -> Result<Vec<Ref>> {
    let record = view
        .admission()
        .assignments
        .get(assignment)
        .ok_or_else(|| Denial::new("assignment_missing", "No admitted assignment"))?;
    let mut refs = vec![record.references.last().unwrap().clone()];
    if let Some(account) = view
        .treasury()
        .and_then(|book| book.accounts.get(&record.reservation))
    {
        refs.push(account.last.clone());
    }
    if let Some(lock) = view.path_locks().get(&assignment.erased()) {
        refs.push(lock.last.clone());
    }
    refs.sort();
    refs.dedup();
    Ok(refs)
}
use crate::{
    journal::{AppendResolution, ContentStore, Journal, JournalRead, validate_append},
    ports::resources::ResourceResponse,
    treasury::{ReserveRequest, prepare_reservation},
    workspace_guard::{MediatedAccess, PreparedMediation, WorkspaceGuard},
};
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::resources::{Allowance, CostEstimate, ResourceDemand};
/// Live bearer secret, never a journal value or an automatically reconstructed credential.
/// ```compile_fail
/// let grant: ymp_kernel::gatekeeper::GrantToken = serde_json::from_str("{}").unwrap();
/// ```
pub struct GrantToken {
    admission: Ref,
    session: Id,
    grant: Id<Grant>,
    secret: [u8; 32],
}
impl GrantToken {
    pub fn grant(&self) -> &Id<Grant> {
        &self.grant
    }
    pub fn session(&self) -> &Id {
        &self.session
    }
}
pub struct AdmissionRequest {
    pub assignment: Id<Assignment>,
    pub award: Ref,
    pub role: RoleKind,
    pub workspace: Id<ymp_domain::workspace::Workspace>,
    pub access: BTreeSet<Capability>,
    pub reservation: Id<Reservation>,
    pub grant: Id<Grant>,
    pub operations: BTreeSet<TeamOperation>,
    pub lease: Lease,
    pub estimate: ResourceResponse<CostEstimate>,
    pub allowance: ResourceResponse<Allowance>,
    pub files: Option<PreparedMediation>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionStatus {
    Prepared,
    Committed,
    Delivered,
}
enum PreparedOutcome {
    Admit {
        intent: Box<AdmissionIntent>,
        token: Option<GrantToken>,
        files: Option<Box<PreparedMediation>>,
    },
    Reject(Denial),
}
/// One attempt with local secrets and access plans. Neither serialization nor Clone
/// can recreate its one-time right to finish capability delivery.
/// ```compile_fail
/// let attempt: ymp_kernel::gatekeeper::PreparedAdmission = serde_json::from_str("{}").unwrap();
/// ```
pub struct PreparedAdmission {
    session: Id,
    expected: u64,
    events: Vec<Envelope<Event>>,
    issuer: Arc<()>,
    outcome: PreparedOutcome,
    status: AdmissionStatus,
    cancellation: Option<(Vec<Envelope<Event>>, Denial)>,
}
impl PreparedAdmission {
    pub fn status(&self) -> AdmissionStatus {
        self.status
    }
    pub fn committed_assignment(&self) -> Option<&Assignment> {
        if self.status == AdmissionStatus::Prepared {
            return None;
        }
        match &self.outcome {
            PreparedOutcome::Admit { intent, .. } => Some(&intent.assignment),
            PreparedOutcome::Reject(_) => None,
        }
    }
}
pub struct AdmittedAssignment<J: Journal> {
    pub assignment: Assignment,
    pub grant: GrantToken,
    pub files: Option<MediatedAccess<J>>,
}
pub struct Gatekeeper<J: Journal, C: ContentStore> {
    journal: Arc<J>,
    workspace: WorkspaceGuard<J, C>,
    issuer: Arc<()>,
}
impl<J: Journal, C: ContentStore> Gatekeeper<J, C> {
    pub fn new(journal: Arc<J>, content: Arc<C>) -> Self {
        Self {
            workspace: WorkspaceGuard::new(journal.clone(), content),
            journal,
            issuer: Arc::new(()),
        }
    }
    pub fn workspace(&self) -> &WorkspaceGuard<J, C> {
        &self.workspace
    }
    pub fn view(&self, session: &Id) -> Result<SessionView> {
        self.journal
            .read(session)?
            .view_with_schemas(session, None, self.journal.schemas())
    }
    fn events(
        &self,
        current: &JournalRead,
        session: &Id,
        at: u64,
        payloads: Vec<Event>,
    ) -> Result<Vec<Envelope<Event>>> {
        let mut view = current.view_with_schemas(session, None, self.journal.schemas())?;
        let mut events = vec![];
        for payload in payloads {
            let (policy, input, refs) = match &payload {
                Event::ReservationChanged { .. } => crate::treasury::attribution(&payload)?,
                Event::LockChanged { change, .. } => (
                    None,
                    None,
                    crate::workspace_locks::attribution(&view, change)?,
                ),
                Event::CommitmentChanged { .. } => crate::arbiter::attribution(&view, &payload)?,
                Event::GrantIssued { .. } | Event::AssignmentAdmitted { .. } => {
                    (None, None, attribution(&view)?)
                }
                Event::AssignmentRevoked { assignment, .. } => {
                    (None, None, revocation_refs(&view, assignment)?)
                }
                _ => {
                    return Err(Denial::new(
                        "admission_event",
                        "Unsupported admission event",
                    ));
                }
            };
            let event = Envelope {
                seq: view.revision().checked_add(1).ok_or_else(|| {
                    Denial::new("revision_overflow", "Journal sequence exhausted")
                })?,
                session: session.clone(),
                at,
                actor: ymp_domain::journal::Actor::Runtime,
                policy,
                input,
                refs,
                payload,
            };
            view.apply(&event, self.journal.schemas())?;
            events.push(event);
        }
        validate_append(
            current,
            session,
            current.revision,
            &events,
            self.journal.schemas(),
        )?;
        if crate::workspace_locks::touches_ownership(&events) {
            let inventory = self.journal.workspace_inventory(session)?;
            crate::journal::validate_workspace_append(
                &inventory,
                session,
                current.revision,
                &events,
                self.journal.schemas(),
            )?;
        }
        Ok(events)
    }
    /// Stage a trusted runtime preparation failure against exactly one current
    /// Proposed award. The caller still commits this decision through admit.
    pub fn reject(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        award: Ref,
        denial: Denial,
    ) -> Result<PreparedAdmission> {
        let current = self.journal.read(session)?;
        if current.revision != expected {
            return Err(Denial::new(
                "stale_revision",
                "Cannot reject a changed admission source",
            ));
        }
        let view = current.view_with_schemas(session, None, self.journal.schemas())?;
        let source = view
            .coordination()
            .awards()
            .values()
            .find(|record| record.reference == award)
            .ok_or_else(|| {
                Denial::new("award_missing", "No matching persisted award").with_ref(award.clone())
            })?;
        let reason = format!("{}: {}", denial.code, denial.message)
            .chars()
            .take(1024)
            .collect();
        let events = self.events(
            &current,
            session,
            at,
            vec![Event::CommitmentChanged {
                version: 1,
                change: CommitmentChange::Cancelled {
                    commitment: source.value.commitment.id.clone(),
                    award,
                    reason,
                },
            }],
        )?;
        Ok(PreparedAdmission {
            session: session.clone(),
            expected,
            events,
            issuer: self.issuer.clone(),
            outcome: PreparedOutcome::Reject(denial),
            status: AdmissionStatus::Prepared,
            cancellation: None,
        })
    }
    /// Construct a complete decision without committing it. A prepared rejection
    /// must also pass through admit to cancel its exact Proposed commitment.
    pub fn prepare(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        request: AdmissionRequest,
    ) -> Result<PreparedAdmission> {
        let current = self.journal.read(session)?;
        if current.revision != expected {
            return Err(Denial::new("stale_revision", "Admission input changed"));
        }
        let view = current.view_with_schemas(session, None, self.journal.schemas())?;
        let award = view
            .coordination()
            .awards()
            .values()
            .find(|value| value.reference == request.award)
            .ok_or_else(|| {
                Denial::new("award_missing", "No matching persisted award")
                    .with_ref(request.award.clone())
            })?;
        let commitment_id = award.value.commitment.id.clone();
        if view
            .coordination()
            .commitments()
            .get(&commitment_id)
            .is_none_or(|value| value.state != CommitmentState::Proposed)
        {
            return Err(Denial::new(
                "commitment_state",
                "Award no longer has a Proposed commitment",
            )
            .with_ref(request.award.clone()));
        }
        let award_ref = request.award.clone();
        let decision = (|| -> Result<(Vec<Envelope<Event>>, PreparedOutcome)> {
            let offer = &view.coordination().offers()[&award.value.decision.outcome.offer].value;
            let contribution =
                &view.coordination().contributions()[&award.value.commitment.subject].value;
            let provider = view
                .registry()
                .and_then(|pool| {
                    pool.input
                        .facts
                        .agents
                        .iter()
                        .find(|agent| agent.id == offer.agent)
                })
                .ok_or_else(|| {
                    Denial::new("agent_missing", "Awarded agent is no longer registered")
                })?
                .provider
                .clone();
            if let Some(plan) = &request.files {
                self.workspace.validate_prepared_mediation(plan)?;
                if plan.input() != &view.digest()? {
                    return Err(Denial::new(
                        "stale_access_plan",
                        "Access preparation names another input view",
                    ));
                }
            }
            let mut secret = [0u8; 32];
            let mut nonce = [0u8; 32];
            getrandom::fill(&mut secret)
                .map_err(|_| Denial::new("grant_entropy", "Cannot create a grant secret"))?;
            getrandom::fill(&mut nonce).map_err(|_| {
                Denial::new("admission_entropy", "Cannot create an admission identity")
            })?;
            let allowance = request.allowance.proposal.value.clone();
            let expires = at
                .checked_add(allowance.timeout)
                .ok_or_else(|| Denial::new("deadline", "Assignment lifetime overflows"))?;
            let intent = AdmissionIntent {
                nonce: Digest::of(nonce),
                award: request.award.clone(),
                assignment: Assignment {
                    id: request.assignment.clone(),
                    session: session.clone(),
                    agent: offer.agent.clone(),
                    profile: offer.profile.clone(),
                    contribution: contribution.id.clone(),
                    role: request.role,
                    access: request.access.clone(),
                    workspace: request.workspace.clone(),
                    allowance,
                    grant: request.grant.clone(),
                    commitment: commitment_id.clone(),
                    state: AssignmentState::Admitted,
                },
                grant: Grant {
                    id: request.grant.clone(),
                    assignment: request.assignment.clone(),
                    operations: request.operations.clone(),
                    expires,
                    token_digest: Digest::of(secret),
                },
                lease: request.lease.clone(),
                access_owner: request
                    .files
                    .as_ref()
                    .and_then(|plan| plan.acquisition().mediated_owner.clone()),
            };
            let mut funding = prepare_reservation(
                &view,
                at,
                ReserveRequest {
                    id: request.reservation.clone(),
                    assignment: request.assignment.erased(),
                    demand: ResourceDemand {
                        contribution: contribution.id.erased(),
                        kind: contribution.kind,
                        difficulty: contribution.difficulty,
                        provider,
                        profile: offer.profile.clone(),
                    },
                    estimate: request.estimate,
                    allowance: request.allowance,
                },
            )?;
            funding.admission = Some(Box::new(intent.clone()));
            let mut payloads = vec![Event::ReservationChanged {
                version: 1,
                change: ReservationChange::Reserved(Box::new(funding)),
            }];
            if let Some(plan) = &request.files {
                payloads.push(Event::LockChanged {
                    version: 1,
                    change: LockChange::Acquired(Box::new(plan.acquisition().clone())),
                });
            }
            payloads.extend([
                Event::GrantIssued {
                    version: 1,
                    nonce: intent.nonce.clone(),
                    grant: intent.grant.clone(),
                },
                Event::CommitmentChanged {
                    version: 1,
                    change: CommitmentChange::Activated {
                        nonce: intent.nonce.clone(),
                        commitment: commitment_id.clone(),
                        lease: intent.lease.clone(),
                    },
                },
                Event::AssignmentAdmitted {
                    version: 1,
                    nonce: intent.nonce.clone(),
                    assignment: request.assignment.clone(),
                },
            ]);
            let events = self.events(&current, session, at, payloads)?;
            let admission_reference = events.last().expect("complete admission").reference()?;
            Ok((
                events,
                PreparedOutcome::Admit {
                    intent: Box::new(intent),
                    token: Some(GrantToken {
                        admission: admission_reference,
                        session: session.clone(),
                        grant: request.grant.clone(),
                        secret,
                    }),
                    files: request.files.map(Box::new),
                },
            ))
        })();
        let (events, outcome) = match decision {
            Ok(value) => value,
            Err(denial) => {
                let reason = format!("{}: {}", denial.code, denial.message)
                    .chars()
                    .take(1024)
                    .collect();
                let events = self.events(
                    &current,
                    session,
                    at,
                    vec![Event::CommitmentChanged {
                        version: 1,
                        change: CommitmentChange::Cancelled {
                            commitment: commitment_id,
                            award: award_ref,
                            reason,
                        },
                    }],
                )?;
                (events, PreparedOutcome::Reject(denial))
            }
        };
        Ok(PreparedAdmission {
            session: session.clone(),
            expected,
            events,
            issuer: self.issuer.clone(),
            outcome,
            status: AdmissionStatus::Prepared,
            cancellation: None,
        })
    }
    fn resolve_packet(
        &self,
        session: &Id,
        expected: u64,
        events: &[Envelope<Event>],
    ) -> Result<AppendResolution> {
        crate::journal::resolve_append(
            &self.journal.read(session)?,
            session,
            expected,
            events,
            self.journal.schemas(),
        )
    }
    fn begin_cancellation(&self, prepared: &mut PreparedAdmission, denial: Denial) -> Result<()> {
        let intent = match &prepared.outcome {
            PreparedOutcome::Admit { intent, .. } => intent,
            PreparedOutcome::Reject(_) => return Err(denial),
        };
        let current = self.journal.read(&prepared.session)?;
        if current.revision != prepared.expected {
            return Err(Denial::new(
                "stale_revision",
                "Cannot cancel a changed admission source",
            ));
        }
        let reason = format!("{}: {}", denial.code, denial.message)
            .chars()
            .take(1024)
            .collect();
        let events = self.events(
            &current,
            &prepared.session,
            prepared.events[0].at,
            vec![Event::CommitmentChanged {
                version: 1,
                change: CommitmentChange::Cancelled {
                    commitment: intent.assignment.commitment.clone(),
                    award: intent.award.clone(),
                    reason,
                },
            }],
        )?;
        prepared.cancellation = Some((events, denial));
        Ok(())
    }
    /// Retain the original capability plan until one of the competing packets is
    /// proved committed. A failed cancellation cannot discard a late admission.
    fn finish_cancellation(&self, prepared: &mut PreparedAdmission) -> Result<()> {
        let mut attempted = false;
        loop {
            let (events, denial) = prepared
                .cancellation
                .as_ref()
                .expect("pending cancellation");
            let current = self.journal.read(&prepared.session)?;
            let original = crate::journal::resolve_append(
                &current,
                &prepared.session,
                prepared.expected,
                &prepared.events,
                self.journal.schemas(),
            )?;
            let cancellation = crate::journal::resolve_append(
                &current,
                &prepared.session,
                prepared.expected,
                events,
                self.journal.schemas(),
            )?;
            if matches!(original, AppendResolution::Committed(_)) {
                prepared.cancellation = None;
                prepared.status = AdmissionStatus::Committed;
                return Ok(());
            }
            if matches!(cancellation, AppendResolution::Committed(_)) {
                let reference = events.last().unwrap().reference()?;
                let reason = denial.clone();
                prepared.events = events.clone();
                prepared.outcome = PreparedOutcome::Reject(reason.clone());
                prepared.cancellation = None;
                prepared.status = AdmissionStatus::Delivered;
                return Err(reason.with_ref(reference));
            }
            if !matches!(
                (original, cancellation),
                (AppendResolution::Absent, AppendResolution::Absent)
            ) {
                return Err(Denial::new(
                    "stale_revision",
                    "Another decision changed the proposed commitment",
                ));
            }
            if attempted {
                return Err(Denial::new(
                    "admission_cancellation_uncertain",
                    "Admission was not committed and its cancellation is not confirmed",
                ));
            }
            attempted = true;
            let _ = self
                .journal
                .append(&prepared.session, prepared.expected, events);
        }
    }
    /// Commit one complete attempt, then finish local capability delivery once.
    /// On acknowledgement or delivery failure the caller retains this same plan.
    pub fn admit(&self, prepared: &mut PreparedAdmission) -> Result<AdmittedAssignment<J>> {
        if !Arc::ptr_eq(&prepared.issuer, &self.issuer)
            || prepared.status == AdmissionStatus::Delivered
        {
            return Err(Denial::new(
                "admission_plan",
                "Admission plan is foreign or already delivered",
            ));
        }
        if prepared.cancellation.is_some() {
            self.finish_cancellation(prepared)?;
        }
        if prepared.status == AdmissionStatus::Prepared {
            let end = prepared.expected + prepared.events.len() as u64;
            match self.resolve_packet(&prepared.session, prepared.expected, &prepared.events)? {
                AppendResolution::Committed(revision) if revision == end => {}
                AppendResolution::Absent => {
                    let appended =
                        self.journal
                            .append(&prepared.session, prepared.expected, &prepared.events);
                    match self.resolve_packet(
                        &prepared.session,
                        prepared.expected,
                        &prepared.events,
                    ) {
                        Ok(AppendResolution::Committed(revision)) if revision == end => {}
                        Ok(AppendResolution::Absent)
                            if appended.is_err()
                                && matches!(prepared.outcome, PreparedOutcome::Admit { .. }) =>
                        {
                            self.begin_cancellation(prepared, appended.err().unwrap())?;
                            self.finish_cancellation(prepared)?;
                        }
                        _ => return Err(appended.err().unwrap_or_else(|| {
                            Denial::new(
                                "admission_uncertain",
                                "The stored admission packet does not match the prepared decision",
                            )
                        })),
                    }
                }
                _ => {
                    return Err(Denial::new(
                        "stale_revision",
                        "Another decision changed the admission input",
                    ));
                }
            }
            prepared.status = AdmissionStatus::Committed;
        }
        match &mut prepared.outcome {
            PreparedOutcome::Reject(denial) => {
                prepared.status = AdmissionStatus::Delivered;
                Err(denial
                    .clone()
                    .with_ref(prepared.events.last().unwrap().reference()?))
            }
            PreparedOutcome::Admit {
                intent,
                token,
                files,
            } => {
                if token.is_none() {
                    return Err(Denial::new(
                        "admission_capability",
                        "Committed assignment has no local grant secret",
                    ));
                }
                let view = self.view(&prepared.session)?;
                let record = view
                    .admission()
                    .assignments()
                    .get(&intent.assignment.id)
                    .ok_or_else(|| {
                        Denial::new("admission_uncertain", "Committed assignment is unavailable")
                    })?;
                if record.intent != **intent
                    || record.intent.assignment.state == AssignmentState::Revoked
                {
                    return Err(Denial::new(
                        "admission_committed_unavailable",
                        "Assignment is committed but cannot receive local capabilities",
                    ));
                }
                let access = match files.as_mut() {
                    Some(plan) => Some(self.workspace.complete_mediation(plan)?),
                    None => None,
                };
                let grant = token.take().expect("checked grant secret");
                prepared.status = AdmissionStatus::Delivered;
                Ok(AdmittedAssignment {
                    assignment: record.intent.assignment.clone(),
                    grant,
                    files: access,
                })
            }
        }
    }
    pub fn authorize(
        &self,
        token: &GrantToken,
        operation: TeamOperation,
        at: u64,
    ) -> Result<Assignment> {
        let view = self.view(&token.session)?;
        view.resolve(&token.admission)?;
        let record = view
            .admission()
            .assignments()
            .values()
            .find(|record| record.intent.grant.id == token.grant)
            .ok_or_else(|| Denial::new("grant_missing", "No matching recorded grant"))?;
        let assignment = &record.intent.assignment;
        let grant = &record.intent.grant;
        let funded = view
            .treasury()
            .and_then(|book| book.accounts.get(&record.reservation))
            .is_some_and(|account| {
                account.reservation.state == ReservationState::Held && !account.revoked
            });
        let paths = match (
            &record.intent.access_owner,
            view.path_locks().get(&assignment.id.erased()),
        ) {
            (None, None) => true,
            (Some(owner), Some(lock)) => {
                lock.acquired.mediated_owner.as_ref() == Some(owner)
                    && lock.acquired.profile == assignment.profile
                    && lock.acquired.workspace == assignment.workspace
                    && !lock.revoked
                    && lock.released.is_none()
            }
            _ => false,
        };
        if grant.token_digest != Digest::of(token.secret)
            || !grant.operations.contains(&operation)
            || at < view.latest_at()
            || at >= grant.expires
            || !matches!(
                assignment.state,
                AssignmentState::Admitted | AssignmentState::Running
            )
            || !funded
            || !paths
        {
            return Err(Denial::new(
                "grant_denied",
                "Grant is expired, revoked, unfunded or outside its permitted operation",
            ));
        }
        Ok(assignment.clone())
    }
    pub fn revoke(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        assignment: &Id<Assignment>,
        reason: String,
    ) -> Result<u64> {
        ymp_domain::require_text(&reason, 4096)?;
        let current = self.journal.read(session)?;
        if current.revision != expected {
            return Err(Denial::new("stale_revision", "Revocation input changed"));
        }
        let view = current.view_with_schemas(session, None, self.journal.schemas())?;
        let record = view
            .admission()
            .assignments()
            .get(assignment)
            .ok_or_else(|| Denial::new("assignment_missing", "No admitted assignment"))?;
        if record.intent.assignment.state == AssignmentState::Revoked {
            return Ok(current.revision);
        }
        let account = &view
            .treasury()
            .ok_or_else(|| Denial::new("budget_missing", "No budget"))?
            .accounts[&record.reservation];
        let mut payloads = vec![];
        if account.reservation.state == ReservationState::Held && !account.revoked {
            payloads.push(Event::ReservationChanged {
                version: 1,
                change: ReservationChange::Revoked {
                    reservation: record.reservation.clone(),
                    reason: reason.clone(),
                },
            });
        }
        if view
            .path_locks()
            .get(&assignment.erased())
            .is_some_and(|lock| !lock.revoked && lock.released.is_none())
        {
            payloads.push(Event::LockChanged {
                version: 1,
                change: LockChange::Revoked {
                    assignment: assignment.erased(),
                    reason: reason.clone(),
                },
            });
        }
        payloads.push(Event::AssignmentRevoked {
            version: 1,
            assignment: assignment.clone(),
            reason,
        });
        let events = self.events(&current, session, at, payloads)?;
        let end = expected + events.len() as u64;
        let appended = self.journal.append(session, expected, &events);
        if matches!(self.journal.resolve_append(session,expected,&events),Ok(AppendResolution::Committed(revision)) if revision==end)
        {
            return Ok(end);
        }
        Err(appended.err().unwrap_or_else(|| {
            Denial::new("revocation_uncertain", "Cannot establish revocation commit")
        }))
    }
}
