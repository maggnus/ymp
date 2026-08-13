//! Generated concurrent command schedules and the invariants every one of them must preserve.
//!
//! One ordering proves nothing about contention. The generator builds a fixed cast of
//! participants, offers and consent, then interleaves a pool of commands issued by different
//! principals — including commands that are only valid in some orderings, and commands that must
//! never be valid at all — and replays the whole pool in a seeded order. Refusals are expected and
//! are not failures; what the schedule asserts is that after every single command the facts still
//! conserve every budget dimension and still agree with the records the kernel keeps, that no more
//! slots were awarded than were funded, that one consent formed one contract and one contract one
//! obligation, that nothing advanced under a stale fencing token, that no obligation closed while
//! its causal work was outstanding, and that nobody closed work it did not hold.
//!
//! Three things the pool alone cannot do are added here. One participant's own commands are merged
//! in as a causal sequence rather than permuted, because a uniform shuffle practically never
//! reaches a state that takes a dozen ordered commands to build. The accounts are recomputed from
//! the facts the ledger emitted, so that what is checked is not the same records the kernel writes.
//! And the pool asks for capacity that does not exist and closes work under participants that do
//! not hold it, so that the checks refusing those things are reached rather than merely present.

use std::collections::{BTreeMap, BTreeSet};

use super::budget::{BudgetVector, DIMENSION_COUNT, DIMENSIONS, Dimension};
use super::ledger::{CommitmentLedger, DisabledChecks};
use super::protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CancelContract, CommitmentCommand, CommitmentError,
    CommitmentEvent, Reassign, RecordBid, RegisterParticipant, RenewLease, ReturnObligation,
    SettleOffer, StartAttempt, SubmitResult, WithdrawBid, WithdrawOffer,
};
use super::records::{
    AccountRef, FundingSource, ObligationState, OfferPolicy, OfferState, Outcome,
};

pub(crate) const ROOT_PARTICIPANT: &str = "sponsor-root";
pub(crate) const ROOT_PRINCIPAL: &str = "principal-root";
pub(crate) const ROOT_OBLIGATION: &str = "obligation-root";
pub(crate) const ALPHA: &str = "p-alpha";
pub(crate) const BETA: &str = "p-beta";
pub(crate) const GAMMA: &str = "p-gamma";
pub(crate) const MAIN_OFFER: &str = "offer-main";
pub(crate) const OPEN_OFFER: &str = "offer-open";
pub(crate) const CHILD_OFFER: &str = "offer-child";
/// An offer nobody contends for, which gives one participant two contracts of its own.
pub(crate) const SOLO_OFFER: &str = "offer-solo";
/// The contract whose escrow funds the delegated offer, and which then closes.
pub(crate) const SOLO_FUNDING_CONTRACT: &str = "contract-solo-a";
/// The obligation the delegated offer hangs under, held by the same participant but belonging to
/// its other contract.
pub(crate) const SOLO_PARENT_OBLIGATION: &str = "obligation-solo-b";
/// The offer funded from one contract and parented under the obligation of another.
pub(crate) const CROSS_OFFER: &str = "offer-cross";
/// The offer advertised from whatever the settlement of the cross offer returned.
pub(crate) const SECOND_OFFER: &str = "offer-second";
/// The offer advertised once the funding contract has closed for good.
pub(crate) const THIRD_OFFER: &str = "offer-third";
/// An offer reserving more than the run was ever funded with, which no ordering may accept.
pub(crate) const UNFUNDED_OFFER: &str = "offer-unfunded";
pub(crate) const MAIN_MAX_AWARDS: u32 = 2;
pub(crate) const LEASE_MS: u64 = 100;
pub(crate) const DEADLINE: u64 = 800;

/// The opaque tokens the kernel may only compare for equality, gathered so that a whole schedule
/// can be replayed under a different alphabet.
#[derive(Clone, Debug)]
pub(crate) struct Tokens {
    pub task_scope: String,
    pub artifact_class: String,
    pub intent_digest: String,
    pub base_digest: String,
    pub dependency: String,
    pub capability: String,
    pub proposal_prefix: String,
}

impl Tokens {
    pub(crate) fn variant(label: &str) -> Self {
        Self {
            task_scope: format!("scope-{label}"),
            artifact_class: format!("class-{label}"),
            intent_digest: digest(&format!("intent-{label}")),
            base_digest: digest(&format!("base-{label}")),
            dependency: format!("dependency-{label}"),
            capability: format!("capability-{label}"),
            proposal_prefix: format!("proposal-{label}"),
        }
    }

    fn proposal(&self, who: &str) -> String {
        digest(&format!("{}-{who}", self.proposal_prefix))
    }

    /// Every opaque value this alphabet contributes, paired with the value the other alphabet puts
    /// in the same place.
    pub(crate) fn substitutions(&self, other: &Self) -> Vec<(String, String)> {
        let mut pairs = vec![
            (self.task_scope.clone(), other.task_scope.clone()),
            (self.artifact_class.clone(), other.artifact_class.clone()),
            (self.intent_digest.clone(), other.intent_digest.clone()),
            (self.base_digest.clone(), other.base_digest.clone()),
            (self.dependency.clone(), other.dependency.clone()),
            (self.capability.clone(), other.capability.clone()),
        ];
        for who in [
            ALPHA,
            BETA,
            GAMMA,
            "child",
            "second",
            "bid-solo-a",
            "bid-solo-b",
        ] {
            pairs.push((self.proposal(who), other.proposal(who)));
        }
        pairs
    }
}

pub(crate) fn digest(tag: &str) -> String {
    crate::digest_bytes(tag.as_bytes())
}

fn root_budget() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 10_000_000)
        .with(Dimension::ModelTokens, 10_000_000)
        .with(Dimension::WallTimeMs, 10_000_000)
        .with(Dimension::VerificationQueries, 1_000)
        .with(Dimension::ExternalActions, 1_000)
        .with(Dimension::ParticipantStarts, 10)
        .with(Dimension::AttemptStarts, 1_000)
        .with(Dimension::OfferCreations, 1_000)
        .with(Dimension::ObligationCreations, 1_000)
}

