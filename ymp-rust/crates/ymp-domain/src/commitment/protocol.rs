//! The typed commands a participant may issue, the facts they commit, and the typed refusals.
//!
//! A command carries identifiers, digests, integer quantities and deadlines. It carries no free
//! text: an intent or a proposal reaches the kernel only as the digest of an inert message, and a
//! digest is stored and compared, never read. There is consequently no field a transition could
//! consult to learn a skill, a model, a rank or how persuasive a bid is.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::budget::{BudgetVector, Dimension};
use super::records::{AccountRef, BidOrigin, FundingSource, OfferPolicy, Outcome};
use crate::MAX_IDENTIFIER_CHARS;

/// The largest number of awards one offer may fund.
pub const MAX_AWARDS: u32 = 64;
/// The longest lease a single command may buy, in milliseconds.
pub const MAX_LEASE_MS: u64 = 24 * 60 * 60 * 1000;
/// The largest number of mechanically declared dependency or capability tokens on one offer.
pub const MAX_SCOPE_ENTRIES: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegisterParticipant {
    pub participant_id: String,
    pub principal_id: String,
    /// The participant whose own capacity and participant-start authority fund the registration.
    pub sponsor: String,
    pub endowment: BudgetVector,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Advertise {
    pub offer_id: String,
    pub sponsor: String,
    pub parent_obligation: String,
    pub funding_source: FundingSource,
    pub task_scope: String,
    pub base_digest: String,
    pub intent_digest: String,
    pub artifact_class: String,
    pub dependencies: Vec<String>,
    pub capability_scope: Vec<String>,
    pub execution_escrow: BudgetVector,
    pub policy: OfferPolicy,
    pub bid_deadline: u64,
    pub offer_deadline: u64,
    pub max_awards: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordBid {
    pub bid_id: String,
    pub offer_id: String,
    pub bidder: String,
    pub requested_escrow: BudgetVector,
    pub artifact_class: String,
    pub proposal_digest: Option<String>,
    pub expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WithdrawBid {
    pub bid_id: String,
    pub bidder: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WithdrawOffer {
    pub offer_id: String,
    pub sponsor: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettleOffer {
    pub offer_id: String,
    pub sponsor: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Award {
    pub contract_id: String,
    pub obligation_id: String,
    pub lease_id: String,
    pub offer_id: String,
    /// The consent the sponsor awards. The sponsor names it; the kernel never selects one.
    pub bid_id: String,
    pub sponsor: String,
    pub lease_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AcceptOpen {
    pub contract_id: String,
    pub obligation_id: String,
    pub lease_id: String,
    pub bid_id: String,
    pub offer_id: String,
    pub participant: String,
    pub requested_escrow: BudgetVector,
    pub artifact_class: String,
    pub proposal_digest: Option<String>,
    pub lease_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StartAttempt {
    pub attempt_id: String,
    pub contract_id: String,
    pub participant: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RenewLease {
    pub contract_id: String,
    pub holder: String,
    pub generation: u64,
    pub lease_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SubmitResult {
    pub contract_id: String,
    pub attempt_id: String,
    pub participant: String,
    pub generation: u64,
    pub candidate_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reassign {
    pub contract_id: String,
    pub sponsor: String,
    /// Consent from the replacement contractor. The sponsor names it, as with any award.
    pub bid_id: String,
    pub lease_id: String,
    pub lease_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReturnObligation {
    pub contract_id: String,
    pub participant: String,
    pub generation: u64,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CancelContract {
    pub contract_id: String,
    pub sponsor: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdvanceClock {
    pub to: u64,
}

/// One typed shared effect requested against the ledger.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum CommitmentCommand {
    RegisterParticipant(RegisterParticipant),
    Advertise(Advertise),
    RecordBid(RecordBid),
    WithdrawBid(WithdrawBid),
    WithdrawOffer(WithdrawOffer),
    SettleOffer(SettleOffer),
    Award(Award),
    AcceptOpen(AcceptOpen),
    StartAttempt(StartAttempt),
    RenewLease(RenewLease),
    SubmitResult(SubmitResult),
    Reassign(Reassign),
    ReturnObligation(ReturnObligation),
    CancelContract(CancelContract),
    AdvanceClock(AdvanceClock),
}

impl CommitmentCommand {
    /// The digest of the exact command content, which is what makes a retry recognizable.
    pub fn digest(&self) -> Result<String, serde_json::Error> {
        Ok(crate::digest_bytes(&serde_json::to_vec(self)?))
    }
}

/// One committed fact. A command commits either all of its facts or none of them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CommitmentEvent {
    ClockAdvanced {
        to: u64,
    },
    ParticipantRegistered {
        participant_id: String,
        principal_id: String,
    },
    /// Capacity changing hands. It is the only way an account gains anything.
    BudgetTransferred {
        from: AccountRef,
        to: AccountRef,
        amount: BudgetVector,
    },
    /// Capacity leaving the accounts for good. It is the only way an account loses anything that
    /// no other account gains.
    BudgetConsumed {
        account: AccountRef,
        amount: BudgetVector,
    },
    OfferAdvertised {
        offer_id: String,
        sponsor: String,
        parent_obligation: String,
        funding_source: FundingSource,
        task_scope: String,
        base_digest: String,
        intent_digest: String,
        artifact_class: String,
        dependencies: Vec<String>,
        capability_scope: Vec<String>,
        execution_escrow: BudgetVector,
        policy: OfferPolicy,
        bid_deadline: u64,
        offer_deadline: u64,
        max_awards: u32,
    },
    OfferWithdrawn {
        offer_id: String,
    },
    OfferSettled {
        offer_id: String,
    },
    BidRecorded {
        bid_id: String,
        offer_id: String,
        bidder: String,
        requested_escrow: BudgetVector,
        artifact_class: String,
        proposal_digest: Option<String>,
        expires_at: u64,
        origin: BidOrigin,
    },
    BidWithdrawn {
        bid_id: String,
    },
    TaskContractFormed {
        contract_id: String,
        offer_id: String,
        bid_id: String,
        sponsor: String,
        contractor: String,
        obligation_id: String,
        task_scope: String,
        base_digest: String,
    },
    ObligationCreated {
        obligation_id: String,
        parent: String,
        owner: String,
        contract_id: String,
    },
    LeaseIssued {
        contract_id: String,
        lease_id: String,
        holder: String,
        generation: u64,
        expires_at: u64,
    },
    LeaseRenewed {
        contract_id: String,
        generation: u64,
        expires_at: u64,
    },
    ContractReassigned {
        contract_id: String,
        bid_id: String,
        previous_holder: String,
        holder: String,
        generation: u64,
    },
    AttemptStarted {
        attempt_id: String,
        contract_id: String,
        participant: String,
        generation: u64,
    },
    SubmissionRecorded {
        contract_id: String,
        attempt_id: String,
        generation: u64,
        candidate_digest: String,
    },
    ObligationReturned {
        obligation_id: String,
        contract_id: String,
        /// Who closed the work, so that ownership can be audited from the facts alone.
        participant: String,
        generation: u64,
        outcome: Outcome,
    },
    ContractCancelled {
        contract_id: String,
        obligation_id: String,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CommitmentError {
    #[error("{kind} must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    InvalidIdentifier { kind: &'static str },
    #[error("{kind} is not a canonical lowercase SHA-256 digest")]
    InvalidDigest { kind: &'static str },
    #[error("{kind} {id} is already recorded")]
    DuplicateIdentifier { kind: &'static str, id: String },
    #[error("unknown {kind}: {id}")]
    Unknown { kind: &'static str, id: String },
    #[error("{principal} does not hold the authority to issue this command")]
    NotAuthorized { principal: String },
    #[error("offer {offer_id} is not open for consent")]
    OfferNotOpen { offer_id: String },
    #[error("offer {offer_id} is not settleable while it is still advertised before {deadline}")]
    OfferStillOpen { offer_id: String, deadline: u64 },
    #[error("offer {offer_id} closed at {deadline} and the clock reads {now}")]
    OfferExpired {
        offer_id: String,
        deadline: u64,
        now: u64,
    },
    #[error("bid {bid_id} expired at {deadline} and the clock reads {now}")]
    BidExpired {
        bid_id: String,
        deadline: u64,
        now: u64,
    },
    #[error("bid {bid_id} is not live consent")]
    BidNotLive { bid_id: String },
    #[error("bid {bid_id} names offer {named}, not offer {offer_id}")]
    BidOfferMismatch {
        bid_id: String,
        named: String,
        offer_id: String,
    },
    #[error("bid {bid_id} names an artifact class offer {offer_id} did not advertise")]
    ArtifactClassMismatch { bid_id: String, offer_id: String },
    #[error("offer {offer_id} funds less {dimension} per award than bid {bid_id} asks for")]
    EscrowExceedsOffer {
        offer_id: String,
        bid_id: String,
        dimension: Dimension,
    },
    #[error("the escrow of bid {bid_id} does not fund a {lease_ms} ms first lease")]
    LeaseNotFunded { bid_id: String, lease_ms: u64 },
    #[error("offer {offer_id} has already awarded all {max_awards} funded slots")]
    AwardsExhausted { offer_id: String, max_awards: u32 },
    #[error("offer {offer_id} admits consent only under a different policy")]
    PolicyMismatch { offer_id: String },
    #[error("offer {offer_id} is targeted at {participant_id}")]
    TargetMismatch {
        offer_id: String,
        participant_id: String,
    },
    #[error("account {account} holds no further {dimension}")]
    InsufficientBudget {
        account: String,
        dimension: Dimension,
    },
    #[error("budget arithmetic would overflow in {dimension}")]
    BudgetOverflow { dimension: Dimension },
    #[error(
        "account {account} does not hold the {dimension} the facts of this command move out of it"
    )]
    UncoveredDebit {
        account: String,
        dimension: Dimension,
    },
    #[error("task contract {contract_id} is not active")]
    ContractNotActive { contract_id: String },
    #[error("fencing generation {seen} is stale: task contract {contract_id} is at {current}")]
    StaleGeneration {
        contract_id: String,
        seen: u64,
        current: u64,
    },
    #[error("the lease on task contract {contract_id} expired at {expires_at}, clock reads {now}")]
    LeaseExpired {
        contract_id: String,
        expires_at: u64,
        now: u64,
    },
    #[error("the lease on task contract {contract_id} is live until {expires_at}")]
    LeaseStillLive {
        contract_id: String,
        expires_at: u64,
    },
    #[error("attempt {attempt_id} does not belong to task contract {contract_id}")]
    AttemptMismatch {
        attempt_id: String,
        contract_id: String,
    },
    #[error("obligation {obligation_id} still has an outstanding descendant: {descendant}")]
    DescendantOutstanding {
        obligation_id: String,
        descendant: String,
    },
    #[error("obligation {obligation_id} still parents unsettled offer {offer_id}")]
    OfferUnsettled {
        obligation_id: String,
        offer_id: String,
    },
    #[error("obligation {obligation_id} is already terminal")]
    ObligationTerminal { obligation_id: String },
    #[error("the clock cannot move from {now} back to {to}")]
    ClockRegression { now: u64, to: u64 },
    #[error("max_awards must be between 1 and {MAX_AWARDS}")]
    InvalidAwardCount,
    #[error("a lease must last between 1 and {MAX_LEASE_MS} milliseconds")]
    InvalidLeaseDuration,
    #[error("a bid deadline may not fall after its offer deadline")]
    InvalidDeadlines,
    #[error("{kind} may not exceed {MAX_SCOPE_ENTRIES} entries")]
    TooManyEntries { kind: &'static str },
    #[error("task contract {contract_id} does not fund this offer")]
    FundingContractMismatch { contract_id: String },
    #[error(
        "task contract {contract_id} reached a terminal state and is no longer an account that can be drawn on or settled into"
    )]
    AccountClosed { contract_id: String },
    #[error(
        "task contract {contract_id} still backs {outstanding}, whose reservation returns to it"
    )]
    ReservationOutstanding {
        contract_id: String,
        outstanding: String,
    },
    #[error("command serialization failed: {0}")]
    Serialization(String),
}
