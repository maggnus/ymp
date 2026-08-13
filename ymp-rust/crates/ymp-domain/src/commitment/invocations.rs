//! The durable shape of a supervised process slice: what a participant yields, what may wake it,
//! and how a run stops.
//!
//! An invocation is the only place in this module where a participant is running. Everything else
//! here exists so that it can stop running without the run losing track of it: a cursor into the
//! committed facts, a bounded set of conditions naming facts that would be worth resuming for, and
//! a deadline after which the wake is no longer paid for.
//!
//! A wake condition names a typed committed fact and an identifier. There is no field here that
//! could carry a sentence, a topic, an urgency or a request to be answered, so no reading of a
//! message can make one condition match: what matches is a fact the kernel itself committed, and
//! the sequence of that fact is recorded in the resumption so the authority for every wake can be
//! read back out of the stream.

use serde::{Deserialize, Serialize};

use super::protocol::CommitmentEvent;

/// Where an invocation is in its life. A yielded invocation has no process: it holds a cursor, a
/// wake registration and its lease, and nothing else.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationState {
    Running,
    Yielded,
    Closed,
}

/// Why a process slice ended for good. The kernel stores the reason and never reads it to decide
/// anything: what it decides from is that the invocation is closed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationClosure {
    /// The runtime ended after an accepted explicit task action. It is not candidate acceptance.
    Completed,
    /// The wake deadline passed while the invocation was still yielded.
    WakeDeadlineExpired,
    /// An authorized sponsor stopped this slice.
    Cancelled,
    /// The participant stopped answering and its sponsor recorded the loss.
    ParticipantLost,
    /// The task contract this slice belonged to closed or changed hands under it, so there is
    /// nothing left for it to advance.
    Superseded,
    /// The runtime, the model route or the trusted execution around them failed.
    RuntimeError,
    ModelRouteError,
    InfrastructureError,
    /// A bounded dimension ran out under this slice.
    LimitExceeded,
}

impl InvocationClosure {
    /// Whether this reason is the participant's own doing or something that happened to it. The
    /// distinction is recorded, not acted on.
    pub const fn is_fault(self) -> bool {
        !matches!(self, Self::Completed)
    }
}

/// A fact a yielded participant asked to be resumed for.
///
/// Every variant names one committed fact kind and one identifier the kernel compares for equality.
/// A condition therefore either matches a fact that exists in the stream or it does not, and no
/// third answer is available to it: there is nothing to interpret, weigh or read.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "wake", rename_all = "snake_case")]
pub enum WakeCondition {
    /// A named work obligation returned, whatever it returned.
    ObligationReturned { obligation_id: String },
    /// Consent was recorded against a named offer.
    BidRecorded { offer_id: String },
    /// A named offer left the state it was advertised in.
    OfferClosed { offer_id: String },
    /// A named task contract changed hands, which is what a displaced holder is told by.
    LeaseIssued { contract_id: String },
    /// A submission was recorded against a named task contract.
    SubmissionRecorded { contract_id: String },
    /// A named task contract was cancelled.
    ContractCancelled { contract_id: String },
    /// A protected query returned a verdict on a named task contract.
    VerificationRecorded { contract_id: String },
    /// An authorized human stopped the run.
    RunStopped,
}

impl WakeCondition {
    /// Whether one committed fact is the fact this condition names.
    pub fn matches(&self, event: &CommitmentEvent) -> bool {
        match (self, event) {
            (
                Self::ObligationReturned { obligation_id },
                CommitmentEvent::ObligationReturned {
                    obligation_id: returned,
                    ..
                },
            ) => obligation_id == returned,
            (
                Self::BidRecorded { offer_id },
                CommitmentEvent::BidRecorded {
                    offer_id: recorded, ..
                },
            ) => offer_id == recorded,
            (
                Self::OfferClosed { offer_id },
                CommitmentEvent::OfferWithdrawn { offer_id: closed }
                | CommitmentEvent::OfferSettled { offer_id: closed },
            ) => offer_id == closed,
            (
                Self::LeaseIssued { contract_id },
                CommitmentEvent::LeaseIssued {
                    contract_id: issued,
                    ..
                },
            ) => contract_id == issued,
            (
                Self::SubmissionRecorded { contract_id },
                CommitmentEvent::SubmissionRecorded {
                    contract_id: recorded,
                    ..
                },
            ) => contract_id == recorded,
            (
                Self::ContractCancelled { contract_id },
                CommitmentEvent::ContractCancelled {
                    contract_id: cancelled,
                    ..
                },
            ) => contract_id == cancelled,
            (
                Self::VerificationRecorded { contract_id },
                CommitmentEvent::VerificationRecorded {
                    contract_id: verified,
                    ..
                },
            ) => contract_id == verified,
            (Self::RunStopped, CommitmentEvent::RunStopped { .. }) => true,
            _ => false,
        }
    }
}