fn endowment() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 100_000)
        .with(Dimension::ModelTokens, 100_000)
        .with(Dimension::WallTimeMs, 100_000)
        .with(Dimension::VerificationQueries, 10)
        .with(Dimension::ExternalActions, 10)
        .with(Dimension::AttemptStarts, 20)
        .with(Dimension::OfferCreations, 20)
        .with(Dimension::ObligationCreations, 20)
}

/// What one award of the main offer funds at most.
fn execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 20_000)
        .with(Dimension::ModelTokens, 20_000)
        .with(Dimension::WallTimeMs, 20_000)
        .with(Dimension::VerificationQueries, 2)
        .with(Dimension::ExternalActions, 1)
        .with(Dimension::AttemptStarts, 4)
        .with(Dimension::OfferCreations, 2)
        .with(Dimension::ObligationCreations, 2)
}

/// What each bid asks for: deliberately below the funded per-award escrow, so the offer pool keeps
/// slack. Without slack an unfunded extra award would be stopped by arithmetic alone, and the
/// funded-award-count check would look load-bearing when it was not.
pub(crate) fn requested_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 8_000)
        .with(Dimension::ModelTokens, 8_000)
        .with(Dimension::WallTimeMs, 8_000)
        .with(Dimension::VerificationQueries, 1)
        .with(Dimension::AttemptStarts, 2)
        .with(Dimension::OfferCreations, 1)
        .with(Dimension::ObligationCreations, 1)
}

/// What an open acceptance asks for: the same capacity as a bid, plus the creation authority a
/// contractor needs in order to delegate part of its escrow onward and still fund an award.
pub(crate) fn delegating_escrow() -> BudgetVector {
    requested_escrow()
        .with(Dimension::OfferCreations, 2)
        .with(Dimension::ObligationCreations, 2)
}

/// What one award of the delegated offer funds. The offer-creation unit inside the pool is what
/// makes a settled reservation spendable again, if a settlement may land in a contract that has
/// already closed.
pub(crate) fn cross_execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 2_000)
        .with(Dimension::ModelTokens, 2_000)
        .with(Dimension::WallTimeMs, 2_000)
        .with(Dimension::AttemptStarts, 1)
        .with(Dimension::OfferCreations, 1)
        .with(Dimension::ObligationCreations, 1)
}

/// What one award of the offer advertised after the settlement funds.
pub(crate) fn second_execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 1_000)
        .with(Dimension::ModelTokens, 1_000)
        .with(Dimension::WallTimeMs, 1_000)
        .with(Dimension::AttemptStarts, 1)
}

/// The counter-offer of the consent held in reserve for a replacement holder. A replacement can
/// only ask for what the contract still holds, so it asks for less than the original did.
pub(crate) fn spare_requested_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 3_000)
        .with(Dimension::ModelTokens, 3_000)
        .with(Dimension::WallTimeMs, 3_000)
}

fn child_execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 2_000)
        .with(Dimension::ModelTokens, 2_000)
        .with(Dimension::WallTimeMs, 2_000)
        .with(Dimension::AttemptStarts, 1)
}

fn child_requested_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 1_000)
        .with(Dimension::ModelTokens, 1_000)
        .with(Dimension::WallTimeMs, 1_000)
        .with(Dimension::AttemptStarts, 1)
}

pub(crate) fn new_ledger() -> CommitmentLedger {
    CommitmentLedger::new(
        ROOT_PARTICIPANT,
        ROOT_PRINCIPAL,
        ROOT_OBLIGATION,
        root_budget(),
    )
    .expect("the root ledger is well formed")
}

fn register(participant_id: &str) -> CommitmentCommand {
    CommitmentCommand::RegisterParticipant(RegisterParticipant {
        participant_id: participant_id.to_owned(),
        principal_id: format!("principal-{participant_id}"),
        sponsor: ROOT_PARTICIPANT.to_owned(),
        endowment: endowment(),
    })
}

pub(crate) fn advertise_main(tokens: &Tokens, policy: OfferPolicy) -> CommitmentCommand {
    CommitmentCommand::Advertise(Advertise {
        offer_id: MAIN_OFFER.to_owned(),
        sponsor: ROOT_PARTICIPANT.to_owned(),
        parent_obligation: ROOT_OBLIGATION.to_owned(),
        funding_source: FundingSource::Participant,
        task_scope: tokens.task_scope.clone(),
        base_digest: tokens.base_digest.clone(),
        intent_digest: tokens.intent_digest.clone(),
        artifact_class: tokens.artifact_class.clone(),
        dependencies: vec![tokens.dependency.clone()],
        capability_scope: vec![tokens.capability.clone()],
        execution_escrow: execution_escrow(),
        policy,
        bid_deadline: DEADLINE,
        offer_deadline: DEADLINE,
        max_awards: MAIN_MAX_AWARDS,
    })
}

pub(crate) fn bid_on_main(tokens: &Tokens, bid_id: &str, bidder: &str) -> CommitmentCommand {
    bid_on_main_for(tokens, bid_id, bidder, requested_escrow())
}

pub(crate) fn bid_on_main_for(
    tokens: &Tokens,
    bid_id: &str,
    bidder: &str,
    requested_escrow: BudgetVector,
) -> CommitmentCommand {
    CommitmentCommand::RecordBid(RecordBid {
        bid_id: bid_id.to_owned(),
        offer_id: MAIN_OFFER.to_owned(),
        bidder: bidder.to_owned(),
        requested_escrow,
        artifact_class: tokens.artifact_class.clone(),
        proposal_digest: Some(tokens.proposal(bidder)),
        expires_at: DEADLINE,
    })
}

