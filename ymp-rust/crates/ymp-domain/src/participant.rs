//! The participant a run ignites on, and the life its record states.
//!
//! A run creates participants from the pool it froze at creation, and the first of them is not
//! chosen: it is the entry the frozen record already names as its origin (owner decision D2). What
//! is recorded here is that participant being started, yielding, being resumed and ending — the
//! journal facts an operator reads the life of a run through, and the facts a restart rebuilds that
//! life from.
//!
//! Two rules are structural rather than checked, and both are about where the route comes from.
//!
//! * **The command that starts the origin names no entry.** It names the participant, the attempt
//!   and the workspace, and the provider, engine and model are read from the frozen record when the
//!   transition is decided. A caller therefore cannot start a run on a route the record does not
//!   carry, in the same way that a caller cannot name the origin entry when the pool is frozen.
//! * **The frozen record is the only thing read.** Nothing here reaches the pools, the providers or
//!   the engine records under the product root. A provider held back or a pool edited after the
//!   freeze belongs to the next run, and the participant of this one starts where the record says.
//!
//! Whether the host still admits the engine that route names is a separate question and is answered
//! where the runtime is actually started, not here: this level states what the run may do, and
//! admission states what this host will do.

use serde::{Deserialize, Serialize};

use crate::pool::EntryIdentity;

/// The origin participant of one run, as the record states its start.
///
/// Every field is written once. The entry is a copy of the frozen record's origin rather than a
/// reference to it, so the route a participant ran under is read from the start fact alone.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParticipantStart {
    pub participant_id: String,
    /// The attempt this participant runs as. One start authorization is one attempt.
    pub attempt_id: String,
    /// The provider, engine and model the participant runs under, copied from the frozen record.
    pub entry: EntryIdentity,
    /// The private workspace the attempt runs in, as this run's own store names it.
    pub workspace: String,
}

/// How a participant's process slice ended.
///
/// It is what the runtime did and never a judgement of the work: whether the candidate an attempt
/// produced is accepted is decided by a protected query, and none of these variants states it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ParticipantOutcome {
    /// The runtime reached the end of its work and reported completion.
    Completed,
    /// The slice was interrupted, which is what a cancellation reaches the runtime as.
    Interrupted,
    /// The runtime, the route or the supervision around them failed, in the words that failure was
    /// stated in.
    Failed { reason: String },
}

impl std::fmt::Display for ParticipantOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Completed => formatter.write_str("completed"),
            Self::Interrupted => formatter.write_str("interrupted"),
            Self::Failed { reason } => write!(formatter, "failed · {reason}"),
        }
    }
}

/// Where a started participant stands, as the facts recorded so far state it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantState {
    /// A process slice is running for this participant.
    Running,
    /// The slice stopped without ending the attempt and is waiting to be resumed.
    Yielded,
    /// The attempt is over. Nothing resumes it, and the run starts no second attempt for the same
    /// start authorization.
    Finished,
}

impl ParticipantState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Yielded => "yielded",
            Self::Finished => "finished",
        }
    }
}

/// The origin participant as a run's projection holds it: what its start recorded, and where its
/// life stands now.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OriginParticipant {
    pub start: ParticipantStart,
    pub state: ParticipantState,
    /// How the attempt ended, once it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<ParticipantOutcome>,
}

impl OriginParticipant {
    /// The participant a start fact records, before anything else has happened to it.
    pub fn started(start: ParticipantStart) -> Self {
        Self {
            start,
            state: ParticipantState::Running,
            outcome: None,
        }
    }

    pub fn participant_id(&self) -> &str {
        &self.start.participant_id
    }

    pub fn attempt_id(&self) -> &str {
        &self.start.attempt_id
    }

    /// The route this participant runs under, as its start recorded it.
    pub const fn entry(&self) -> &EntryIdentity {
        &self.start.entry
    }
}
