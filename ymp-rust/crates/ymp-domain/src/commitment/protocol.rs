//! The typed commands a participant may issue, the facts they commit, and the typed refusals.
//!
//! A command carries identifiers, digests, integer quantities and deadlines. It carries no free
//! text: an intent or a proposal reaches the kernel only as the digest of an inert message, and a
//! digest is stored and compared, never read. There is consequently no field a transition could
//! consult to learn a skill, a model, a rank or how persuasive a bid is.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::budget::{BudgetVector, Dimension};
use super::candidates::{
    BundleChange, Contribution, MAX_BUNDLE_CHANGES, MAX_BUNDLE_PARENTS, MAX_PATH_BYTES,
};
use super::invocations::{InvocationClosure, StopReason, Verdict, WakeCondition};
use super::records::{AccountRef, BidOrigin, FundingSource, OfferPolicy, Outcome};
use crate::MAX_IDENTIFIER_CHARS;

/// The largest number of awards one offer may fund.
pub const MAX_AWARDS: u32 = 64;
/// The longest lease a single command may buy, in milliseconds.
pub const MAX_LEASE_MS: u64 = 24 * 60 * 60 * 1000;
/// The largest number of mechanically declared dependency or capability tokens on one offer.
pub const MAX_SCOPE_ENTRIES: usize = 64;
/// The largest number of typed conditions one yield may register. A yield that asked about
/// everything would be woken by everything, which is the same as not yielding.
pub const MAX_WAKE_CONDITIONS: usize = 16;
/// How many times the process slices of one attempt may be resumed in total. It is the bound that
/// makes a participant which keeps yielding on a condition it keeps meeting a finite thing.
pub const MAX_ATTEMPT_WAKES: u32 = 8;

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

/// State that an immutable object is stored whole, so that a bundle may name it.
///
/// The bytes are written and flushed before this is issued, and the kernel never opens them. What
/// the fact carries is that the object exists in full: an attempt that stopped part-way through
/// writing one never issued this command, so nothing it half-wrote can enter a result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordObject {
    pub contract_id: String,
    pub participant: String,
    pub generation: u64,
    pub object_digest: String,
}

/// Publish a bundle and form the immutable result it constructs.
///
/// The kernel checks the base, the fencing token, the completeness of every object named, and
/// whether the contributions this bundle carries forward disagree anywhere it says nothing about.
/// It computes the identity of the result rather than accepting one, so no caller can claim an
/// identifier for a construction it did not state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SubmitBundle {
    pub contract_id: String,
    pub attempt_id: String,
    pub participant: String,
    pub generation: u64,
    pub base_digest: String,
    /// The candidates this result carries forward: none for a first attempt, one for a rebase,
    /// several for a synthesis.
    pub parents: Vec<String>,
    pub changes: Vec<BundleChange>,
}

/// Record where named candidates put different bytes at the same path.
///
/// The paths are computed by the kernel from the candidates themselves; the command only names
/// which candidates to compare. Recording the disagreement decides nothing about it — it is the
/// evidence a participant funds an offer against.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordConflict {
    pub participant: String,
    pub candidates: Vec<String>,
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

/// Begin one supervised process slice under a live lease. The cursor states how far into the
/// committed facts the participant has already read, so that a wake registered later can only match
/// something it has not seen.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StartInvocation {
    pub invocation_id: String,
    pub attempt_id: String,
    pub contract_id: String,
    pub participant: String,
    pub generation: u64,
    pub cursor: u64,
}

/// End the process slice without returning the task contract, registering what would be worth
/// resuming for and until when.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct YieldInvocation {
    pub invocation_id: String,
    pub participant: String,
    pub generation: u64,
    /// How far the participant has read by the time it stops. It may only move forward.
    pub cursor: u64,
    pub conditions: Vec<WakeCondition>,
    pub wake_deadline: u64,
}

/// Admit a yielded slice back into a running process. It is the only transition that reads the
/// committed facts to decide: without one after the cursor that a registered condition names, there
/// is nothing to resume for.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResumeInvocation {
    pub invocation_id: String,
    pub participant: String,
    pub generation: u64,
}

/// End a process slice for good, whether the participant finished, stopped answering, or its wake
/// deadline passed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloseInvocation {
    pub invocation_id: String,
    /// The participant running the slice, or the sponsor of its task contract.
    pub closer: String,
    pub reason: InvocationClosure,
}

