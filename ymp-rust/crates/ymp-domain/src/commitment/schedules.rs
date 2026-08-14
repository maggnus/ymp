//! Generated concurrent command schedules and the invariants every one of them must preserve.
//!
//! One ordering proves nothing about contention. The generator builds a fixed cast of
//! participants, offers and consent, then interleaves a pool of commands issued by different
//! principals — including commands that are only valid in some orderings, and commands that must
//! never be valid at all — and replays the whole pool in a seeded order. Refusals are expected and
//! are not failures; what the schedule asserts is that after every single command the accounts the
//! facts describe still agree with the accounts the kernel keeps, that no more slots were awarded
//! than were funded, that one consent formed one contract and one contract one obligation, that
//! nothing advanced under a stale fencing token, that no obligation closed while its causal work
//! was outstanding, and that nobody closed work it did not hold.
//!
//! One further question is asked of every accepted command, and it is the only one here whose
//! answer does not come out of the run itself: whether the facts a command committed moved the
//! amounts that command named, out of and into the accounts it named. Everything else compares
//! facts with facts, or with records built by applying those facts, and a fact corrupted where it
//! is generated satisfies all of them at once — whether what was corrupted is how much moved or
//! whose account it came out of.
//!
//! Three things the pool alone cannot do are added here. One participant's own commands are merged
//! in as a causal sequence rather than permuted, because a uniform shuffle practically never
//! reaches a state that takes a dozen ordered commands to build. The accounts are recomputed from
//! the facts the ledger emitted, so that what is checked is not the same records the kernel writes.
//! And the pool asks for capacity that does not exist and closes work under participants that do
//! not hold it, so that the checks refusing those things are reached rather than merely present.

use std::collections::{BTreeMap, BTreeSet};

