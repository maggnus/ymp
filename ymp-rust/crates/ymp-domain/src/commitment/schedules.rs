//! Generated concurrent command schedules and the invariants every one of them must preserve.
//!
//! One ordering proves nothing about contention. The generator builds a fixed cast of
//! participants, offers and consent, then interleaves a pool of commands issued by different
//! principals — including commands that are only valid in some orderings, and commands that must
//! never be valid at all — and replays the whole pool in a seeded order. Refusals are expected and
//! are not failures; what the schedule asserts is that after every single command the ledger still
//! conserves every budget dimension, has awarded no more slots than it funded, holds one contract
//! per consent and one obligation per contract, has advanced nothing under a stale fencing token,
//! and has closed no obligation whose causal work is still outstanding.

use std::collections::{BTreeMap, BTreeSet};

use super::budget::{BudgetVector, DIMENSIONS, Dimension};
use super::ledger::{CommitmentLedger, DisabledChecks};
use super::protocol::{
    AcceptOpen, AdvanceClock, Advertise, Award, CancelContract, CommitmentCommand, CommitmentEvent,
    Reassign, RecordBid, RegisterParticipant, RenewLease, ReturnObligation, SettleOffer,
    StartAttempt, SubmitResult, WithdrawBid, WithdrawOffer,
};
use super::records::{FundingSource, ObligationState, OfferPolicy, OfferState, Outcome};

pub(crate) const ROOT_PARTICIPANT: &str = "sponsor-root";
pub(crate) const ROOT_PRINCIPAL: &str = "principal-root";
pub(crate) const ROOT_OBLIGATION: &str = "obligation-root";
pub(crate) const ALPHA: &str = "p-alpha";
pub(crate) const BETA: &str = "p-beta";
pub(crate) const GAMMA: &str = "p-gamma";
pub(crate) const MAIN_OFFER: &str = "offer-main";
pub(crate) const OPEN_OFFER: &str = "offer-open";
pub(crate) const CHILD_OFFER: &str = "offer-child";
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
        for who in [ALPHA, BETA, GAMMA, "child"] {
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

fn accept_open(tokens: &Tokens, participant: &str, suffix: &str) -> CommitmentCommand {
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
    ]
}

/// The commands whose order is generated. Several are valid only in some orderings, and several
/// must never be valid in any: a stale fencing token, an award beyond the funded count, a return
/// issued over work that is still outstanding.
pub(crate) fn contention_pool(tokens: &Tokens) -> Vec<CommitmentCommand> {
    let candidate = digest("candidate-one");
    let stale_candidate = digest("candidate-stale");
    vec![
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
        // sponsor names. Everything issued under the previous generation is stale from here on.
        CommitmentCommand::Reassign(Reassign {
            contract_id: "contract-alpha".to_owned(),
            sponsor: ROOT_PARTICIPANT.to_owned(),
            bid_id: "bid-alpha-spare".to_owned(),
            lease_id: "lease-alpha-2".to_owned(),
            lease_ms: LEASE_MS,
        }),
        CommitmentCommand::StartAttempt(StartAttempt {
            attempt_id: "attempt-alpha-2".to_owned(),
            contract_id: "contract-alpha".to_owned(),
            participant: ALPHA.to_owned(),
            generation: 2,
        }),
        CommitmentCommand::SubmitResult(SubmitResult {
            contract_id: "contract-alpha".to_owned(),
            attempt_id: "attempt-alpha-2".to_owned(),
            participant: ALPHA.to_owned(),
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
            participant: ALPHA.to_owned(),
            generation: 2,
            outcome: Outcome::Result {
                candidate_digest: digest("candidate-one"),
            },
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Violation {
    /// A dimension no longer adds up to what the run started with.
    Conservation {
        dimension: Dimension,
        expected: u64,
        found: u64,
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
    /// A refused command changed the ledger.
    RefusalMutatedState { position: usize },
}

#[derive(Debug)]
pub(crate) struct ScheduleReport {
    pub violations: Vec<Violation>,
    pub committed: usize,
    pub refused: usize,
    /// Which facts this schedule actually committed. A pool that quietly stopped reaching
    /// contention would otherwise pass every invariant by doing nothing.
    pub reached: BTreeSet<&'static str>,
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
    };
    for command in setup(tokens) {
        ledger
            .execute(&command)
            .unwrap_or_else(|error| panic!("the deterministic prefix must be accepted: {error}"));
    }
    for (position, command) in shuffled(seed, contention_pool(tokens))
        .into_iter()
        .enumerate()
    {
        let before = ledger.clone();
        match ledger.execute(&command) {
            Ok(events) => {
                report.committed += 1;
                report.reached.extend(events.iter().map(fact_name));
                report
                    .violations
                    .extend(fencing_violations(&ledger, &events));
            }
            Err(_) => {
                report.refused += 1;
                if ledger != before {
                    report
                        .violations
                        .push(Violation::RefusalMutatedState { position });
                }
            }
        }
        report.violations.extend(state_violations(&ledger));
    }
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

/// The invariants that must hold of the ledger itself after every command.
pub(crate) fn state_violations(ledger: &CommitmentLedger) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut total = *ledger.consumed();
    for participant in ledger.participants().values() {
        total = sum(total, &participant.balance);
    }
    for offer in ledger.offers().values() {
        total = sum(total, &offer.escrow);
    }
    for contract in ledger.contracts().values() {
        total = sum(total, &contract.escrow);
    }
    for dimension in DIMENSIONS {
        let expected = ledger.initial_total().get(dimension);
        let found = total.get(dimension);
        if expected != found {
            violations.push(Violation::Conservation {
                dimension,
                expected,
                found,
            });
        }
    }

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

fn sum(total: BudgetVector, addend: &BudgetVector) -> BudgetVector {
    total
        .checked_add(addend)
        .expect("ledger totals stay within the range the run started with")
}
