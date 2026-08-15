//! The typed commands this plane accepts, the facts they commit, and the typed refusals.
//!
//! A command carries identifiers, digests, integer quantities and deadlines. The one thing no
//! command carries is content: a payload reaches the board as a [`Payload`] — an identity and a
//! length — so there is no field a transition could read to learn what was said, and therefore no
//! field that could ask for anything.
//!
//! Three commands are the controller's rather than a participant's, and they are marked by naming
//! the controller as their subject: the two that mirror what the control plane recorded about a
//! result, and the one that records a controlled intervention. Mirroring is one-way. It brings an
//! observation in for reading and takes nothing back out, because there is no way out of this crate
//! to take it through.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Payload;
use crate::budget::{Allowance, CommunicationAllowance};
use crate::records::{
    AccountRef, Audience, InterventionKind, MessageKind, ObservedVerdict, Reference, Relation,
    ReviewPolicy, Rights, ScopeKind,
};

/// One member a scope is opened with, and the grant identifier its admission is recorded under.
///
/// A task contract supplies the minimum sponsor–contractor membership automatically, and this is
/// how that arrives: as ordinary expiring grants that spend ordinary membership authority. Automatic
/// is not free, and it is not permanent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InitialMember {
    pub grant_id: String,
    pub participant: String,
    pub rights: Rights,
}

/// Open a board account for a participant the control plane already created.
///
/// This plane creates no participant. What it does is fund one to talk, out of the capacity of a
/// participant that already holds some.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegisterParticipant {
    pub participant_id: String,
    pub sponsor: String,
    pub endowment: CommunicationAllowance,
}

/// Open a detailed audience, with the membership its opening supplies.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OpenScope {
    /// The controller. A detailed audience exists because the control plane recorded the work it
    /// belongs to; a participant cannot conjure one for itself.
    pub controller: String,
    pub scope_id: String,
    pub kind: ScopeKind,
    /// The local scope sponsor: the participant that may admit and decline afterwards, and whose
    /// allowance funds the membership opened here.
    pub sponsor: String,
    pub review_policy: Option<ReviewPolicy>,
    pub initial_members: Vec<InitialMember>,
    /// When the memberships opened here expire. They expire like any other.
    pub member_expires_at: u64,
}

/// Ask a scope sponsor for admission. Recording the request compels nothing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestAudience {
    pub request_id: String,
    pub scope_id: String,
    pub participant: String,
}

/// Admit a participant to one scope, with bounded rights, until a stated moment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GrantAudience {
    pub grant_id: String,
    pub scope_id: String,
    pub sponsor: String,
    pub participant: String,
    pub rights: Rights,
    pub expires_at: u64,
}

/// Relinquish an admission. It changes no obligation and returns the concurrent membership to the
/// account that funded it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LeaveAudience {
    pub grant_id: String,
    pub participant: String,
}

/// Append one attributed, bounded, inert message to an audience the author may publish to.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Publish {
    pub message_id: String,
    pub author: String,
    pub audience: Audience,
    pub kind: MessageKind,
    /// The identity and length of the payload. Its bytes are not here and are read by nothing in
    /// this crate.
    pub payload: Payload,
    /// How long the message stays in the active projection. The record outlives it.
    pub salience_ms: u64,
    pub references: Vec<Reference>,
    pub relation: Relation,
    pub claimed_decision_basis: Vec<String>,
}

/// Publish an earlier message again so that it stays salient. It is a new attributed message with
/// a new cost; it rewrites nothing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RefreshSalience {
    pub message_id: String,
    /// The earlier message whose salience is being bought again.
    pub refreshes: String,
    pub author: String,
    pub salience_ms: u64,
}

/// Take delivery of what lies after this reader's cursor, bounded by bytes, and advance only this
/// reader's cursor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReadBoard {
    pub reader: String,
    pub limit_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdvanceClock {
    pub to: u64,
}

/// Commit the digest of a bounded independent assessment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommitAssessment {
    pub assessment_id: String,
    pub scope_id: String,
    pub reviewer: String,
    pub assessment_digest: String,
}

/// Reveal the later context the review policy allows, which is only possible once a first
/// assessment is durable.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RevealContext {
    pub scope_id: String,
    pub reviewer: String,
}

/// Mirror the ancestry the control plane recorded for a result, for reading beside the
/// conversation about it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NoteAncestry {
    pub controller: String,
    pub candidate_digest: String,
    pub parents: Vec<String>,
}