use super::budget::{BudgetVector, DIMENSION_COUNT, DIMENSIONS, Dimension};
use super::invocations::{
    InvocationClosure, InvocationRecord, InvocationState, OpenAuthority, RootTerminal, StopReason,
    Verdict, WakeCondition,
};
use super::ledger::{AlteredFacts, CommitmentLedger, DisabledChecks};
use super::protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CancelContract, CloseInvocation, CommitmentCommand,
    CommitmentError, CommitmentEvent, MAX_ATTEMPT_WAKES, Reassign, RecordBid, RecordVerification,
    RegisterParticipant, RenewLease, ResumeInvocation, ReturnObligation, SettleOffer, StartAttempt,
    StartInvocation, StopRun, SubmitResult, WithdrawBid, WithdrawOffer, YieldInvocation,
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
/// The slice whose whole life the pool replays.
pub(crate) const ALPHA_INVOCATION: &str = "invocation-alpha-1";
/// An offer identifier no command in this pool ever advertises, so nothing can close it.
pub(crate) const UNREACHED_OFFER: &str = "offer-unreached";
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
        .with(Dimension::InvocationStarts, 1_000)
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
        .with(Dimension::InvocationStarts, 40)
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
        .with(Dimension::InvocationStarts, 24)
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
        // Deliberately more process slices than the wake bound admits, so that what stops a
        // participant from being woken forever is that bound and not the escrow running dry — and
        // still under a third of what the offer pool holds, so that an award beyond the funded
        // count is refused by that count rather than by arithmetic.
        .with(Dimension::InvocationStarts, 12)
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
        CommitmentCommand::RenewLease(RenewLease {
            contract_id: "contract-alpha".to_owned(),
            holder: ALPHA.to_owned(),
            generation: 1,
            lease_ms: LEASE_MS,
        }),
        // Neither the participant running the slice nor the sponsor of its contract.
        CommitmentCommand::CloseInvocation(CloseInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            closer: GAMMA.to_owned(),
            reason: InvocationClosure::ParticipantLost,
        }),
        // A participant that does not own the root obligation cannot stop the run.
        CommitmentCommand::StopRun(StopRun {
            authority: BETA.to_owned(),
            reason: StopReason::Cancelled,
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
        // A slice of the participant the contract was taken from, offered again after the
        // reassignment. Its token is the one it started under and is no longer current.
        CommitmentCommand::ResumeInvocation(ResumeInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }),
        // A slice under the replacement holder's own attempt and token.
        CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: "invocation-alpha-2".to_owned(),
            attempt_id: "attempt-alpha-2".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: GAMMA.to_owned(),
            generation: 2,
            cursor: 0,
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

/// One participant's own causally ordered sequence on the task contract it was awarded: it takes
/// an attempt, begins a process slice, stops it without returning the contract, submits, is
/// admitted again on the fact that submission committed, closes the slice and spends its protected
/// query.
///
/// It is issued in this order because a participant issues its own commands in order, and because a
/// uniform shuffle practically never reaches a resumption: admitting one takes a slice that has
/// begun, a yield, a committed fact the yield asked about and a clock still inside the wake
/// deadline, which is four things in one order. What the seed varies is where every other
/// participant's contended commands, and every clock advance, fall around it.
pub(crate) fn slice_chain() -> Vec<CommitmentCommand> {
    let candidate = digest("candidate-one");
    vec![
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: "attempt-alpha-1".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }),
        // One participant's process slice, taken through its whole life: it begins under the lease
        // it holds, stops without returning the contract, is admitted again once something it
        // named has been committed, and is finally closed.
        CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            attempt_id: "attempt-alpha-1".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor: 0,
        }),
        CommitmentCommand::YieldInvocation(YieldInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor: 0,
            conditions: vec![
                WakeCondition::SubmissionRecorded {
                    contract_id: "contract-alpha".to_owned(),
                },
                WakeCondition::ObligationReturned {
                    obligation_id: "obligation-child".to_owned(),
                },
            ],
            wake_deadline: LEASE_MS,
        }),
        CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: "contract-alpha".to_owned(),
            attempt_id: "attempt-alpha-1".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: candidate.clone(),
        }),
        // A protected query against the exact candidate the contract recorded, and one against a
        // digest it never recorded.
        CommitmentCommand::RecordVerification(RecordVerification {
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: digest("candidate-one"),
            verdict: Verdict::Failed,
        }),
        CommitmentCommand::RecordVerification(RecordVerification {
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: digest("candidate-elsewhere"),
            verdict: Verdict::Passed,
        }),
        CommitmentCommand::ResumeInvocation(ResumeInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }),
        // A second yield, this time on a fact no ordering of this pool ever commits. Nothing but
        // the rule that a resumption needs a committed fact stands between the participant and
        // being woken for nothing.
        CommitmentCommand::YieldInvocation(YieldInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor: 0,
            conditions: vec![WakeCondition::OfferClosed {
                offer_id: UNREACHED_OFFER.to_owned(),
            }],
            wake_deadline: LEASE_MS,
        }),
        CommitmentCommand::ResumeInvocation(ResumeInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }),
        CommitmentCommand::CloseInvocation(CloseInvocation {
            invocation_id: ALPHA_INVOCATION.to_owned(),
            closer: ALPHA.to_owned(),
            reason: InvocationClosure::Completed,
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
    let background = interleaved(
        seed,
        &shuffled(seed, contention_pool(tokens)),
        &settlement_chain(tokens),
    );
    interleaved(seed ^ 0x51ED_2701_FA13_C3A9, &background, &slice_chain())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Violation {
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
    /// A committed fact moved capacity through an account the accepted command did not name, or
    /// moved through a named account an amount other than the one it named.
    FactContradictsCommand {
        command: &'static str,
        subject: String,
        dimension: Dimension,
        commanded: i128,
        emitted: i128,
    },
    /// A refused command changed the ledger.
    RefusalMutatedState { position: usize },
    /// A process slice was begun or resumed without one unit of creation authority being spent for
    /// it, so the run can run processes it was never funded to run.
    UnfundedInvocationStart { invocation_id: String },
    /// A slice was resumed although no committed fact after its cursor matched anything it
    /// registered. Nothing happened, and the participant was woken for it.
    ResumedWithoutEvent {
        invocation_id: String,
        matched_sequence: u64,
    },
    /// The slices of one attempt were resumed more times than the bound allows.
    WakeCountExceeded { attempt_id: String, wakes: u32 },
    /// Two process slices of one attempt were running at the same time, so the run was executing
    /// twice what it counts, funds and reads facts for once.
    TwoRunningSlices {
        attempt_id: String,
        running: Vec<String>,
    },
    /// The run reported a terminal state although a command it would still accept could advance it.
    ///
    /// What is compared is not the ledger's own reason for being open — that is the same function
    /// the terminal is defined by, and comparing it with itself proves nothing. What is compared is
    /// the transition: the resumption a yielded slice is for, and the beginning of a slice on an
    /// existing attempt, are offered to the kernel, and a run that reports itself finished while
    /// accepting either has shed a control object it still honours.
    ///
    /// No ordering in this suite provokes it, and the reason is structural rather than accidental:
    /// every transition that could still advance work needs an active task contract, an active
    /// contract carries an obligation that is not terminal, and an obligation that is not terminal is
    /// itself a control object, so no terminal state can be reported while one exists. What the check
    /// guards is a change to that structure — a rule that sheds control objects, of which the
    /// stopped-run rule is the first, shedding one that is still honoured.
    TerminalWhileOpen {
        terminal: RootTerminal,
        outstanding: String,
    },
    /// The run reported acceptance without a passing protected query against a candidate whose root
    /// scope holds up. Quiescence, a spent budget and a contractor's own result are not acceptance.
    ///
    /// Root scope is recomputed here from the obligation tree the facts describe, rather than read
    /// off the flag the kernel wrote into the verification fact: the terminal state is derived from
    /// that flag, so trusting it would make this check agree with the kernel by construction.
    ///
    /// What separates the two sides is a verification fact whose stored scope disagrees with where
    /// its contract actually hangs. Producing one needs a task contract below root scope that a
    /// passing query is recorded against, and neither the lifecycle cast nor the reachability
    /// alphabet holds such a contract — both open exactly one contract, directly under the root
    /// obligation, so in those two spaces this check is evaluated everywhere and provoked nowhere.
    /// The composition in [`nested_scope_run`] holds one: a contractor delegates part of its work
    /// and the one protected query the run spends passes against the delegated contract. Read
    /// honestly, that run is not accepted; with the scope the fact states overstated, the kernel
    /// reports acceptance and this check is what says the evidence is not there.
    AcceptedWithoutVerification,
    /// The run reached a terminal state its scenario does not describe.
    DishonestTerminal {
        expected: RootTerminal,
        actual: Option<RootTerminal>,
    },
    /// A funded control object still holds the run open after everything that could wind it down
    /// has been offered.
    NeverQuiescent { outstanding: String },
    /// The run is waiting on a wake nobody can use: its deadline has passed, its lease has run out,
    /// its contract has changed hands or closed, or its escrow no longer pays for the resumption.
    DeadWakeHoldsRunOpen {
        invocation_id: String,
        why: &'static str,
    },
}

/// Which side of an account one fact moves capacity through.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Flow {
    /// Capacity arriving in the account.
    In,
    /// Capacity leaving it for another account.
    Out,
    /// Capacity leaving the accounts for good.
    Spent,
}

/// What a command names about one movement.
#[derive(Clone, Copy, Debug)]
enum Commanded {
    /// A quantity the command states outright, which the facts must move exactly.
    Amount(BudgetVector),
    /// An account the command names without stating a quantity. What a settlement returns is
    /// whatever is left of an account rather than a number any command states, so the account it
    /// reaches is compared and the amount is left to the projection.
    AccountOnly,
}

/// The movements one command says its facts must make, keyed by the account and the direction.
/// A movement absent from here is one the command never named, and the facts must not make it.
#[derive(Debug, Default)]
struct Movements {
    named: BTreeMap<(AccountRef, Flow), Commanded>,
}

impl Movements {
    fn amount(&mut self, account: AccountRef, flow: Flow, commanded: BudgetVector) {
        let named = match self.named.get(&(account.clone(), flow)) {
            Some(Commanded::Amount(already)) => already
                .checked_add(&commanded)
                .expect("the amounts one command names do not overflow"),
            _ => commanded,
        };
        self.named.insert((account, flow), Commanded::Amount(named));
    }

    fn account_only(&mut self, account: AccountRef, flow: Flow) {
        self.named.insert((account, flow), Commanded::AccountOnly);
    }
}

/// What the accepted command itself says its facts must move, and through which accounts.
///
/// Every other check in this file compares facts with other facts, or with records the kernel built
/// by applying those same facts. A value corrupted where the fact is generated satisfies all of
/// them at once: the registry is built from the corrupted fact and therefore agrees with it in
/// every account. The expectation here is taken from somewhere the kernel never writes — the
/// command the schedule issued — so the two sides of the comparison are produced by different code,
/// and a fact that no longer states what its command named moves only one of them.
///
/// Both ends of every movement are compared, not only the quantity. A transfer that carries the
/// commanded amount into the commanded account while taking it out of somebody else's account
/// leaves every account the facts describe agreeing with every account the kernel keeps, and the
/// run stays internally consistent under it. What it contradicts is the command, which named the
/// account the money was to come out of.
///
/// The accounts a command does not name outright are the ones an offer's reservation was taken from
/// and returns to. They are remembered from the command that advertised the offer and the command
/// that awarded the contract, so both sides of the comparison still come from the schedule rather
/// than from the kernel. A movement whose account cannot be resolved that way — consent or an offer
/// from before the run — leaves the command uncompared rather than guessed at.
#[derive(Debug, Default)]
pub(crate) struct CommandedAmounts {
    /// What each recorded consent asked for, as the command that recorded it named it.
    requested: BTreeMap<String, BudgetVector>,
    /// The account each offer's reservation came out of, as the command that advertised it named
    /// it. It is also the account that reservation returns to.
    funding: BTreeMap<String, AccountRef>,
    /// Which offer each task contract was formed from, as the command that formed it named it.
    formed_from: BTreeMap<String, String>,
    /// Which task contract each process slice belongs to, as the command that started it named it.
    invocation_contract: BTreeMap<String, String>,
    /// Which commands this run actually compared its facts against. A command whose accounts could
    /// never be resolved would be passed over in silence, and the comparison would then hold of it
    /// by never being applied, which is not the same as holding.
    compared: BTreeSet<&'static str>,
}

impl CommandedAmounts {
    /// Compare the facts one accepted command committed with the movements that command named.
    pub(crate) fn observe(
        &mut self,
        command: &CommitmentCommand,
        events: &[CommitmentEvent],
    ) -> Vec<Violation> {
        let name = command_name(command);
        let mut violations = match self.movements(command) {
            Some(movements) => {
                self.compared.insert(name);
                compare_movements(name, events, &movements)
            }
            None => Vec::new(),
        };
        violations.extend(consent_violations(command, events));
        self.remember(command);
        violations
    }

    pub(crate) const fn compared(&self) -> &BTreeSet<&'static str> {
        &self.compared
    }

    /// Every movement the command names, or `None` when an account it needs was never named by any
    /// command in this run.
    fn movements(&self, command: &CommitmentCommand) -> Option<Movements> {
        let mut movements = Movements::default();
        match command {
            CommitmentCommand::RegisterParticipant(command) => {
                let sponsor = participant_account(&command.sponsor);
                movements.amount(
                    sponsor.clone(),
                    Flow::Spent,
                    BudgetVector::unit(Dimension::ParticipantStarts),
                );
                movements.amount(sponsor, Flow::Out, command.endowment);
                movements.amount(
                    participant_account(&command.participant_id),
                    Flow::In,
                    command.endowment,
                );
            }
            CommitmentCommand::Advertise(command) => {
                // What an offer reserves is what one award funds, once per funded slot, and it
                // comes out of the account the command named as the funding source.
                let pool = command
                    .execution_escrow
                    .checked_scale(u64::from(command.max_awards))
                    .ok()?;
                let funding = funding_account(&command.funding_source, &command.sponsor);
                movements.amount(
                    funding.clone(),
                    Flow::Spent,
                    BudgetVector::unit(Dimension::OfferCreations),
                );
                movements.amount(funding, Flow::Out, pool);
                movements.amount(
                    AccountRef::Offer {
                        offer_id: command.offer_id.clone(),
                    },
                    Flow::In,
                    pool,
                );
            }
            CommitmentCommand::Award(command) => self.formation(
                &mut movements,
                &command.offer_id,
                &command.contract_id,
                *self.requested.get(&command.bid_id)?,
                command.lease_ms,
            )?,
            CommitmentCommand::AcceptOpen(command) => self.formation(
                &mut movements,
                &command.offer_id,
                &command.contract_id,
                command.requested_escrow,
                command.lease_ms,
            )?,
            // An attempt is bought with one unit of start authority, out of the escrow of the task
            // contract the command named.
            CommitmentCommand::StartAttempt(command) => movements.amount(
                contract_account(&command.contract_id),
                Flow::Spent,
                BudgetVector::unit(Dimension::AttemptStarts),
            ),
            CommitmentCommand::RenewLease(command) => movements.amount(
                contract_account(&command.contract_id),
                Flow::Spent,
                BudgetVector::units(Dimension::WallTimeMs, command.lease_ms),
            ),
            CommitmentCommand::Reassign(command) => movements.amount(
                contract_account(&command.contract_id),
                Flow::Spent,
                BudgetVector::units(Dimension::WallTimeMs, command.lease_ms),
            ),
            // A settlement returns what an offer still holds to the account that funded it. The
            // quantity is a remainder rather than anything the command states, so only the two
            // accounts are compared.
            CommitmentCommand::SettleOffer(command) => {
                movements.account_only(
                    AccountRef::Offer {
                        offer_id: command.offer_id.clone(),
                    },
                    Flow::Out,
                );
                movements.account_only(self.funding.get(&command.offer_id)?.clone(), Flow::In);
            }
            // Closing a task contract returns whatever its escrow still holds the same way, so the
            // same two accounts are compared and the same remainder is left to the projection.
            CommitmentCommand::ReturnObligation(command) => {
                self.closure(&mut movements, &command.contract_id)?;
            }
            CommitmentCommand::CancelContract(command) => {
                self.closure(&mut movements, &command.contract_id)?;
            }
            // Beginning a process slice is bought with one unit of creation authority out of the
            // escrow of the task contract the command named.
            CommitmentCommand::StartInvocation(command) => movements.amount(
                contract_account(&command.contract_id),
                Flow::Spent,
                BudgetVector::unit(Dimension::InvocationStarts),
            ),
            // Resuming one costs the same. The command names only the slice, so the account comes
            // from the command that started it — still the schedule's own record and never the
            // ledger's.
            CommitmentCommand::ResumeInvocation(command) => movements.amount(
                contract_account(self.invocation_contract.get(&command.invocation_id)?),
                Flow::Spent,
                BudgetVector::unit(Dimension::InvocationStarts),
            ),
            CommitmentCommand::RecordVerification(command) => movements.amount(
                contract_account(&command.contract_id),
                Flow::Spent,
                BudgetVector::unit(Dimension::VerificationQueries),
            ),
            // Consent, its withdrawal, an offer's withdrawal, a submission, a yield, the closing of
            // a slice, stopping the run and the clock move no capacity at all, which is itself
            // compared: an empty expectation makes any movement these commit a contradiction.
            CommitmentCommand::RecordBid(_)
            | CommitmentCommand::WithdrawBid(_)
            | CommitmentCommand::WithdrawOffer(_)
            | CommitmentCommand::SubmitResult(_)
            | CommitmentCommand::AdvanceClock(_)
            | CommitmentCommand::YieldInvocation(_)
            | CommitmentCommand::CloseInvocation(_)
            | CommitmentCommand::StopRun(_) => {}
        }
        Some(movements)
    }

    /// What forming a task contract must move: out of the offer the command named and into the
    /// contract it named exactly the capacity the consent it named asked for, one unit of creation
    /// authority out of the account funding that offer, and out of the new contract exactly the
    /// wall time the command bought its first lease with.
    fn formation(
        &self,
        movements: &mut Movements,
        offer_id: &str,
        contract_id: &str,
        requested: BudgetVector,
        lease_ms: u64,
    ) -> Option<()> {
        let contract = contract_account(contract_id);
        movements.amount(
            self.funding.get(offer_id)?.clone(),
            Flow::Spent,
            BudgetVector::unit(Dimension::ObligationCreations),
        );
        movements.amount(
            AccountRef::Offer {
                offer_id: offer_id.to_owned(),
            },
            Flow::Out,
            requested,
        );
        movements.amount(contract.clone(), Flow::In, requested);
        movements.amount(
            contract,
            Flow::Spent,
            BudgetVector::units(Dimension::WallTimeMs, lease_ms),
        );
        Some(())
    }

    /// Where a closing task contract's remaining escrow goes: back to the account that funded the
    /// offer the contract was awarded from.
    fn closure(&self, movements: &mut Movements, contract_id: &str) -> Option<()> {
        let offer_id = self.formed_from.get(contract_id)?;
        movements.account_only(contract_account(contract_id), Flow::Out);
        movements.account_only(self.funding.get(offer_id)?.clone(), Flow::In);
        Some(())
    }

    /// The accounts and amounts later commands are compared against, taken from the commands that
    /// named them and never from the ledger.
    fn remember(&mut self, command: &CommitmentCommand) {
        match command {
            CommitmentCommand::Advertise(command) => {
                self.funding.insert(
                    command.offer_id.clone(),
                    funding_account(&command.funding_source, &command.sponsor),
                );
            }
            CommitmentCommand::RecordBid(command) => {
                self.requested
                    .insert(command.bid_id.clone(), command.requested_escrow);
            }
            CommitmentCommand::AcceptOpen(command) => {
                self.requested
                    .insert(command.bid_id.clone(), command.requested_escrow);
                self.formed_from
                    .insert(command.contract_id.clone(), command.offer_id.clone());
            }
            CommitmentCommand::Award(command) => {
                self.formed_from
                    .insert(command.contract_id.clone(), command.offer_id.clone());
            }
            CommitmentCommand::StartInvocation(command) => {
                self.invocation_contract
                    .insert(command.invocation_id.clone(), command.contract_id.clone());
            }
            _ => {}
        }
    }
}

