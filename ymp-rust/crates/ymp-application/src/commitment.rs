//! The transactional boundary in front of the local-commitment ledger.
//!
//! Participants issue commands concurrently. This is where they are serialized into one order, so
//! that two awards contending for the last funded slot become two decisions taken against
//! different states rather than one decision taken twice. A command that is refused leaves no
//! sequence number, no fact and no trace, and a command identifier that arrives twice returns the
//! result the first delivery committed instead of committing a second effect.
//!
//! The ledger itself decides; this type only orders, records and replays.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ymp_domain::MAX_IDENTIFIER_CHARS;
use ymp_domain::commitment::{
    CommitmentCommand, CommitmentError, CommitmentEvent, CommitmentLedger,
};

/// One committed fact in the order it was committed, with the command that committed it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordedFact {
    pub sequence: u64,
    pub command_id: String,
    pub event: CommitmentEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommitmentOutcome {
    /// The sequence of the first fact this command committed.
    pub first_sequence: u64,
    pub events: Vec<CommitmentEvent>,
    /// Whether this is the recorded result of an earlier delivery of the same command.
    pub replayed: bool,
}

#[derive(Debug, Error)]
pub enum CommitmentServiceError {
    #[error(transparent)]
    Refused(#[from] CommitmentError),
    #[error("command identifier {command_id} was reused with different content")]
    IdempotencyConflict { command_id: String },
    #[error("{kind} must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    InvalidIdentifier { kind: &'static str },
    #[error("command serialization failed: {0}")]
    Serialization(String),
    #[error("the commitment ledger is no longer usable after a failed command")]
    Unusable,
}

struct RecordedCommand {
    digest: String,
    outcome: CommitmentOutcome,
}

struct Inner {
    ledger: CommitmentLedger,
    log: Vec<RecordedFact>,
    results: HashMap<String, RecordedCommand>,
}

pub struct CommitmentService {
    inner: Mutex<Inner>,
}

impl CommitmentService {
    pub fn new(ledger: CommitmentLedger) -> Self {
        Self {
            inner: Mutex::new(Inner {
                ledger,
                log: Vec::new(),
                results: HashMap::new(),
            }),
        }
    }

    /// Decide and commit one command under the single order this service defines.
    pub fn execute(
        &self,
        command_id: &str,
        command: &CommitmentCommand,
    ) -> Result<CommitmentOutcome, CommitmentServiceError> {
        let length = command_id.chars().count();
        if !(1..=MAX_IDENTIFIER_CHARS).contains(&length) {
            return Err(CommitmentServiceError::InvalidIdentifier { kind: "command_id" });
        }
        let digest = command
            .digest()
            .map_err(|error| CommitmentServiceError::Serialization(error.to_string()))?;
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;

        if let Some(recorded) = inner.results.get(command_id) {
            if recorded.digest != digest {
                return Err(CommitmentServiceError::IdempotencyConflict {
                    command_id: command_id.to_owned(),
                });
            }
            let mut outcome = recorded.outcome.clone();
            outcome.replayed = true;
            return Ok(outcome);
        }

        // A refusal returns here having changed nothing: no sequence is consumed, no fact is
        // logged, and the command identifier stays free for a later attempt against a state where
        // the command may be valid.
        let events = inner.ledger.execute(command)?;
        let first_sequence = inner.log.len() as u64 + 1;
        for (offset, event) in events.iter().enumerate() {
            inner.log.push(RecordedFact {
                sequence: first_sequence + offset as u64,
                command_id: command_id.to_owned(),
                event: event.clone(),
            });
        }
        let outcome = CommitmentOutcome {
            first_sequence,
            events,
            replayed: false,
        };
        inner.results.insert(
            command_id.to_owned(),
            RecordedCommand {
                digest,
                outcome: outcome.clone(),
            },
        );
        Ok(outcome)
    }

    /// Committed facts after a cursor, which is how a lagging reader recovers rather than assuming
    /// it saw every notification.
    pub fn facts_after(&self, cursor: u64) -> Result<Vec<RecordedFact>, CommitmentServiceError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;
        Ok(inner
            .log
            .iter()
            .filter(|fact| fact.sequence > cursor)
            .cloned()
            .collect())
    }

    pub fn committed_facts(&self) -> Result<usize, CommitmentServiceError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;
        Ok(inner.log.len())
    }

    /// A consistent read of the whole ledger.
    pub fn snapshot(&self) -> Result<CommitmentLedger, CommitmentServiceError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;
        Ok(inner.ledger.clone())
    }
}
