#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub mod commitment;
pub mod contract;
pub mod participant;
pub mod pool;

use commitment::{BudgetVector, CommitmentEvent};
use participant::{OriginParticipant, ParticipantOutcome, ParticipantStart, ParticipantState};
use pool::{EntryIdentity, FrozenEntry, FrozenPool, PoolFreezeError};

/// Journal schema version 7 adds the four records of the participant a run ignites on: it started
/// on the entry the frozen pool names, it yielded, it was resumed, and it ended. Version 6 added the
/// record a run writes when its pool is frozen: which entries it may create participants from, which
/// of them it ignites on, and the digest of the ordered set. Version 5 added the record a run writes
/// when the construction of its result is more than the commitment kernel can state, version 4 added
/// the four commitment facts that carry a result's ancestry, version 3 added the two commitment tags,
/// version 2 added `contract_approved`, and version 1 had none of them; see `ymp-rust/SCHEMA.md`.
pub const EVENT_SCHEMA_VERSION: u32 = 7;
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
    /// Bind the run to the contract it is judged against. The contract itself is an immutable
    /// object in the content-addressed store; the command carries only its identity.
    ApproveContract {
        contract_id: String,
        contract_digest: String,
        oracle_digest: String,
    },
    /// Freeze the pool this run may create participants from.
    ///
    /// The command carries what the product root resolved — the pool's name, its entries in
    /// declared order and its own digest — and nothing about which entry the run should ignite on.
    /// That is decided from the entries themselves, by position and measured readiness alone, so
    /// no caller can name an ignition entry the rule would not have reached.
    FreezePool {
        pool: String,
        entries: Vec<FrozenEntry>,
        digest: String,
    },
    StartAttempt {
        attempt_id: String,
    },
    /// Start the participant this run ignites on.
    ///
    /// The command names the participant, the attempt it runs as and the workspace it runs in, and
    /// it deliberately names no entry: which provider, engine and model the participant runs under
    /// is read from the frozen record when this is decided. A caller therefore cannot start a run
    /// on a route the record does not carry, which is the same rule the freeze itself follows.
    StartOriginParticipant {
        participant_id: String,
        attempt_id: String,
        workspace: String,
    },
    /// Record that the participant's slice stopped without ending the attempt, at the cursor the
    /// runtime stated.
    YieldParticipant {
        participant_id: String,
        cursor: String,
    },
    /// Admit one resumption of the yielded participant. It resumes the attempt that already exists
    /// and starts no second one.
    ResumeParticipant {
        participant_id: String,
        cursor: String,
    },
    /// Record how the participant's attempt ended.
    FinishParticipant {
        participant_id: String,
        /// Carried beside the command's own tag rather than nested under a field of the same name,
        /// so a reader of the record reads the ending directly.
        #[serde(flatten)]
        outcome: ParticipantOutcome,
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

/// The contract a run is judged against, as the journal records it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractBinding {
    pub contract_id: String,
    /// The digest of the exact stored contract bytes.
    pub contract_digest: String,
    /// The digest of the acceptance condition the contract declares.
    pub oracle_digest: String,
}

/// Which bound stopped the commitment kernel from stating the construction of a result.
///
/// Both are bounds on what one submission states at once and not on how large a tree may be. Both
/// are about the shape of the change set alone: nothing here reads what a change means, and neither
/// bound is a judgement about the work.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ProvenanceLimit {
    /// The result changes more paths than one bundle may carry.
    TooManyChanges { changes: u64 },
    /// One path is in a shape the protocol does not admit.
    UnacceptablePath { path: String },
}