fn participant_account(participant_id: &str) -> AccountRef {
    AccountRef::Participant {
        participant_id: participant_id.to_owned(),
    }
}

fn contract_account(contract_id: &str) -> AccountRef {
    AccountRef::TaskContract {
        contract_id: contract_id.to_owned(),
    }
}

/// The account an offer is funded from, as its own command names it.
fn funding_account(source: &FundingSource, sponsor: &str) -> AccountRef {
    match source {
        FundingSource::Participant => participant_account(sponsor),
        FundingSource::TaskContract { contract_id } => contract_account(contract_id),
    }
}

fn command_name(command: &CommitmentCommand) -> &'static str {
    match command {
        CommitmentCommand::RegisterParticipant(_) => "register_participant",
        CommitmentCommand::Advertise(_) => "advertise",
        CommitmentCommand::RecordBid(_) => "record_bid",
        CommitmentCommand::WithdrawBid(_) => "withdraw_bid",
        CommitmentCommand::WithdrawOffer(_) => "withdraw_offer",
        CommitmentCommand::SettleOffer(_) => "settle_offer",
        CommitmentCommand::Award(_) => "award",
        CommitmentCommand::AcceptOpen(_) => "accept_open",
        CommitmentCommand::StartAttempt(_) => "start_attempt",
        CommitmentCommand::RenewLease(_) => "renew_lease",
        CommitmentCommand::SubmitResult(_) => "submit_result",
        CommitmentCommand::Reassign(_) => "reassign",
        CommitmentCommand::ReturnObligation(_) => "return_obligation",
        CommitmentCommand::CancelContract(_) => "cancel_contract",
        CommitmentCommand::AdvanceClock(_) => "advance_clock",
        CommitmentCommand::StartInvocation(_) => "start_invocation",
        CommitmentCommand::YieldInvocation(_) => "yield_invocation",
        CommitmentCommand::ResumeInvocation(_) => "resume_invocation",
        CommitmentCommand::CloseInvocation(_) => "close_invocation",
        CommitmentCommand::RecordVerification(_) => "record_verification",
        CommitmentCommand::StopRun(_) => "stop_run",
    }
}

