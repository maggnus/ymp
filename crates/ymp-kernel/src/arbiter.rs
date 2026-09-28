//! Persisted contribution, RuntimeProxy offer and award foundations for admission.
mod lifecycle;
use crate::{
    events::Event,
    journal::{Journal, ParameterSchemas, validate_append},
    view::SessionView,
};
pub use lifecycle::{CommitmentEnd, CommitmentLink, CommitmentLinkKind, Delegation};
pub(crate) use lifecycle::{initial_lease, validate_delegation};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use ymp_domain::{
    Denial, Digest, Id, Proposal, Ref, Result,
    assignment::{Contribution, ContributionAuthor},
    coordination::*,
    identity::{ExecutionProfile, Readiness},
    journal::{Actor, Decision, Envelope},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Recorded<T> {
    pub value: T,
    pub reference: Ref,
    pub at: u64,
}
/// The exact acceptance contract under which the immutable contribution was proposed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContributionRecord {
    pub value: Contribution,
    pub reference: Ref,
    pub at: u64,
    pub contract: Ref,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Awarded {
    pub decision: Decision<Award>,
    pub commitment: Commitment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms: Option<Decision<CommitmentTerms>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitmentChange {
    Renewed {
        commitment: Id<Commitment>,
        previous: Ref,
        signal: ProgressSignal,
        lease: Lease,
        basis: Option<Ref>,
    },
    Ended {
        commitment: Id<Commitment>,
        previous: Ref,
        outcome: CommitmentEnd,
        reason: String,
    },
    Cancelled {
        commitment: Id<Commitment>,
        award: Ref,
        reason: String,
    },
    Activated {
        nonce: Digest,
        commitment: Id<Commitment>,
        lease: Lease,
    },
    Proposed(Commitment),
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CoordinationView {
    contributions: BTreeMap<Id<Contribution>, ContributionRecord>,
    solicitations: BTreeMap<Id<Solicitation>, Recorded<Solicitation>>,
    offers: BTreeMap<Id<Offer>, Recorded<Offer>>,
    awards: BTreeMap<Id<Solicitation>, Recorded<Awarded>>,
    commitments: BTreeMap<Id<Commitment>, Commitment>,
    pending_award: Option<(Commitment, Ref)>,
    links: BTreeMap<Id<Solicitation>, CommitmentLink>,
    pending_reopen: Option<Id<Solicitation>>,
    progress_renewals: BTreeMap<Id<Commitment>, Vec<(Ref, Ref)>>,
}
impl CoordinationView {
    pub fn contributions(&self) -> &BTreeMap<Id<Contribution>, ContributionRecord> {
        &self.contributions
    }
    pub fn solicitations(&self) -> &BTreeMap<Id<Solicitation>, Recorded<Solicitation>> {
        &self.solicitations
    }
    pub fn offers(&self) -> &BTreeMap<Id<Offer>, Recorded<Offer>> {
        &self.offers
    }
    pub fn awards(&self) -> &BTreeMap<Id<Solicitation>, Recorded<Awarded>> {
        &self.awards
    }
    pub fn commitments(&self) -> &BTreeMap<Id<Commitment>, Commitment> {
        &self.commitments
    }
    pub(crate) fn complete(&self) -> Result<()> {
        if self.pending_reopen.is_some() {
            return Err(Denial::new(
                "release_incomplete",
                "Release and reopened solicitation must commit together",
            ));
        }
        if self.pending_award.is_some() {
            return Err(Denial::new(
                "award_incomplete",
                "Award and Proposed commitment must be committed together",
            ));
        }
        Ok(())
    }
    pub(crate) fn check_next(&self, event: &Event) -> Result<()> {
        if let Some(expected) = &self.pending_reopen {
            return if matches!(event, Event::SolicitationOpened { solicitation, .. } if &solicitation.id == expected)
            {
                Ok(())
            } else {
                Err(Denial::new(
                    "release_incomplete",
                    "Only the linked reopening may follow release",
                ))
            };
        }
        if self.pending_award.is_some()
            && !matches!(
                event,
                Event::CommitmentChanged {
                    change: CommitmentChange::Proposed(_),
                    ..
                }
            )
        {
            return Err(Denial::new(
                "award_incomplete",
                "No event may interrupt an award/commitment packet",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AwardView {
    pub journal: Digest,
    pub contribution: ContributionRecord,
    pub solicitation: Recorded<Solicitation>,
    pub offers: Vec<Recorded<Offer>>,
    pub at: u64,
}
pub fn award_view(
    view: &SessionView,
    solicitation: &Id<Solicitation>,
    at: u64,
) -> Result<AwardView> {
    let book = view.coordination();
    book.complete()?;
    let solicitation = book
        .solicitations
        .get(solicitation)
        .ok_or_else(|| Denial::new("solicitation_missing", "No recorded solicitation"))?;
    if solicitation.value.state != SolicitationState::Open {
        return Err(
            Denial::new("solicitation_state", "Solicitation is not open")
                .with_ref(solicitation.reference.clone()),
        );
    }
    if at < solicitation.value.deadline {
        return Err(Denial::new(
            "solicitation_open",
            "Wait for the offer deadline before awarding",
        )
        .with_ref(solicitation.reference.clone()));
    }
    let contribution = book
        .contributions
        .get(&solicitation.value.contribution)
        .ok_or_else(|| Denial::new("contribution_missing", "No recorded contribution"))?;
    let mut offers: Vec<_> = book
        .offers
        .values()
        .filter(|offer| offer.value.solicitation == solicitation.value.id)
        .cloned()
        .collect();
    offers.sort_by(|a, b| (a.value.at, &a.value.id).cmp(&(b.value.at, &b.value.id)));
    Ok(AwardView {
        journal: view.digest()?,
        contribution: contribution.clone(),
        solicitation: solicitation.clone(),
        offers,
        at,
    })
}
fn current_profile(view: &SessionView, profile: &ExecutionProfile) -> Result<()> {
    profile.validate()?;
    let pool = view
        .registry()
        .ok_or_else(|| Denial::new("registry_missing", "No Registry observation"))?;
    let task = view
        .task()
        .ok_or_else(|| Denial::new("task_missing", "No task"))?;
    if task.constraints != pool.input.constraints
        || !pool
            .decisions
            .iter()
            .any(|decision| decision.profile == *profile && decision.outcome == Readiness::Ready)
    {
        return Err(Denial::new(
            "profile_excluded",
            "Profile is not eligible under current constraints",
        ));
    }
    Ok(())
}
pub(crate) fn attribution(
    view: &SessionView,
    event: &Event,
) -> Result<(Option<ymp_domain::PolicyRef>, Option<Digest>, Vec<Ref>)> {
    let book = view.coordination();
    let mut refs = vec![];
    let mut policy = None;
    let mut input = None;
    match event {
        Event::ContributionProposed { contribution, .. } => {
            refs.extend(contribution.basis.clone());
            if let Some(subject) = &contribution.subject {
                refs.push(subject.reference().clone());
            }
            refs.push(
                view.contract()
                    .ok_or_else(|| Denial::new("contract_missing", "No acceptance contract"))?
                    .reference(),
            );
        }
        Event::SolicitationOpened {
            solicitation,
            predecessor,
            ..
        } => {
            if let Some(link) = predecessor {
                refs.push(link.previous.clone());
            }
            refs.push(
                book.contributions
                    .get(&solicitation.contribution)
                    .ok_or_else(|| Denial::new("contribution_missing", "No recorded contribution"))?
                    .reference
                    .clone(),
            );
        }
        Event::OfferSubmitted { offer, .. } => refs.push(
            book.solicitations
                .get(&offer.solicitation)
                .ok_or_else(|| Denial::new("solicitation_missing", "No recorded solicitation"))?
                .reference
                .clone(),
        ),
        Event::Awarded { data, .. } => {
            policy = Some(data.decision.effective.policy.clone());
            input = Some(data.decision.input.clone());
            refs.extend(data.decision.proposal.basis.clone());
            if let Some(terms) = &data.terms {
                refs.extend(terms.proposal.basis.clone());
            }
            refs.push(
                book.solicitations
                    .get(&data.decision.outcome.solicitation)
                    .ok_or_else(|| Denial::new("solicitation_missing", "No recorded solicitation"))?
                    .reference
                    .clone(),
            );
            refs.push(
                book.offers
                    .get(&data.decision.outcome.offer)
                    .ok_or_else(|| Denial::new("offer_missing", "No recorded offer"))?
                    .reference
                    .clone(),
            );
        }
        Event::CommitmentChanged {
            change: change @ (CommitmentChange::Renewed { .. } | CommitmentChange::Ended { .. }),
            ..
        } => {
            return lifecycle::attribution(view, change);
        }
        Event::CommitmentChanged {
            change:
                CommitmentChange::Cancelled {
                    commitment, award, ..
                },
            ..
        } => {
            refs.push(award.clone());
            refs.push(
                book.commitments
                    .get(commitment)
                    .and_then(|value| value.history.last())
                    .ok_or_else(|| Denial::new("commitment_missing", "No proposed commitment"))?
                    .clone(),
            );
        }
        Event::CommitmentChanged {
            change: CommitmentChange::Activated { .. },
            ..
        } => refs.extend(crate::gatekeeper::attribution(view)?),
        Event::CommitmentChanged {
            change: CommitmentChange::Proposed(_),
            ..
        } => refs.push(
            book.pending_award
                .as_ref()
                .ok_or_else(|| {
                    Denial::new(
                        "award_incomplete",
                        "Proposed commitment requires the immediately preceding award",
                    )
                })?
                .1
                .clone(),
        ),
        _ => {
            return Err(Denial::new(
                "coordination_event",
                "Not a coordination event",
            ));
        }
    }
    refs.sort();
    refs.dedup();
    Ok((policy, input, refs))
}
pub(crate) fn apply(
    view: &SessionView,
    event: &Envelope<Event>,
    schemas: &ParameterSchemas,
) -> Result<CoordinationView> {
    if event.at < view.latest_at() {
        return Err(Denial::new(
            "coordination_time",
            "Coordination cannot predate its recorded inputs",
        ));
    }
    let mut book = view.coordination().clone();
    book.check_next(&event.payload)?;
    if attribution(view, &event.payload)?
        != (
            event.policy.clone(),
            event.input.clone(),
            event.refs.clone(),
        )
    {
        return Err(Denial::new(
            "coordination_attribution",
            "Coordination attribution differs from recorded inputs",
        ));
    }
    let reference = event.reference()?;
    if lifecycle::apply(view, event, &mut book)? {
        return Ok(book);
    }
    match &event.payload {
        Event::ContributionProposed { contribution, .. } => {
            contribution.validate()?;
            crate::results::subject_item(view, contribution)?;
            if contribution.session != *view.session()
                || contribution.proposed_by != ContributionAuthor::Runtime
                || book.contributions.contains_key(&contribution.id)
                || book.contributions.len() >= 4096
                || contribution
                    .targets
                    .iter()
                    .any(|id| !view.criteria().iter().any(|criterion| &criterion.id == id))
            {
                return Err(Denial::new(
                    "contribution",
                    "Contribution has invalid identity, author, targets or capacity",
                ));
            }
            book.contributions.insert(
                contribution.id.clone(),
                ContributionRecord {
                    value: (**contribution).clone(),
                    reference,
                    at: event.at,
                    contract: view.contract().expect("attributed contract").reference(),
                },
            );
        }
        Event::SolicitationOpened {
            solicitation,
            predecessor,
            ..
        } => {
            solicitation.validate()?;
            lifecycle::validate_link(view, solicitation, predecessor.as_ref())?;
            let pool = view
                .registry()
                .ok_or_else(|| Denial::new("registry_missing", "No Registry observation"))?;
            if solicitation.state != SolicitationState::Open
                || (predecessor.is_none() && solicitation.reopened != 0)
                || event.at < book.contributions[&solicitation.contribution].at
                || solicitation.deadline < event.at
                || view
                    .task()
                    .and_then(|task| task.constraints.deadline)
                    .is_some_and(|end| solicitation.deadline > end)
                || !solicitation.eligible.is_subset(&pool.outcome.eligible)
                || book.solicitations.contains_key(&solicitation.id)
                || book.solicitations.len() >= 4096
                || book.solicitations.values().any(|prior| {
                    prior.value.contribution == solicitation.contribution
                        && !lifecycle::is_delegation_source(
                            view,
                            predecessor.as_ref(),
                            &prior.value.id,
                        )
                        && matches!(
                            prior.value.state,
                            SolicitationState::Open | SolicitationState::Awarded
                        )
                })
            {
                return Err(Denial::new(
                    "solicitation",
                    "Solicitation state, deadline, eligibility or identity is invalid",
                ));
            }
            book.solicitations.insert(
                solicitation.id.clone(),
                Recorded {
                    value: (**solicitation).clone(),
                    reference,
                    at: event.at,
                },
            );
            if let Some(link) = predecessor {
                book.links.insert(solicitation.id.clone(), link.clone());
            }
            book.pending_reopen = None;
        }
        Event::OfferSubmitted { offer, .. } => {
            offer.validate()?;
            current_profile(view, &offer.profile)?;
            let source = book
                .solicitations
                .get(&offer.solicitation)
                .expect("attributed solicitation");
            let solicitation = &source.value;
            let contribution = &book.contributions[&solicitation.contribution].value;
            if offer.source != OfferSource::RuntimeProxy
                || offer.at != event.at
                || offer.at < source.at
                || offer.at > solicitation.deadline
                || solicitation.state != SolicitationState::Open
                || !solicitation.eligible.contains(&offer.agent)
                || offer
                    .forecast
                    .delta_belief
                    .keys()
                    .any(|id| !contribution.targets.contains(id))
                || book.offers.contains_key(&offer.id)
                || book.offers.len() >= 4096
                || book
                    .offers
                    .values()
                    .filter(|prior| prior.value.solicitation == offer.solicitation)
                    .count()
                    >= 64
            {
                return Err(Denial::new(
                    "offer",
                    "RuntimeProxy offer has invalid source, timing, eligibility or identity",
                ));
            }
            book.offers.insert(
                offer.id.clone(),
                Recorded {
                    value: (**offer).clone(),
                    reference,
                    at: event.at,
                },
            );
        }
        Event::Awarded { data, .. } => {
            data.decision.proposal.validate()?;
            data.decision.outcome.validate()?;
            data.commitment.validate()?;
            schemas.validate(&data.decision.effective)?;
            let input = award_view(view, &data.decision.outcome.solicitation, event.at)?;
            lifecycle::validate_terms(view, data, &input)?;
            let offer = book
                .offers
                .get(&data.decision.outcome.offer)
                .expect("attributed offer");
            current_profile(view, &offer.value.profile)?;
            if view.policies().get("AwardPolicy") != Some(&data.decision.effective)
                || data.decision.proposal.policy != data.decision.effective.policy
                || data.decision.outcome != data.decision.proposal.value
                || data.decision.input != Digest::of_value(&input)?
                || data.decision.selection_change.is_some()
                || offer.value.solicitation != input.solicitation.value.id
                || offer.value.at > event.at
                || view
                    .task()
                    .and_then(|task| task.constraints.deadline)
                    .is_some_and(|end| event.at > end)
                || data.commitment.debtor != offer.value.agent
                || data.commitment.subject != input.contribution.value.id
                || data.commitment.state != CommitmentState::Proposed
                || !data.commitment.history.is_empty()
                || data.commitment.lease.expires <= event.at
                || book.commitments.contains_key(&data.commitment.id)
                || book.commitments.len() >= 4096
                || book.awards.contains_key(&input.solicitation.value.id)
            {
                return Err(Denial::new(
                    "award",
                    "Award, selected policy, input or Proposed commitment disagree",
                ));
            }
            book.solicitations
                .get_mut(&input.solicitation.value.id)
                .unwrap()
                .value
                .state = SolicitationState::Awarded;
            book.solicitations
                .get_mut(&input.solicitation.value.id)
                .unwrap()
                .reference = reference.clone();
            book.solicitations
                .get_mut(&input.solicitation.value.id)
                .unwrap()
                .at = event.at;
            book.pending_award = Some((data.commitment.clone(), reference.clone()));
            book.awards.insert(
                input.solicitation.value.id.clone(),
                Recorded {
                    value: (**data).clone(),
                    reference,
                    at: event.at,
                },
            );
        }
        Event::CommitmentChanged {
            change:
                CommitmentChange::Cancelled {
                    commitment,
                    award,
                    reason,
                },
            ..
        } => {
            ymp_domain::require_text(reason, 4096)?;
            let source = book
                .awards
                .values()
                .find(|value| value.reference == *award)
                .ok_or_else(|| {
                    Denial::new("award_missing", "No matching award for cancellation")
                })?;
            let solicitation = source.value.decision.outcome.solicitation.clone();
            if source.value.commitment.id != *commitment
                || book.solicitations[&solicitation].value.state != SolicitationState::Awarded
            {
                return Err(Denial::new(
                    "commitment_basis",
                    "Cancellation differs from the current award",
                ));
            }
            let current = book
                .commitments
                .get_mut(commitment)
                .ok_or_else(|| Denial::new("commitment_missing", "No Proposed commitment"))?;
            if current.state != CommitmentState::Proposed {
                return Err(Denial::new(
                    "commitment_state",
                    "Only this award's Proposed commitment can be cancelled by admission",
                ));
            }
            current.state = CommitmentState::Cancelled(reason.clone());
            current.history.push(reference.clone());
            let solicitation = book.solicitations.get_mut(&solicitation).unwrap();
            solicitation.value.state = SolicitationState::Withdrawn;
            solicitation.reference = reference;
            solicitation.at = event.at;
        }
        Event::CommitmentChanged {
            change:
                CommitmentChange::Activated {
                    commitment, lease, ..
                },
            ..
        } => {
            let current = book
                .commitments
                .get_mut(commitment)
                .ok_or_else(|| Denial::new("commitment_missing", "No Proposed commitment"))?;
            if current.state != CommitmentState::Proposed {
                return Err(Denial::new(
                    "commitment_state",
                    "Only a Proposed commitment may activate",
                ));
            }
            current.state = CommitmentState::Active;
            current.lease = lease.clone();
            current.history.push(reference);
        }
        Event::CommitmentChanged {
            change: CommitmentChange::Proposed(commitment),
            ..
        } => {
            let (expected, award) = book
                .pending_award
                .take()
                .ok_or_else(|| Denial::new("award_incomplete", "No pending award"))?;
            if commitment != &expected {
                return Err(Denial::new(
                    "commitment",
                    "Proposed commitment differs from the award",
                ));
            }
            let mut committed = commitment.clone();
            committed.history = vec![award, reference];
            book.commitments.insert(committed.id.clone(), committed);
        }
        _ => {
            return Err(Denial::new(
                "coordination_event",
                "Not a coordination event",
            ));
        }
    }
    Ok(book)
}
pub struct Arbiter<J: Journal> {
    journal: Arc<J>,
}
impl<J: Journal> Arbiter<J> {
    pub fn new(journal: Arc<J>) -> Self {
        Self { journal }
    }
    pub fn view(&self, session: &Id) -> Result<SessionView> {
        self.journal
            .read(session)?
            .view_with_schemas(session, None, self.journal.schemas())
    }
    fn commit(&self, session: &Id, expected: u64, at: u64, payloads: Vec<Event>) -> Result<u64> {
        let current = self.journal.read(session)?;
        if current.revision != expected {
            return Err(Denial::new(
                "stale_revision",
                "Coordination changed before commit",
            ));
        }
        let mut view = current.view_with_schemas(session, None, self.journal.schemas())?;
        let mut events = vec![];
        for payload in payloads {
            let (policy, input, refs) = attribution(&view, &payload)?;
            let event = Envelope {
                seq: view.revision().checked_add(1).ok_or_else(|| {
                    Denial::new("revision_overflow", "Journal sequence exhausted")
                })?,
                session: session.clone(),
                at,
                actor: Actor::Runtime,
                policy,
                input,
                refs,
                payload,
            };
            view.apply(&event, self.journal.schemas())?;
            events.push(event);
        }
        validate_append(&current, session, expected, &events, self.journal.schemas())?;
        let committed = self.journal.append(session, expected, &events)?;
        if committed != view.revision() {
            return Err(Denial::new(
                "journal_append",
                "Unexpected coordination commit revision",
            ));
        }
        Ok(committed)
    }
    pub fn propose(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        contribution: Contribution,
    ) -> Result<u64> {
        self.commit(
            session,
            expected,
            at,
            vec![Event::ContributionProposed {
                version: 1,
                contribution: Box::new(contribution),
            }],
        )
    }
    pub fn open(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        solicitation: Solicitation,
    ) -> Result<u64> {
        self.commit(
            session,
            expected,
            at,
            vec![Event::SolicitationOpened {
                version: 1,
                solicitation: Box::new(solicitation),
                predecessor: None,
            }],
        )
    }
    pub fn submit(&self, session: &Id, expected: u64, at: u64, offer: Offer) -> Result<u64> {
        self.commit(
            session,
            expected,
            at,
            vec![Event::OfferSubmitted {
                version: 1,
                offer: Box::new(offer),
            }],
        )
    }
    pub fn award(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        proposal: Proposal<Award>,
        input: Digest,
        commitment: Commitment,
    ) -> Result<u64> {
        self.award_with_terms(session, expected, at, proposal, input, commitment, None)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn award_with_terms(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        proposal: Proposal<Award>,
        input: Digest,
        commitment: Commitment,
        terms: Option<Proposal<CommitmentTerms>>,
    ) -> Result<u64> {
        let view = self.view(session)?;
        let effective = view
            .policies()
            .get("AwardPolicy")
            .ok_or_else(|| Denial::new("policy_selection", "No AwardPolicy selected"))?
            .clone();
        let data = Awarded {
            terms: terms.map(|proposal| Decision {
                outcome: proposal.value.clone(),
                proposal,
                effective: effective.clone(),
                input: input.clone(),
                selection_change: None,
            }),
            decision: Decision {
                outcome: proposal.value.clone(),
                proposal,
                effective,
                input,
                selection_change: None,
            },
            commitment: commitment.clone(),
        };
        self.commit(
            session,
            expected,
            at,
            vec![
                Event::Awarded {
                    version: 1,
                    data: Box::new(data),
                },
                Event::CommitmentChanged {
                    version: 1,
                    change: CommitmentChange::Proposed(commitment),
                },
            ],
        )
    }
}