pub(crate) fn award_main(bid_id: &str, suffix: &str) -> CommitmentCommand {
    CommitmentCommand::Award(Award {
        contract_id: format!("contract-{suffix}"),
        obligation_id: format!("obligation-{suffix}"),
        lease_id: format!("lease-{suffix}"),
        offer_id: MAIN_OFFER.to_owned(),
        bid_id: bid_id.to_owned(),
        sponsor: ROOT_PARTICIPANT.to_owned(),
        lease_ms: LEASE_MS,
    })
}

pub(crate) fn accept_open(tokens: &Tokens, participant: &str, suffix: &str) -> CommitmentCommand {
    CommitmentCommand::AcceptOpen(AcceptOpen {
        contract_id: format!("contract-open-{suffix}"),
        obligation_id: format!("obligation-open-{suffix}"),
        lease_id: format!("lease-open-{suffix}"),
        bid_id: format!("bid-open-{suffix}"),
        offer_id: OPEN_OFFER.to_owned(),
        participant: participant.to_owned(),
        requested_escrow: requested_escrow(),
        artifact_class: tokens.artifact_class.clone(),
        proposal_digest: Some(tokens.proposal(participant)),
        lease_ms: LEASE_MS,
    })
}

/// An offer a contractor funds from the escrow of one of its contracts and hangs under the
/// obligation of another. Both relations are its own, and each is checked on its own terms: the
/// parent obligation must be active and owned by the sponsor, and the funding contract must be an
/// account the sponsor can still spend from. Keeping them apart is what lets the funding contract
/// close first, since the causal check reads the obligation and not the account.
fn delegated_offer(
    tokens: &Tokens,
    offer_id: &str,
    execution_escrow: BudgetVector,
) -> CommitmentCommand {
    CommitmentCommand::Advertise(Advertise {
        offer_id: offer_id.to_owned(),
        sponsor: ALPHA.to_owned(),
        parent_obligation: SOLO_PARENT_OBLIGATION.to_owned(),
        funding_source: FundingSource::TaskContract {
            contract_id: SOLO_FUNDING_CONTRACT.to_owned(),
        },
        task_scope: tokens.task_scope.clone(),
        base_digest: tokens.base_digest.clone(),
        intent_digest: tokens.intent_digest.clone(),
        artifact_class: tokens.artifact_class.clone(),
        dependencies: vec![tokens.dependency.clone()],
        capability_scope: vec![tokens.capability.clone()],
        execution_escrow,
        policy: OfferPolicy::Negotiated,
        bid_deadline: DEADLINE,
        offer_deadline: DEADLINE,
        max_awards: 1,
    })
}

/// One participant's own causally ordered sequence: it takes two uncontended contracts, funds an
/// offer from the escrow of the first while hanging it under the obligation of the second, closes
/// the funding contract, settles the offer, and then advertises and awards from whatever the
/// settlement returned.
///
/// It is issued in this order because a participant issues its own commands in order. What varies
/// between seeds is how every other participant's contended commands fall around it.
pub(crate) fn settlement_chain(tokens: &Tokens) -> Vec<CommitmentCommand> {
    let solo_bid = |bid_id: &str| {
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: bid_id.to_owned(),
            offer_id: SOLO_OFFER.to_owned(),
            bidder: ALPHA.to_owned(),
            requested_escrow: delegating_escrow(),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: Some(tokens.proposal(bid_id)),
            expires_at: DEADLINE,
        })
    };
    let solo_award = |suffix: &str, bid_id: &str| {
        CommitmentCommand::Award(Award {
            contract_id: format!("contract-solo-{suffix}"),
            obligation_id: format!("obligation-solo-{suffix}"),
            lease_id: format!("lease-solo-{suffix}"),
            offer_id: SOLO_OFFER.to_owned(),
            bid_id: bid_id.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            // Long enough to outlast every clock advance in the pool, so that what the schedule
            // varies is the order of the commands and not whether a lease happened to expire.
            lease_ms: DEADLINE,
        })
    };
    vec![
        CommitmentCommand::Advertise(Advertise {
            offer_id: SOLO_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            parent_obligation: ROOT_OBLIGATION.to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: vec![tokens.dependency.clone()],
            capability_scope: vec![tokens.capability.clone()],
            execution_escrow: delegating_escrow(),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 2,
        }),
        solo_bid("bid-solo-a"),
        solo_bid("bid-solo-b"),
        solo_award("a", "bid-solo-a"),
        solo_award("b", "bid-solo-b"),
        delegated_offer(tokens, CROSS_OFFER, cross_execution_escrow()),
        // The funding contract closes while the offer its escrow paid for is still outstanding.
        // Nothing below its own obligation is open, so the causal check has nothing to say.
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: SOLO_FUNDING_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            outcome: Outcome::Declined,
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: CROSS_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        // The reservation comes back. The question this whole chain exists to ask is where to.
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: CROSS_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        delegated_offer(tokens, SECOND_OFFER, second_execution_escrow()),
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-second".to_owned(),
            offer_id: SECOND_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: second_execution_escrow(),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: Some(tokens.proposal("second")),
            expires_at: DEADLINE,
        }),
        CommitmentCommand::Award(Award {
            contract_id: "contract-second".to_owned(),
            obligation_id: "obligation-second".to_owned(),
            lease_id: "lease-second".to_owned(),
            offer_id: SECOND_OFFER.to_owned(),
            bid_id: "bid-second".to_owned(),
            sponsor: ALPHA.to_owned(),
            lease_ms: DEADLINE,
        }),
        // Winding the branch down in the order the rule requires: everything the escrow funded
        // comes back first, and only then does the account close.
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-second".to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            outcome: Outcome::Exhausted,
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: SECOND_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: SECOND_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: SOLO_FUNDING_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            outcome: Outcome::Declined,
        }),
        // And once it has closed, it is no longer an account anything may be funded from.
        delegated_offer(tokens, THIRD_OFFER, second_execution_escrow()),
    ]
}