/// Every account one command's facts moved capacity through, in each direction, against the
/// movements the command named. A movement through an account the command never named is compared
/// against nothing, which is what makes money taken out of the wrong account visible.
fn compare_movements(
    command: &'static str,
    events: &[CommitmentEvent],
    named: &Movements,
) -> Vec<Violation> {
    let mut emitted: BTreeMap<(AccountRef, Flow), [i128; DIMENSION_COUNT]> = BTreeMap::new();
    for event in events {
        match event {
            CommitmentEvent::BudgetTransferred { from, to, amount } => {
                accumulate(&mut emitted, from, Flow::Out, amount);
                accumulate(&mut emitted, to, Flow::In, amount);
            }
            CommitmentEvent::BudgetConsumed { account, amount } => {
                accumulate(&mut emitted, account, Flow::Spent, amount);
            }
            _ => {}
        }
    }
    let mut violations = Vec::new();
    let movements: BTreeSet<&(AccountRef, Flow)> =
        named.named.keys().chain(emitted.keys()).collect();
    for movement in movements {
        let commanded = match named.named.get(movement) {
            Some(Commanded::Amount(commanded)) => *commanded,
            Some(Commanded::AccountOnly) => continue,
            // An account this command never named: nothing of it may move here.
            None => BudgetVector::ZERO,
        };
        let moved = emitted
            .get(movement)
            .copied()
            .unwrap_or([0; DIMENSION_COUNT]);
        violations.extend(compare(
            command,
            &movement_label(&movement.0, movement.1),
            &commanded,
            &moved,
        ));
    }
    violations
}

fn accumulate(
    emitted: &mut BTreeMap<(AccountRef, Flow), [i128; DIMENSION_COUNT]>,
    account: &AccountRef,
    flow: Flow,
    amount: &BudgetVector,
) {
    let totals = emitted
        .entry((account.clone(), flow))
        .or_insert([0; DIMENSION_COUNT]);
    for dimension in DIMENSIONS {
        totals[dimension.index()] += i128::from(amount.get(dimension));
    }
}

fn movement_label(account: &AccountRef, flow: Flow) -> String {
    match flow {
        Flow::In => format!("what reached {}", label(account)),
        Flow::Out => format!("what left {}", label(account)),
        Flow::Spent => format!("what was spent from {}", label(account)),
    }
}

/// What a recorded consent says it asks for, against what the command that recorded it named.
fn consent_violations(issued: &CommitmentCommand, events: &[CommitmentEvent]) -> Vec<Violation> {
    let (command, bid_id, commanded) = match issued {
        CommitmentCommand::RecordBid(issued) => {
            ("record_bid", &issued.bid_id, &issued.requested_escrow)
        }
        CommitmentCommand::AcceptOpen(issued) => {
            ("accept_open", &issued.bid_id, &issued.requested_escrow)
        }
        _ => return Vec::new(),
    };
    let mut violations = Vec::new();
    for event in events {
        if let CommitmentEvent::BidRecorded {
            bid_id: recorded,
            requested_escrow,
            ..
        } = event
            && recorded == bid_id
        {
            let mut emitted = [0_i128; DIMENSION_COUNT];
            for dimension in DIMENSIONS {
                emitted[dimension.index()] = i128::from(requested_escrow.get(dimension));
            }
            violations.extend(compare(
                command,
                &format!("consent {bid_id}"),
                commanded,
                &emitted,
            ));
        }
    }
    violations
}

