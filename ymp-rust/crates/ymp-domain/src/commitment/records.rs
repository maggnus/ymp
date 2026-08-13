//! The durable shape of a local commitment: participants, offers, consent, task contracts,
//! escrow accounts, leases and work obligations.
//!
//! Every field here is mechanical. Identifiers are opaque tokens the kernel compares for equality;
//! digests name immutable content the kernel never opens; quantities are integer smallest units.
//! Nothing states what work means, how good a participant is, or how convincing a bid reads —
//! those live in inert collaboration messages that only appear here as a digest.

use serde::{Deserialize, Serialize};

use super::budget::BudgetVector;

/// Which account a debit or credit names.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "account", rename_all = "snake_case")]
pub enum AccountRef {
    Participant { participant_id: String },
    Offer { offer_id: String },
    TaskContract { contract_id: String },
}

/// Which account funds an offer: a participant's own balance, or escrow it holds under a task
/// contract it is currently the contractor of.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "funding", rename_all = "snake_case")]
pub enum FundingSource {
    Participant,
    TaskContract { contract_id: String },
}

/// How consent may be given on an offer. The sponsor chooses it; the kernel validates structure
/// and resources, never whether the choice is wise.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum OfferPolicy {
    /// Bids are recorded, and the sponsor names the one it awards.
    Negotiated,
    /// The sponsor has pre-authorized the first mechanically valid acceptance.
    OpenAccept,
    /// Only the named participant may consent. It is still consent: a targeted offer compels
    /// nothing.
    Targeted { participant_id: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferState {
    Advertised,
    Withdrawn,
    Settled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BidState {
    Live,
    Withdrawn,
    Awarded,
}

/// Whether consent was recorded as a bid awaiting an award, or as an acceptance of a
/// pre-authorized open offer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BidOrigin {
    Bid,
    OpenAccept,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractState {
    Active,
    Returned,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationState {
    Active,
    Terminal,
}

/// How outstanding causal work ended. A failed or empty return still closes the work; it does not
/// make the parent successful, and the kernel never reads it to decide anything else.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Result { candidate_digest: String },
    DeadEnd,
    Declined,
    Exhausted,
    Cancelled,
    InfrastructureError,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Running,
    Closed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParticipantRecord {
    pub participant_id: String,
    /// The authenticated principal this participant acts as. Rate and admission rules inspect it
    /// as an identifier and nothing more.
    pub principal_id: String,
    pub balance: BudgetVector,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OfferRecord {
    pub offer_id: String,
    pub sponsor: String,
    pub parent_obligation: String,
    pub funding_source: FundingSource,
    /// An opaque scope token. Equality is the only operation performed on it.
    pub task_scope: String,
    pub base_digest: String,
    /// The digest of the inert intent message. The kernel stores it and never opens it.
    pub intent_digest: String,
    /// An opaque class token that consent must match exactly.
    pub artifact_class: String,
    pub dependencies: Vec<String>,
    pub capability_scope: Vec<String>,
    /// What one award transfers at most.
    pub execution_escrow: BudgetVector,
    /// What the offer still holds for its unawarded slots and unspent slack.
    pub escrow: BudgetVector,
    pub policy: OfferPolicy,
    pub bid_deadline: u64,
    pub offer_deadline: u64,
    pub max_awards: u32,
    pub awards_made: u32,
    pub state: OfferState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BidRecord {
    pub bid_id: String,
    pub offer_id: String,
    pub bidder: String,
    /// The resource counter-offer. It is compared against what the offer funds, and against
    /// nothing else — never against another bid.
    pub requested_escrow: BudgetVector,
    pub artifact_class: String,
    /// The digest of an optional inert proposal message.
    pub proposal_digest: Option<String>,
    pub expires_at: u64,
    pub origin: BidOrigin,
    pub state: BidState,
}

/// The temporary right to advance one task contract, with the fencing token every state-changing
/// command must repeat.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LeaseRecord {
    pub lease_id: String,
    pub holder: String,
    pub generation: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskContractRecord {
    pub contract_id: String,
    pub offer_id: String,
    pub bid_id: String,
    /// Sponsor and contractor are temporary relations scoped to this contract alone.
    pub sponsor: String,
    pub contractor: String,
    pub obligation_id: String,
    pub task_scope: String,
    pub base_digest: String,
    /// Escrow held for this contract and drawn on by its holder.
    pub escrow: BudgetVector,
    pub lease: LeaseRecord,
    pub candidate_digest: Option<String>,
    pub state: ContractState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObligationRecord {
    pub obligation_id: String,
    pub parent: Option<String>,
    pub owner: String,
    pub contract_id: Option<String>,
    pub children: Vec<String>,
    pub state: ObligationState,
    pub outcome: Option<Outcome>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttemptRecord {
    pub attempt_id: String,
    pub contract_id: String,
    pub participant: String,
    /// The fencing generation this attempt was started under.
    pub generation: u64,
    pub state: AttemptState,
}