/// The deterministic prefix every schedule starts from: three participants, two offers and four
/// live bids. It is executed in order and every command must be accepted.
pub(crate) fn setup(tokens: &Tokens) -> Vec<CommitmentCommand> {
    vec![
        register(ALPHA),
        register(BETA),
        register(GAMMA),
        advertise_main(tokens, OfferPolicy::Negotiated),
        CommitmentCommand::Advertise(Advertise {
            offer_id: OPEN_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            parent_obligation: ROOT_OBLIGATION.to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: vec![tokens.dependency.clone()],
            capability_scope: vec![tokens.capability.clone()],
            execution_escrow: execution_escrow(),
            policy: OfferPolicy::OpenAccept,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        }),
        bid_on_main(tokens, "bid-alpha", ALPHA),
        bid_on_main(tokens, "bid-beta", BETA),
        bid_on_main(tokens, "bid-gamma", GAMMA),
        bid_on_main_for(tokens, "bid-alpha-spare", ALPHA, spare_requested_escrow()),
        // Consent from a third participant, held in reserve so that a reassignment in the pool
        // hands a contract to somebody other than the participant that was executing it. Without
        // it every reassignment would return the contract to the same holder, and the rule that
        // only the holder closes the work would have no ordering in which it could be wrong.
        bid_on_main_for(tokens, "bid-gamma-spare", GAMMA, spare_requested_escrow()),
    ]
}

/// The commands whose order is generated. Several are valid only in some orderings, and several
/// must never be valid in any: a stale fencing token, an award beyond the funded count, a return
/// issued over work that is still outstanding, a reservation larger than the whole run was funded
/// with.
pub(crate) fn contention_pool(tokens: &Tokens) -> Vec<CommitmentCommand> {
    let candidate = digest("candidate-one");
    let stale_candidate = digest("candidate-stale");
    vec![
        // A reservation a hundred times the money the run was ever given. No ordering can make it
        // affordable, so the only thing that can accept it is a kernel that stopped checking
        // whether the capacity is there — and the facts it would then commit move a hundred times
        // more money than exists.
        CommitmentCommand::Advertise(Advertise {
            offer_id: UNFUNDED_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            parent_obligation: ROOT_OBLIGATION.to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: vec![tokens.dependency.clone()],
            capability_scope: vec![tokens.capability.clone()],
            execution_escrow: BudgetVector::ZERO.with(Dimension::MoneyMicros, 1_000_000_000),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        }),
        award_main("bid-alpha", "alpha"),
        award_main("bid-beta", "beta"),
        award_main("bid-gamma", "gamma"),
        accept_open(tokens, ALPHA, "a"),
        accept_open(tokens, BETA, "b"),
        accept_open(tokens, GAMMA, "c"),
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: "attempt-alpha-1".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }),
        CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: "contract-alpha".to_owned(),
            attempt_id: "attempt-alpha-1".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: candidate.clone(),
        }),
        CommitmentCommand::RenewLease(RenewLease {
            contract_id: "contract-alpha".to_owned(),
            holder: ALPHA.to_owned(),
            generation: 1,
            lease_ms: LEASE_MS,
        }),
        CommitmentCommand::AdvanceClock(AdvanceClock { to: 50 }),
        CommitmentCommand::AdvanceClock(AdvanceClock { to: 300 }),
        CommitmentCommand::AdvanceClock(AdvanceClock { to: 600 }),
        // A lease that has run out is replaced by the next fencing generation, with consent the
        // sponsor names. The contract changes hands here: everything issued under the previous
        // generation is stale from here on, and the participant that was executing it is no longer
        // the one who may advance or close it.
        CommitmentCommand::Reassign(Reassign {
            contract_id: "contract-alpha".to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            bid_id: "bid-gamma-spare".to_owned(),
            lease_id: "lease-alpha-2".to_owned(),
            lease_ms: LEASE_MS,
        }),
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: "attempt-alpha-2".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: GAMMA.to_owned(),
            generation: 2,
        }),
        CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: "contract-alpha".to_owned(),
            attempt_id: "attempt-alpha-2".to_owned(),
            participant: GAMMA.to_owned(),
            generation: 2,
            candidate_digest: candidate,
        }),
        // Stale generation, current holder: refused on the fencing token alone.
        CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: "contract-alpha".to_owned(),
            attempt_id: "attempt-alpha-1".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: stale_candidate,
        }),
        CommitmentCommand::RenewLease(RenewLease {
            contract_id: "contract-alpha".to_owned(),
            holder: ALPHA.to_owned(),
            generation: 1,
            lease_ms: LEASE_MS,
        }),
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            outcome: Outcome::DeadEnd,
        }),
        // A contractor delegating from its own escrow: a second generation of causal work.
        CommitmentCommand::Advertise(Advertise {
            offer_id: CHILD_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
            parent_obligation: "obligation-alpha".to_owned(),
            funding_source: FundingSource::TaskContract {
                contract_id: "contract-alpha".to_owned(),
            },
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: vec![tokens.dependency.clone()],
            capability_scope: vec![tokens.capability.clone()],
            execution_escrow: child_execution_escrow(),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        }),
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: "bid-child".to_owned(),
            offer_id: CHILD_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: child_requested_escrow(),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: Some(tokens.proposal("child")),
            expires_at: DEADLINE,
        }),
        CommitmentCommand::Award(Award {
            contract_id: "contract-child".to_owned(),
            obligation_id: "obligation-child".to_owned(),
            lease_id: "lease-child".to_owned(),
            offer_id: CHILD_OFFER.to_owned(),
            bid_id: "bid-child".to_owned(),
            sponsor: ALPHA.to_owned(),
            lease_ms: LEASE_MS,
        }),
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-child".to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            outcome: Outcome::Declined,
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: CHILD_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: CHILD_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        // The current holder closing its own obligation. Whether this is accepted depends only on
        // whether the work below it is closed and its own token is current.
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-alpha".to_owned(),
            participant: GAMMA.to_owned(),
            generation: 2,
            outcome: Outcome::Result {
                candidate_digest: digest("candidate-one"),
            },
        }),
        // The displaced holder closing the same obligation, at the generation that is current
        // after the reassignment. Its token is not stale; what it lacks is the lease.
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 2,
            outcome: Outcome::Result {
                candidate_digest: digest("candidate-one"),
            },
        }),
        // And a participant that never held this contract at all, likewise at the current
        // generation. Nothing but the holder rule stands between it and somebody else's work.
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-alpha".to_owned(),
            participant: BETA.to_owned(),
            generation: 2,
            outcome: Outcome::DeadEnd,
        }),
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: "contract-beta".to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            outcome: Outcome::Exhausted,
        }),
        CommitmentCommand::CancelContract(CancelContract {
            contract_id: "contract-gamma".to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
        CommitmentCommand::WithdrawBid(WithdrawBid {
            bid_id: "bid-gamma".to_owned(),
            bidder: GAMMA.to_owned(),
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: MAIN_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: MAIN_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: OPEN_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: OPEN_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
    ]
}

/// A seeded stream, used only to order commands. It never decides an outcome.
pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    pub(crate) const fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x2545_F491_4F6C_DD1D,
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            usize::try_from(self.next_u64() % bound as u64).unwrap_or(0)
        }
    }
}