fn compare(
    command: &'static str,
    subject: &str,
    commanded: &BudgetVector,
    emitted: &[i128; DIMENSION_COUNT],
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for dimension in DIMENSIONS {
        let named = i128::from(commanded.get(dimension));
        if emitted[dimension.index()] != named {
            violations.push(Violation::FactContradictsCommand {
                command,
                subject: subject.to_owned(),
                dimension,
                commanded: named,
                emitted: emitted[dimension.index()],
            });
        }
    }
    violations
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

    /// Where the accounts the facts describe and the accounts the kernel keeps have parted company.
    /// Two independent additions of the same movements must agree in every dimension of every
    /// account, including the capacity that has left the accounts for good.
    ///
    /// This is what conservation is asserted by. Adding the projection up and comparing the total
    /// with the opening budget would state nothing: every fact that moves capacity takes it out of
    /// one account and puts the same quantity into another, so that total equals the opening budget
    /// for any stream of facts whatsoever, including a stream that is wrong. What can be false is
    /// the comparison below — the projection and the registry are built by different code from the
    /// same facts, and a stream that no longer describes what was committed parts company with the
    /// records here.
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

/// What the fact stream alone says about process slices.
///
/// Nothing here reads an invocation record: the records are built by applying the same facts, so a
/// check against them would agree with the kernel by construction. What is read is the stream —
/// which slice was begun, what its yield registered, what its cursor was, and which fact each
/// resumption claims authorized it — and the claim is settled by looking that fact up.
#[derive(Debug, Default)]
pub(crate) struct SliceFacts {
    /// Every fact so far, so that the fact a resumption names can be read back out.
    facts: Vec<CommitmentEvent>,
    /// Which attempt each slice belongs to, as the fact that began it stated.
    attempt: BTreeMap<String, String>,
    /// The cursor and conditions of the last yield of each slice.
    registered: BTreeMap<String, (u64, Vec<WakeCondition>)>,
    /// How many times each slice has been resumed, as the facts stated.
    wakes: BTreeMap<String, u32>,
    /// Which slices were begun or resumed but not yet paid for within the same command.
    started: BTreeSet<String>,
    /// Which slices the facts say are running now: begun or resumed, and neither yielded nor closed
    /// since. It is kept here rather than read off the invocation records for the same reason as
    /// everything else in this projection — the records are built by applying these facts.
    running: BTreeSet<String>,
}

impl SliceFacts {
    /// Take in the facts of one accepted command and report what they broke.
    ///
    /// A slice and the unit of authority that buys it are one indivisible pair, so the pairing is
    /// judged over the facts of one command and not over the stream as a whole: a run that spent
    /// the right number of units in total while beginning a slice nobody paid for would satisfy
    /// every total there is.
    pub(crate) fn observe(&mut self, events: &[CommitmentEvent]) -> Vec<Violation> {
        let mut violations = Vec::new();
        let mut unpaid: Vec<String> = Vec::new();
        let mut paid = 0_u64;
        for event in events {
            match event {
                CommitmentEvent::InvocationStarted {
                    invocation_id,
                    attempt_id,
                    ..
                } => {
                    self.attempt
                        .insert(invocation_id.clone(), attempt_id.clone());
                    self.started.insert(invocation_id.clone());
                    self.running.insert(invocation_id.clone());
                    unpaid.push(invocation_id.clone());
                }
                CommitmentEvent::InvocationYielded {
                    invocation_id,
                    cursor,
                    conditions,
                    ..
                } => {
                    self.registered
                        .insert(invocation_id.clone(), (*cursor, conditions.clone()));
                    self.running.remove(invocation_id);
                }
                CommitmentEvent::InvocationResumed {
                    invocation_id,
                    matched_sequence,
                    ..
                } => {
                    unpaid.push(invocation_id.clone());
                    self.running.insert(invocation_id.clone());
                    *self.wakes.entry(invocation_id.clone()).or_default() += 1;
                    violations.extend(self.unjustified(invocation_id, *matched_sequence));
                }
                CommitmentEvent::InvocationClosed { invocation_id, .. } => {
                    self.running.remove(invocation_id);
                }
                CommitmentEvent::BudgetConsumed { amount, .. } => {
                    paid += amount.get(Dimension::InvocationStarts);
                }
                _ => {}
            }
            self.facts.push(event.clone());
        }
        for invocation_id in unpaid
            .into_iter()
            .skip(usize::try_from(paid).unwrap_or(usize::MAX))
        {
            violations.push(Violation::UnfundedInvocationStart { invocation_id });
        }
        let mut per_attempt: BTreeMap<&str, u32> = BTreeMap::new();
        for (invocation_id, wakes) in &self.wakes {
            if let Some(attempt_id) = self.attempt.get(invocation_id) {
                *per_attempt.entry(attempt_id.as_str()).or_default() += wakes;
            }
        }
        for (attempt_id, wakes) in per_attempt {
            if wakes > MAX_ATTEMPT_WAKES {
                violations.push(Violation::WakeCountExceeded {
                    attempt_id: attempt_id.to_owned(),
                    wakes,
                });
            }
        }
        violations.extend(self.concurrent_slices());
        violations
    }

    /// Whether the facts say one attempt is running more than one process slice.
    fn concurrent_slices(&self) -> Vec<Violation> {
        let mut per_attempt: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for invocation_id in &self.running {
            if let Some(attempt_id) = self.attempt.get(invocation_id) {
                per_attempt
                    .entry(attempt_id.as_str())
                    .or_default()
                    .push(invocation_id.clone());
            }
        }
        per_attempt
            .into_iter()
            .filter(|(_, running)| running.len() > 1)
            .map(|(attempt_id, running)| Violation::TwoRunningSlices {
                attempt_id: attempt_id.to_owned(),
                running,
            })
            .collect()
    }

    /// Whether the fact a resumption named is a fact, lies after the cursor of the yield it
    /// resumes, and is one the yield asked about.
    fn unjustified(&self, invocation_id: &str, matched_sequence: u64) -> Option<Violation> {
        let unjustified = || {
            Some(Violation::ResumedWithoutEvent {
                invocation_id: invocation_id.to_owned(),
                matched_sequence,
            })
        };
        let (cursor, conditions) = self.registered.get(invocation_id)?;
        if matched_sequence == 0 || matched_sequence <= *cursor {
            return unjustified();
        }
        let index = usize::try_from(matched_sequence - 1).ok()?;
        let Some(fact) = self.facts.get(index) else {
            return unjustified();
        };
        if conditions.iter().any(|condition| condition.matches(fact)) {
            None
        } else {
            unjustified()
        }
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
    /// Which commands had their facts compared with what they named. A command the comparison
    /// passed over would otherwise be indistinguishable from one it found nothing wrong with.
    pub compared: BTreeSet<&'static str>,
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
        CommitmentEvent::InvocationStarted { .. } => "invocation_started",
        CommitmentEvent::InvocationYielded { .. } => "invocation_yielded",
        CommitmentEvent::InvocationResumed { .. } => "invocation_resumed",
        CommitmentEvent::InvocationClosed { .. } => "invocation_closed",
        CommitmentEvent::VerificationRecorded { .. } => "verification_recorded",
        CommitmentEvent::RunStopped { .. } => "run_stopped",
    }
}

/// Replay one generated schedule and report every invariant it broke.
pub(crate) fn run_schedule(seed: u64, tokens: &Tokens, disabled: DisabledChecks) -> ScheduleReport {
    run_altered_schedule(seed, tokens, disabled, AlteredFacts::default())
}

/// The same replay against a kernel whose facts have been deliberately corrupted, which is how the
/// comparison with the command is shown to be the check that catches such a fact.
pub(crate) fn run_altered_schedule(
    seed: u64,
    tokens: &Tokens,
    disabled: DisabledChecks,
    altered: AlteredFacts,
) -> ScheduleReport {
    let mut ledger = new_ledger();
    ledger.disable_checks(disabled);
    ledger.alter_facts(altered);
    let mut report = ScheduleReport {
        violations: Vec::new(),
        committed: 0,
        refused: 0,
        reached: BTreeSet::new(),
        guarded: BTreeSet::new(),
        compared: BTreeSet::new(),
    };
    let mut accounts = FactAccounts::opening(ROOT_PARTICIPANT, *ledger.initial_total());
    let mut commanded = CommandedAmounts::default();
    let mut slices = SliceFacts::default();
    for command in setup(tokens) {
        let events = ledger
            .execute(&command)
            .unwrap_or_else(|error| panic!("the deterministic prefix must be accepted: {error}"));
        report
            .violations
            .extend(commanded.observe(&command, &events));
        report.violations.extend(accounts.observe(&events));
        report.violations.extend(slices.observe(&events));
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
                report
                    .violations
                    .extend(commanded.observe(&command, &events));
                report.violations.extend(accounts.observe(&events));
                report.violations.extend(slices.observe(&events));
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
                    // A resumption offered for a slice that registered a fact this pool never
                    // commits. Nothing about its lease, its token or its funding is wrong.
                    CommitmentError::NoMatchingEvent { .. } => {
                        report.guarded.insert("no_matching_event")
                    }
                    CommitmentError::WakeBudgetExhausted { .. } => {
                        report.guarded.insert("wake_budget_exhausted")
                    }
                    CommitmentError::NotAuthorized { .. }
                        if matches!(command, CommitmentCommand::CloseInvocation(_)) =>
                    {
                        report.guarded.insert("close_non_holder")
                    }
                    CommitmentError::NotAuthorized { .. }
                        if matches!(command, CommitmentCommand::StopRun(_)) =>
                    {
                        report.guarded.insert("stop_run_unauthorized")
                    }
                    CommitmentError::CandidateMismatch { .. } => {
                        report.guarded.insert("candidate_mismatch")
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
        report.violations.extend(accounts.divergence(&ledger));
    }
    // Whether a reservation is stranded is a question about the end of the schedule: until then it
    // is only unsettled, which is ordinary.
    report.violations.extend(accounts.stranded());
    report.compared = commanded.compared().clone();
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
/// answered by [`FactAccounts::divergence`], which compares the accounts the facts describe with
/// the accounts the kernel keeps.
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

// ---------------------------------------------------------------------------------------------
// Generated lifecycle schedules and the terminal state each of them must reach
// ---------------------------------------------------------------------------------------------

pub(crate) const LIFE_OFFER: &str = "offer-life";
pub(crate) const LIFE_BID: &str = "bid-life";
pub(crate) const LIFE_CONTRACT: &str = "contract-life";
pub(crate) const LIFE_OBLIGATION: &str = "obligation-life";
pub(crate) const LIFE_ATTEMPT: &str = "attempt-life";
pub(crate) const LIFE_INVOCATION: &str = "invocation-life";
pub(crate) const LIFE_LEASE_MS: u64 = 1_000;
/// The last moment a wake registered by these schedules may be admitted. It is inside the funded
/// lease, and every clock advance the background makes falls before it.
pub(crate) const LIFE_WAKE_DEADLINE: u64 = 500;

/// The ways a run can stop, one scenario each. Every one of them is reached by something finite
/// running out or by an authorized command, and the state it must reach is stated with it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Scenario {
    /// The exact root candidate passed the approved oracle.
    Accepted,
    /// The same candidate, rejected. A result nobody accepted is not an acceptance.
    Rejected,
    /// The protected query could not be carried out, so nothing was learned and assurance is gone.
    VerifierFailure,
    /// The contractor found nothing and said so.
    NoSolution,
    /// The contractor stopped under an allowed stopping policy.
    Abstention,
    /// The participant stopped answering while its slice was yielded.
    ParticipantLoss,
    /// The lease ran out under a yielded slice nobody woke.
    LeaseExpiry,
    /// An authorized human stopped the run.
    Cancellation,
    /// A participant that keeps yielding on a fact it keeps meeting.
    Churn,
}

impl Scenario {
    pub(crate) const ALL: [Self; 9] = [
        Self::Accepted,
        Self::Rejected,
        Self::VerifierFailure,
        Self::NoSolution,
        Self::Abstention,
        Self::ParticipantLoss,
        Self::LeaseExpiry,
        Self::Cancellation,
        Self::Churn,
    ];

    /// The state this scenario must reach. Every non-success state is a different reason, and the
    /// one success state is the only one a passing protected query can produce.
    pub(crate) const fn expected(self) -> RootTerminal {
        match self {
            Self::Accepted => RootTerminal::Accepted,
            Self::VerifierFailure => RootTerminal::InfrastructureError,
            Self::Abstention => RootTerminal::Abstained,
            Self::Cancellation => RootTerminal::Cancelled,
            Self::Rejected
            | Self::NoSolution
            | Self::ParticipantLoss
            | Self::LeaseExpiry
            | Self::Churn => RootTerminal::Exhausted,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::VerifierFailure => "verifier_failure",
            Self::NoSolution => "no_solution",
            Self::Abstention => "abstention",
            Self::ParticipantLoss => "participant_loss",
            Self::LeaseExpiry => "lease_expiry",
            Self::Cancellation => "cancellation",
            Self::Churn => "churn",
        }
    }
}

fn life_execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 40_000)
        .with(Dimension::ModelTokens, 40_000)
        .with(Dimension::WallTimeMs, 8_000)
        .with(Dimension::VerificationQueries, 2)
        .with(Dimension::AttemptStarts, 4)
        .with(Dimension::InvocationStarts, 24)
}

