#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const EVENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_IDENTIFIER_CHARS: usize = 128;
pub const MAX_REASON_BYTES: usize = 1024;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Budget {
    pub attempts_remaining: u32,
    pub verification_queries_remaining: u32,
}

impl Budget {
    pub const fn new(attempts: u32, verification_queries: u32) -> Self {
        Self {
            attempts_remaining: attempts,
            verification_queries_remaining: verification_queries,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Accepted,
    Exhausted,
    Abstained,
    Cancelled,
    InfrastructureError,
}

impl RunStatus {
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    StartAttempt {
        attempt_id: String,
    },
    SubmitCandidate {
        attempt_id: String,
        base_digest: String,
        object_digest: String,
    },
    Abstain {
        reason: String,
    },
    Cancel {
        reason: String,
    },
    FailInfrastructure {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationDecision {
    Accept,
    Reject,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationRecord {
    pub candidate_digest: String,
    pub contract_digest: String,
    pub oracle_digest: String,
    pub environment_digest: String,
    pub evidence_digest: String,
    pub decision: VerificationDecision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    RunStarted {
        budget: Budget,
    },
    AttemptStarted {
        attempt_id: String,
    },
    CandidateSubmitted {
        attempt_id: String,
        base_digest: String,
        object_digest: String,
    },
    VerificationRecorded {
        candidate_digest: String,
        contract_digest: String,
        oracle_digest: String,
        evidence_digest: String,
        accepted: bool,
    },
    RunExhausted {
        reason: String,
    },
    RunAbstained {
        reason: String,
    },
    RunCancelled {
        reason: String,
    },
    RunFailed {
        reason: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub run_id: String,
    pub sequence: u64,
    pub command_id: String,
    pub command_digest: String,
    pub predecessor_digest: Option<String>,
    pub event: EventKind,
    pub digest: String,
}

#[derive(Serialize)]
struct EventDigestInput<'a> {
    schema_version: u32,
    run_id: &'a str,
    sequence: u64,
    command_id: &'a str,
    command_digest: &'a str,
    predecessor_digest: &'a Option<String>,
    event: &'a EventKind,
}

impl EventEnvelope {
    pub fn new(
        run_id: impl Into<String>,
        sequence: u64,
        command_id: impl Into<String>,
        command_digest: impl Into<String>,
        predecessor_digest: Option<String>,
        event: EventKind,
    ) -> Result<Self, serde_json::Error> {
        let mut envelope = Self {
            schema_version: EVENT_SCHEMA_VERSION,
            run_id: run_id.into(),
            sequence,
            command_id: command_id.into(),
            command_digest: command_digest.into(),
            predecessor_digest,
            event,
            digest: String::new(),
        };
        envelope.digest = envelope.calculate_digest()?;
        Ok(envelope)
    }

    pub fn calculate_digest(&self) -> Result<String, serde_json::Error> {
        let input = EventDigestInput {
            schema_version: self.schema_version,
            run_id: &self.run_id,
            sequence: self.sequence,
            command_id: &self.command_id,
            command_digest: &self.command_digest,
            predecessor_digest: &self.predecessor_digest,
            event: &self.event,
        };
        let bytes = serde_json::to_vec(&input)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }

    pub fn has_valid_digest(&self) -> bool {
        self.calculate_digest()
            .is_ok_and(|digest| digest == self.digest)
    }
}

impl Command {
    pub fn digest(&self) -> Result<String, serde_json::Error> {
        Ok(digest_bytes(&serde_json::to_vec(self)?))
    }
}

pub fn digest_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunState {
    pub run_id: String,
    pub status: RunStatus,
    pub budget: Budget,
    pub active_attempts: Vec<String>,
    pub candidate_digest: Option<String>,
    pub last_sequence: u64,
    pub last_event_digest: String,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TransitionError {
    #[error("run is already terminal: {0:?}")]
    Terminal(RunStatus),
    #[error("attempt does not exist: {0}")]
    UnknownAttempt(String),
    #[error("candidate does not match the current immutable candidate")]
    CandidateMismatch,
    #[error("{kind} must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    InvalidIdentifier { kind: &'static str },
    #[error("{kind} is not a canonical lowercase SHA-256 digest")]
    InvalidDigest { kind: &'static str },
    #[error("terminal reason must contain between 1 and {MAX_REASON_BYTES} bytes")]
    InvalidReason,
}

impl RunState {
    pub fn from_start(event: &EventEnvelope) -> Option<Self> {
        let EventKind::RunStarted { budget } = &event.event else {
            return None;
        };
        Some(Self {
            run_id: event.run_id.clone(),
            status: RunStatus::Running,
            budget: budget.clone(),
            active_attempts: Vec::new(),
            candidate_digest: None,
            last_sequence: event.sequence,
            last_event_digest: event.digest.clone(),
        })
    }

    pub fn decide(&self, command: &Command) -> Result<EventKind, TransitionError> {
        if self.status.is_terminal() {
            return Err(TransitionError::Terminal(self.status));
        }

        match command {
            Command::StartAttempt { attempt_id } => {
                validate_identifier("attempt_id", attempt_id)?;
                if self.budget.attempts_remaining == 0 {
                    Ok(EventKind::RunExhausted {
                        reason: "attempt budget exhausted".to_owned(),
                    })
                } else {
                    Ok(EventKind::AttemptStarted {
                        attempt_id: attempt_id.clone(),
                    })
                }
            }
            Command::SubmitCandidate {
                attempt_id,
                base_digest,
                object_digest,
            } => {
                validate_identifier("attempt_id", attempt_id)?;
                validate_digest("base_digest", base_digest)?;
                validate_digest("object_digest", object_digest)?;
                if !self.active_attempts.contains(attempt_id) {
                    return Err(TransitionError::UnknownAttempt(attempt_id.clone()));
                }
                Ok(EventKind::CandidateSubmitted {
                    attempt_id: attempt_id.clone(),
                    base_digest: base_digest.clone(),
                    object_digest: object_digest.clone(),
                })
            }
            Command::Abstain { reason } => {
                validate_reason(reason)?;
                Ok(EventKind::RunAbstained {
                    reason: reason.clone(),
                })
            }
            Command::Cancel { reason } => {
                validate_reason(reason)?;
                Ok(EventKind::RunCancelled {
                    reason: reason.clone(),
                })
            }
            Command::FailInfrastructure { reason } => {
                validate_reason(reason)?;
                Ok(EventKind::RunFailed {
                    reason: reason.clone(),
                })
            }
        }
    }

    pub fn decide_verification(
        &self,
        record: &VerificationRecord,
    ) -> Result<EventKind, TransitionError> {
        if self.status.is_terminal() {
            return Err(TransitionError::Terminal(self.status));
        }
        validate_digest("candidate_digest", &record.candidate_digest)?;
        validate_digest("contract_digest", &record.contract_digest)?;
        validate_digest("oracle_digest", &record.oracle_digest)?;
        validate_digest("environment_digest", &record.environment_digest)?;
        validate_digest("evidence_digest", &record.evidence_digest)?;
        if self.candidate_digest.as_ref() != Some(&record.candidate_digest) {
            return Err(TransitionError::CandidateMismatch);
        }
        if self.budget.verification_queries_remaining == 0 {
            return Ok(EventKind::RunExhausted {
                reason: "verification-query budget exhausted".to_owned(),
            });
        }
        Ok(EventKind::VerificationRecorded {
            candidate_digest: record.candidate_digest.clone(),
            contract_digest: record.contract_digest.clone(),
            oracle_digest: record.oracle_digest.clone(),
            evidence_digest: record.evidence_digest.clone(),
            accepted: record.decision == VerificationDecision::Accept,
        })
    }

    pub fn apply(&mut self, envelope: &EventEnvelope) {
        match &envelope.event {
            EventKind::RunStarted { budget } => {
                self.budget = budget.clone();
                self.status = RunStatus::Running;
            }
            EventKind::AttemptStarted { attempt_id } => {
                self.budget.attempts_remaining = self.budget.attempts_remaining.saturating_sub(1);
                if !self.active_attempts.contains(attempt_id) {
                    self.active_attempts.push(attempt_id.clone());
                }
            }
            EventKind::CandidateSubmitted { object_digest, .. } => {
                self.candidate_digest = Some(object_digest.clone());
            }
            EventKind::VerificationRecorded { accepted, .. } => {
                self.budget.verification_queries_remaining =
                    self.budget.verification_queries_remaining.saturating_sub(1);
                if *accepted {
                    self.status = RunStatus::Accepted;
                    self.active_attempts.clear();
                }
            }
            EventKind::RunExhausted { .. } => {
                self.status = RunStatus::Exhausted;
                self.active_attempts.clear();
            }
            EventKind::RunAbstained { .. } => {
                self.status = RunStatus::Abstained;
                self.active_attempts.clear();
            }
            EventKind::RunCancelled { .. } => {
                self.status = RunStatus::Cancelled;
                self.active_attempts.clear();
            }
            EventKind::RunFailed { .. } => {
                self.status = RunStatus::InfrastructureError;
                self.active_attempts.clear();
            }
        }
        self.last_sequence = envelope.sequence;
        self.last_event_digest.clone_from(&envelope.digest);
    }
}

fn validate_identifier(kind: &'static str, value: &str) -> Result<(), TransitionError> {
    let length = value.chars().count();
    if (1..=MAX_IDENTIFIER_CHARS).contains(&length) {
        Ok(())
    } else {
        Err(TransitionError::InvalidIdentifier { kind })
    }
}

fn validate_digest(kind: &'static str, value: &str) -> Result<(), TransitionError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(TransitionError::InvalidDigest { kind })
    }
}

fn validate_reason(value: &str) -> Result<(), TransitionError> {
    if !value.is_empty() && value.len() <= MAX_REASON_BYTES {
        Ok(())
    } else {
        Err(TransitionError::InvalidReason)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Budget, Command, EventEnvelope, EventKind, MAX_IDENTIFIER_CHARS, MAX_REASON_BYTES,
        RunState, TransitionError, VerificationDecision, VerificationRecord,
    };

    fn running_state() -> RunState {
        let start = EventEnvelope::new(
            "run-1",
            1,
            "bootstrap",
            "0".repeat(64),
            None,
            EventKind::RunStarted {
                budget: Budget::new(1, 1),
            },
        )
        .expect("start envelope");
        RunState::from_start(&start).expect("running state")
    }

    #[test]
    fn identifiers_digests_and_terminal_reasons_are_bounded() {
        let state = running_state();
        assert!(matches!(
            state.decide(&Command::StartAttempt {
                attempt_id: "x".repeat(MAX_IDENTIFIER_CHARS + 1)
            }),
            Err(TransitionError::InvalidIdentifier { .. })
        ));
        assert!(matches!(
            state.decide(&Command::SubmitCandidate {
                attempt_id: "attempt-1".to_owned(),
                base_digest: "not-a-digest".to_owned(),
                object_digest: "0".repeat(64)
            }),
            Err(TransitionError::InvalidDigest { .. })
        ));
        assert!(matches!(
            state.decide(&Command::Cancel {
                reason: "x".repeat(MAX_REASON_BYTES + 1)
            }),
            Err(TransitionError::InvalidReason)
        ));
        assert!(matches!(
            state.decide_verification(&VerificationRecord {
                candidate_digest: "0".repeat(64),
                contract_digest: "1".repeat(64),
                oracle_digest: "2".repeat(64),
                environment_digest: "not-a-digest".to_owned(),
                evidence_digest: "3".repeat(64),
                decision: VerificationDecision::Accept,
            }),
            Err(TransitionError::InvalidDigest {
                kind: "environment_digest"
            })
        ));
    }
}
