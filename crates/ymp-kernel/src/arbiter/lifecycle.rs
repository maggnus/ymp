//! P2 transition consumer. Completion/acceptance producers remain separate kernel work.
use super::*;
use crate::{
    gatekeeper::{AdmissionIntent, AdmissionRequest, Gatekeeper, GrantToken, PreparedAdmission},
    journal::ContentStore,
};
use ymp_domain::assignment::{Assignment, AssignmentState, TeamOperation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitmentLinkKind {
    Reopen,
    Delegate,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentLink {
    pub commitment: Id<Commitment>,
    pub previous: Ref,
    pub kind: CommitmentLinkKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delegation {
    pub commitment: Id<Commitment>,
    pub previous: Ref,
    pub assignment: Id<Assignment>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitmentEnd {
    Released {
        solicitation: Id<Solicitation>,
    },
    Expired,
    Cancelled {
        basis: Ref,
    },
    Discharged {
        basis: Ref,
    },
    Delegated {
        successor: Id<Commitment>,
        assignment: Id<Assignment>,
        nonce: Digest,
    },
}

fn commitment<'a>(view: &'a SessionView, id: &Id<Commitment>) -> Result<&'a Commitment> {
    view.coordination()
        .commitments
        .get(id)
        .ok_or_else(|| Denial::new("commitment_missing", "No recorded commitment"))
}
pub(super) fn award<'a>(
    view: &'a SessionView,
    id: &Id<Commitment>,
) -> Result<&'a Recorded<Awarded>> {
    view.coordination()
        .awards
        .values()
        .find(|a| a.value.commitment.id == *id)
        .ok_or_else(|| Denial::new("award_missing", "Commitment has no recorded award"))
}
fn terms<'a>(view: &'a SessionView, id: &Id<Commitment>) -> Result<&'a Decision<CommitmentTerms>> {
    award(view, id)?.value.terms.as_ref().ok_or_else(|| {
        Denial::new(
            "commitment_terms",
            "This award predates recorded lifecycle terms",
        )
    })
}
fn assignment<'a>(
    view: &'a SessionView,
    id: &Id<Commitment>,
) -> Result<&'a crate::gatekeeper::AdmissionRecord> {
    view.admission()
        .assignments()
        .values()
        .find(|a| a.intent.assignment.commitment == *id)
        .ok_or_else(|| {
            Denial::new(
                "assignment_missing",
                "Commitment has no admitted assignment",
            )
        })
}
pub(super) fn validate_terms(view: &SessionView, data: &Awarded, input: &AwardView) -> Result<()> {
    if let Some(decision) = &data.terms {
        decision.proposal.validate()?;
        decision.outcome.validate()?;
        if decision.effective != data.decision.effective
            || decision.proposal.policy != decision.effective.policy
            || decision.input != Digest::of_value(input)?
            || decision.outcome != decision.proposal.value
            || decision.selection_change.is_some()
        {
            return Err(Denial::new(
                "commitment_terms",
                "Commitment terms do not match this award decision",
            ));
        }
        for reference in &decision.proposal.basis {
            view.resolve(reference)?;
        }
    }
    Ok(())
}
pub(crate) fn initial_lease(
    view: &SessionView,
    id: &Id<Commitment>,
    at: u64,
    timeout: u64,
) -> Result<Option<Lease>> {
    award(view, id)?
        .value
        .terms
        .as_ref()
        .map(|decision| decision.outcome.initial_lease(at, timeout))
        .transpose()
}
/// A kernel-owned observation projection. Only the heartbeat has a producer in W1-0007.
struct ProgressProjection {
    session: Id,
    assignment: Id<Assignment>,
    contribution: Id<Contribution>,
    signal: ProgressSignal,
    source: Option<Ref>,
}
fn resolve_progress(
    view: &SessionView,
    current: &Commitment,
    signal: ProgressSignal,
    basis: Option<&Ref>,
) -> Result<ProgressProjection> {
    if let Some(reference) = basis {
        view.resolve(reference)?;
    }
    let source = assignment(view, &current.id)?;
    if let Some(reference) = basis {
        let (invocation, observed) = view
            .execution()
            .invocations()
            .values()
            .find_map(|invocation| {
                invocation
                    .observations
                    .values()
                    .find(|(_, at)| at == reference)
                    .map(|(event, _)| (invocation, event))
            })
            .ok_or_else(|| {
                Denial::new(
                    "progress_basis_unsupported",
                    "No matching kernel invocation observation",
                )
            })?;
        if invocation.dispatch.assignment.id != source.intent.assignment.id
            || invocation.dispatch.assignment.commitment != current.id
            || invocation.terminal.is_some()
        {
            return Err(Denial::new(
                "progress_basis",
                "Progress belongs to different or ended responsibility",
            ));
        }
        let crate::ports::execution::BackendObservation::Progress {
            signal: actual,
            basis: underlying,
        } = &observed.observation
        else {
            return Err(Denial::new(
                "progress_basis",
                "Source is not a progress observation",
            ));
        };
        if *actual != signal {
            return Err(Denial::new(
                "progress_basis",
                "Progress signal differs from its observation",
            ));
        }
        match (signal, underlying) {
            (ProgressSignal::Heartbeat, None) => {}
            (ProgressSignal::CheckRun, Some(reference)) => {
                let run = view
                    .check_runs()
                    .values()
                    .find(|run| run.reference().is_ok_and(|actual| actual == *reference))
                    .ok_or_else(|| {
                        Denial::new("progress_basis_unsupported", "No exact recorded check run")
                    })?;
                let check = view
                    .checks()
                    .get(&run.check)
                    .ok_or_else(|| Denial::new("progress_basis", "Check run lost its check"))?;
                let contribution = &view.coordination().contributions[&current.subject].value;
                if !contribution.targets.contains(&check.criterion)
                    || run.at < invocation.dispatch.at
                    || view.snapshots().get(&run.target).is_none_or(|snapshot| {
                        snapshot.workspace != source.intent.assignment.workspace
                    })
                {
                    return Err(Denial::new(
                        "progress_basis",
                        "Check run does not cover this responsibility and workspace",
                    ));
                }
            }
            _ => {
                return Err(Denial::new(
                    "progress_basis_unsupported",
                    "This signal needs its owning kernel observation",
                ));
            }
        }
    } else if signal != ProgressSignal::Heartbeat {
        return Err(Denial::new(
            "progress_basis_unsupported",
            "This signal needs its owning kernel observation",
        ));
    }
    Ok(ProgressProjection {
        session: view.session().clone(),
        assignment: source.intent.assignment.id.clone(),
        contribution: current.subject.clone(),
        signal,
        source: if signal == ProgressSignal::Heartbeat {
            None
        } else {
            basis.cloned()
        },
    })
}
fn next_lease(
    view: &SessionView,
    current: &Commitment,
    signal: ProgressSignal,
    at: u64,
    basis: Option<&Ref>,
) -> Result<Lease> {
    let source = assignment(view, &current.id)?;
    let progress = resolve_progress(view, current, signal, basis)?;
    let account = view
        .treasury()
        .and_then(|book| book.accounts.get(&source.reservation))
        .ok_or_else(|| Denial::new("reservation_missing", "Commitment lost its funding"))?;
    if account.revoked
        || account.reservation.state != ymp_domain::resources::ReservationState::Held
        || view
            .path_locks()
            .get(&source.intent.assignment.id.erased())
            .is_some_and(|lock| lock.revoked || lock.released.is_some())
    {
        return Err(Denial::new(
            "lease_renewal",
            "Revoked or closed resources cannot renew authority",
        ));
    }
    projected_renewal(
        current,
        &source.intent.assignment,
        &terms(view, &current.id)?.outcome,
        source.intent.grant.expires.min(
            view.task()
                .and_then(|task| task.constraints.deadline)
                .unwrap_or(u64::MAX),
        ),
        at,
        &progress,
    )
}
fn projected_renewal(
    current: &Commitment,
    source: &Assignment,
    terms: &CommitmentTerms,
    deadline: u64,
    at: u64,
    progress: &ProgressProjection,
) -> Result<Lease> {
    terms.validate()?;
    if progress.session != source.session
        || progress.assignment != source.id
        || progress.contribution != current.subject
        || source.commitment != current.id
        || source.contribution != current.subject
        || source.agent != current.debtor
        || (progress.signal == ProgressSignal::Heartbeat) != progress.source.is_none()
    {
        return Err(Denial::new(
            "progress_basis",
            "Progress belongs to different responsibility or lacks its typed source",
        ));
    }
    // P2 expires only when now > lease.expires. Grant expiry remains exclusive.
    if current.state != CommitmentState::Active
        || at > current.lease.expires
        || at >= deadline
        || current.lease.renewals_left == 0
        || !current.lease.renew_on.contains(&progress.signal)
        || !matches!(
            source.state,
            AssignmentState::Admitted | AssignmentState::Running
        )
    {
        return Err(Denial::new(
            "lease_renewal",
            "Only a live configured commitment with renewals remaining can renew",
        ));
    }
    let mut lease = current.lease.clone();
    lease.expires = lease
        .expires
        .checked_add(terms.renewal_duration)
        .ok_or_else(|| Denial::new("lease_overflow", "Renewal expiry overflows"))?
        .min(deadline);
    if lease.expires <= current.lease.expires {
        return Err(Denial::new(
            "lease_limit",
            "Renewal cannot extend the original assignment lifetime",
        ));
    }
    lease.renewals_left -= 1;
    Ok(lease)
}
pub(super) fn is_delegation_source(
    view: &SessionView,
    link: Option<&CommitmentLink>,
    solicitation: &Id<Solicitation>,
) -> bool {
    link.filter(|link| link.kind == CommitmentLinkKind::Delegate)
        .and_then(|link| award(view, &link.commitment).ok())
        .is_some_and(|source| &source.value.decision.outcome.solicitation == solicitation)
}
pub(super) fn validate_link(
    view: &SessionView,
    next: &Solicitation,
    link: Option<&CommitmentLink>,
) -> Result<()> {
    let Some(link) = link else {
        return Ok(());
    };
    let current = commitment(view, &link.commitment)?;
    let source = award(view, &link.commitment)?;
    let prior =
        &view.coordination().solicitations[&source.value.decision.outcome.solicitation].value;
    let mut stimulus = prior.stimulus;
    let state_ok = match link.kind {
        CommitmentLinkKind::Delegate => current.state == CommitmentState::Active,
        CommitmentLinkKind::Reopen => matches!(
            current.state,
            CommitmentState::Released(_) | CommitmentState::Expired
        ),
    };
    if matches!(current.state, CommitmentState::Released(_)) {
        stimulus = ymp_domain::task::Real::new(
            stimulus.get() + terms(view, &current.id)?.outcome.release_delta.get(),
        )?;
    }
    if !state_ok
        || current.history.last() != Some(&link.previous)
        || next.contribution != current.subject
        || next.id == prior.id
        || next.reopened
            != prior
                .reopened
                .checked_add(1)
                .ok_or_else(|| Denial::new("solicitation_limit", "Reopening count overflows"))?
        || next.stimulus != stimulus
    {
        return Err(Denial::new(
            "commitment_link",
            "Solicitation does not follow the exact recorded commitment",
        ));
    }
    Ok(())
}
pub(crate) fn validate_delegation(
    view: &SessionView,
    intent: &AdmissionIntent,
    at: u64,
) -> Result<()> {
    let source = award(view, &intent.assignment.commitment)?;
    let link = view
        .coordination()
        .links
        .get(&source.value.decision.outcome.solicitation)
        .filter(|link| link.kind == CommitmentLinkKind::Delegate);
    match (link, &intent.delegation) {
        (None, None) => Ok(()),
        (Some(link), Some(transfer)) => {
            let prior = commitment(view, &link.commitment)?;
            let prior_assignment = assignment(view, &link.commitment)?;
            let successor = commitment(view, &intent.assignment.commitment)?;
            if transfer.commitment != link.commitment
                || transfer.previous != link.previous
                || prior.history.last() != Some(&link.previous)
                || prior.state != CommitmentState::Active
                || transfer.assignment != prior_assignment.intent.assignment.id
                || !matches!(
                    prior_assignment.intent.assignment.state,
                    AssignmentState::Admitted | AssignmentState::Running
                )
                || at > prior.lease.expires
                || at >= prior_assignment.intent.grant.expires
                || prior.debtor == intent.assignment.agent
                || prior.subject != successor.subject
                || prior.creditor != successor.creditor
                || prior.condition != successor.condition
            {
                return Err(Denial::new(
                    "delegation_basis",
                    "Delegation changes responsibility or uses a stale or identical holder",
                ));
            }
            Ok(())
        }
        _ => Err(Denial::new(
            "delegation_required",
            "Successor admission must be bound to its exact delegation",
        )),
    }
}
pub(super) fn attribution(
    view: &SessionView,
    change: &CommitmentChange,
) -> Result<(Option<ymp_domain::PolicyRef>, Option<Digest>, Vec<Ref>)> {
    let (id, previous) = match change {
        CommitmentChange::Renewed {
            commitment,
            previous,
            ..
        }
        | CommitmentChange::Ended {
            commitment,
            previous,
            ..
        } => (commitment, previous),
        _ => return Err(Denial::new("commitment_event", "Not a lifecycle event")),
    };
    let mut refs = vec![previous.clone(), award(view, id)?.reference.clone()];
    for (solicitation, link) in &view.coordination().links {
        let record = &view.coordination().solicitations[solicitation];
        if link.commitment == *id
            && link.kind == CommitmentLinkKind::Delegate
            && matches!(
                record.value.state,
                SolicitationState::Open | SolicitationState::Awarded
            )
        {
            refs.push(record.reference.clone());
            if let Some(awarded) = view.coordination().awards.get(solicitation) {
                refs.push(
                    commitment(view, &awarded.value.commitment.id)?
                        .history
                        .last()
                        .unwrap()
                        .clone(),
                );
            }
        }
    }
    if let CommitmentChange::Ended {
        outcome: CommitmentEnd::Cancelled { basis } | CommitmentEnd::Discharged { basis },
        ..
    } = change
    {
        refs.push(basis.clone());
    }
    if let CommitmentChange::Renewed {
        basis: Some(reference),
        ..
    } = change
    {
        refs.push(reference.clone());
    }
    refs.sort();
    refs.dedup();
    if matches!(change, CommitmentChange::Renewed { .. }) {
        Ok((
            Some(terms(view, id)?.effective.policy.clone()),
            Some(view.digest()?),
            refs,
        ))
    } else {
        Ok((None, None, refs))
    }
}
pub(super) fn apply(
    view: &SessionView,
    event: &Envelope<Event>,
    book: &mut CoordinationView,
) -> Result<bool> {
    let Event::CommitmentChanged { change, .. } = &event.payload else {
        return Ok(false);
    };
    let (id, previous) = match change {
        CommitmentChange::Renewed {
            commitment,
            previous,
            ..
        }
        | CommitmentChange::Ended {
            commitment,
            previous,
            ..
        } => (commitment, previous),
        _ => return Ok(false),
    };
    let current = commitment(view, id)?;
    if current.state != CommitmentState::Active || current.history.last() != Some(previous) {
        return Err(Denial::new(
            "commitment_state",
            "Transition requires the exact current Active commitment",
        ));
    }
    let mut updated = current.clone();
    match change {
        CommitmentChange::Renewed {
            signal,
            lease,
            basis,
            ..
        } => {
            if let Some(basis) = basis {
                let used = book.progress_renewals.entry(id.clone()).or_default();
                if used.iter().any(|(prior, _)| prior == basis) {
                    return Err(Denial::new(
                        "progress_reused",
                        "One recorded progress observation can renew only once",
                    ));
                }
                used.push((basis.clone(), event.reference()?));
            }
            if *lease != next_lease(view, current, *signal, event.at, basis.as_ref())? {
                return Err(Denial::new(
                    "lease_renewal",
                    "Renewal differs from the recorded policy and remaining lifetime",
                ));
            }
            updated.lease = lease.clone();
        }
        CommitmentChange::Ended {
            outcome, reason, ..
        } => {
            ymp_domain::require_text(reason, 4096)?;
            let source = assignment(view, id)?;
            if matches!(
                source.intent.assignment.state,
                AssignmentState::Admitted | AssignmentState::Running
            ) {
                return Err(Denial::new(
                    "commitment_authority",
                    "Revoke assignment authority before ending responsibility",
                ));
            }
            updated.state = match outcome {
                CommitmentEnd::Released { solicitation } => {
                    if event.at > current.lease.expires || event.at >= source.intent.grant.expires {
                        return Err(Denial::new(
                            "lease_expired",
                            "An expired holder cannot voluntarily release responsibility",
                        ));
                    }
                    terms(view, id)?;
                    book.pending_reopen = Some(solicitation.clone());
                    CommitmentState::Released(reason.clone())
                }
                CommitmentEnd::Expired if event.at > current.lease.expires => {
                    CommitmentState::Expired
                }
                CommitmentEnd::Expired => {
                    return Err(Denial::new(
                        "lease_live",
                        "The commitment lease has not expired",
                    ));
                }
                CommitmentEnd::Cancelled { basis } | CommitmentEnd::Discharged { basis } => {
                    let input = resolve_completion(view, basis)?;
                    completion_transition(
                        view,
                        current,
                        &input,
                        matches!(outcome, CommitmentEnd::Cancelled { .. }),
                    )?
                }
                CommitmentEnd::Delegated {
                    successor,
                    assignment,
                    nonce,
                } => {
                    crate::gatekeeper::validate_transfer_end(
                        view, id, successor, assignment, nonce,
                    )?;
                    let next = commitment(view, successor)?;
                    if next.state != CommitmentState::Active
                        || next.debtor == current.debtor
                        || next.subject != current.subject
                    {
                        return Err(Denial::new(
                            "delegation_basis",
                            "Successor does not hold this responsibility",
                        ));
                    }
                    CommitmentState::Delegated(next.debtor.clone())
                }
            };
            let source = award(view, id)?;
            let solicitation = book
                .solicitations
                .get_mut(&source.value.decision.outcome.solicitation)
                .unwrap();
            solicitation.value.state = SolicitationState::Withdrawn;
            solicitation.reference = event.reference()?;
            solicitation.at = event.at;
        }
        _ => unreachable!(),
    }
    let keep = match change {
        CommitmentChange::Ended {
            outcome: CommitmentEnd::Delegated { successor, .. },
            ..
        } => Some(
            award(view, successor)?
                .value
                .decision
                .outcome
                .solicitation
                .clone(),
        ),
        _ => None,
    };
    withdraw_delegations(book, id, keep.as_ref(), &event.reference()?, event.at)?;
    updated.history.push(event.reference()?);
    updated.validate()?;
    book.commitments.insert(id.clone(), updated);
    Ok(true)
}
fn withdraw_delegations(
    book: &mut CoordinationView,
    predecessor: &Id<Commitment>,
    keep: Option<&Id<Solicitation>>,
    source: &Ref,
    at: u64,
) -> Result<()> {
    let linked: Vec<_> = book
        .links
        .iter()
        .filter(|(id, link)| {
            link.commitment == *predecessor
                && link.kind == CommitmentLinkKind::Delegate
                && Some(*id) != keep
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in linked {
        let solicitation = book.solicitations.get_mut(&id).unwrap();
        if !matches!(
            solicitation.value.state,
            SolicitationState::Open | SolicitationState::Awarded
        ) {
            continue;
        }
        solicitation.value.state = SolicitationState::Withdrawn;
        solicitation.reference = source.clone();
        solicitation.at = at;
        if let Some(award) = book.awards.get(&id) {
            let proposed = book
                .commitments
                .get_mut(&award.value.commitment.id)
                .unwrap();
            if proposed.state != CommitmentState::Proposed {
                return Err(Denial::new(
                    "delegation_state",
                    "A changed predecessor cannot cancel admitted successor responsibility",
                ));
            }
            proposed.state = CommitmentState::Cancelled("Predecessor commitment changed".into());
            proposed.history.push(source.clone());
            proposed.validate()?;
        }
    }
    Ok(())
}
fn unsupported_basis(reference: &Ref) -> Denial {
    Denial::new(
        "commitment_basis_unsupported",
        "Completion, acceptance or plan facts require their owning kernel producer",
    )
    .with_ref(reference.clone())
}

impl<J: Journal> Arbiter<J> {
    pub fn reopen(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        solicitation: Solicitation,
        predecessor: Id<Commitment>,
    ) -> Result<u64> {
        let view = self.view(session)?;
        let prior = commitment(&view, &predecessor)?;
        self.commit(
            session,
            expected,
            at,
            vec![Event::SolicitationOpened {
                version: 1,
                solicitation: Box::new(solicitation),
                predecessor: Some(CommitmentLink {
                    commitment: predecessor,
                    previous: prior.history.last().unwrap().clone(),
                    kind: CommitmentLinkKind::Reopen,
                }),
            }],
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn open_delegation<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        token: &GrantToken,
        expected: u64,
        at: u64,
        solicitation: Solicitation,
    ) -> Result<u64> {
        gate.require_journal(&self.journal)?;
        let holder =
            gate.authorize_revision(token, Some(TeamOperation::CommitmentDelegate), at, expected)?;
        let view = self.view(&holder.session)?;
        let prior = commitment(&view, &holder.commitment)?;
        self.commit(
            &holder.session,
            expected,
            at,
            vec![Event::SolicitationOpened {
                version: 1,
                solicitation: Box::new(solicitation),
                predecessor: Some(CommitmentLink {
                    commitment: holder.commitment,
                    previous: prior.history.last().unwrap().clone(),
                    kind: CommitmentLinkKind::Delegate,
                }),
            }],
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_delegation<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        token: &GrantToken,
        expected: u64,
        at: u64,
        request: AdmissionRequest,
    ) -> Result<PreparedAdmission> {
        gate.require_journal(&self.journal)?;
        let holder =
            gate.authorize_revision(token, Some(TeamOperation::CommitmentDelegate), at, expected)?;
        let view = self.view(&holder.session)?;
        let prior = commitment(&view, &holder.commitment)?;
        gate.prepare_transfer(
            &holder.session,
            expected,
            at,
            request,
            Delegation {
                commitment: holder.commitment,
                previous: prior.history.last().unwrap().clone(),
                assignment: holder.id,
            },
        )
    }
    pub fn renew<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        token: &GrantToken,
        expected: u64,
        at: u64,
        signal: ProgressSignal,
    ) -> Result<u64> {
        self.renew_with_basis(gate, token, expected, at, signal, None)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn renew_with_basis<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        token: &GrantToken,
        expected: u64,
        at: u64,
        signal: ProgressSignal,
        basis: Option<Ref>,
    ) -> Result<u64> {
        gate.require_journal(&self.journal)?;
        let holder = gate.authorize_revision(token, None, at, expected)?;
        let view = self.view(&holder.session)?;
        let current = commitment(&view, &holder.commitment)?;
        if let Some(basis) = &basis
            && view
                .coordination()
                .progress_renewals
                .get(&holder.commitment)
                .is_some_and(|used| used.iter().any(|(prior, _)| prior == basis))
        {
            return Ok(view.revision());
        }
        let lease = next_lease(&view, current, signal, at, basis.as_ref())?;
        self.commit(
            &holder.session,
            expected,
            at,
            vec![Event::CommitmentChanged {
                version: 1,
                change: CommitmentChange::Renewed {
                    commitment: holder.commitment,
                    previous: current.history.last().unwrap().clone(),
                    signal,
                    lease,
                    basis,
                },
            }],
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn release<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        token: &GrantToken,
        expected: u64,
        at: u64,
        reason: String,
        solicitation: Solicitation,
    ) -> Result<u64> {
        gate.require_journal(&self.journal)?;
        let holder =
            gate.authorize_revision(token, Some(TeamOperation::CommitmentRelease), at, expected)?;
        self.end(
            gate,
            &holder.session,
            expected,
            at,
            &holder.commitment,
            CommitmentEnd::Released {
                solicitation: solicitation.id.clone(),
            },
            reason,
            Some(solicitation),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn end<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        session: &Id,
        expected: u64,
        at: u64,
        id: &Id<Commitment>,
        outcome: CommitmentEnd,
        reason: String,
        reopen: Option<Solicitation>,
    ) -> Result<u64> {
        gate.require_journal(&self.journal)?;
        let current = self.journal.read(session)?;
        if current.revision != expected {
            return Err(Denial::new(
                "stale_revision",
                "Commitment changed before transition",
            ));
        }
        let view = current.view_with_schemas(session, None, self.journal.schemas())?;
        let held = commitment(&view, id)?;
        let record = assignment(&view, id)?;
        let mut payloads = crate::gatekeeper::revocation_payloads(
            &view,
            &record.intent.assignment.id,
            reason.clone(),
        )?;
        payloads.push(Event::CommitmentChanged {
            version: 1,
            change: CommitmentChange::Ended {
                commitment: id.clone(),
                previous: held.history.last().unwrap().clone(),
                outcome,
                reason,
            },
        });
        if let Some(solicitation) = reopen {
            let prefix = gate.compose_events(&current, session, at, payloads.clone())?;
            payloads.push(Event::SolicitationOpened {
                version: 1,
                solicitation: Box::new(solicitation),
                predecessor: Some(CommitmentLink {
                    commitment: id.clone(),
                    previous: prefix.last().unwrap().reference()?,
                    kind: CommitmentLinkKind::Reopen,
                }),
            });
        }
        let events = gate.events(&current, session, at, payloads)?;
        let result = self.journal.append(session, expected, &events);
        match self.journal.resolve_append(session, expected, &events) {
            Ok(crate::journal::AppendResolution::Committed(end)) => Ok(end),
            _ => Err(result.err().unwrap_or_else(|| {
                Denial::new(
                    "commitment_uncertain",
                    "Cannot establish the complete commitment transition",
                )
            })),
        }
    }
    pub fn tick<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        session: &Id,
        now: u64,
    ) -> Result<Vec<Id<Commitment>>> {
        gate.require_journal(&self.journal)?;
        let view = self.view(session)?;
        if now < view.latest_at() {
            return Err(Denial::new(
                "coordination_time",
                "Clock predates recorded inputs",
            ));
        }
        let expired: Vec<_> = view
            .coordination()
            .commitments
            .values()
            .filter(|value| value.state == CommitmentState::Active && now > value.lease.expires)
            .map(|value| value.id.clone())
            .collect();
        let mut completed = vec![];
        for id in expired {
            let revision = self.view(session)?.revision();
            self.end(
                gate,
                session,
                revision,
                now,
                &id,
                CommitmentEnd::Expired,
                "Commitment lease expired".into(),
                None,
            )?;
            completed.push(id);
        }
        Ok(completed)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn discharge<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        session: &Id,
        expected: u64,
        at: u64,
        id: &Id<Commitment>,
        basis: &Ref,
    ) -> Result<u64> {
        self.complete_from_basis(gate, session, expected, at, id, basis, false)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn cancel<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        session: &Id,
        expected: u64,
        at: u64,
        id: &Id<Commitment>,
        basis: &Ref,
    ) -> Result<u64> {
        self.complete_from_basis(gate, session, expected, at, id, basis, true)
    }
    #[allow(clippy::too_many_arguments)]
    fn complete_from_basis<C: ContentStore>(
        &self,
        gate: &Gatekeeper<J, C>,
        session: &Id,
        expected: u64,
        at: u64,
        id: &Id<Commitment>,
        basis: &Ref,
        cancel: bool,
    ) -> Result<u64> {
        gate.require_journal(&self.journal)?;
        let view = self.view(session)?;
        let current = commitment(&view, id)?;
        let input = resolve_completion(&view, basis)?;
        completion_transition(&view, current, &input, cancel)?;
        let outcome = if cancel {
            CommitmentEnd::Cancelled {
                basis: basis.clone(),
            }
        } else {
            CommitmentEnd::Discharged {
                basis: basis.clone(),
            }
        };
        self.end(
            gate,
            session,
            expected,
            at,
            id,
            outcome,
            "Recorded kernel completion basis".into(),
            None,
        )
    }
}

/// A typed consumer projection, constructible only inside the kernel. Owning
/// producers will resolve real journal facts before constructing this input.
struct CompletionProjection {
    session: Id,
    assignment: Id<Assignment>,
    contribution: Id<Contribution>,
    contract: Ref,
    source: Ref,
    fact: CompletionFact,
}
#[allow(dead_code)]
enum CompletionFact {
    NonArtifactCompleted,
    ArtifactAccepted {
        subject: Ref,
    },
    TargetsSatisfied {
        targets: std::collections::BTreeSet<Id<ymp_domain::task::Criterion>>,
    },
    PlanRevised {
        affected: std::collections::BTreeSet<Id<Contribution>>,
    },
}
fn resolve_completion(view: &SessionView, source: &Ref) -> Result<CompletionProjection> {
    view.resolve(source)?;
    if let Some(accepted) = view
        .acceptances()
        .values()
        .find(|a| a.acceptance.reference().is_ok_and(|r| r == *source))
    {
        use ymp_domain::{
            plan::AttemptOutcome,
            verification::{AcceptanceDecision, AcceptanceSubject},
        };
        crate::ledger::validate_context(view, &accepted.context)?;
        let AcceptanceSubject::ResultVersion(id) = &accepted.acceptance.subject else {
            return Err(unsupported_basis(source));
        };
        let result = view
            .results()
            .results()
            .get(id)
            .ok_or_else(|| unsupported_basis(source))?;
        let attempt = view
            .results()
            .attempts()
            .get(&accepted.attempt)
            .ok_or_else(|| unsupported_basis(source))?;
        let item = &view.results().items()[&result.item];
        let assignment = &view.admission().assignments()[&attempt.attempt.assignment]
            .intent
            .assignment;
        let contribution = &view.coordination().contributions[&assignment.contribution];
        if accepted.acceptance.decision != AcceptanceDecision::Accepted
            || accepted.result != result.reference()?
            || accepted.contract != view.contract().unwrap().reference()
            || attempt.attempt.outcome != AttemptOutcome::Accepted
            || attempt.attempt.result.as_ref() != Some(id)
            || attempt.attempt.item != result.item
            || item.accepted.as_ref() != Some(id)
            || assignment.agent != result.producer
            || assignment.profile != result.profile
            || (contribution.contract != view.results().plans()[&item.plan].contract
                && !crate::results::plan_current(view, &item.plan)?)
            || contribution.value.subject.as_ref().map(|s| s.reference())
                != Some(&item.reference()?)
        {
            return Err(Denial::new(
                "commitment_basis",
                "Acceptance is rejected, stale or belongs to another production responsibility",
            )
            .with_ref(source.clone()));
        }
        return Ok(CompletionProjection {
            session: view.session().clone(),
            assignment: assignment.id.clone(),
            contribution: contribution.value.id.clone(),
            contract: contribution.contract.clone(),
            source: source.clone(),
            fact: CompletionFact::ArtifactAccepted {
                subject: item.reference()?,
            },
        });
    }
    let invocation = view
        .execution()
        .invocations()
        .values()
        .find(|record| record.end.as_ref() == Some(source))
        .ok_or_else(|| unsupported_basis(source))?;
    if invocation.terminal != Some(ymp_domain::assignment::InvocationTerminal::Completed)
        || !invocation.confirmed_terminal
        || invocation.invocation.is_none()
        || !crate::execution::closed(view, &invocation.dispatch.assignment.id)
        || crate::execution::limit_reason(view, invocation, invocation.ended_at.unwrap())?.is_some()
    {
        return Err(Denial::new(
            "commitment_basis",
            "Invocation lacks bounded confirmed completion and scoped cessation",
        )
        .with_ref(source.clone()));
    }
    let contribution =
        &view.coordination().contributions[&invocation.dispatch.assignment.contribution];
    Ok(CompletionProjection {
        session: view.session().clone(),
        assignment: invocation.dispatch.assignment.id.clone(),
        contribution: contribution.value.id.clone(),
        contract: contribution.contract.clone(),
        source: source.clone(),
        fact: CompletionFact::NonArtifactCompleted,
    })
}

fn completion_transition(
    view: &SessionView,
    current: &Commitment,
    input: &CompletionProjection,
    cancel: bool,
) -> Result<CommitmentState> {
    let source = assignment(view, &current.id)?;
    let contribution = &view.coordination().contributions[&current.subject];
    view.resolve(&input.source)?;
    projected_completion(
        current,
        &source.intent.assignment,
        &contribution.value,
        &contribution.contract,
        input,
        cancel,
    )
}
fn projected_completion(
    current: &Commitment,
    source: &Assignment,
    contribution: &Contribution,
    contract: &Ref,
    input: &CompletionProjection,
    cancel: bool,
) -> Result<CommitmentState> {
    if input.session != source.session
        || input.assignment != source.id
        || input.contribution != current.subject
        || &input.contract != contract
        || current.state != CommitmentState::Active
        || source.commitment != current.id
        || source.contribution != current.subject
        || source.agent != current.debtor
    {
        return Err(Denial::new(
            "commitment_basis",
            "Completion projection names different responsibility",
        ));
    }
    match (&input.fact, cancel, &contribution.subject) {
        (
            CompletionFact::NonArtifactCompleted,
            false,
            Some(ymp_domain::assignment::ContributionSubject::ResultVersion(_)),
        ) if matches!(
            contribution.kind,
            ymp_domain::assignment::ContributionKind::Review
                | ymp_domain::assignment::ContributionKind::Verify
                | ymp_domain::assignment::ContributionKind::Diagnose
        ) =>
        {
            Ok(CommitmentState::Discharged)
        }
        (CompletionFact::NonArtifactCompleted, false, None)
            if !matches!(
                contribution.kind,
                ymp_domain::assignment::ContributionKind::Produce
                    | ymp_domain::assignment::ContributionKind::Alternative
                    | ymp_domain::assignment::ContributionKind::Integrate
            ) =>
        {
            Ok(CommitmentState::Discharged)
        }
        (CompletionFact::ArtifactAccepted { subject }, false, Some(expected))
            if subject == expected.reference() =>
        {
            Ok(CommitmentState::Discharged)
        }
        (CompletionFact::TargetsSatisfied { targets }, true, _)
            if !contribution.targets.is_empty() && contribution.targets.is_subset(targets) =>
        {
            Ok(CommitmentState::Cancelled(
                "Recorded satisfied targets".into(),
            ))
        }
        (CompletionFact::PlanRevised { affected }, true, _)
            if affected.contains(&current.subject) =>
        {
            Ok(CommitmentState::Cancelled(
                "Recorded applicable plan revision".into(),
            ))
        }
        _ => Err(Denial::new(
            "commitment_basis",
            "Completion kind does not cover this contribution",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use ymp_domain::{
        Prob,
        assignment::*,
        resources::{Allowance, Difficulty},
        task::Real,
    };
    fn id<T>(text: &str) -> Id<T> {
        Id::new(text).unwrap()
    }
    fn reference(text: &str) -> Ref {
        Ref {
            id: id(text),
            version: Digest::of(text),
        }
    }
    fn scenario() -> (Assignment, Commitment, Contribution, Ref, CommitmentTerms) {
        let profile = ExecutionProfile {
            agent: id("holder"),
            model: "synthetic".into(),
            provider_version: None,
            family: None,
            effort: None,
        };
        let terms = CommitmentTerms {
            lease_duration: 20,
            renewal_duration: 7,
            renew_on: BTreeSet::from([
                ProgressSignal::Heartbeat,
                ProgressSignal::CheckRun,
                ProgressSignal::EvidenceAdded,
                ProgressSignal::ResultSubmitted,
            ]),
            renewals: 2,
            release_delta: Real::new(2.0).unwrap(),
        };
        let current = Commitment {
            id: id("commitment"),
            debtor: id("holder"),
            creditor: Creditor::Team,
            subject: id("contribution"),
            condition: Some("Preserve the target".into()),
            lease: terms.initial_lease(0, 100).unwrap(),
            state: CommitmentState::Active,
            history: vec![reference("active")],
        };
        let source = Assignment {
            id: id("assignment"),
            session: id("session"),
            agent: id("holder"),
            profile: profile.clone(),
            contribution: id("contribution"),
            role: RoleKind::Planner,
            access: BTreeSet::new(),
            workspace: id("workspace"),
            allowance: Allowance {
                cost: Real::new(1.0).unwrap(),
                timeout: 100,
                native_turns: 1,
                output_chars: 100,
            },
            grant: id("grant"),
            commitment: id("commitment"),
            state: AssignmentState::Admitted,
        };
        let contribution = Contribution {
            id: id("contribution"),
            session: id("session"),
            kind: ContributionKind::Plan,
            targets: BTreeSet::from([id("criterion")]),
            subject: None,
            needs: BTreeSet::new(),
            forecast: Forecast {
                p_success: Prob::new(0.5).unwrap(),
                delta_belief: BTreeMap::new(),
                source: ForecastSource::Agent(profile),
            },
            cost: CostEstimate {
                expected: Real::new(1.0).unwrap(),
                p90: Real::new(1.0).unwrap(),
            },
            difficulty: Difficulty::Simple,
            proposed_by: ContributionAuthor::Runtime,
            basis: vec![],
        };
        (source, current, contribution, reference("contract"), terms)
    }
    #[test]
    fn synthetic_progress_projections_use_the_same_bounded_renewal_consumer() {
        let (source, current, _, _, mut terms) = scenario();
        for signal in &terms.renew_on {
            let mut input = ProgressProjection {
                session: source.session.clone(),
                assignment: source.id.clone(),
                contribution: current.subject.clone(),
                signal: *signal,
                source: (*signal != ProgressSignal::Heartbeat)
                    .then(|| reference("synthetic-observation")),
            };
            assert_eq!(
                projected_renewal(&current, &source, &terms, 100, 20, &input)
                    .unwrap()
                    .expires,
                27
            );
            assert!(projected_renewal(&current, &source, &terms, 100, 21, &input).is_err());
            input.assignment = id("another-assignment");
            assert_eq!(
                projected_renewal(&current, &source, &terms, 100, 20, &input)
                    .unwrap_err()
                    .code,
                "progress_basis"
            );
        }
        terms.renewals = 4001;
        assert!(terms.validate().is_err());
    }
    #[test]
    fn synthetic_completion_projections_validate_scope_without_minting_journal_facts() {
        let (source, current, mut contribution, contract, _) = scenario();
        let mut input = CompletionProjection {
            session: source.session.clone(),
            assignment: source.id.clone(),
            contribution: contribution.id.clone(),
            contract: contract.clone(),
            source: reference("synthetic-completion"),
            fact: CompletionFact::NonArtifactCompleted,
        };
        assert_eq!(
            projected_completion(&current, &source, &contribution, &contract, &input, false)
                .unwrap(),
            CommitmentState::Discharged
        );
        input.assignment = id("foreign");
        assert!(
            projected_completion(&current, &source, &contribution, &contract, &input, false)
                .is_err()
        );
        input.assignment = source.id.clone();
        input.fact = CompletionFact::TargetsSatisfied {
            targets: BTreeSet::new(),
        };
        assert!(
            projected_completion(&current, &source, &contribution, &contract, &input, true)
                .is_err()
        );
        input.fact = CompletionFact::TargetsSatisfied {
            targets: contribution.targets.clone(),
        };
        assert!(matches!(
            projected_completion(&current, &source, &contribution, &contract, &input, true)
                .unwrap(),
            CommitmentState::Cancelled(_)
        ));
        input.fact = CompletionFact::PlanRevised {
            affected: BTreeSet::from([id("unrelated")]),
        };
        assert!(
            projected_completion(&current, &source, &contribution, &contract, &input, true)
                .is_err()
        );
        input.fact = CompletionFact::PlanRevised {
            affected: BTreeSet::from([current.subject.clone()]),
        };
        assert!(matches!(
            projected_completion(&current, &source, &contribution, &contract, &input, true)
                .unwrap(),
            CommitmentState::Cancelled(_)
        ));
        contribution.kind = ContributionKind::Produce;
        contribution.subject = Some(ContributionSubject::ResultVersion(reference("result-v1")));
        input.fact = CompletionFact::ArtifactAccepted {
            subject: reference("result-v2"),
        };
        assert!(
            projected_completion(&current, &source, &contribution, &contract, &input, false)
                .is_err()
        );
        input.fact = CompletionFact::ArtifactAccepted {
            subject: reference("result-v1"),
        };
        assert_eq!(
            projected_completion(&current, &source, &contribution, &contract, &input, false)
                .unwrap(),
            CommitmentState::Discharged
        );
    }
}