fn life_requested_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 20_000)
        .with(Dimension::ModelTokens, 20_000)
        .with(Dimension::WallTimeMs, 5_000)
        .with(Dimension::VerificationQueries, 1)
        .with(Dimension::AttemptStarts, 2)
        // Room for more process slices than the wake bound admits, so that what stops a churning
        // participant is the bound rather than the escrow.
        .with(Dimension::InvocationStarts, 20)
}

/// One participant holding one task contract directly under the root obligation. Everything a
/// scenario varies happens inside it.
pub(crate) fn life_setup(tokens: &Tokens) -> Vec<CommitmentCommand> {
    vec![
        register(ALPHA),
        register(BETA),
        register(GAMMA),
        CommitmentCommand::Advertise(Advertise {
            offer_id: LIFE_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            parent_obligation: ROOT_OBLIGATION.to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: vec![tokens.dependency.clone()],
            capability_scope: vec![tokens.capability.clone()],
            execution_escrow: life_execution_escrow(),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        }),
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: LIFE_BID.to_owned(),
            offer_id: LIFE_OFFER.to_owned(),
            bidder: ALPHA.to_owned(),
            requested_escrow: life_requested_escrow(),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: Some(tokens.proposal(ALPHA)),
            expires_at: DEADLINE,
        }),
        CommitmentCommand::Award(Award {
            contract_id: LIFE_CONTRACT.to_owned(),
            obligation_id: LIFE_OBLIGATION.to_owned(),
            lease_id: "lease-life".to_owned(),
            offer_id: LIFE_OFFER.to_owned(),
            bid_id: LIFE_BID.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            lease_ms: LIFE_LEASE_MS,
        }),
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: LIFE_ATTEMPT.to_owned(),
            contract_id: LIFE_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
        }),
    ]
}

fn life_start() -> CommitmentCommand {
    CommitmentCommand::StartInvocation(StartInvocation {
        invocation_id: LIFE_INVOCATION.to_owned(),
        attempt_id: LIFE_ATTEMPT.to_owned(),
        contract_id: LIFE_CONTRACT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        cursor: 0,
    })
}

/// A yield that asks about the submission of its own task contract. Its cursor stays where it is,
/// so once a submission exists the same fact answers every later yield: this is the participant
/// that could be woken forever if nothing bounded it.
fn life_yield() -> CommitmentCommand {
    CommitmentCommand::YieldInvocation(YieldInvocation {
        invocation_id: LIFE_INVOCATION.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        cursor: 0,
        conditions: vec![WakeCondition::SubmissionRecorded {
            contract_id: LIFE_CONTRACT.to_owned(),
        }],
        wake_deadline: LIFE_WAKE_DEADLINE,
    })
}

fn life_resume() -> CommitmentCommand {
    CommitmentCommand::ResumeInvocation(ResumeInvocation {
        invocation_id: LIFE_INVOCATION.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
    })
}

fn life_submit() -> CommitmentCommand {
    CommitmentCommand::SubmitResult(SubmitResult {
        contract_id: LIFE_CONTRACT.to_owned(),
        attempt_id: LIFE_ATTEMPT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        candidate_digest: digest("candidate-life"),
    })
}

fn life_verify(verdict: Verdict) -> CommitmentCommand {
    CommitmentCommand::RecordVerification(RecordVerification {
        contract_id: LIFE_CONTRACT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        candidate_digest: digest("candidate-life"),
        verdict,
    })
}

fn life_return(outcome: Outcome) -> CommitmentCommand {
    CommitmentCommand::ReturnObligation(ReturnObligation {
        contract_id: LIFE_CONTRACT.to_owned(),
        participant: ALPHA.to_owned(),
        generation: 1,
        outcome,
    })
}

fn life_close(closer: &str, reason: InvocationClosure) -> CommitmentCommand {
    CommitmentCommand::CloseInvocation(CloseInvocation {
        invocation_id: LIFE_INVOCATION.to_owned(),
        closer: closer.to_owned(),
        reason,
    })
}

/// The causally ordered commands one scenario is made of. A participant issues its own commands in
/// order; what the seed varies is where everything else falls around them.
pub(crate) fn life_chain(scenario: Scenario) -> Vec<CommitmentCommand> {
    let result = Outcome::Result {
        candidate_digest: digest("candidate-life"),
    };
    match scenario {
        Scenario::Accepted | Scenario::Rejected | Scenario::VerifierFailure => {
            let verdict = match scenario {
                Scenario::Accepted => Verdict::Passed,
                Scenario::Rejected => Verdict::Failed,
                _ => Verdict::InfrastructureError,
            };
            vec![
                life_start(),
                life_yield(),
                life_submit(),
                life_resume(),
                life_verify(verdict),
                life_close(ALPHA, InvocationClosure::Completed),
                life_return(result),
            ]
        }
        Scenario::NoSolution => vec![
            life_start(),
            life_close(ALPHA, InvocationClosure::Completed),
            life_return(Outcome::Exhausted),
        ],
        Scenario::Abstention => vec![
            life_start(),
            life_yield(),
            life_submit(),
            life_resume(),
            life_close(ALPHA, InvocationClosure::Completed),
            life_return(Outcome::Declined),
        ],
        // Nobody ever resumes it, and nobody has to: the sponsor records the loss and takes the
        // contract back.
        Scenario::ParticipantLoss => vec![
            life_start(),
            life_yield(),
            life_close(ROOT_PARTICIPANT, InvocationClosure::ParticipantLost),
        ],
        // The clock passes the wake deadline and then the lease. What was funded has run out, and
        // the yielded slice stops being anything the run waits for.
        Scenario::LeaseExpiry => vec![
            life_start(),
            life_yield(),
            CommitmentCommand::AdvanceClock(AdvanceClock { to: 600 }),
            life_close(ALPHA, InvocationClosure::WakeDeadlineExpired),
            CommitmentCommand::AdvanceClock(AdvanceClock { to: 2_000 }),
        ],
        Scenario::Cancellation => vec![
            life_start(),
            life_yield(),
            CommitmentCommand::StopRun(StopRun {
                authority: ROOT_PARTICIPANT.to_owned(),
                reason: StopReason::Cancelled,
            }),
        ],
        Scenario::Churn => {
            let mut chain = vec![life_start(), life_yield(), life_submit()];
            for _ in 0..MAX_ATTEMPT_WAKES + 4 {
                chain.push(life_resume());
                chain.push(life_yield());
            }
            chain.push(life_close(ALPHA, InvocationClosure::Completed));
            chain.push(life_return(Outcome::Exhausted));
            chain
        }
    }
}

/// Commands no ordering may accept, so that the rules refusing them are reached rather than merely
/// present, plus clock advances that stay inside the funded lease.
pub(crate) fn life_background() -> Vec<CommitmentCommand> {
    vec![
        // A slice belongs to the participant running it. Nobody else stops it or starts it again.
        CommitmentCommand::YieldInvocation(YieldInvocation {
            invocation_id: LIFE_INVOCATION.to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            cursor: 0,
            conditions: vec![WakeCondition::RunStopped],
            wake_deadline: LIFE_WAKE_DEADLINE,
        }),
        CommitmentCommand::ResumeInvocation(ResumeInvocation {
            invocation_id: LIFE_INVOCATION.to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
        }),
        life_close(GAMMA, InvocationClosure::ParticipantLost),
        // The participant's own resumption, offered wherever the seed puts it. Before the fact its
        // yield asked about is committed there is nothing to resume for, and in the scenarios where
        // that fact is never committed at all there never is.
        life_resume(),
        // A token from a generation this contract never reached.
        CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: "invocation-life-stale".to_owned(),
            attempt_id: LIFE_ATTEMPT.to_owned(),
            contract_id: LIFE_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 99,
            cursor: 0,
        }),
        // A cursor naming a fact the run has not committed.
        CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: "invocation-life-ahead".to_owned(),
            attempt_id: LIFE_ATTEMPT.to_owned(),
            contract_id: LIFE_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            cursor: u64::MAX,
        }),
        // A verdict attached to a bundle this contract never submitted.
        CommitmentCommand::RecordVerification(RecordVerification {
            contract_id: LIFE_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            candidate_digest: digest("candidate-elsewhere"),
            verdict: Verdict::Passed,
        }),
        // The run belongs to the participant the root obligation belongs to.
        CommitmentCommand::StopRun(StopRun {
            authority: GAMMA.to_owned(),
            reason: StopReason::Cancelled,
        }),
        CommitmentCommand::AdvanceClock(AdvanceClock { to: 100 }),
        CommitmentCommand::AdvanceClock(AdvanceClock { to: 200 }),
    ]
}