pub(crate) fn shuffled(seed: u64, mut commands: Vec<CommitmentCommand>) -> Vec<CommitmentCommand> {
    let mut rng = Rng::new(seed);
    for index in (1..commands.len()).rev() {
        commands.swap(index, rng.below(index + 1));
    }
    commands
}

/// Merge one participant's causal sequence into a shuffled background, keeping the sequence in the
/// order it was issued while everything else falls around it at random.
///
/// A schedule that permuted every command uniformly would practically never reach a state that
/// takes a dozen causally ordered commands to build, so exactly the deepest accounting would go
/// untested. Interleaving keeps the contention that matters and still reaches that state.
pub(crate) fn interleaved(
    seed: u64,
    background: &[CommitmentCommand],
    chain: &[CommitmentCommand],
) -> Vec<CommitmentCommand> {
    let mut rng = Rng::new(seed ^ 0x5DEE_CE66_D1B0_9F3D);
    let mut merged = Vec::with_capacity(background.len() + chain.len());
    let (mut taken_background, mut taken_chain) = (0, 0);
    while taken_background < background.len() || taken_chain < chain.len() {
        let remaining_chain = chain.len() - taken_chain;
        let remaining = background.len() - taken_background + remaining_chain;
        if taken_background == background.len() || rng.below(remaining) < remaining_chain {
            merged.push(chain[taken_chain].clone());
            taken_chain += 1;
        } else {
            merged.push(background[taken_background].clone());
            taken_background += 1;
        }
    }
    merged
}

