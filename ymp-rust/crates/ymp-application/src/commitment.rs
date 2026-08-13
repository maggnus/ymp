//! The transactional boundary in front of the local-commitment ledger.
//!
//! Participants issue commands concurrently. This is where they are serialized into one order, so
//! that two awards contending for the last funded slot become two decisions taken against
//! different states rather than one decision taken twice. A command that is refused leaves no
//! sequence number, no fact and no trace, and a command identifier that arrives twice returns the
//! result the first delivery committed instead of committing a second effect.
//!
//! The ledger itself decides; this type only orders, records and replays.
//!
//! Waking a reader is the one thing here that is allowed to fail. A subscriber is handed a bounded
//! channel carrying nothing but a sequence number, and when that channel is full the number is
//! dropped rather than waited on: a slow reader must not be able to stall the order every other
//! participant is serialized into. Nothing is lost by that, because the channel is not where the
//! facts are. What a reader missed is whatever lies after the cursor it last recorded, and it asks
//! the committed stream for exactly that.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ymp_domain::MAX_IDENTIFIER_CHARS;
use ymp_domain::commitment::{
    CommitmentCommand, CommitmentError, CommitmentEvent, CommitmentLedger, OpenAuthority,
    RootTerminal,
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
    #[error("notification capacity must be between 1 and 1024")]
    InvalidNotificationCapacity,
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
    /// Where readers are told that something happened. They hold nothing but a sequence number and
    /// are allowed to lose it.
    subscribers: Vec<SyncSender<u64>>,
}

impl Inner {
    /// Tell every live subscriber where the run now stands, and drop the number for any that
    /// cannot take it. A full channel is an ordinary outcome here, not an error to report.
    fn notify(&mut self, sequence: u64) {
        self.subscribers
            .retain(|subscriber| match subscriber.try_send(sequence) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
    }
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
                subscribers: Vec::new(),
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
        let last = inner.log.len() as u64;
        inner.notify(last);
        Ok(outcome)
    }

    /// A bounded channel carrying the sequence of the last committed fact.
    ///
    /// It is a hint and never a record. A subscriber that reads slowly is skipped rather than
    /// waited on, so what arrives may be one number standing for several commands, the same number
    /// twice, or nothing at all. None of that can lose a fact, because the facts are not here.
    pub fn subscribe(&self, capacity: usize) -> Result<Receiver<u64>, CommitmentServiceError> {
        if !(1..=1024).contains(&capacity) {
            return Err(CommitmentServiceError::InvalidNotificationCapacity);
        }
        let (sender, receiver) = sync_channel(capacity);
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;
        inner.subscribers.push(sender);
        Ok(receiver)
    }

    /// The yielded process slices a controller may admit now, in the order it must admit them.
    /// The answer is recomputed from the committed facts, so it is the same whether every
    /// notification arrived or none did.
    pub fn admission_order(&self) -> Result<Vec<String>, CommitmentServiceError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;
        Ok(inner
            .ledger
            .admission_order()
            .into_iter()
            .map(|invocation| invocation.invocation_id.clone())
            .collect())
    }

    /// The first funded control object that can still advance the run, or `None` when it is
    /// quiescent.
    pub fn open_authority(&self) -> Result<Option<OpenAuthority>, CommitmentServiceError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;
        Ok(inner.ledger.open_authority())
    }

    /// How the run ended, or `None` while it can still advance.
    pub fn root_terminal(&self) -> Result<Option<RootTerminal>, CommitmentServiceError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CommitmentServiceError::Unusable)?;
        Ok(inner.ledger.root_terminal())
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