/// Spend one protected-query reservation on an exact candidate digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordVerification {
    pub contract_id: String,
    pub participant: String,
    pub generation: u64,
    pub candidate_digest: String,
    pub verdict: Verdict,
}

/// Stop the run. Nothing new may be created afterwards; what already exists still has to be wound
/// down, so the accounting closes rather than being abandoned.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StopRun {
    /// The participant that owns the root obligation. Nobody else may stop the run.
    pub authority: String,
    pub reason: StopReason,
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
    RecordObject(RecordObject),
    SubmitBundle(SubmitBundle),
    RecordConflict(RecordConflict),
    Reassign(Reassign),
    ReturnObligation(ReturnObligation),
    CancelContract(CancelContract),
    AdvanceClock(AdvanceClock),
    StartInvocation(StartInvocation),
    YieldInvocation(YieldInvocation),
    ResumeInvocation(ResumeInvocation),
    CloseInvocation(CloseInvocation),
    RecordVerification(RecordVerification),
    StopRun(StopRun),
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
    /// An immutable object is stored whole and may be named by a bundle.
    ObjectRecorded {
        object_digest: String,
    },
    /// The immutable content of one bundle. Publishing the same content twice states the same
    /// fact twice and adds nothing, because a bundle is its bytes.
    BundleRecorded {
        bundle_digest: String,
        base_digest: String,
        parents: Vec<String>,
        changes: Vec<BundleChange>,
    },
    /// One immutable result and everything needed to reproduce its construction: the base it was
    /// built from, the bundle that stated it, the objects standing at every changed path, and every
    /// candidate, obligation and participant that contributed.
    CandidateFormed {
        candidate_digest: String,
        content_digest: String,
        contract_id: String,
        obligation_id: String,
        participant: String,
        generation: u64,
        base_digest: String,
        bundle_digest: String,
        contributions: Vec<Contribution>,
        changes: Vec<BundleChange>,
    },
    /// Where named candidates put different bytes at the same path. It is evidence, and it
    /// resolves nothing.
    ConflictRecorded {
        conflict_digest: String,
        base_digest: String,
        candidates: Vec<String>,
        paths: Vec<String>,
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
    InvocationStarted {
        invocation_id: String,
        attempt_id: String,
        contract_id: String,
        participant: String,
        generation: u64,
        cursor: u64,
    },
    InvocationYielded {
        invocation_id: String,
        cursor: u64,
        conditions: Vec<WakeCondition>,
        wake_deadline: u64,
    },
    InvocationResumed {
        invocation_id: String,
        contract_id: String,
        participant: String,
        generation: u64,
        /// The sequence of the committed fact that authorized this resumption. It is part of the
        /// record so that every wake can be traced back to the exact fact it matched, instead of
        /// to a notification nobody kept.
        matched_sequence: u64,
        wakes_used: u32,
    },
    InvocationClosed {
        invocation_id: String,
        reason: InvocationClosure,
    },
    VerificationRecorded {
        contract_id: String,
        candidate_digest: String,
        verdict: Verdict,
        root_scope: bool,
    },
    RunStopped {
        authority: String,
        reason: StopReason,
    },
}