/// The whole generated schedule for one seed: the contended pool in a seeded order, with the
/// settlement chain merged into it.
pub(crate) fn schedule(seed: u64, tokens: &Tokens) -> Vec<CommitmentCommand> {
    interleaved(
        seed,
        &shuffled(seed, contention_pool(tokens)),
        &settlement_chain(tokens),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Violation {
    /// The facts no longer add up in one dimension to what the run started with.
    Conservation {
        dimension: Dimension,
        expected: i128,
        found: i128,
    },
    /// The accounts the facts describe and the accounts the kernel keeps disagree.
    RegistryDivergence {
        account: String,
        dimension: Dimension,
        facts: i128,
        registry: i128,
    },
    /// A command the kernel decided moves out of an account more than the facts ever put into it.
    UncoveredDebit {
        account: String,
        dimension: Dimension,
    },
    /// A committed fact closed the obligation of a task contract under a participant that did not
    /// hold its lease.
    ClosedByNonHolder {
        contract_id: String,
        participant: String,
        holder: String,
    },
    /// More awards than the offer funded.
    AwardCountExceeded {
        offer_id: String,
        awards_made: u32,
        max_awards: u32,
    },
    /// One recorded consent turned into more than one task contract.
    ConsentSpentTwice { bid_id: String },
    /// One task contract carries more than one child obligation.
    DuplicateObligation { contract_id: String },
    /// A committed fact carries a fencing generation that is not the current one.
    StaleGenerationAdvanced {
        contract_id: String,
        seen: u64,
        current: u64,
    },
    /// An obligation closed while causal work below it was still outstanding.
    WorkClosedTooEarly {
        obligation_id: String,
        outstanding: String,
    },
    /// A committed fact moved capacity into or out of a task-contract account after that contract
    /// had already reached a terminal state.
    SpentAfterClose { contract_id: String },
    /// A task contract reached a terminal state while the facts say it still holds capacity, so
    /// something is still owed to an account nobody can spend from.
    ClosedHoldingEscrow {
        contract_id: String,
        dimension: Dimension,
    },
    /// An offer still holds a reservation that is due back to a task contract which has closed, so
    /// the capacity can never reach any account again.
    StrandedReservation {
        offer_id: String,
        contract_id: String,
    },
    /// A refused command changed the ledger.
    RefusalMutatedState { position: usize },
}

/// What the emitted facts alone say about every account, and about who holds what.
///
/// This projection is built from the event stream and never reads a ledger field after the run has
/// begun. A check that consults the same records the kernel writes agrees with the kernel by
/// construction and can only confirm what the kernel already believes; recomputing the accounts
/// from the facts that were actually committed is what makes a divergence between the two visible,
/// and what allows a command whose facts nothing can pay for to be recognized as such.
///
/// The one field read from the ledger is the opening budget, taken before any command runs. That
/// position is the premise of the run rather than something a command wrote.
///
/// Balances are signed on purpose. Whether a movement was covered is a question the projection
/// answers, so it follows a fact wherever it leads instead of saturating at zero.
#[derive(Debug)]
pub(crate) struct FactAccounts {
    balances: BTreeMap<AccountRef, [i128; DIMENSION_COUNT]>,
    consumed: [i128; DIMENSION_COUNT],
    opening: BudgetVector,
    /// Which task contract each offer's reservation is due back to, as the offer was advertised.
    funded_by: BTreeMap<String, String>,
    closed: BTreeSet<String>,
    /// Who each task contract's lease was last issued to.
    holders: BTreeMap<String, String>,
}

impl FactAccounts {
    /// The accounts as the run begins: one participant holding the whole budget of the run.
    pub(crate) fn opening(root_participant: &str, opening: BudgetVector) -> Self {
        let mut balances = BTreeMap::new();
        let mut root = [0; DIMENSION_COUNT];
        for dimension in DIMENSIONS {
            root[dimension.index()] = i128::from(opening.get(dimension));
        }
        balances.insert(
            AccountRef::Participant {
                participant_id: root_participant.to_owned(),
            },
            root,
        );
        Self {
            balances,
            consumed: [0; DIMENSION_COUNT],
            opening,
            funded_by: BTreeMap::new(),
            closed: BTreeSet::new(),
            holders: BTreeMap::new(),
        }
    }

    /// Take in every fact one command committed and report what they broke. The facts of a command
    /// are one indivisible set, so whether an account was still live and who held a lease are
    /// judged against the state before the command, and what an account holds is judged after all
    /// of them.
    pub(crate) fn observe(&mut self, events: &[CommitmentEvent]) -> Vec<Violation> {
        let mut violations = Vec::new();
        for event in events {
            for (account, _, _) in moved(event) {
                if let AccountRef::TaskContract { contract_id } = account
                    && self.closed.contains(contract_id)
                {
                    violations.push(Violation::SpentAfterClose {
                        contract_id: contract_id.clone(),
                    });
                }
            }
        }
        for event in events {
            match event {
                CommitmentEvent::OfferAdvertised {
                    offer_id,
                    funding_source: FundingSource::TaskContract { contract_id },
                    ..
                } => {
                    self.funded_by.insert(offer_id.clone(), contract_id.clone());
                }
                CommitmentEvent::LeaseIssued {
                    contract_id,
                    holder,
                    ..
                }
                | CommitmentEvent::ContractReassigned {
                    contract_id,
                    holder,
                    ..
                } => {
                    self.holders.insert(contract_id.clone(), holder.clone());
                }
                CommitmentEvent::ObligationReturned {
                    contract_id,
                    participant,
                    ..
                } => {
                    if let Some(holder) = self.holders.get(contract_id)
                        && holder != participant
                    {
                        violations.push(Violation::ClosedByNonHolder {
                            contract_id: contract_id.clone(),
                            participant: participant.clone(),
                            holder: holder.clone(),
                        });
                    }
                }
                _ => {}
            }
            for (account, sign, amount) in moved(event) {
                let balance = self
                    .balances
                    .entry(account.clone())
                    .or_insert([0; DIMENSION_COUNT]);
                for dimension in DIMENSIONS {
                    balance[dimension.index()] += sign * i128::from(amount.get(dimension));
                }
            }
            if let CommitmentEvent::BudgetConsumed { amount, .. } = event {
                for dimension in DIMENSIONS {
                    self.consumed[dimension.index()] += i128::from(amount.get(dimension));
                }
            }
        }
        for event in events {
            let contract_id = match event {
                CommitmentEvent::ObligationReturned { contract_id, .. }
                | CommitmentEvent::ContractCancelled { contract_id, .. } => contract_id,
                _ => continue,
            };
            self.closed.insert(contract_id.clone());
            let Some(balance) = self.balances.get(&AccountRef::TaskContract {
                contract_id: contract_id.clone(),
            }) else {
                continue;
            };
            for dimension in DIMENSIONS {
                if balance[dimension.index()] != 0 {
                    violations.push(Violation::ClosedHoldingEscrow {
                        contract_id: contract_id.clone(),
                        dimension,
                    });
                }
            }
        }
        violations
    }

    /// What the run started with is what the accounts and the spent capacity still add up to. This
    /// is computed over the facts and the opening position, so no field the kernel maintains takes
    /// part in it.
    pub(crate) fn conservation(&self) -> Vec<Violation> {
        let mut violations = Vec::new();
        for dimension in DIMENSIONS {
            let index = dimension.index();
            let found = self.consumed[index]
                + self
                    .balances
                    .values()
                    .map(|balance| balance[index])
                    .sum::<i128>();
            let expected = i128::from(self.opening.get(dimension));
            if found != expected {
                violations.push(Violation::Conservation {
                    dimension,
                    expected,
                    found,
                });
            }
        }
        violations
    }

    /// Where the accounts the facts describe and the accounts the kernel keeps have parted company.
    /// Two independent additions of the same movements must agree in every dimension of every
    /// account, including the capacity that has left the accounts for good.
    pub(crate) fn divergence(&self, ledger: &CommitmentLedger) -> Vec<Violation> {
        let mut registry: BTreeMap<AccountRef, BudgetVector> = BTreeMap::new();
        for participant in ledger.participants().values() {
            registry.insert(
                AccountRef::Participant {
                    participant_id: participant.participant_id.clone(),
                },
                participant.balance,
            );
        }
        for offer in ledger.offers().values() {
            registry.insert(
                AccountRef::Offer {
                    offer_id: offer.offer_id.clone(),
                },
                offer.escrow,
            );
        }
        for contract in ledger.contracts().values() {
            registry.insert(
                AccountRef::TaskContract {
                    contract_id: contract.contract_id.clone(),
                },
                contract.escrow,
            );
        }
        let mut violations = Vec::new();
        let accounts: BTreeSet<&AccountRef> = self.balances.keys().chain(registry.keys()).collect();
        for account in accounts {
            let facts = self.balances.get(account).copied().unwrap_or_default();
            let held = registry.get(account).copied().unwrap_or_default();
            for dimension in DIMENSIONS {
                let recorded = i128::from(held.get(dimension));
                if facts[dimension.index()] != recorded {
                    violations.push(Violation::RegistryDivergence {
                        account: label(account),
                        dimension,
                        facts: facts[dimension.index()],
                        registry: recorded,
                    });
                }
            }
        }
        for dimension in DIMENSIONS {
            let recorded = i128::from(ledger.consumed().get(dimension));
            if self.consumed[dimension.index()] != recorded {
                violations.push(Violation::RegistryDivergence {
                    account: "consumed".to_owned(),
                    dimension,
                    facts: self.consumed[dimension.index()],
                    registry: recorded,
                });
            }
        }
        violations
    }

    /// What the facts of a command the kernel decided but could not commit would have taken out of
    /// an account that never held it. The facts are followed into a copy of the accounts, so what
    /// is reported is the shortfall the facts themselves describe.
    pub(crate) fn uncovered(&self, events: &[CommitmentEvent]) -> Vec<Violation> {
        let mut projected = self.balances.clone();
        let mut violations = Vec::new();
        for event in events {
            for (account, sign, amount) in moved(event) {
                let balance = projected
                    .entry(account.clone())
                    .or_insert([0; DIMENSION_COUNT]);
                for dimension in DIMENSIONS {
                    let index = dimension.index();
                    balance[index] += sign * i128::from(amount.get(dimension));
                    if balance[index] < 0 {
                        violations.push(Violation::UncoveredDebit {
                            account: label(account),
                            dimension,
                        });
                    }
                }
            }
        }
        violations.dedup();
        violations
    }

    /// Capacity that no account can reach any more: an offer still holding a reservation whose
    /// destination has closed. Refusing to settle into a closed contract without also keeping that
    /// contract open would trade one accounting break for this one, so the schedules look for both.
    pub(crate) fn stranded(&self) -> Vec<Violation> {
        let mut violations = Vec::new();
        for (offer_id, contract_id) in &self.funded_by {
            let Some(balance) = self.balances.get(&AccountRef::Offer {
                offer_id: offer_id.clone(),
            }) else {
                continue;
            };
            if self.closed.contains(contract_id) && balance.iter().any(|units| *units != 0) {
                violations.push(Violation::StrandedReservation {
                    offer_id: offer_id.clone(),
                    contract_id: contract_id.clone(),
                });
            }
        }
        violations
    }
}

fn label(account: &AccountRef) -> String {
    match account {
        AccountRef::Participant { participant_id } => format!("participant {participant_id}"),
        AccountRef::Offer { offer_id } => format!("offer {offer_id}"),
        AccountRef::TaskContract { contract_id } => format!("task contract {contract_id}"),
    }
}

/// The accounts one fact moves capacity through, each with the direction and the quantity it
/// moves. Participant balances are left to the totals the schedule already checks.
fn moved(event: &CommitmentEvent) -> Vec<(&AccountRef, i128, &BudgetVector)> {
    match event {
        CommitmentEvent::BudgetTransferred { from, to, amount } => {
            vec![(from, -1, amount), (to, 1, amount)]
        }
        CommitmentEvent::BudgetConsumed { account, amount } => vec![(account, -1, amount)],
        _ => Vec::new(),
    }
}

#[derive(Debug)]
pub(crate) struct ScheduleReport {
    pub violations: Vec<Violation>,
    pub committed: usize,
    pub refused: usize,
    /// Which facts this schedule actually committed. A pool that quietly stopped reaching
    /// contention would otherwise pass every invariant by doing nothing.
    pub reached: BTreeSet<&'static str>,
    /// Which guarded refusals this schedule actually provoked. A guard that no ordering ever
    /// reaches holds by never being asked, which is not the same as holding.
    pub guarded: BTreeSet<&'static str>,
}

fn fact_name(event: &CommitmentEvent) -> &'static str {
    match event {
        CommitmentEvent::ClockAdvanced { .. } => "clock_advanced",
        CommitmentEvent::ParticipantRegistered { .. } => "participant_registered",
        CommitmentEvent::BudgetTransferred { .. } => "budget_transferred",
        CommitmentEvent::BudgetConsumed { .. } => "budget_consumed",
        CommitmentEvent::OfferAdvertised { .. } => "offer_advertised",
        CommitmentEvent::OfferWithdrawn { .. } => "offer_withdrawn",
        CommitmentEvent::OfferSettled { .. } => "offer_settled",
        CommitmentEvent::BidRecorded { .. } => "bid_recorded",
        CommitmentEvent::BidWithdrawn { .. } => "bid_withdrawn",
        CommitmentEvent::TaskContractFormed { .. } => "task_contract_formed",
        CommitmentEvent::ObligationCreated { .. } => "obligation_created",
        CommitmentEvent::LeaseIssued { .. } => "lease_issued",
        CommitmentEvent::LeaseRenewed { .. } => "lease_renewed",
        CommitmentEvent::ContractReassigned { .. } => "contract_reassigned",
        CommitmentEvent::AttemptStarted { .. } => "attempt_started",
        CommitmentEvent::SubmissionRecorded { .. } => "submission_recorded",
        CommitmentEvent::ObligationReturned { .. } => "obligation_returned",
        CommitmentEvent::ContractCancelled { .. } => "contract_cancelled",
    }
}

/// Replay one generated schedule and report every invariant it broke.
pub(crate) fn run_schedule(seed: u64, tokens: &Tokens, disabled: DisabledChecks) -> ScheduleReport {
    let mut ledger = new_ledger();
    ledger.disable_checks(disabled);
    let mut report = ScheduleReport {
        violations: Vec::new(),
        committed: 0,
        refused: 0,
        reached: BTreeSet::new(),
        guarded: BTreeSet::new(),
    };
    let mut accounts = FactAccounts::opening(ROOT_PARTICIPANT, *ledger.initial_total());
    for command in setup(tokens) {
        let events = ledger
            .execute(&command)
            .unwrap_or_else(|error| panic!("the deterministic prefix must be accepted: {error}"));
        report.violations.extend(accounts.observe(&events));
    }
    for (position, command) in schedule(seed, tokens).into_iter().enumerate() {
        let before = ledger.clone();
        match ledger.execute(&command) {
            Ok(events) => {
                report.committed += 1;
                report.reached.extend(events.iter().map(fact_name));
                report
                    .violations
                    .extend(fencing_violations(&ledger, &events));
                report.violations.extend(accounts.observe(&events));
            }
            Err(error) => {
                report.refused += 1;
                match error {
                    CommitmentError::AccountClosed { .. } => {
                        report.guarded.insert("account_closed")
                    }
                    CommitmentError::ReservationOutstanding { .. } => {
                        report.guarded.insert("reservation_outstanding")
                    }
                    CommitmentError::InsufficientBudget { .. } => {
                        report.guarded.insert("insufficient_budget")
                    }
                    // On the return path this refusal has one meaning: a participant that does not
                    // hold the lease tried to close the work.
                    CommitmentError::NotAuthorized { .. }
                        if matches!(command, CommitmentCommand::ReturnObligation(_)) =>
                    {
                        report.guarded.insert("return_non_holder")
                    }
                    _ => false,
                };
                // A command the kernel decided and then could not pay for is an accounting break
                // whichever way it ends: the facts it produced move capacity no account holds.
                // Deciding changes nothing, so the same facts can be re-derived and followed into a
                // copy of the projection to say exactly what they could not pay for.
                if let CommitmentError::UncoveredDebit { account, dimension } = error {
                    let mut broken = ledger
                        .decide(&command)
                        .map(|events| accounts.uncovered(&events))
                        .unwrap_or_default();
                    if broken.is_empty() {
                        broken.push(Violation::UncoveredDebit { account, dimension });
                    }
                    report.violations.extend(broken);
                }
                if ledger != before {
                    report
                        .violations
                        .push(Violation::RefusalMutatedState { position });
                }
            }
        }
        report.violations.extend(state_violations(&ledger));
        report.violations.extend(accounts.conservation());
        report.violations.extend(accounts.divergence(&ledger));
    }
    // Whether a reservation is stranded is a question about the end of the schedule: until then it
    // is only unsettled, which is ordinary.
    report.violations.extend(accounts.stranded());
    report.violations.dedup();
    report
}

/// Every fact that names a fencing generation must name the one the task contract now holds.
fn fencing_violations(ledger: &CommitmentLedger, events: &[CommitmentEvent]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for event in events {
        let (contract_id, seen) = match event {
            CommitmentEvent::SubmissionRecorded {
                contract_id,
                generation,
                ..
            }
            | CommitmentEvent::LeaseRenewed {
                contract_id,
                generation,
                ..
            }
            | CommitmentEvent::ObligationReturned {
                contract_id,
                generation,
                ..
            }
            | CommitmentEvent::AttemptStarted {
                contract_id,
                generation,
                ..
            } => (contract_id, *generation),
            _ => continue,
        };
        if let Some(contract) = ledger.contracts().get(contract_id)
            && contract.lease.generation != seen
        {
            violations.push(Violation::StaleGenerationAdvanced {
                contract_id: contract_id.clone(),
                seen,
                current: contract.lease.generation,
            });
        }
    }
    violations
}

/// The structural invariants that must hold of the ledger itself after every command: how many
/// awards an offer made, how many contracts one consent formed, and whether causal work was closed
/// in order.
///
/// Conservation is deliberately not among them. Adding the same fields the kernel writes and
/// comparing the sum with the opening total agrees with the kernel by construction, and a check
/// that agrees by construction cannot report the divergence it exists to catch. That question is
/// answered by [`FactAccounts`], from the facts.
pub(crate) fn state_violations(ledger: &CommitmentLedger) -> Vec<Violation> {
    let mut violations = Vec::new();
    for offer in ledger.offers().values() {
        if offer.awards_made > offer.max_awards {
            violations.push(Violation::AwardCountExceeded {
                offer_id: offer.offer_id.clone(),
                awards_made: offer.awards_made,
                max_awards: offer.max_awards,
            });
        }
        let contracts = ledger
            .contracts()
            .values()
            .filter(|contract| contract.offer_id == offer.offer_id)
            .count();
        if u32::try_from(contracts).unwrap_or(u32::MAX) > offer.max_awards {
            violations.push(Violation::AwardCountExceeded {
                offer_id: offer.offer_id.clone(),
                awards_made: u32::try_from(contracts).unwrap_or(u32::MAX),
                max_awards: offer.max_awards,
            });
        }
    }

    let mut consent: BTreeMap<&str, usize> = BTreeMap::new();
    let mut obligations: BTreeMap<&str, usize> = BTreeMap::new();
    for contract in ledger.contracts().values() {
        *consent.entry(contract.bid_id.as_str()).or_default() += 1;
    }
    for obligation in ledger.obligations().values() {
        if let Some(contract_id) = &obligation.contract_id {
            *obligations.entry(contract_id.as_str()).or_default() += 1;
        }
    }
    for (bid_id, count) in consent {
        if count > 1 {
            violations.push(Violation::ConsentSpentTwice {
                bid_id: bid_id.to_owned(),
            });
        }
    }
    for (contract_id, count) in obligations {
        if count > 1 {
            violations.push(Violation::DuplicateObligation {
                contract_id: contract_id.to_owned(),
            });
        }
    }

    for obligation in ledger.obligations().values() {
        if obligation.state != ObligationState::Terminal {
            continue;
        }
        for child in &obligation.children {
            if ledger
                .obligations()
                .get(child)
                .is_none_or(|child| child.state != ObligationState::Terminal)
            {
                violations.push(Violation::WorkClosedTooEarly {
                    obligation_id: obligation.obligation_id.clone(),
                    outstanding: child.clone(),
                });
            }
        }
        for offer in ledger.offers().values() {
            if offer.parent_obligation == obligation.obligation_id
                && offer.state != OfferState::Settled
            {
                violations.push(Violation::WorkClosedTooEarly {
                    obligation_id: obligation.obligation_id.clone(),
                    outstanding: offer.offer_id.clone(),
                });
            }
        }
    }
    violations
}