/// What a yield registered: the facts worth resuming for and the moment after which the resumption
/// is no longer paid for.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WakeRegistration {
    pub conditions: Vec<WakeCondition>,
    /// The last moment a resumption may be admitted. It never outlives the funded lease, so a wake
    /// cannot hold a run open past the wall time somebody paid for.
    pub wake_deadline: u64,
}

/// One supervised process slice of one participant inside one attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InvocationRecord {
    pub invocation_id: String,
    pub attempt_id: String,
    pub contract_id: String,
    pub participant: String,
    /// The fencing generation the slice runs under. A resumption presents the same token, so a
    /// contract that changed hands cannot be resumed into by the participant it was taken from.
    pub generation: u64,
    /// How far into the committed facts this participant has read. Everything a wake may match
    /// lies after it.
    pub cursor: u64,
    pub state: InvocationState,
    /// What the last yield registered, and `None` while the slice is running or closed.
    pub wake: Option<WakeRegistration>,
    /// The run sequence of the fact that recorded the last yield. Turns are taken in this order
    /// within one principal, so waiting longer in recorded order is the only thing that advances a
    /// slice in the queue.
    pub yielded_at: u64,
    /// How many times this slice has been resumed. It is bounded, so a participant cannot be
    /// woken forever by a stream of facts it keeps asking about.
    pub wakes_used: u32,
    pub closure: Option<InvocationClosure>,
}

/// The verdict of one protected query. `InfrastructureError` consumes the reservation like any
/// other verdict and is deliberately not a rejection of the candidate: nothing was learned about it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Passed,
    Failed,
    InfrastructureError,
}

/// One recorded result of a protected query against an exact candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationRecord {
    pub contract_id: String,
    pub candidate_digest: String,
    pub verdict: Verdict,
    /// Whether the contract this query named hangs directly under the root obligation. A pass at
    /// root scope can end the run; a pass anywhere else is evidence for one parent and nothing more.
    pub root_scope: bool,
}

/// The reason an authorized command stopped the run before its work was finished.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// An authorized human stopped the run.
    Cancelled,
    /// Trusted execution or evidence integrity was lost.
    InfrastructureError,
}

/// How a run ends. Every one of these is reached by running out of something finite or by an
/// authorized command, and `Accepted` is reached by neither: it needs a passing protected query
/// against the exact root candidate, so no amount of quiet can produce it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RootTerminal {
    /// Trusted execution or evidence integrity was lost, so nothing else can be claimed.
    InfrastructureError,
    /// An authorized human stopped the run.
    Cancelled,
    /// The exact root candidate passed the approved oracle.
    Accepted,
    /// Participants stopped under an allowed stopping policy with insufficient evidence.
    Abstained,
    /// No funded work remains.
    Exhausted,
}

/// Why a run is not quiescent yet: the first funded control object that can still advance it.
///
/// A run is kept alive by funded control objects and by nothing else. An unread message, a stale
/// help request and an expired projection are not among the variants because they are not control
/// objects; a wake whose funding or deadline has gone is not among them either, which is what stops
/// a yielded participant nobody will ever wake from holding a run open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenAuthority {
    /// A process slice is running.
    Invocation { invocation_id: String },
    /// A yielded slice still holds a funded wake whose deadline has not passed.
    FundedWake { invocation_id: String },
    /// A work obligation has not returned.
    Obligation { obligation_id: String },
    /// An offer still holds a reservation nobody has settled.
    Offer { offer_id: String },
}

impl OpenAuthority {
    pub fn label(&self) -> String {
        match self {
            Self::Invocation { invocation_id } => format!("invocation {invocation_id}"),
            Self::FundedWake { invocation_id } => {
                format!("funded wake of invocation {invocation_id}")
            }
            Self::Obligation { obligation_id } => format!("obligation {obligation_id}"),
            Self::Offer { offer_id } => format!("offer {offer_id}"),
        }
    }
}