/// Mirror the verdict the control plane recorded for a result. This plane produces no verdict and
/// changes none.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NoteVerdict {
    pub controller: String,
    pub candidate_digest: String,
    pub verdict: ObservedVerdict,
}

/// Record one controlled intervention, which is the only ground on which anything here is labelled
/// causal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordIntervention {
    pub controller: String,
    pub intervention_id: String,
    pub kind: InterventionKind,
    pub subject_message: String,
    pub receiver: String,
    pub replications: u32,
    pub matched_budget: bool,
}

/// One typed effect requested against the board.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum BoardCommand {
    RegisterParticipant(RegisterParticipant),
    OpenScope(OpenScope),
    RequestAudience(RequestAudience),
    GrantAudience(GrantAudience),
    LeaveAudience(LeaveAudience),
    Publish(Publish),
    RefreshSalience(RefreshSalience),
    ReadBoard(ReadBoard),
    AdvanceClock(AdvanceClock),
    CommitAssessment(CommitAssessment),
    RevealContext(RevealContext),
    NoteAncestry(NoteAncestry),
    NoteVerdict(NoteVerdict),
    RecordIntervention(RecordIntervention),
}

impl BoardCommand {
    /// The digest of the exact command content, which is what makes a retry recognizable.
    pub fn digest(&self) -> Result<String, serde_json::Error> {
        Ok(crate::digest_bytes(&serde_json::to_vec(self)?))
    }
}

/// One committed board fact. A command commits either all of its facts or none of them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BoardEvent {
    ClockAdvanced {
        to: u64,
    },
    ParticipantRegistered {
        participant_id: String,
    },
    /// Communication capacity changing hands. It is the only way a board account gains anything.
    AllowanceTransferred {
        from: AccountRef,
        to: AccountRef,
        amount: CommunicationAllowance,
    },
    /// Communication capacity leaving the accounts for good.
    AllowanceConsumed {
        account: AccountRef,
        amount: CommunicationAllowance,
    },
    ScopeOpened {
        scope_id: String,
        kind: ScopeKind,
        sponsor: String,
        review_policy: Option<ReviewPolicy>,
    },
    AudienceRequested {
        request_id: String,
        scope_id: String,
        participant: String,
    },
    AudienceGranted {
        grant_id: String,
        scope_id: String,
        participant: String,
        rights: Rights,
        granted_by: String,
        expires_at: u64,
    },
    AudienceReleased {
        grant_id: String,
    },
    AudienceExpired {
        grant_id: String,
    },
    MessagePublished {
        message_id: String,
        author: String,
        audience: Audience,
        kind: MessageKind,
        payload_digest: String,
        payload_bytes: u64,
        published_at: u64,
        salience_expires_at: u64,
        references: Vec<Reference>,
        relation: Relation,
        claimed_decision_basis: Vec<String>,
    },
    /// A receipt: these bytes were made available to this reader, and its cursor moved this far.
    /// It states nothing about reading, belief or understanding.
    DeliveryRecorded {
        reader: String,
        from_cursor: u64,
        to_cursor: u64,
        message_ids: Vec<String>,
        bytes: u64,
    },
    AssessmentCommitted {
        assessment_id: String,
        scope_id: String,
        reviewer: String,
        assessment_digest: String,
        admissibility: crate::records::Admissibility,
    },
    ContextRevealed {
        scope_id: String,
        reviewer: String,
    },
    AncestryNoted {
        candidate_digest: String,
        parents: Vec<String>,
    },
    VerdictNoted {
        candidate_digest: String,
        verdict: ObservedVerdict,
    },
    InterventionRecorded {
        intervention_id: String,
        kind: InterventionKind,
        subject_message: String,
        receiver: String,
        replications: u32,
        matched_budget: bool,
    },
}