/// What a sponsor offers once nothing else is going to happen: the slice is closed, the contract is
/// taken back, and the reservation comes home. Refusals here are ordinary — most of it is already
/// done in most scenarios — and what it establishes is that winding a run down needs no state
/// repair, only the commands the protocol already has.
pub(crate) fn life_wind_down() -> Vec<CommitmentCommand> {
    vec![
        life_close(ROOT_PARTICIPANT, InvocationClosure::ParticipantLost),
        CommitmentCommand::CancelContract(CancelContract {
            contract_id: LIFE_CONTRACT.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: LIFE_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
        CommitmentCommand::AdvanceClock(AdvanceClock { to: 5_000 }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: LIFE_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
    ]
}

pub(crate) const NESTED_OFFER: &str = "offer-nested";
pub(crate) const NESTED_BID: &str = "bid-nested";
pub(crate) const NESTED_CONTRACT: &str = "contract-nested";
pub(crate) const NESTED_OBLIGATION: &str = "obligation-nested";
pub(crate) const NESTED_ATTEMPT: &str = "attempt-nested";

/// What one award of the delegated offer funds. It buys the one protected query the composition
/// spends and the attempt that produces the candidate that query is spent on.
fn nested_execution_escrow() -> BudgetVector {
    BudgetVector::ZERO
        .with(Dimension::MoneyMicros, 4_000)
        .with(Dimension::ModelTokens, 4_000)
        .with(Dimension::WallTimeMs, 4_000)
        .with(Dimension::VerificationQueries, 1)
        .with(Dimension::AttemptStarts, 1)
}

/// A run whose passing protected query was spent one level below the root.
///
/// The contractor of the root-scope contract delegates part of its work: it hangs a second offer
/// under its own obligation, awards it, and the participant that takes it submits a candidate and
/// has that candidate verified. The verdict therefore passes against a task contract that does not
/// hang directly under the root obligation, which is the one shape that can separate the scope a
/// verification fact states from the scope its contract actually has.
///
/// Everything is issued in causal order and every command must be accepted: what this composition
/// exists to vary is not the ordering but the honesty of the scope the kernel stamps on the fact.
pub(crate) fn nested_scope_run(altered: AlteredFacts) -> CommitmentLedger {
    let tokens = Tokens::variant("life");
    let candidate = digest("candidate-nested");
    let mut ledger = new_ledger();
    ledger.alter_facts(altered);
    let commands = life_setup(&tokens).into_iter().chain([
        // The contractor of the root-scope work delegates part of it. The offer hangs under the
        // obligation that work carries, and is paid for out of the contractor's own balance, so
        // the two relations stay independent as everywhere else.
        CommitmentCommand::Advertise(Advertise {
            offer_id: NESTED_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
            parent_obligation: LIFE_OBLIGATION.to_owned(),
            funding_source: FundingSource::Participant,
            task_scope: tokens.task_scope.clone(),
            base_digest: tokens.base_digest.clone(),
            intent_digest: tokens.intent_digest.clone(),
            artifact_class: tokens.artifact_class.clone(),
            dependencies: vec![tokens.dependency.clone()],
            capability_scope: vec![tokens.capability.clone()],
            execution_escrow: nested_execution_escrow(),
            policy: OfferPolicy::Negotiated,
            bid_deadline: DEADLINE,
            offer_deadline: DEADLINE,
            max_awards: 1,
        }),
        CommitmentCommand::RecordBid(RecordBid {
            bid_id: NESTED_BID.to_owned(),
            offer_id: NESTED_OFFER.to_owned(),
            bidder: BETA.to_owned(),
            requested_escrow: nested_execution_escrow(),
            artifact_class: tokens.artifact_class.clone(),
            proposal_digest: Some(tokens.proposal(BETA)),
            expires_at: DEADLINE,
        }),
        CommitmentCommand::Award(Award {
            contract_id: NESTED_CONTRACT.to_owned(),
            obligation_id: NESTED_OBLIGATION.to_owned(),
            lease_id: "lease-nested".to_owned(),
            offer_id: NESTED_OFFER.to_owned(),
            bid_id: NESTED_BID.to_owned(),
            sponsor: ALPHA.to_owned(),
            lease_ms: DEADLINE,
        }),
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: NESTED_ATTEMPT.to_owned(),
            contract_id: NESTED_CONTRACT.to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
        }),
        CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: NESTED_CONTRACT.to_owned(),
            attempt_id: NESTED_ATTEMPT.to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            candidate_digest: candidate.clone(),
        }),
        // The only protected query this composition spends, and it is spent below the root.
        CommitmentCommand::RecordVerification(RecordVerification {
            contract_id: NESTED_CONTRACT.to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            candidate_digest: candidate.clone(),
            verdict: Verdict::Passed,
        }),
        // Winding down from the leaves inward: the delegated work closes and its reservation
        // comes home before the work above it may close at all.
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: NESTED_CONTRACT.to_owned(),
            participant: BETA.to_owned(),
            generation: 1,
            outcome: Outcome::Result {
                candidate_digest: candidate,
            },
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: NESTED_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: NESTED_OFFER.to_owned(),
            sponsor: ALPHA.to_owned(),
        }),
        CommitmentCommand::ReturnObligation(ReturnObligation {
            contract_id: LIFE_CONTRACT.to_owned(),
            participant: ALPHA.to_owned(),
            generation: 1,
            outcome: Outcome::Exhausted,
        }),
        CommitmentCommand::WithdrawOffer(WithdrawOffer {
            offer_id: LIFE_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
        CommitmentCommand::SettleOffer(SettleOffer {
            offer_id: LIFE_OFFER.to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
        }),
    ]);
    for command in commands {
        ledger
            .execute(&command)
            .unwrap_or_else(|error| panic!("the nested composition must be accepted: {error}"));
    }
    ledger
}

/// The resumption a yielded slice holds its wake for, as its own participant would issue it.
pub(crate) fn resumption(invocation: &InvocationRecord) -> CommitmentCommand {
    CommitmentCommand::ResumeInvocation(ResumeInvocation {
        invocation_id: invocation.invocation_id.clone(),
        participant: invocation.participant.clone(),
        generation: invocation.generation,
    })
}

/// Whether a refusal is one no later command can lift, and how to say so.
///
/// A run may honestly wait for a fact nobody has committed yet, and a resumption refused for that
/// reason is a wake still worth counting. Every reason listed here is different, because each rests
/// on something the protocol only moves one way: the clock never runs backwards, a stopped run is
/// never restarted, a fencing generation never returns to a displaced holder, a contract that
/// reached a terminal state never reopens, and creation authority is never credited back into an
/// account it left.
pub(crate) fn permanent_refusal(error: &CommitmentError) -> Option<&'static str> {
    match error {
        CommitmentError::RunStopped => Some("the run is stopped and refuses every resumption"),
        CommitmentError::WakeDeadlinePassed { .. } => Some("its deadline has passed"),
        CommitmentError::WakeBudgetExhausted { .. } => {
            Some("its attempt has used every wake it was funded for")
        }
        CommitmentError::StaleGeneration { .. } | CommitmentError::NotAuthorized { .. } => {
            Some("its contract has changed hands")
        }
        CommitmentError::LeaseExpired { .. } => Some("its lease has run out"),
        CommitmentError::ContractNotActive { .. } => Some("its task contract has closed"),
        CommitmentError::InsufficientBudget { .. } => {
            Some("nothing left in its escrow pays for the resumption")
        }
        CommitmentError::InvocationNotYielded { .. } => Some("the slice registered no wake at all"),
        _ => None,
    }
}

