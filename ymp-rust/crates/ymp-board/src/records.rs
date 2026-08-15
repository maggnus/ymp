//! The durable shape of a collaboration record: scopes, audiences, grants, messages, delivery
//! cursors, blinded assessments, and the observations the control plane mirrors here for reading.
//!
//! Every field is mechanical. Identifiers are opaque tokens compared for equality, digests name
//! content this plane never opens, quantities are integer smallest units, and deadlines are
//! compared against a clock. Nothing here states what a message means, how good it is or whether it
//! should be believed, because nothing in this plane is allowed to decide any of that.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::budget::CommunicationAllowance;

/// Which account a charge or transfer names.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "account", rename_all = "snake_case")]
pub enum AccountRef {
    Participant {
        participant_id: String,
    },
    /// A live grant, which holds the concurrent membership it was funded with until it ends.
    Grant {
        grant_id: String,
    },
}

/// What a run's liveness owes to this plane.
///
/// The type has no variants, and that is the statement. A run is kept alive by funded control
/// objects — an attempt, an unexpired task contract, an open obligation, a pending query — and a
/// board holds none of them. An unread message, a stale help request and a projection nobody
/// refreshed are not control objects and never become one, so the honest return of
/// [`crate::BoardLedger::run_keeping_authority`] is one this type makes the only possible return.
/// Giving the board such an object would mean adding a variant here, which is a change to a public
/// type rather than a quiet one inside a function.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RunKeepingAuthority {}

/// What a scope is for. Both are detailed audiences; they differ in the reveal order that applies
/// to them.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    /// The audience of one task, whose minimum membership a task contract supplies.
    Task,
    /// The audience of one candidate review, which may obey a stricter reveal order.
    CandidateReview,
}

/// The reveal order a candidate-review scope was opened under.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewPolicy {
    /// The result under review. An opaque token compared for equality.
    pub candidate_digest: String,
    /// Whether a reviewer must commit its own assessment before any board content is disclosed to
    /// it.
    pub blinded: bool,
    /// The scopes withheld from a blinded reviewer until its first assessment is committed. The
    /// producing task scope belongs here: a rationale read before the commitment is exactly the
    /// disclosure the order exists to prevent.
    pub withheld_scopes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScopeRecord {
    pub scope_id: String,
    pub kind: ScopeKind,
    /// The participant that may grant and decline membership of this scope. It is the local scope
    /// sponsor and nothing wider: sponsorship of one scope confers nothing anywhere else.
    pub sponsor: String,
    pub opened_at: u64,
    pub review_policy: Option<ReviewPolicy>,
    /// Where each reviewer of this scope stands in the reveal order.
    pub reviewers: BTreeMap<String, ReviewerRecord>,
}

/// Who a message is addressed to.
///
/// Discovery is bounded and open to every registered participant; the other two are detailed and
/// reachable only through an explicit unexpired grant. `Named` narrows a detailed audience further
/// and never widens one: its recipients must already be admitted to the scope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "audience", rename_all = "snake_case")]
pub enum Audience {
    ProjectDiscovery,
    Scope {
        scope_id: String,
    },
    Named {
        scope_id: String,
        recipients: Vec<String>,
    },
}

impl Audience {
    /// The scope this audience is inside, if it is a detailed one.
    pub fn scope_id(&self) -> Option<&str> {
        match self {
            Self::ProjectDiscovery => None,
            Self::Scope { scope_id } | Self::Named { scope_id, .. } => Some(scope_id),
        }
    }

    pub const fn is_discovery(&self) -> bool {
        matches!(self, Self::ProjectDiscovery)
    }
}

/// What a grant permits. Reading and publishing are separate, so a participant may be admitted to
/// listen without being admitted to speak.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Rights {
    pub read: bool,
    pub publish: bool,
}