impl BoardEvent {
    /// The name this fact is recorded under, which is the tag its serialized form carries. A reader
    /// that shows a fact states this name rather than one of its own.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ClockAdvanced { .. } => "clock_advanced",
            Self::ParticipantRegistered { .. } => "participant_registered",
            Self::AllowanceTransferred { .. } => "allowance_transferred",
            Self::AllowanceConsumed { .. } => "allowance_consumed",
            Self::ScopeOpened { .. } => "scope_opened",
            Self::AudienceRequested { .. } => "audience_requested",
            Self::AudienceGranted { .. } => "audience_granted",
            Self::AudienceReleased { .. } => "audience_released",
            Self::AudienceExpired { .. } => "audience_expired",
            Self::MessagePublished { .. } => "message_published",
            Self::DeliveryRecorded { .. } => "delivery_recorded",
            Self::AssessmentCommitted { .. } => "assessment_committed",
            Self::ContextRevealed { .. } => "context_revealed",
            Self::AncestryNoted { .. } => "ancestry_noted",
            Self::VerdictNoted { .. } => "verdict_noted",
            Self::InterventionRecorded { .. } => "intervention_recorded",
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BoardError {
    #[error(
        "{kind} must contain between 1 and {} characters",
        crate::MAX_IDENTIFIER_CHARS
    )]
    InvalidIdentifier { kind: &'static str },
    #[error("{kind} is not a canonical lowercase SHA-256 digest")]
    InvalidDigest { kind: &'static str },
    #[error("{kind} {id} is already recorded")]
    DuplicateIdentifier { kind: &'static str, id: String },
    #[error("unknown {kind}: {id}")]
    Unknown { kind: &'static str, id: String },
    #[error("{participant} does not hold the authority to issue this command")]
    NotAuthorized { participant: String },
    #[error("{participant} holds no live grant on scope {scope_id}")]
    NotAdmitted {
        participant: String,
        scope_id: String,
    },
    #[error("grant {grant_id} expired at {expires_at} and the clock reads {now}")]
    GrantExpired {
        grant_id: String,
        expires_at: u64,
        now: u64,
    },
    #[error("grant {grant_id} is no longer live")]
    GrantNotLive { grant_id: String },
    #[error("{participant} is admitted to scope {scope_id} to read and not to publish")]
    PublishNotPermitted {
        participant: String,
        scope_id: String,
    },
    #[error("recipient {participant} holds no live grant on scope {scope_id}")]
    RecipientNotAdmitted {
        participant: String,
        scope_id: String,
    },
    #[error("a payload of {bytes} bytes exceeds the {limit} this audience admits")]
    PayloadTooLarge { bytes: u64, limit: u64 },
    #[error("{kind} may not exceed {limit} entries")]
    TooManyEntries { kind: &'static str, limit: usize },
    #[error(
        "salience must last between 1 and {} milliseconds",
        crate::MAX_SALIENCE_MS
    )]
    InvalidSalience,
    #[error(
        "a grant must expire between the current moment and {} milliseconds after it",
        crate::MAX_GRANT_MS
    )]
    InvalidGrantWindow,
    #[error(
        "a delivery must be bounded by between 1 and {} bytes",
        crate::MAX_DELIVERY_BYTES
    )]
    InvalidDeliveryLimit,
    #[error("message {message_id} is not one this author is admitted to read")]
    ReferenceNotAdmitted { message_id: String },
    #[error(
        "message {message_id} stands in another audience than the one this message is addressed to"
    )]
    RelationOutsideAudience { message_id: String },
    #[error("a challenge names the message it challenges, and only a challenge does")]
    ChallengeRelationMismatch,
    #[error(
        "message {message_id} was published by {author}, so it is not this author's own position to revise"
    )]
    RevisionOfAnother { message_id: String, author: String },
    #[error("only a decision states the earlier evidence it claims to have used")]
    DecisionBasisOnOtherKind,
    #[error("a refresh is issued as a refresh, not published as an ordinary message")]
    RefreshIsNotPublication,
    #[error("account {account} holds no further {allowance}")]
    InsufficientAllowance {
        account: String,
        allowance: Allowance,
    },
    #[error("communication accounting would overflow in {allowance}")]
    AllowanceOverflow { allowance: Allowance },
    #[error(
        "account {account} does not hold the {allowance} the facts of this command move out of it"
    )]
    UncoveredDebit {
        account: String,
        allowance: Allowance,
    },
    #[error("the clock cannot move from {now} back to {to}")]
    ClockRegression { now: u64, to: u64 },
    #[error(
        "the reveal order of scope {scope_id} has not disclosed the board to {reviewer}, which happens only after its own assessment is durable"
    )]
    BoardWithheldByReviewOrder { scope_id: String, reviewer: String },
    #[error("scope {scope_id} was not opened as a candidate review")]
    NotUnderReview { scope_id: String },
    #[error(
        "the review policy of scope {scope_id} withholds nothing, so there is no context to reveal"
    )]
    ReviewNotBlinded { scope_id: String },
    #[error(
        "scope {scope_id} withholds itself, so it may not also be listed among the scopes it withholds"
    )]
    InvalidReviewPolicy { scope_id: String },
    #[error("command serialization failed: {0}")]
    Serialization(String),
}