/// Whether the run is waiting on a wake that nothing can honour any more.
///
/// It is asked after every command rather than at the end, because winding a run down closes such a
/// slice for reasons of its own and the question would then never be reached.
///
/// What settles it is not a second copy of the rules that keep a wake alive — that would restate the
/// projection the ledger already computes and agree with it wherever both are wrong together. It is
/// the transition itself: the kernel is offered the very resumption the wake was registered for, and
/// a run that counts a wake while refusing its resumption for a reason no later command can lift is
/// holding itself open on something nobody will ever use.
fn dead_wake(ledger: &CommitmentLedger) -> Option<Violation> {
    let OpenAuthority::FundedWake { invocation_id } = ledger.open_authority()? else {
        return None;
    };
    let invocation = ledger.invocations().get(&invocation_id)?;
    let refusal = ledger.decide(&resumption(invocation)).err()?;
    let why = permanent_refusal(&refusal)?;
    Some(Violation::DeadWakeHoldsRunOpen { invocation_id, why })
}

/// A control object the kernel would still honour, or `None` when nothing it accepts can advance the
/// run any further.
///
/// This is the other direction of the same question `dead_wake` asks, and it is decided the same
/// way: by offering the kernel a transition instead of by reading the projection its own terminal
/// state is defined by. A slice that is running, a yielded slice whose resumption would be accepted,
/// and an attempt that could still begin a slice are each work the run has not finished.
fn still_advancing(ledger: &CommitmentLedger) -> Option<String> {
    for invocation in ledger.invocations().values() {
        if invocation.state == InvocationState::Running {
            return Some(format!("running slice {}", invocation.invocation_id));
        }
        if ledger.decide(&resumption(invocation)).is_ok() {
            return Some(format!("resumable slice {}", invocation.invocation_id));
        }
    }
    for attempt in ledger.attempts().values() {
        let probe = CommitmentCommand::StartInvocation(StartInvocation {
            invocation_id: format!("probe-{}", attempt.attempt_id),
            attempt_id: attempt.attempt_id.clone(),
            contract_id: attempt.contract_id.clone(),
            participant: attempt.participant.clone(),
            generation: attempt.generation,
            cursor: 0,
        });
        if ledger.decide(&probe).is_ok() {
            return Some(format!("startable attempt {}", attempt.attempt_id));
        }
    }
    None
}

/// Whether the fact stream carries a passing protected query against a candidate of a task contract
/// that really does hang directly under the root obligation.
///
/// The kernel decides root scope while it commits the verification and stores its answer in the
/// fact; the terminal state is then derived from that stored answer. Reading the same flag back here
/// would compare the kernel with itself, so the obligation tree is rebuilt from the facts that
/// created it and the scope of every passing verdict is settled against that.
fn verified_at_root_scope(ledger: &CommitmentLedger) -> bool {
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    let mut carried: BTreeMap<&str, &str> = BTreeMap::new();
    for fact in ledger.facts() {
        match fact {
            CommitmentEvent::ObligationCreated {
                obligation_id,
                parent: above,
                ..
            } => {
                parent.insert(obligation_id, above);
            }
            CommitmentEvent::TaskContractFormed {
                contract_id,
                obligation_id,
                ..
            } => {
                carried.insert(contract_id, obligation_id);
            }
            _ => {}
        }
    }
    ledger.facts().iter().any(|fact| {
        let CommitmentEvent::VerificationRecorded {
            contract_id,
            verdict: Verdict::Passed,
            ..
        } = fact
        else {
            return false;
        };
        carried
            .get(contract_id.as_str())
            .and_then(|obligation_id| parent.get(*obligation_id))
            .is_some_and(|above| *above == ledger.root_obligation())
    })
}

/// What the terminal state a run reports must agree with: the transitions the kernel would still
/// accept, and the evidence acceptance is supposed to rest on.
///
/// It is answered for a whole reachable state rather than for one interleaving, so the reachability
/// sweep asks it of every state it reaches and the lifecycle schedules ask it of the state each of
/// them ends in.
pub(crate) fn terminal_violations(
    ledger: &CommitmentLedger,
    terminal: Option<RootTerminal>,
) -> Vec<Violation> {
    let Some(terminal) = terminal else {
        return Vec::new();
    };
    let mut violations = Vec::new();
    if let Some(outstanding) = still_advancing(ledger) {
        violations.push(Violation::TerminalWhileOpen {
            terminal,
            outstanding,
        });
    }
    if terminal == RootTerminal::Accepted && !verified_at_root_scope(ledger) {
        violations.push(Violation::AcceptedWithoutVerification);
    }
    violations
}

#[derive(Debug)]
pub(crate) struct LifecycleReport {
    pub violations: Vec<Violation>,
    pub terminal: Option<RootTerminal>,
    /// Which facts this scenario actually committed.
    pub reached: BTreeSet<&'static str>,
    /// Which guarded refusals this scenario actually provoked.
    pub guarded: BTreeSet<&'static str>,
}

/// Replay one scenario in a seeded interleaving and report the state the run reached.
pub(crate) fn run_lifecycle(
    seed: u64,
    scenario: Scenario,
    disabled: DisabledChecks,
) -> LifecycleReport {
    let tokens = Tokens::variant("life");
    let mut ledger = new_ledger();
    ledger.disable_checks(disabled);
    let mut report = LifecycleReport {
        violations: Vec::new(),
        terminal: None,
        reached: BTreeSet::new(),
        guarded: BTreeSet::new(),
    };
    // The deterministic prefix every scenario shares. It must be accepted, and the projections are
    // built from the facts it committed so that what follows is compared against a complete
    // account of the run rather than against its tail.
    let mut accounts = FactAccounts::opening(ROOT_PARTICIPANT, *ledger.initial_total());
    let mut commanded = CommandedAmounts::default();
    let mut slices = SliceFacts::default();
    for command in life_setup(&tokens) {
        let events = ledger
            .execute(&command)
            .unwrap_or_else(|error| panic!("the deterministic prefix must be accepted: {error}"));
        report
            .violations
            .extend(commanded.observe(&command, &events));
        report.violations.extend(accounts.observe(&events));
        report.violations.extend(slices.observe(&events));
    }
    let mut observe = |ledger: &mut CommitmentLedger,

                       report: &mut LifecycleReport,
                       command: &CommitmentCommand| match ledger
        .execute(command)
    {
        Ok(events) => {
            report.reached.extend(events.iter().map(fact_name));
            report
                .violations
                .extend(commanded.observe(command, &events));
            report.violations.extend(accounts.observe(&events));
            report.violations.extend(slices.observe(&events));
            report.violations.extend(state_violations(ledger));
            report.violations.extend(accounts.divergence(ledger));
            report.violations.extend(dead_wake(ledger));
        }
        Err(error) => {
            match error {
                CommitmentError::NoMatchingEvent { .. } => {
                    report.guarded.insert("no_matching_event")
                }
                CommitmentError::WakeBudgetExhausted { .. } => {
                    report.guarded.insert("wake_budget_exhausted")
                }
                CommitmentError::WakeDeadlinePassed { .. } => {
                    report.guarded.insert("wake_deadline_passed")
                }
                CommitmentError::CursorAhead { .. } => report.guarded.insert("cursor_ahead"),
                CommitmentError::CandidateMismatch { .. } => {
                    report.guarded.insert("candidate_mismatch")
                }
                CommitmentError::RunStopped => report.guarded.insert("run_stopped"),
                CommitmentError::StaleGeneration { .. } => {
                    report.guarded.insert("stale_generation")
                }
                CommitmentError::NotAuthorized { .. } => report.guarded.insert("not_authorized"),
                _ => false,
            };
        }
    };
    let merged = interleaved(
        seed,
        &shuffled(seed, life_background()),
        &life_chain(scenario),
    );
    for command in merged.into_iter().chain(life_wind_down()) {
        observe(&mut ledger, &mut report, &command);
    }
    report.terminal = ledger.root_terminal();
    report
        .violations
        .extend(terminal_violations(&ledger, report.terminal));
    // Everything that could wind this run down has been offered by now, so a run still reporting no
    // terminal never wound down at all, and it must at least be able to name what it waits for.
    if report.terminal.is_none() {
        report.violations.push(Violation::NeverQuiescent {
            outstanding: ledger
                .open_authority()
                .map_or_else(|| "nothing it can name".to_owned(), |open| open.label()),
        });
    }
    if report.terminal != Some(scenario.expected()) {
        report.violations.push(Violation::DishonestTerminal {
            expected: scenario.expected(),
            actual: report.terminal,
        });
    }
    report.violations.dedup();
    report
}