impl Rights {
    pub const READ: Self = Self {
        read: true,
        publish: false,
    };
    pub const READ_AND_PUBLISH: Self = Self {
        read: true,
        publish: true,
    };
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantState {
    Live,
    /// The holder relinquished it. Leaving changes no task obligation.
    Released,
    /// Its expiry passed. A grant is never renewed in place; a further one is granted and spends
    /// membership authority again.
    Expired,
}

/// One explicit, expiring admission to one scope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GrantRecord {
    pub grant_id: String,
    pub scope_id: String,
    pub participant: String,
    pub rights: Rights,
    /// The scope sponsor that granted it, and the account the concurrent membership returns to.
    pub granted_by: String,
    pub expires_at: u64,
    pub state: GrantState,
    /// The concurrent membership this grant holds while it is live.
    pub held: CommunicationAllowance,
}

impl GrantRecord {
    /// Whether this grant admits anything at `now`. An expiry that has arrived admits nothing, and
    /// no read is answered from a grant that is not live.
    pub const fn is_live_at(&self, now: u64) -> bool {
        matches!(self.state, GrantState::Live) && now < self.expires_at
    }
}

/// What a participant recorded a message as. The kernel stores the kind and never reads it to
/// decide anything: it is how an author labels its own contribution for a reader.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Proposal,
    Question,
    Hypothesis,
    Observation,
    Constraint,
    DeadEnd,
    Challenge,
    Confirmation,
    Decision,
    HelpRequest,
}

impl MessageKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Question => "question",
            Self::Hypothesis => "hypothesis",
            Self::Observation => "observation",
            Self::Constraint => "constraint",
            Self::DeadEnd => "dead_end",
            Self::Challenge => "challenge",
            Self::Confirmation => "confirmation",
            Self::Decision => "decision",
            Self::HelpRequest => "help_request",
        }
    }
}

/// What a message points at. Every variant is a token this plane stores and compares; quoting a
/// control record here discusses it and exercises nothing, because exercising it needs an
/// authenticated command in a plane this crate cannot reach.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "reference", rename_all = "snake_case")]
pub enum Reference {
    Message { message_id: String },
    Artifact { object_digest: String },
    Candidate { candidate_digest: String },
    ControlRecord { record_id: String },
}

/// How a message stands to an earlier one.
///
/// The relation is structural and is what the observatory draws its edges from. It states that an
/// author replied, disagreed, changed its own stated position, or paid to keep an earlier finding
/// salient — never that any of those changed what anyone else did.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "relation", rename_all = "snake_case")]
pub enum Relation {
    Standalone,
    ReplyTo {
        message_id: String,
    },
    /// A challenge to an earlier message. Recording it settles nothing and closes nothing: an
    /// unresolved disagreement stays visible as one.
    Challenges {
        message_id: String,
    },
    /// A revision of the author's own earlier message. The earlier record stays exactly as it was,
    /// because history is never rewritten here.
    Revises {
        message_id: String,
    },
    /// A refresh of an earlier message's salience. It is a new attributed message, published at a
    /// new cost.
    Refreshes {
        message_id: String,
    },
}

impl Relation {
    /// The earlier message this one stands to, if any.
    pub fn target(&self) -> Option<&str> {
        match self {
            Self::Standalone => None,
            Self::ReplyTo { message_id }
            | Self::Challenges { message_id }
            | Self::Revises { message_id }
            | Self::Refreshes { message_id } => Some(message_id),
        }
    }
}

/// One appended message. The record is permanent; only its active salience expires.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MessageRecord {
    pub schema_version: u32,
    /// Board-local publication order, counted from one. It is reliable publication order and no
    /// more: it does not state that an earlier message caused a later one.
    pub sequence: u64,
    pub message_id: String,
    pub author: String,
    pub audience: Audience,
    pub kind: MessageKind,
    /// The identity of the inert payload. The board holds this and the length, never the bytes.
    pub payload_digest: String,
    pub payload_bytes: u64,
    pub published_at: u64,
    /// When the active projection stops carrying it. The record itself never expires.
    pub salience_expires_at: u64,
    pub references: Vec<Reference>,
    pub relation: Relation,
    /// The earlier published evidence a decision states it used. It is the author's claim about
    /// its own reasoning and is recorded as a claim.
    pub claimed_decision_basis: Vec<String>,
}

impl MessageRecord {
    /// Whether the active projection still carries this message at `now`.
    pub const fn is_salient_at(&self, now: u64) -> bool {
        now < self.salience_expires_at
    }
}