impl std::fmt::Display for ProvenanceLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyChanges { changes } => write!(formatter, "too_many_changes({changes})"),
            Self::UnacceptablePath { path } => write!(formatter, "unacceptable_path({path})"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    RunStarted {
        budget: Budget,
    },
    ContractApproved {
        contract_id: String,
        contract_digest: String,
        oracle_digest: String,
    },
    /// The pool this run may create participants from, held by value.
    ///
    /// It is written once, when the run is created, and every later decision about recruitment
    /// reads it rather than the record under the product root. A pool edited, a provider disabled
    /// or a model discovered after this record was written changes the next run and not this one
    /// (`ymp-docs/design/COLLECTIVE-DESIGN.md` §7).
    PoolFrozen(FrozenPool),
    AttemptStarted {
        attempt_id: String,
    },
    /// The participant this run ignited on, started on the entry the frozen record names.
    ///
    /// It is the attempt start of the origin as well as its registration: one start authorization
    /// is one attempt, and the record states the route and the workspace that attempt ran in, so
    /// what a participant ran under is read from this fact and never reconstructed from a pool.
    ParticipantStarted(ParticipantStart),
    /// The participant's slice stopped without ending its attempt, at the cursor the runtime stated.
    ParticipantYielded {
        participant_id: String,
        cursor: String,
    },
    /// The yielded participant was resumed. The attempt is the one that was already running.
    ParticipantResumed {
        participant_id: String,
        cursor: String,
    },
    /// The participant's attempt ended, and how.
    ParticipantFinished {
        participant_id: String,
        #[serde(flatten)]
        outcome: ParticipantOutcome,
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
    /// The state a commitment kernel starts from: the participant the whole run is accountable
    /// through, the principal it acts as, the obligation that work hangs under, and the budget
    /// that participant opens with. A ledger is rebuilt from this record and the facts that
    /// follow it, so nothing about its starting point has to be carried across a restart.
    CommitmentKernelOpened {
        root_participant: String,
        root_principal: String,
        root_obligation: String,
        budget: BudgetVector,
    },
    /// Every fact one commitment command committed, in the order the kernel committed them.
    ///
    /// A command commits all of its facts or none of them, and one journal record is what keeps
    /// that true across a restart: a reader either has the whole command or does not have the
    /// record at all, so no recovery can rebuild a ledger holding half an award.
    CommitmentFactsRecorded {
        facts: Vec<CommitmentEvent>,
    },
    /// A result whose construction the commitment kernel could not state, and the rule that stopped
    /// it.
    ///
    /// The kernel keeps the facts of one submission inside a single durable record, so a bundle
    /// states a bounded number of paths and a path a bounded number of bytes. A result past those
    /// bounds is committed and judged like any other — the work it produced is ordinary work — but
    /// its ancestry is not in the ledger, and nothing in the ledger would say why. This record is
    /// where the run says it: which result it was about, which bound it exceeded, and the kernel's
    /// own words for that bound. It commits no transition and moves no accounting.
    CandidateProvenanceUnrecorded {
        attempt_id: String,
        /// The result the run committed, named as its own journal names it.
        candidate_digest: String,
        reason: ProvenanceLimit,
        /// The rule the kernel applied, in the kernel's words, so an operator reads the bound and
        /// not a paraphrase of it.
        protocol_rule: String,
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
    /// The contract this run is judged against, once one has been approved for it.
    #[serde(default)]
    pub contract: Option<ContractBinding>,
    /// The pool this run may create participants from, once it has been frozen for it. It is the
    /// value every later recruitment decision is held to, and never a reference to the record the
    /// product root holds now.
    #[serde(default)]
    pub frozen_pool: Option<FrozenPool>,
    /// The participant this run ignited on, once its start has been recorded. It carries the route
    /// that start read out of the frozen record, so a reader is told what the participant ran under
    /// without opening the snapshot again.
    #[serde(default)]
    pub origin_participant: Option<OriginParticipant>,
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
    #[error("the run is already bound to contract {0}")]
    ContractAlreadyBound(String),
    #[error("the pool of this run is already frozen at {0}")]
    PoolAlreadyFrozen(String),
    #[error("{0}")]
    PoolFreeze(#[from] PoolFreezeError),
    #[error(
        "this run has frozen no pool, so there is no entry it ignites on and no participant it may \
         start"
    )]
    PoolNotFrozen,
    #[error(
        "the snapshot this run stands on ignites on {0}, which that same snapshot does not permit"
    )]
    OriginNotPermitted(Box<EntryIdentity>),
    #[error("this run already ignited on participant {participant_id}")]
    OriginParticipantAlreadyStarted { participant_id: String },
    #[error("this run has started no participant named {participant_id}")]
    UnknownParticipant { participant_id: String },
    #[error("participant {participant_id} is {state}, and this transition needs it {needed}")]
    ParticipantNotInState {
        participant_id: String,
        state: &'static str,
        needed: &'static str,
    },
    #[error("{kind} must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    InvalidIdentifier { kind: &'static str },
    #[error("{kind} is not a canonical lowercase SHA-256 digest")]
    InvalidDigest { kind: &'static str },
    #[error("terminal reason must contain between 1 and {MAX_REASON_BYTES} bytes")]
    InvalidReason,
    #[error("a workspace path must contain between 1 and {MAX_REASON_BYTES} bytes")]
    InvalidWorkspace,
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
            contract: None,
            frozen_pool: None,
            origin_participant: None,
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
            Command::ApproveContract {
                contract_id,
                contract_digest,
                oracle_digest,
            } => {
                validate_identifier("contract_id", contract_id)?;
                validate_digest("contract_digest", contract_digest)?;
                validate_digest("oracle_digest", oracle_digest)?;
                if let Some(bound) = &self.contract {
                    return Err(TransitionError::ContractAlreadyBound(
                        bound.contract_id.clone(),
                    ));
                }
                Ok(EventKind::ContractApproved {
                    contract_id: contract_id.clone(),
                    contract_digest: contract_digest.clone(),
                    oracle_digest: oracle_digest.clone(),
                })
            }
            Command::FreezePool {
                pool,
                entries,
                digest,
            } => {
                // One run, one freeze. A second one would state a different capability boundary for
                // a run that has already been created under the first, and every decision taken
                // since then was taken against the boundary this run actually carries.
                if let Some(frozen) = &self.frozen_pool {
                    return Err(TransitionError::PoolAlreadyFrozen(frozen.digest.clone()));
                }
                Ok(EventKind::PoolFrozen(FrozenPool::freeze(
                    pool.clone(),
                    entries.clone(),
                    digest.clone(),
                )?))
            }
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
            // The route is read out of the frozen record here and is not part of the command, so
            // the entry a participant starts on cannot enter the journal from anywhere but the
            // snapshot this run was created under. Nothing in this transition reads a pool, a
            // provider or an engine record: a pool edited or an account held back since the freeze
            // belongs to the next run.
            Command::StartOriginParticipant {
                participant_id,
                attempt_id,
                workspace,
            } => {
                validate_identifier("participant_id", participant_id)?;
                validate_identifier("attempt_id", attempt_id)?;
                validate_workspace(workspace)?;
                if let Some(started) = &self.origin_participant {
                    return Err(TransitionError::OriginParticipantAlreadyStarted {
                        participant_id: started.participant_id().to_owned(),
                    });
                }
                let frozen = self
                    .frozen_pool
                    .as_ref()
                    .ok_or(TransitionError::PoolNotFrozen)?;
                let entry = frozen.origin.clone();
                // The containment check every decision naming an entry is held to. A snapshot whose
                // origin it refuses starts nothing, rather than starting on an entry the run was
                // never permitted to use.
                if !frozen.permits(&entry) {
                    return Err(TransitionError::OriginNotPermitted(Box::new(entry)));
                }
                if self.budget.attempts_remaining == 0 {
                    return Ok(EventKind::RunExhausted {
                        reason: "attempt budget exhausted".to_owned(),
                    });
                }
                Ok(EventKind::ParticipantStarted(ParticipantStart {
                    participant_id: participant_id.clone(),
                    attempt_id: attempt_id.clone(),
                    entry,
                    workspace: workspace.clone(),
                }))
            }
            Command::YieldParticipant {
                participant_id,
                cursor,
            } => {
                validate_reason(cursor)?;
                let participant = self.participant(participant_id)?;
                if participant.state != ParticipantState::Running {
                    return Err(not_in_state(participant, "running"));
                }
                Ok(EventKind::ParticipantYielded {
                    participant_id: participant_id.clone(),
                    cursor: cursor.clone(),
                })
            }
            // A resumption puts the slice that already exists back to work. It carries no attempt
            // identifier and creates none: one start authorization is one attempt, however often it
            // yields and is resumed.
            Command::ResumeParticipant {
                participant_id,
                cursor,
            } => {
                validate_reason(cursor)?;
                let participant = self.participant(participant_id)?;
                if participant.state != ParticipantState::Yielded {
                    return Err(not_in_state(participant, "yielded"));
                }
                Ok(EventKind::ParticipantResumed {
                    participant_id: participant_id.clone(),
                    cursor: cursor.clone(),
                })
            }
            Command::FinishParticipant {
                participant_id,
                outcome,
            } => {
                if let ParticipantOutcome::Failed { reason } = outcome {
                    validate_reason(reason)?;
                }
                let participant = self.participant(participant_id)?;
                if participant.state == ParticipantState::Finished {
                    return Err(not_in_state(participant, "still running or yielded"));
                }
                Ok(EventKind::ParticipantFinished {
                    participant_id: participant_id.clone(),
                    outcome: outcome.clone(),
                })
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

    /// The participant a command names, or the refusal that this run has started no such one.
    ///
    /// This build ignites one participant per run and recruits none, so the answer is the origin or
    /// nothing. Recruitment adds participants beside it; it does not change that a command is
    /// decided against the participant its own identifier names.
    fn participant(&self, participant_id: &str) -> Result<&OriginParticipant, TransitionError> {
        self.origin_participant
            .as_ref()
            .filter(|participant| participant.participant_id() == participant_id)
            .ok_or_else(|| TransitionError::UnknownParticipant {
                participant_id: participant_id.to_owned(),
            })
    }

    fn set_participant_state(&mut self, participant_id: &str, state: ParticipantState) {
        if let Some(participant) = self.origin_participant.as_mut()
            && participant.participant_id() == participant_id
        {
            participant.state = state;
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
            EventKind::ContractApproved {
                contract_id,
                contract_digest,
                oracle_digest,
            } => {
                self.contract.get_or_insert_with(|| ContractBinding {
                    contract_id: contract_id.clone(),
                    contract_digest: contract_digest.clone(),
                    oracle_digest: oracle_digest.clone(),
                });
            }
            // The freeze is a value set once. A record that arrives over one already held is read
            // as the record of the freeze this run carries, never as a second boundary replacing
            // it, which is the same rule the contract binding follows.
            EventKind::PoolFrozen(frozen) => {
                self.frozen_pool.get_or_insert_with(|| frozen.clone());
            }
            EventKind::AttemptStarted { attempt_id } => {
                self.budget.attempts_remaining = self.budget.attempts_remaining.saturating_sub(1);
                if !self.active_attempts.contains(attempt_id) {
                    self.active_attempts.push(attempt_id.clone());
                }
            }
            // The origin's start is its attempt start: one authorization, one attempt, one
            // participant. A record that arrives over one already held is read as the record of the
            // start this run carries and never as a second participant, which is the rule the
            // contract binding and the freeze both follow.
            EventKind::ParticipantStarted(start) => {
                if self.origin_participant.is_none() {
                    self.budget.attempts_remaining =
                        self.budget.attempts_remaining.saturating_sub(1);
                    if !self.active_attempts.contains(&start.attempt_id) {
                        self.active_attempts.push(start.attempt_id.clone());
                    }
                    self.origin_participant = Some(OriginParticipant::started(start.clone()));
                }
            }
            EventKind::ParticipantYielded { participant_id, .. } => {
                self.set_participant_state(participant_id, ParticipantState::Yielded);
            }
            EventKind::ParticipantResumed { participant_id, .. } => {
                self.set_participant_state(participant_id, ParticipantState::Running);
            }
            EventKind::ParticipantFinished {
                participant_id,
                outcome,
            } => {
                if let Some(participant) = self.origin_participant.as_mut()
                    && participant.participant_id() == participant_id
                {
                    participant.state = ParticipantState::Finished;
                    participant.outcome.get_or_insert_with(|| outcome.clone());
                }
                // The attempt stays where the run's own transitions put it. A slice that ended is
                // not a run that ended: the candidate this attempt produced is submitted after its
                // process is gone, and only a terminal of the run itself closes the attempt.
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
            // The commitment kernel keeps its own accounting, and this projection carries none of
            // it: the run status, the attempt budget and the immutable candidate are decided by
            // the run's own transitions and by nothing a commitment records. What the kernel could
            // not state about a result is likewise a note about the record and not a transition of
            // the run: the result stands, and the verdict it is waiting for is unaffected.
            EventKind::CommitmentKernelOpened { .. }
            | EventKind::CommitmentFactsRecorded { .. }
            | EventKind::CandidateProvenanceUnrecorded { .. } => {}
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

/// The workspace a start names, bounded like every other content a record carries. It is the path
/// the run's own store holds the attempt's private copy at, and it is recorded rather than read: no
/// transition opens it.
fn validate_workspace(value: &str) -> Result<(), TransitionError> {
    if value.is_empty() || value.len() > MAX_REASON_BYTES {
        return Err(TransitionError::InvalidWorkspace);
    }
    Ok(())
}

/// The refusal a lifecycle command receives when the participant it names is somewhere else.
fn not_in_state(participant: &OriginParticipant, needed: &'static str) -> TransitionError {
    TransitionError::ParticipantNotInState {
        participant_id: participant.participant_id().to_owned(),
        state: participant.state.label(),
        needed,
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
        ParticipantOutcome, ParticipantStart, ParticipantState, RunState, TransitionError,
        VerificationDecision, VerificationRecord,
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
    fn approving_a_contract_binds_the_run_once_and_refuses_a_second_binding() {
        let mut state = running_state();
        assert!(state.contract.is_none());
        let command = Command::ApproveContract {
            contract_id: "contract-000000000000".to_owned(),
            contract_digest: "1".repeat(64),
            oracle_digest: "2".repeat(64),
        };
        let event = state.decide(&command).expect("first binding");
        let envelope = EventEnvelope::new("run-1", 2, "approve", "0".repeat(64), None, event)
            .expect("approval envelope");
        state.apply(&envelope);
        let bound = state.contract.as_ref().expect("the run is bound");
        assert_eq!(bound.contract_digest, "1".repeat(64));

        assert_eq!(
            state.decide(&Command::ApproveContract {
                contract_id: "contract-111111111111".to_owned(),
                contract_digest: "3".repeat(64),
                oracle_digest: "4".repeat(64),
            }),
            Err(TransitionError::ContractAlreadyBound(
                "contract-000000000000".to_owned()
            ))
        );
    }

    /// The freeze is committed once and carries the whole snapshot. A second one is refused with
    /// the digest the run already stands on, so the reader is told which boundary is in force
    /// rather than which command was rejected.
    #[test]
    fn freezing_the_pool_records_the_snapshot_once_and_refuses_a_second_freeze() {
        use crate::pool::{EntryIdentity, FrozenEntry};

        let mut state = running_state();
        assert!(state.frozen_pool.is_none());
        let command = Command::FreezePool {
            pool: "default".to_owned(),
            entries: vec![
                FrozenEntry::unavailable(
                    "openai",
                    "codex",
                    "gpt-5",
                    "the account states no credential",
                ),
                FrozenEntry::admissible("anthropic", "claude-code", "claude-opus-5"),
            ],
            digest: "b".repeat(64),
        };
        let event = state.decide(&command).expect("the first freeze");
        let envelope = EventEnvelope::new("run-1", 2, "freeze", "0".repeat(64), None, event)
            .expect("freeze envelope");
        state.apply(&envelope);

        let frozen = state.frozen_pool.as_ref().expect("the run is frozen");
        assert_eq!(frozen.pool, "default");
        assert_eq!(frozen.digest, "b".repeat(64));
        assert_eq!(frozen.entries.len(), 2);
        assert_eq!(
            frozen.origin,
            EntryIdentity::new("anthropic", "claude-code", "claude-opus-5")
        );
        assert!(frozen.permits(&EntryIdentity::new(
            "anthropic",
            "claude-code",
            "claude-opus-5"
        )));
        assert!(!frozen.permits(&EntryIdentity::new("openai", "codex", "gpt-5")));

        assert_eq!(
            state.decide(&Command::FreezePool {
                pool: "default".to_owned(),
                entries: vec![FrozenEntry::admissible("openai", "codex", "gpt-5")],
                digest: "c".repeat(64),
            }),
            Err(TransitionError::PoolAlreadyFrozen("b".repeat(64)))
        );
    }

    /// A run whose pool permits nothing live is never frozen, so the refusal is the domain's and
    /// not a surface's reading of an empty list.
    #[test]
    fn a_pool_with_no_live_entry_freezes_nothing() {
        use crate::pool::{FrozenEntry, PoolFreezeError};

        let state = running_state();
        assert_eq!(
            state.decide(&Command::FreezePool {
                pool: "default".to_owned(),
                entries: vec![FrozenEntry::unavailable(
                    "anthropic",
                    "claude-code",
                    "claude-opus-5",
                    "the account is not enabled",
                )],
                digest: "b".repeat(64),
            }),
            Err(TransitionError::PoolFreeze(
                PoolFreezeError::NoAdmissibleEntry {
                    pool: "default".to_owned(),
                    permitted: 1,
                }
            ))
        );
    }

    /// The record carries the snapshot beside its own tag, so a reader of the journal reads the
    /// entries, the ignition entry and the digest without opening a nested object.
    #[test]
    fn the_frozen_record_states_the_snapshot_beside_its_tag() {
        use crate::pool::FrozenEntry;

        let event = EventKind::PoolFrozen(
            crate::pool::FrozenPool::freeze(
                "default",
                vec![FrozenEntry::admissible(
                    "anthropic",
                    "claude-code",
                    "claude-opus-5",
                )],
                "b".repeat(64),
            )
            .expect("the pool freezes"),
        );
        let json = serde_json::to_value(&event).expect("the record serializes");
        assert_eq!(json["type"], "pool_frozen");
        assert_eq!(json["pool"], "default");
        assert_eq!(json["digest"], "b".repeat(64));
        assert_eq!(json["origin"]["model"], "claude-opus-5");
        assert_eq!(json["entries"][0]["admissible"], true);
        assert_eq!(
            serde_json::from_value::<EventKind>(json).expect("the record reads back"),
            event
        );
    }

    /// Freeze one pool into a running state, so the participant transitions have a record to read.
    fn frozen_state(entries: Vec<crate::pool::FrozenEntry>) -> RunState {
        let mut state = running_state();
        let event = state
            .decide(&Command::FreezePool {
                pool: "default".to_owned(),
                entries,
                digest: "b".repeat(64),
            })
            .expect("the pool freezes");
        let envelope = EventEnvelope::new("run-1", 2, "freeze", "0".repeat(64), None, event)
            .expect("freeze envelope");
        state.apply(&envelope);
        state
    }

    fn start_origin() -> Command {
        Command::StartOriginParticipant {
            participant_id: "origin".to_owned(),
            attempt_id: "attempt-origin".to_owned(),
            workspace: "/store/workspaces/attempt-origin".to_owned(),
        }
    }

    /// Apply one decided event to a state, as the journal does.
    fn commit(state: &mut RunState, sequence: u64, command: &Command) -> EventKind {
        let event = state.decide(command).expect("the transition is admitted");
        let envelope = EventEnvelope::new(
            "run-1",
            sequence,
            format!("command-{sequence}"),
            "0".repeat(64),
            None,
            event.clone(),
        )
        .expect("envelope");
        state.apply(&envelope);
        event
    }

    /// The route a start records is read out of the frozen record, and the command that produces it
    /// carries no entry at all. The start is also the attempt start of the origin: one
    /// authorization, one attempt, one participant.
    #[test]
    fn the_origin_starts_on_the_entry_the_frozen_record_names_and_is_its_attempt() {
        use crate::pool::{EntryIdentity, FrozenEntry};

        let mut state = frozen_state(vec![
            FrozenEntry::unavailable(
                "openai",
                "codex",
                "gpt-5",
                "the account states no credential",
            ),
            FrozenEntry::admissible("anthropic", "claude-code", "claude-opus-5"),
            FrozenEntry::admissible("anthropic", "claude-code", "claude-sonnet-5"),
        ]);
        let attempts_before = state.budget.attempts_remaining;

        let EventKind::ParticipantStarted(start) = commit(&mut state, 3, &start_origin()) else {
            panic!("the start of the origin is not recorded as one");
        };
        assert_eq!(
            start.entry,
            EntryIdentity::new("anthropic", "claude-code", "claude-opus-5"),
            "the start recorded a route the frozen record does not name"
        );
        let participant = state
            .origin_participant
            .as_ref()
            .expect("the run states its participant");
        assert_eq!(participant.state, ParticipantState::Running);
        assert_eq!(
            state.budget.attempts_remaining,
            attempts_before - 1,
            "the origin's start did not spend the attempt it runs as"
        );
        assert_eq!(state.active_attempts, vec!["attempt-origin".to_owned()]);

        // A second start is refused with the participant this run already ignited on, whatever
        // identifiers it names.
        assert_eq!(
            state.decide(&Command::StartOriginParticipant {
                participant_id: "second".to_owned(),
                attempt_id: "attempt-second".to_owned(),
                workspace: "/store/workspaces/attempt-second".to_owned(),
            }),
            Err(TransitionError::OriginParticipantAlreadyStarted {
                participant_id: "origin".to_owned()
            })
        );
    }

    /// A run whose pool is not frozen has no entry to ignite on, so it starts nothing. The refusal
    /// is the domain's, which is what keeps a surface from choosing a route where the record has
    /// none.
    #[test]
    fn a_run_with_no_frozen_pool_starts_no_participant() {
        assert_eq!(
            running_state().decide(&start_origin()),
            Err(TransitionError::PoolNotFrozen)
        );
    }

    /// A snapshot whose own containment check refuses its origin starts nothing. The record is read
    /// rather than trusted, so a boundary that contradicts itself stops the run instead of putting
    /// a participant on an entry the run was never permitted to use.
    #[test]
    fn an_origin_the_snapshot_does_not_permit_starts_nothing() {
        use crate::pool::{EntryIdentity, FrozenEntry};

        let mut state = frozen_state(vec![FrozenEntry::admissible(
            "anthropic",
            "claude-code",
            "claude-opus-5",
        )]);
        // The record is put into the one shape the containment predicate refuses: an origin that
        // names an entry the entries themselves do not carry as live.
        if let Some(frozen) = state.frozen_pool.as_mut() {
            frozen.origin = EntryIdentity::new("openai", "codex", "gpt-5");
        }
        assert!(matches!(
            state.decide(&start_origin()),
            Err(TransitionError::OriginNotPermitted(_))
        ));
    }

    /// The participant yields, is resumed and ends, and every one of those transitions is decided
    /// against where the participant actually stands rather than against what a caller asserts.
    #[test]
    fn the_participant_lifecycle_is_decided_against_the_state_the_record_holds() {
        use crate::pool::FrozenEntry;

        let mut state = frozen_state(vec![FrozenEntry::admissible(
            "anthropic",
            "claude-code",
            "claude-opus-5",
        )]);
        let yield_command = Command::YieldParticipant {
            participant_id: "origin".to_owned(),
            cursor: "cursor-1".to_owned(),
        };
        let resume_command = Command::ResumeParticipant {
            participant_id: "origin".to_owned(),
            cursor: "continue".to_owned(),
        };

        // Nothing has started, so nothing yields.
        assert_eq!(
            state.decide(&yield_command),
            Err(TransitionError::UnknownParticipant {
                participant_id: "origin".to_owned()
            })
        );

        commit(&mut state, 3, &start_origin());
        // A running participant is not resumed; a yielded one is not yielded again.
        assert!(matches!(
            state.decide(&resume_command),
            Err(TransitionError::ParticipantNotInState {
                state: "running",
                needed: "yielded",
                ..
            })
        ));
        commit(&mut state, 4, &yield_command);
        assert_eq!(
            state
                .origin_participant
                .as_ref()
                .expect("the participant")
                .state,
            ParticipantState::Yielded
        );
        assert!(matches!(
            state.decide(&yield_command),
            Err(TransitionError::ParticipantNotInState {
                state: "yielded",
                needed: "running",
                ..
            })
        ));

        // The resumption puts the participant back on the attempt it already had, and starts none.
        commit(&mut state, 5, &resume_command);
        assert_eq!(state.active_attempts, vec!["attempt-origin".to_owned()]);
        assert_eq!(state.budget.attempts_remaining, 0);

        commit(
            &mut state,
            6,
            &Command::FinishParticipant {
                participant_id: "origin".to_owned(),
                outcome: ParticipantOutcome::Completed,
            },
        );
        let participant = state.origin_participant.as_ref().expect("the participant");
        assert_eq!(participant.state, ParticipantState::Finished);
        assert_eq!(participant.outcome, Some(ParticipantOutcome::Completed));
        // The attempt stays where the run's own transitions put it: the candidate this attempt
        // produced is submitted after its process is gone.
        assert_eq!(state.active_attempts, vec!["attempt-origin".to_owned()]);
        // And a slice that has ended neither yields nor resumes.
        assert!(matches!(
            state.decide(&resume_command),
            Err(TransitionError::ParticipantNotInState {
                state: "finished",
                ..
            })
        ));
    }

    /// The start records the route beside its own tag, so a reader of the journal reads the
    /// participant, the attempt and the triple without opening a nested object.
    #[test]
    fn the_start_record_states_the_route_beside_its_tag() {
        use crate::pool::EntryIdentity;

        let event = EventKind::ParticipantStarted(ParticipantStart {
            participant_id: "origin".to_owned(),
            attempt_id: "attempt-origin".to_owned(),
            entry: EntryIdentity::new("anthropic", "claude-code", "claude-opus-5"),
            workspace: "/store/workspaces/attempt-origin".to_owned(),
        });
        let json = serde_json::to_value(&event).expect("the record serializes");
        assert_eq!(json["type"], "participant_started");
        assert_eq!(json["participant_id"], "origin");
        assert_eq!(json["attempt_id"], "attempt-origin");
        assert_eq!(json["entry"]["model"], "claude-opus-5");
        assert_eq!(
            serde_json::from_value::<EventKind>(json).expect("the record reads back"),
            event
        );

        let ended = EventKind::ParticipantFinished {
            participant_id: "origin".to_owned(),
            outcome: ParticipantOutcome::Failed {
                reason: "the runtime never came up".to_owned(),
            },
        };
        let json = serde_json::to_value(&ended).expect("the record serializes");
        assert_eq!(json["type"], "participant_finished");
        assert_eq!(json["outcome"], "failed");
        assert_eq!(json["reason"], "the runtime never came up");
        assert_eq!(
            serde_json::from_value::<EventKind>(json).expect("the record reads back"),
            ended
        );
    }

    #[test]
    fn a_contract_approval_without_canonical_digests_is_refused() {
        let state = running_state();
        assert!(matches!(
            state.decide(&Command::ApproveContract {
                contract_id: "contract-000000000000".to_owned(),
                contract_digest: "not-a-digest".to_owned(),
                oracle_digest: "2".repeat(64),
            }),
            Err(TransitionError::InvalidDigest {
                kind: "contract_digest"
            })
        ));
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