impl CommitmentEvent {
    /// The name this fact is recorded under, which is the tag its serialized form carries.
    ///
    /// A reader that shows a fact states this name rather than one of its own, so what an
    /// operator reads on a screen and what the journal holds cannot drift apart.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ClockAdvanced { .. } => "clock_advanced",
            Self::ParticipantRegistered { .. } => "participant_registered",
            Self::BudgetTransferred { .. } => "budget_transferred",
            Self::BudgetConsumed { .. } => "budget_consumed",
            Self::OfferAdvertised { .. } => "offer_advertised",
            Self::OfferWithdrawn { .. } => "offer_withdrawn",
            Self::OfferSettled { .. } => "offer_settled",
            Self::BidRecorded { .. } => "bid_recorded",
            Self::BidWithdrawn { .. } => "bid_withdrawn",
            Self::TaskContractFormed { .. } => "task_contract_formed",
            Self::ObligationCreated { .. } => "obligation_created",
            Self::LeaseIssued { .. } => "lease_issued",
            Self::LeaseRenewed { .. } => "lease_renewed",
            Self::ContractReassigned { .. } => "contract_reassigned",
            Self::AttemptStarted { .. } => "attempt_started",
            Self::SubmissionRecorded { .. } => "submission_recorded",
            Self::ObjectRecorded { .. } => "object_recorded",
            Self::BundleRecorded { .. } => "bundle_recorded",
            Self::CandidateFormed { .. } => "candidate_formed",
            Self::ConflictRecorded { .. } => "conflict_recorded",
            Self::ObligationReturned { .. } => "obligation_returned",
            Self::ContractCancelled { .. } => "contract_cancelled",
            Self::InvocationStarted { .. } => "invocation_started",
            Self::InvocationYielded { .. } => "invocation_yielded",
            Self::InvocationResumed { .. } => "invocation_resumed",
            Self::InvocationClosed { .. } => "invocation_closed",
            Self::VerificationRecorded { .. } => "verification_recorded",
            Self::RunStopped { .. } => "run_stopped",
        }
    }
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
    #[error("invocation {invocation_id} is not running")]
    InvocationNotRunning { invocation_id: String },
    #[error("invocation {invocation_id} is not yielded")]
    InvocationNotYielded { invocation_id: String },
    #[error("a cursor may not move from {current} back to {seen}")]
    CursorRegression { current: u64, seen: u64 },
    #[error("cursor {seen} names a fact that has not been committed; the run is at {committed}")]
    CursorAhead { seen: u64, committed: u64 },
    #[error("a yield may register between 1 and {MAX_WAKE_CONDITIONS} typed conditions")]
    InvalidWakeConditions,
    #[error(
        "a wake deadline of {wake_deadline} outlives the lease on task contract {contract_id}, \
         which is funded to {expires_at}"
    )]
    WakeDeadlineUnfunded {
        contract_id: String,
        wake_deadline: u64,
        expires_at: u64,
    },
    #[error(
        "the wake deadline of invocation {invocation_id} passed at {wake_deadline}, clock reads {now}"
    )]
    WakeDeadlinePassed {
        invocation_id: String,
        wake_deadline: u64,
        now: u64,
    },
    #[error("attempt {attempt_id} has used all {MAX_ATTEMPT_WAKES} of its wakes")]
    WakeBudgetExhausted { attempt_id: String },
    #[error("attempt {attempt_id} is already running invocation {invocation_id}")]
    AttemptAlreadyRunning {
        attempt_id: String,
        invocation_id: String,
    },
    #[error(
        "no committed fact after cursor {cursor} matches a wake condition of invocation {invocation_id}"
    )]
    NoMatchingEvent { invocation_id: String, cursor: u64 },
    #[error("task contract {contract_id} carries no candidate to verify")]
    NoCandidate { contract_id: String },
    #[error(
        "candidate {candidate_digest} is not the candidate task contract {contract_id} recorded"
    )]
    CandidateMismatch {
        contract_id: String,
        candidate_digest: String,
    },
    #[error(
        "the bundle names base {actual}, and task contract {contract_id} was awarded from base {expected}"
    )]
    StaleBase {
        contract_id: String,
        expected: String,
        actual: String,
    },
    #[error(
        "candidate {candidate_digest} was built from base {base_digest}, which is another base"
    )]
    DivergentBase {
        candidate_digest: String,
        base_digest: String,
    },
    #[error("object {object_digest} is not recorded as stored whole in this run")]
    ObjectIncomplete { object_digest: String },
    #[error(
        "task contract {contract_id} already recorded candidate {current}, which {proposed} may not replace"
    )]
    CandidateSealed {
        contract_id: String,
        current: String,
        proposed: String,
    },
    #[error(
        "the contributing candidates disagree at {} path(s) this bundle states nothing about: {}",
        .paths.len(),
        .paths.join(", ")
    )]
    IntegrationConflict { paths: Vec<String> },
    #[error("the named candidates put the same bytes at every path they state")]
    NoConflict,
    #[error("a disagreement is recorded over between 2 and {MAX_BUNDLE_PARENTS} candidates")]
    InvalidConflictScope,
    #[error("a bundle carries between 0 and {MAX_BUNDLE_CHANGES} path changes")]
    InvalidBundleSize,
    #[error("{path} is not a normalized relative path of at most {MAX_PATH_BYTES} bytes")]
    InvalidPath { path: String },
    #[error("the run was stopped and creates nothing further")]
    RunStopped,
    #[error("the run has already been stopped")]
    RunAlreadyStopped,
    #[error("command serialization failed: {0}")]
    Serialization(String),
}