/// How far one reader has been delivered.
///
/// The cursor is the authority a lagging reader recovers from. A notification channel may coalesce,
/// delay or drop what it carries without any of that being recoverable; what a reader missed is
/// whatever lies after the cursor it last recorded, and that question is answered from here.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReaderRecord {
    pub cursor: u64,
    pub delivered_bytes: u64,
}

/// One recorded delivery. It states that bytes were made available to a reader at a moment, and
/// nothing about whether they were read, believed or understood.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeliveryReceipt {
    pub reader: String,
    pub from_cursor: u64,
    pub to_cursor: u64,
    pub message_ids: Vec<String>,
    pub bytes: u64,
    pub delivered_at: u64,
}

/// Where a reviewer stands in a blinded reveal order.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerState {
    /// Admitted to the review and not yet committed. Nothing on the board is delivered to it.
    Blinded,
    /// Its first assessment is durable. Context may now be revealed under the review policy.
    Committed,
    /// Context was revealed. Every assessment from here on is a second one.
    Disclosed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewerRecord {
    pub participant: String,
    pub state: ReviewerState,
    /// The assessments this reviewer committed, in order.
    pub assessments: Vec<String>,
}

/// Whether an assessment may stand in the primary comparison.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Admissibility {
    /// Committed under blinding, before any board content, other assessment or producer rationale
    /// was available to its author.
    Primary,
    /// Committed after disclosure, or after the author had already committed once, or under a
    /// policy that was not blinded. It stays attributable and stays out of the primary comparison.
    Secondary,
}

/// One committed assessment digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssessmentRecord {
    pub assessment_id: String,
    pub scope_id: String,
    pub reviewer: String,
    /// The digest of the bounded assessment. The board stores it and never opens it.
    pub assessment_digest: String,
    pub admissibility: Admissibility,
    pub committed_at: u64,
    /// Board-local order of the commitment, so that a later reveal cannot be presented as earlier.
    pub sequence: u64,
}

/// The verdict of one protected query, as the control plane reported it.
///
/// This plane produces no verdict. The value is mirrored here so that a reader can see a result
/// beside the conversation about it, and it is an observation in both directions: nothing on the
/// board changes it, and it changes nothing on the board.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedVerdict {
    Passed,
    Failed,
    InfrastructureError,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerdictRecord {
    pub candidate_digest: String,
    pub verdict: ObservedVerdict,
    pub observed_at: u64,
}

/// The candidates one result carries forward, as the control plane recorded them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AncestryRecord {
    pub candidate_digest: String,
    pub parents: Vec<String>,
    pub observed_at: u64,
}

/// What a controlled intervention did to one message.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InterventionKind {
    /// The message was withheld from the receiver.
    Removal,
    /// A neutral payload of comparable size stood in its place.
    Replacement,
    /// A message from another task or sender stood in its place.
    Shuffle,
    /// The sender was removed from the run.
    ParticipantLoss,
}

/// One recorded controlled intervention: the only ground on which this plane will label anything
/// causal.
///
/// Order and citation are association. What separates listening from either is repeating the
/// episode with the message changed, under the same total budget, and observing that the receiver's
/// later action distribution changed. The record therefore states what was changed, for whom, how
/// many times it was replicated, and whether the replications ran under a matched budget — and a
/// reading that fails any of those is evidence of an association and is presented as one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InterventionRecord {
    pub intervention_id: String,
    pub kind: InterventionKind,
    /// The message the intervention changed.
    pub subject_message: String,
    /// The participant whose later actions were compared.
    pub receiver: String,
    /// How many stochastic replications the comparison used.
    pub replications: u32,
    /// Whether every replication ran under the same total resource budget.
    pub matched_budget: bool,
    pub recorded_at: u64,
}

/// One participant's board account.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccountRecord {
    pub participant_id: String,
    pub balance: CommunicationAllowance,
}

/// One recorded request for admission. It is inert like everything else here: the sponsor may grant
/// it, decline it or ignore it, and the request compels none of the three.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudienceRequestRecord {
    pub request_id: String,
    pub scope_id: String,
    pub participant: String,
    pub requested_at: u64,
}
