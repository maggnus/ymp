#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use thiserror::Error;
use uuid::Uuid;
use ymp_agent_api::{
    AgentToolCall, AgentToolError, AgentToolHandler, MAX_EVENT_PAGE, ReadEventsArguments,
    SubmitArguments,
};
use ymp_artifacts::{ArtifactError, ArtifactStore, CandidateRef, FileEntry, SubmissionRef};
use ymp_domain::commitment::{
    BudgetVector, BundleChange, CommitmentCommand, CommitmentError, CommitmentEvent,
    CommitmentLedger, PathChange,
};
use ymp_domain::recruitment::{
    AdmittedParticipant, ParticipantStartFailed, ParticipantStartPath, Recruitment,
    RecruitmentPolicy, RecruitmentRefusal, RequestParticipant, RuntimeAdmission,
};
use ymp_domain::{
    Budget, Command, EventEnvelope, EventKind, MAX_IDENTIFIER_CHARS, MAX_REASON_BYTES,
    ProvenanceLimit, RunState, RunStatus, TransitionError, VerificationRecord,
};
use ymp_storage::{
    DataRootLock, Journal, JournalError, JournalLimits, ObjectStore, ObjectStoreError,
};
use ymp_verifier::{
    EnvironmentBoundVerifier, StoredVerificationEvidence, VerifiedEvidence, VerifierError,
};

pub mod answer;
pub mod contract;
pub mod participant;
pub mod pool;
pub mod root;
pub mod verification;

pub use answer::AnswerError;
pub use contract::{
    AcceptanceCondition, ContractRequestError, DEFAULT_RUN_BUDGET, PreparedContract, RunRequest,
    load_contract_package, prepare_contract, run_stem,
};
pub use participant::{
    ORIGIN_ATTEMPT, ORIGIN_INVOCATION, ORIGIN_PARTICIPANT, OriginAttempt, OriginAttemptError,
    OriginStartRefused, OriginStartRequest, ParticipantHost, ParticipantRuntimes,
    PrivateWorkspaces, RouteUnavailable, WorkspaceNotEstablished, ignite_origin_participant,
};
pub use pool::{PoolFreezeRefused, freeze_record, freeze_under};
pub use verification::{VerificationJob, VerificationOutcome};

const BOOTSTRAP_COMMAND_ID: &str = "ymp.bootstrap";

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("application I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error(transparent)]
    ObjectStore(#[from] ObjectStoreError),
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error(transparent)]
    Commitment(#[from] CommitmentError),
    /// A recruitment request the mechanical gate refused. Nothing was written and nothing moved:
    /// the run is exactly as it was, and the constraint that stopped the request is what this says.
    #[error(transparent)]
    RecruitmentRefused(#[from] RecruitmentRefusal),
    #[error("this run has no commitment kernel; nothing has opened one")]
    NoCommitmentKernel,
    #[error("this run already carries a commitment kernel accountable for {root_obligation}")]
    CommitmentKernelAlreadyOpen { root_obligation: String },
    #[error("data root is already initialized")]
    AlreadyInitialized,
    #[error(
        "this store was written under event schema version {actual}; this binary reads version \
         {expected} and left the store unchanged"
    )]
    IncompatibleStore { actual: u32, expected: u32 },
    #[error("data root has no committed run")]
    NotInitialized,
    #[error("data root is already owned by another foreground process")]
    WriterAlreadyActive,
    #[error("first journal event is not run_started")]
    InvalidFirstEvent,
    #[error("command identifier {command_id} was reused with different content")]
    IdempotencyConflict { command_id: String },
    #[error("candidate integration conflicts with immutable candidate {current}: {proposed}")]
    CandidateConflict { current: String, proposed: String },
    #[error("{kind} must contain between 1 and {MAX_IDENTIFIER_CHARS} characters")]
    InvalidIdentifier { kind: &'static str },
    #[error("notification capacity must be between 1 and 1024")]
    InvalidNotificationCapacity,
    #[error("event JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("verifier evidence object digest does not match its declared digest")]
    EvidenceDigestMismatch,
    #[error("the stored contract object does not match the digest the contract was prepared with")]
    ContractDigestMismatch,
    #[error("verification infrastructure error: {0}")]
    VerificationInfrastructure(#[from] VerificationInfrastructureError),
    #[error("a candidate must be submitted before verification")]
    NoCandidateForVerification,
    #[error("this run is judged against no approved contract")]
    NoApprovedContract,
    #[error("the approved contract could not be read: {0}")]
    ContractUnreadable(String),
    #[error("evidence export destination already exists: {0}")]
    ExportAlreadyExists(PathBuf),
    #[error("a candidate must be submitted before evidence can be exported")]
    NoCandidateForExport,
    #[error(
        "only an accepted candidate is applied into a project directory; this run stands at \
         {actual:?}"
    )]
    CandidateNotAccepted { actual: RunStatus },
    #[error(
        "the project directory already holds file(s) the candidate names, and nothing was \
         written: {}",
        .0.join(", ")
    )]
    ApplyWouldOverwrite(Vec<String>),
    #[error(
        "the project directory cannot receive the candidate as it stands, and nothing was \
         written: {}",
        .0.join("; ")
    )]
    ApplyBlocked(Vec<String>),
    #[error(
        "the application stopped after writing {} file(s) into the project directory, which now \
         holds {}: {source}",
        .applied.len(),
        .applied.join(", ")
    )]
    ApplyInterrupted {
        applied: Vec<String>,
        #[source]
        source: Box<ApplicationError>,
    },
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum VerificationInfrastructureError {
    #[error("verification evidence has no environment binding")]
    MissingEnvironmentBinding,
    #[error("version-1 verification evidence has an ambiguous environment binding")]
    AmbiguousLegacyEvidence,
    #[error("verification environment object is missing: {0}")]
    EnvironmentObjectMissing(String),
    #[error("verification environment object does not match digest {0}")]
    EnvironmentDigestMismatch(String),
    #[error("verification evidence does not match its journal record")]
    EvidenceRecordMismatch,
    #[error("verification evidence is invalid: {0}")]
    InvalidEvidence(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandOutcome {
    pub event: EventEnvelope,
    pub replayed: bool,
    pub status: RunStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceCandidateOutcome {
    pub submission: SubmissionRef,
    pub candidate: CandidateRef,
    pub command: CommandOutcome,
}

/// What one commitment command committed, and where in the run's ledger it stands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommitmentOutcome {
    /// The sequence of the first fact this command committed.
    pub first_sequence: u64,
    pub events: Vec<CommitmentEvent>,
    /// Whether this is the recorded result of an earlier delivery of the same command.
    pub replayed: bool,
}

/// The construction of the result a run committed: the base it was built from and the exact object
/// standing at every path it changed.
///
/// It is read out of the two snapshots the run's own record names — the base its attempt started
/// from and the result its submission produced — so what it states is what the store holds, and not
/// what a process happened to remember. Stated in the vocabulary the commitment kernel forms
/// candidates in, it is what carries a result's ancestry from the journal into the ledger.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateConstruction {
    pub base_digest: String,
    /// The identifier the run's journal carries for this result: the digest of the snapshot the
    /// candidate is materialized from. The kernel names the same result by the digest of the whole
    /// construction, which is computed from these facts and not from this field.
    pub candidate_digest: String,
    /// One entry per path the result changed relative to its base, in path order.
    pub changes: Vec<BundleChange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceExportReport {
    pub destination: PathBuf,
    pub candidate_digest: String,
    pub evidence_digests: Vec<String>,
    pub environment_digests: Vec<String>,
    pub event_count: usize,
}

/// What an in-place application of the accepted candidate put into a project directory.
///
/// The paths are the candidate's own, relative to the directory they were applied to, so a
/// report states exactly which files an operator now has and which of them replaced a file that
/// was already there.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateApplyReport {
    pub destination: PathBuf,
    pub candidate_digest: String,
    pub applied_paths: Vec<String>,
    pub replaced_paths: Vec<String>,
}

/// What one admission committed: the participant, the facts that paid for it, the record it was
/// written as, and what the managed start path answered.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParticipantAdmission {
    pub admitted: AdmittedParticipant,
    /// The commitment facts of the charge, in the order the kernel committed them.
    pub facts: Vec<CommitmentEvent>,
    pub command: CommandOutcome,
    /// Why the managed start path could not start this participant, when it could not.
    ///
    /// A start that failed does not take the admission back. The authority to start a participant
    /// is creation authority: spending it is irreversible, and it was spent when the run granted
    /// the permission the fact records. What a caller holds here is therefore a participant that
    /// was admitted and paid for and has no process — a state the record states plainly rather than
    /// one the accounting hides by pretending the request was refused.
    pub start_failure: Option<ParticipantStartFailed>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ApplicationConfig {
    pub journal_limits: JournalLimits,
    /// The ceilings this run recruits under. It is a declared boundary of the run rather than
    /// something read from the host, so a run recruits under the ceiling it was created with even
    /// when the ceilings of the product root move afterwards.
    pub recruitment: RecruitmentPolicy,
}

pub struct Application {
    data_root: PathBuf,
    _lock: DataRootLock,
    journal: Journal,
    object_store: ObjectStore,
    state: RunState,
    command_results: HashMap<String, RecordedCommandResult>,
    candidate_identity: Option<CandidateIdentity>,
    recorded_verifications: HashMap<VerificationIdentity, EventEnvelope>,
    /// The commitment kernel of this run, once a record opened one. It is never carried across a
    /// restart: every ledger this field holds was folded out of the journal by
    /// [`Application::fold_commitment`].
    commitments: Option<CommitmentLedger>,
    /// What each commitment command committed, so a repeated delivery is answered with the facts
    /// and the run sequence of the first one rather than with a second effect.
    commitment_results: HashMap<String, CommitmentOutcome>,
    /// The mechanical gate a recruitment request passes. It holds the run's ceilings and no state:
    /// what every gate reads is the run's own record.
    recruitment: Recruitment,
    notification_senders: Vec<SyncSender<u64>>,
}

#[derive(Clone, Debug)]
struct RecordedCommandResult {
    event: EventEnvelope,
    status: RunStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CandidateIdentity {
    base_digest: String,
    object_digest: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct VerificationIdentity {
    candidate_digest: String,
    contract_digest: String,
    oracle_digest: String,
    environment_digest: String,
}

pub struct AgentSession<'a> {
    app: &'a mut Application,
    attempt_id: String,
    workspace_submission: Option<WorkspaceSubmission>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceSubmission {
    base_digest: String,
    workspace: PathBuf,
    capture_exclusions: Vec<String>,
}

impl WorkspaceSubmission {
    pub fn new(
        base_digest: impl Into<String>,
        workspace: impl Into<PathBuf>,
        capture_exclusions: Vec<String>,
    ) -> Self {
        Self {
            base_digest: base_digest.into(),
            workspace: workspace.into(),
            capture_exclusions,
        }
    }
}

impl Application {
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub fn create(
        data_root: impl AsRef<Path>,
        run_id: impl Into<String>,
        budget: Budget,
    ) -> Result<Self, ApplicationError> {
        Self::create_with_config(data_root, run_id, budget, ApplicationConfig::default())
    }

    pub fn create_with_config(
        data_root: impl AsRef<Path>,
        run_id: impl Into<String>,
        budget: Budget,
        config: ApplicationConfig,
    ) -> Result<Self, ApplicationError> {
        let data_root = data_root.as_ref().to_path_buf();
        let run_id = run_id.into();
        validate_identifier("run_id", &run_id)?;
        store_compatibility(&data_root)?;
        let lock = DataRootLock::acquire(&data_root).map_err(|error| match error {
            ymp_storage::LockError::AlreadyLocked => ApplicationError::WriterAlreadyActive,
            ymp_storage::LockError::Io(source) => ApplicationError::Io(source),
        })?;
        let (mut journal, existing) =
            Journal::open_with_limits(data_root.join("events.jsonl"), config.journal_limits)
                .map_err(journal_open_error)?;
        if !existing.is_empty() {
            return Err(ApplicationError::AlreadyInitialized);
        }
        let object_store = ObjectStore::open(data_root.join("objects/sha256"))?;
        let start = EventEnvelope::new(
            run_id,
            1,
            BOOTSTRAP_COMMAND_ID,
            ymp_domain::digest_bytes(BOOTSTRAP_COMMAND_ID.as_bytes()),
            None,
            EventKind::RunStarted { budget },
        )?;
        journal.append(&start)?;
        let state = RunState::from_start(&start).ok_or(ApplicationError::InvalidFirstEvent)?;
        let command_results = HashMap::from([(
            BOOTSTRAP_COMMAND_ID.to_owned(),
            RecordedCommandResult {
                event: start,
                status: state.status,
            },
        )]);
        let app = Self {
            data_root,
            _lock: lock,
            journal,
            object_store,
            state,
            command_results,
            candidate_identity: None,
            recorded_verifications: HashMap::new(),
            commitments: None,
            commitment_results: HashMap::new(),
            recruitment: Recruitment::new(config.recruitment),
            notification_senders: Vec::new(),
        };
        app.write_metadata()?;
        Ok(app)
    }

    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, ApplicationError> {
        Self::open_with_config(data_root, ApplicationConfig::default())
    }

    pub fn open_with_config(
        data_root: impl AsRef<Path>,
        config: ApplicationConfig,
    ) -> Result<Self, ApplicationError> {
        let data_root = data_root.as_ref().to_path_buf();
        // A store this binary cannot read is refused before anything is opened for writing: no
        // lock is taken, no projection is replaced, and not one byte of it changes. Marking such
        // a store as an infrastructure failure would destroy the record of a run this binary is
        // in no position to judge.
        store_compatibility(&data_root)?;
        let lock = DataRootLock::acquire(&data_root).map_err(|error| match error {
            ymp_storage::LockError::AlreadyLocked => ApplicationError::WriterAlreadyActive,
            ymp_storage::LockError::Io(source) => ApplicationError::Io(source),
        })?;
        match Self::open_locked(data_root.clone(), lock, config) {
            Ok(app) => Ok(app),
            Err(error @ ApplicationError::IncompatibleStore { .. }) => Err(error),
            Err(error) => {
                let _ = mark_infrastructure_error(&data_root);
                Err(error)
            }
        }
    }

    fn open_locked(
        data_root: PathBuf,
        lock: DataRootLock,
        config: ApplicationConfig,
    ) -> Result<Self, ApplicationError> {
        let (journal, events) =
            Journal::open_with_limits(data_root.join("events.jsonl"), config.journal_limits)
                .map_err(journal_open_error)?;
        let first = events.first().ok_or(ApplicationError::NotInitialized)?;
        let state = RunState::from_start(first).ok_or(ApplicationError::InvalidFirstEvent)?;
        let object_store = ObjectStore::open(data_root.join("objects/sha256"))?;
        let command_results = HashMap::from([(
            first.command_id.clone(),
            RecordedCommandResult {
                event: first.clone(),
                status: state.status,
            },
        )]);
        let mut application = Self {
            data_root,
            _lock: lock,
            journal,
            object_store,
            state,
            command_results,
            candidate_identity: None,
            recorded_verifications: HashMap::new(),
            commitments: None,
            commitment_results: HashMap::new(),
            recruitment: Recruitment::new(config.recruitment),
            notification_senders: Vec::new(),
        };
        for event in events.iter().skip(1) {
            application.absorb_committed(event)?;
        }
        Ok(application)
    }

    /// Fold one committed fact of the journal into the in-memory projection.
    ///
    /// Every journal record reaches memory through here, so the projection a restart builds and
    /// the projection a mid-run recovery repairs are the same reading of the same records.
    fn absorb_committed(&mut self, event: &EventEnvelope) -> Result<(), ApplicationError> {
        if let Some(result) = self.command_results.get(&event.command_id)
            && result.event.command_digest != event.command_digest
        {
            return Err(ApplicationError::IdempotencyConflict {
                command_id: event.command_id.clone(),
            });
        }
        match &event.event {
            EventKind::CandidateSubmitted {
                base_digest,
                object_digest,
                ..
            } => {
                self.object_store.verify(object_digest)?;
                let proposed = CandidateIdentity {
                    base_digest: base_digest.clone(),
                    object_digest: object_digest.clone(),
                };
                ensure_candidate_identity(self.candidate_identity.as_ref(), &proposed)?;
                self.candidate_identity = Some(proposed);
            }
            EventKind::VerificationRecorded {
                candidate_digest,
                contract_digest,
                oracle_digest,
                evidence_digest,
                accepted,
            } => {
                let bytes = self.object_store.read(evidence_digest)?;
                let evidence =
                    StoredVerificationEvidence::from_object_bytes(&bytes, evidence_digest)
                        .map_err(verification_evidence_error)?;
                let environment_digest = evidence
                    .environment_digest()
                    .ok_or(VerificationInfrastructureError::MissingEnvironmentBinding)?;
                verify_environment_object(&self.object_store, environment_digest)?;
                if evidence.candidate_digest() != candidate_digest
                    || evidence.contract_digest() != contract_digest
                    || evidence.oracle_digest() != oracle_digest
                    || (evidence.decision() == ymp_domain::VerificationDecision::Accept)
                        != *accepted
                {
                    return Err(VerificationInfrastructureError::EvidenceRecordMismatch.into());
                }
                self.recorded_verifications
                    .entry(VerificationIdentity {
                        candidate_digest: candidate_digest.clone(),
                        contract_digest: contract_digest.clone(),
                        oracle_digest: oracle_digest.clone(),
                        environment_digest: environment_digest.to_owned(),
                    })
                    .or_insert_with(|| event.clone());
            }
            EventKind::CommitmentKernelOpened { .. }
            | EventKind::CommitmentFactsRecorded { .. }
            | EventKind::ParticipantAdmitted { .. } => {
                self.fold_commitment(event)?;
            }
            _ => {}
        }
        self.state.apply(event);
        let status = self.state.status;
        self.command_results
            .entry(event.command_id.clone())
            .or_insert_with(|| RecordedCommandResult {
                event: event.clone(),
                status,
            });
        Ok(())
    }

    /// Bring the projection back to the journal before a command is judged against it.
    ///
    /// Each fact is written to the journal whole and only then applied in memory, so an
    /// interruption between the two — a panic under the lock the controller reads through — ends
    /// the process step with the record complete and memory one fact short of it. Memory is a
    /// reading of the journal and never the other way round, so the facts memory has not seen are
    /// read back and folded in. Nothing is written to the journal here, and no command is carried
    /// out a second time: the interrupted command's own result is now the recorded one, and a
    /// repeat of it is answered from the record.
    ///
    /// Costing this is a comparison of two numbers while the projection is in step, which is
    /// every command of an uninterrupted run; the journal is read again only when it is not.
    fn recover_projection(&mut self) -> Result<(), ApplicationError> {
        let applied = self.state.last_sequence;
        if self.journal.last_sequence() <= applied {
            return Ok(());
        }
        let missing: Vec<EventEnvelope> = self
            .journal
            .read_committed()?
            .into_iter()
            .filter(|event| event.sequence > applied)
            .collect();
        for event in missing {
            self.absorb_committed(&event)?;
            self.notify_subscribers(event.sequence);
        }
        self.write_metadata()
    }

    pub fn execute(
        &mut self,
        command_id: impl Into<String>,
        command: Command,
    ) -> Result<CommandOutcome, ApplicationError> {
        let command_id = command_id.into();
        validate_identifier("command_id", &command_id)?;
        let command_digest = command.digest()?;
        self.recover_projection()?;
        if let Some(outcome) = self.replay(&command_id, &command_digest)? {
            self.write_metadata()?;
            return Ok(outcome);
        }

        if let Command::SubmitCandidate {
            base_digest,
            object_digest,
            ..
        } = &command
        {
            self.object_store.verify(object_digest)?;
            ensure_candidate_identity(
                self.candidate_identity.as_ref(),
                &CandidateIdentity {
                    base_digest: base_digest.clone(),
                    object_digest: object_digest.clone(),
                },
            )?;
        }

        let event = self.state.decide(&command)?;
        self.commit(command_id, command_digest, event)
    }

    /// The commitment kernel of this run, or `None` while no record has opened one.
    ///
    /// What it answers is a reading of the journal and never an accumulation beside it, so a
    /// reader is given the same ledger whether this application has been running since the run
    /// started or opened the store a moment ago.
    pub fn commitments(&self) -> Option<&CommitmentLedger> {
        self.commitments.as_ref()
    }

    /// The same kernel, read after the projection has been brought back to the journal.
    ///
    /// Each fact is appended to the journal whole and only then applied in memory, so a process
    /// step interrupted between the two ends with the record complete and the ledger one command
    /// short of it. A caller deciding what a run still owes — whether its process slice is closed,
    /// which slice the kernel admits next, whether the run has ended — would decide that against a
    /// ledger the record has already moved past. This is the reading such a caller takes: the
    /// journal is caught up with first, exactly as it is before every command.
    pub fn recovered_commitments(&mut self) -> Result<Option<&CommitmentLedger>, ApplicationError> {
        self.recover_projection()?;
        Ok(self.commitments.as_ref())
    }

    /// Record the state the commitment kernel of this run starts from.
    ///
    /// A ledger is the root participant, the principal it acts as, the obligation the run is
    /// accountable for and the budget that participant opens with, and none of those is derivable
    /// from the facts that follow. They are therefore committed as a record of their own: the
    /// journal states what the ledger was built from, and a restart builds the same one instead of
    /// being handed a starting point that only the process that opened it knew.
    pub fn open_commitment_kernel(
        &mut self,
        command_id: impl Into<String>,
        root_participant: impl Into<String>,
        root_principal: impl Into<String>,
        root_obligation: impl Into<String>,
        budget: BudgetVector,
    ) -> Result<CommandOutcome, ApplicationError> {
        let command_id = command_id.into();
        validate_identifier("command_id", &command_id)?;
        let event = EventKind::CommitmentKernelOpened {
            root_participant: root_participant.into(),
            root_principal: root_principal.into(),
            root_obligation: root_obligation.into(),
            budget,
        };
        let command_digest = ymp_domain::digest_bytes(&serde_json::to_vec(&event)?);
        self.recover_projection()?;
        if let Some(outcome) = self.replay(&command_id, &command_digest)? {
            self.write_metadata()?;
            return Ok(outcome);
        }
        if self.state.status.is_terminal() {
            return Err(TransitionError::Terminal(self.state.status).into());
        }
        if let Some(ledger) = &self.commitments {
            return Err(ApplicationError::CommitmentKernelAlreadyOpen {
                root_obligation: ledger.root_obligation().to_owned(),
            });
        }
        // A genesis no ledger can be built from — an identifier of the wrong length, say — is
        // refused here rather than written and discovered by the recovery that has to rebuild it.
        ledger_from(&event)?;
        self.commit(command_id, command_digest, event)
    }

    /// Record that the construction of a committed result is more than the commitment kernel can
    /// state, and which bound stopped it.
    ///
    /// The result itself is committed and judged exactly as any other: what the runtime produced is
    /// ordinary work, and a bound on how much one submission may state at once is no reason to take
    /// its verdict away. What would be dishonest is a ledger that seals the result and says nothing
    /// about why its ancestry is missing, because that record cannot be told from one written
    /// before a construction was ever journalled. So the run states it here, once, naming the
    /// result it is about.
    pub fn record_candidate_provenance_unrecorded(
        &mut self,
        command_id: impl Into<String>,
        attempt_id: impl Into<String>,
        candidate_digest: impl Into<String>,
        reason: ProvenanceLimit,
        protocol_rule: impl Into<String>,
    ) -> Result<CommandOutcome, ApplicationError> {
        let command_id = command_id.into();
        let attempt_id = attempt_id.into();
        validate_identifier("command_id", &command_id)?;
        validate_identifier("attempt_id", &attempt_id)?;
        let event = EventKind::CandidateProvenanceUnrecorded {
            attempt_id,
            candidate_digest: candidate_digest.into(),
            reason,
            protocol_rule: protocol_rule
                .into()
                .chars()
                .take(MAX_REASON_BYTES)
                .collect(),
        };
        let command_digest = ymp_domain::digest_bytes(&serde_json::to_vec(&event)?);
        self.recover_projection()?;
        if let Some(outcome) = self.replay(&command_id, &command_digest)? {
            return Ok(outcome);
        }
        if self.state.status.is_terminal() {
            return Err(TransitionError::Terminal(self.state.status).into());
        }
        self.commit(command_id, command_digest, event)
    }

    /// Decide one commitment command and record every fact it commits before answering.
    ///
    /// The order is the whole guarantee: the ledger is asked what the command commits, the facts
    /// reach the journal as one record, and only then is the result returned. A caller that has
    /// been told a contract was formed, escrow moved or an obligation returned is therefore
    /// holding something the record already states, and a restart at any point returns a ledger
    /// that either has the whole command or has never heard of it.
    ///
    /// A run that has already ended still records what its commitments settle. Stopping the run,
    /// returning its work obligation and settling its offer are the accounting of the ending the
    /// journal states, and they are committed after that ending: an operator's cancellation moves
    /// the journal while the process slice is still open, and what closes that slice is recorded
    /// afterwards. Refused here, the two records of one run would disagree — the journal ended
    /// while the ledger held the slice open on an unjudged candidate — which is the state this
    /// path exists to prevent. Nothing recorded here moves the run's own status: a commitment
    /// record commits no transition of the run, so the terminal the journal states stands
    /// unchanged.
    ///
    /// What such a run no longer does is begin anything. A command that offers work, records
    /// consent, forms a contract, starts an attempt or a process slice, resumes one or buys more
    /// lease is refused from the moment the journal states an ending, whether or not the ledger has
    /// been stopped as well. The two records reach their endings at different moments — an operator
    /// cancels the run, and the kernel is stopped when the slice that was running has been wound
    /// down — and everything created in between was created for a run that had already ended: a
    /// wake offer advertised then draws on an account nothing will spend, and a resumption admitted
    /// then puts a runtime back to work on a cancelled run. The accounting of the ending is not
    /// creation and still lands: the run is stopped, its work returned, its offers withdrawn and
    /// settled, its slice closed, and the verdict a protected query produced is recorded, since
    /// that verdict is what the ending of a finished run is derived from.
    pub fn execute_commitment(
        &mut self,
        command_id: impl Into<String>,
        command: &CommitmentCommand,
    ) -> Result<CommitmentOutcome, ApplicationError> {
        let command_id = command_id.into();
        validate_identifier("command_id", &command_id)?;
        let command_digest = command.digest()?;
        self.recover_projection()?;
        // A repeated delivery is answered before the guard: what it returns is the result the first
        // delivery committed while the run was live, and refusing it would report an ending for a
        // command that had already been decided.
        if let Some(outcome) = self.replay_commitment(&command_id, &command_digest)? {
            return Ok(outcome);
        }
        if self.state.status.is_terminal() && creates(command) {
            return Err(TransitionError::Terminal(self.state.status).into());
        }
        let ledger = self
            .commitments
            .as_ref()
            .ok_or(ApplicationError::NoCommitmentKernel)?;
        // Decided against a copy, so a refusal — and a decided fact the accounts turn out not to
        // honour — leaves the journal, the ledger and the command identifier exactly as they were.
        let first_sequence = ledger.sequence() + 1;
        let facts = ledger.clone().execute(command)?;
        self.commit(
            command_id,
            command_digest,
            EventKind::CommitmentFactsRecorded {
                facts: facts.clone(),
            },
        )?;
        Ok(CommitmentOutcome {
            first_sequence,
            events: facts,
            replayed: false,
        })
    }

    fn replay_commitment(
        &self,
        command_id: &str,
        command_digest: &str,
    ) -> Result<Option<CommitmentOutcome>, ApplicationError> {
        let Some(result) = self.command_results.get(command_id) else {
            return Ok(None);
        };
        let conflict = || ApplicationError::IdempotencyConflict {
            command_id: command_id.to_owned(),
        };
        if result.event.command_digest != command_digest {
            return Err(conflict());
        }
        // The identifier belongs to a command of the run rather than to a commitment. Answering
        // it from here would report facts that command never committed.
        let recorded = self
            .commitment_results
            .get(command_id)
            .ok_or_else(conflict)?;
        Ok(Some(CommitmentOutcome {
            first_sequence: recorded.first_sequence,
            events: recorded.events.clone(),
            replayed: true,
        }))
    }

    /// Fold one commitment record into the kernel this application holds.
    ///
    /// The commit path and the recovery path both come through here, so the ledger a live run
    /// carries and the ledger a restart rebuilds are the same reading of the same records.
    fn fold_commitment(&mut self, envelope: &EventEnvelope) -> Result<(), ApplicationError> {
        match &envelope.event {
            EventKind::CommitmentKernelOpened { .. } => {
                // One run, one kernel. The genesis states what a ledger is built from, and a
                // second one states a different starting point for the same run: the facts that
                // follow it were decided against one of the two ledgers and cannot be replayed
                // into both. Passing it over rebuilt the run from whichever record came first and
                // read the other as if it had never been written, so a store holding two of them
                // is refused here instead — on the recovery path, which is where such a store
                // arrives, and on the commit path alike.
                if let Some(ledger) = &self.commitments {
                    return Err(ApplicationError::CommitmentKernelAlreadyOpen {
                        root_obligation: ledger.root_obligation().to_owned(),
                    });
                }
                self.commitments = Some(ledger_from(&envelope.event)?);
            }
            // An admission and the charge that paid for it are one record, so they are folded
            // together and by the same code the charge of any other command is folded by. A
            // restart therefore rebuilds a ledger in which the admitted participant exists and the
            // authority it cost has been spent, or one in which neither happened.
            EventKind::CommitmentFactsRecorded { facts }
            | EventKind::ParticipantAdmitted { facts, .. } => {
                let ledger = self
                    .commitments
                    .as_mut()
                    .ok_or(ApplicationError::NoCommitmentKernel)?;
                let first_sequence = ledger.sequence() + 1;
                for fact in facts {
                    ledger.replay(fact)?;
                }
                self.commitment_results
                    .entry(envelope.command_id.clone())
                    .or_insert_with(|| CommitmentOutcome {
                        first_sequence,
                        events: facts.clone(),
                        replayed: false,
                    });
            }
            _ => {}
        }
        Ok(())
    }

    /// Decide one recruitment request, record the admission it grants, and hand it to the managed
    /// start path.
    ///
    /// The order is the whole guarantee, and it is the same order every other command of this run
    /// follows. The gate decides against the run's own committed record; the admission and the
    /// facts that pay for it reach the journal as one record; and only then is a process asked for.
    /// A refusal therefore leaves the journal byte for byte as it was — no participant, no charge,
    /// no process — and an admission a caller has been told about is one the record already states.
    ///
    /// Competing requests are serialized because this is the run's single writer: a second request
    /// is decided against a record that already carries the first, so the last participant-start,
    /// the last place under the ceiling and the last entry are each taken once. A repeat of one
    /// request is refused as a duplicate rather than admitting a second participant.
    ///
    /// Which participant is worth recruiting is not decided here and is not decidable from anything
    /// this method reads: the entry is the one the proposer named, and no ordering over the frozen
    /// pool is consulted.
    pub fn request_participant(
        &mut self,
        request: &RequestParticipant,
        runtime: &dyn RuntimeAdmission,
        start: &mut dyn ParticipantStartPath,
    ) -> Result<ParticipantAdmission, ApplicationError> {
        self.recover_projection()?;
        let ledger = self
            .commitments
            .as_ref()
            .ok_or(ApplicationError::NoCommitmentKernel)?;
        let admission = self
            .recruitment
            .admit(request, &self.state, ledger, runtime)?;
        // Decided against a copy, so a charge the accounts turn out not to honour leaves the
        // journal, the ledger and the run exactly as they were.
        let facts = ledger.clone().execute(&admission.charge)?;
        let command_id = format!("ymp.participant.request.{}", request.request_id);
        let command_digest = ymp_domain::digest_bytes(&serde_json::to_vec(request)?);
        let command = self.commit(
            command_id,
            command_digest,
            EventKind::ParticipantAdmitted {
                admitted: admission.participant.clone(),
                facts: facts.clone(),
            },
        )?;
        // The participant is admitted and paid for before anything is started, so a start that
        // fails leaves a record naming exactly what was granted rather than an unaccounted process.
        let start_failure = start.start(&admission.participant).err();
        Ok(ParticipantAdmission {
            admitted: admission.participant,
            facts,
            command,
            start_failure,
        })
    }

    /// Every participant this run has admitted, in the order its journal recorded them.
    pub fn admissions(&self) -> &[AdmittedParticipant] {
        &self.state.admissions
    }

    /// Records evidence issued directly by a verifier.
    ///
    /// Evidence read back from storage has a distinct read-only type and cannot cross this
    /// boundary:
    ///
    /// ```compile_fail
    /// use ymp_application::Application;
    /// use ymp_verifier::StoredVerificationEvidence;
    ///
    /// fn record_stored(app: &mut Application, evidence: &StoredVerificationEvidence) {
    ///     app.record_verification("verify", evidence);
    /// }
    /// ```
    pub fn record_verification(
        &mut self,
        command_id: impl Into<String>,
        evidence: &VerifiedEvidence,
    ) -> Result<CommandOutcome, ApplicationError> {
        let command_id = command_id.into();
        let Some(environment_digest) = evidence.environment_digest() else {
            return self.commit_verification_infrastructure(
                command_id,
                evidence.evidence_digest(),
                "verification evidence has no environment binding",
            );
        };
        let stored_environment_digest = self.object_store.put(evidence.environment_object())?;
        if stored_environment_digest != environment_digest {
            return self.commit_verification_infrastructure(
                command_id,
                evidence.evidence_digest(),
                "verification environment digest mismatch",
            );
        }
        self.record_bound_verification(command_id, evidence, environment_digest)
    }

    pub fn verify_with_environment<V: EnvironmentBoundVerifier>(
        &mut self,
        command_id: impl Into<String>,
        environment_object: &[u8],
        verifier: &V,
    ) -> Result<CommandOutcome, ApplicationError> {
        let command_id = command_id.into();
        let candidate_digest = self
            .state
            .candidate_digest
            .clone()
            .ok_or(ApplicationError::NoCandidateForVerification)?;
        let environment_digest = self.object_store.put(environment_object)?;
        let candidate_path = self.object_store.path_for(&candidate_digest)?;
        let environment_path = self.object_store.path_for(&environment_digest)?;
        let evidence = match verifier.verify_candidate_in_environment(
            &candidate_path,
            &candidate_digest,
            &environment_path,
            &environment_digest,
        ) {
            Ok(evidence) => evidence,
            Err(_) => {
                return self.commit_verification_infrastructure(
                    command_id,
                    &environment_digest,
                    "verifier could not validate the bound environment",
                );
            }
        };
        if evidence.environment_digest() != Some(environment_digest.as_str()) {
            return self.commit_verification_infrastructure(
                command_id,
                evidence.evidence_digest(),
                "verifier returned evidence for a different environment",
            );
        }
        self.record_bound_verification(command_id, &evidence, &environment_digest)
    }

    fn record_bound_verification(
        &mut self,
        command_id: String,
        evidence: &VerifiedEvidence,
        environment_digest: &str,
    ) -> Result<CommandOutcome, ApplicationError> {
        validate_identifier("command_id", &command_id)?;
        let command_digest = evidence.evidence_digest().to_owned();
        self.recover_projection()?;
        if let Some(outcome) = self.replay(&command_id, &command_digest)? {
            self.write_metadata()?;
            return Ok(outcome);
        }
        let identity = VerificationIdentity {
            candidate_digest: evidence.candidate_digest().to_owned(),
            contract_digest: evidence.contract_digest().to_owned(),
            oracle_digest: evidence.oracle_digest().to_owned(),
            environment_digest: environment_digest.to_owned(),
        };
        if let Some(event) = self.recorded_verifications.get(&identity) {
            return Ok(CommandOutcome {
                event: event.clone(),
                replayed: true,
                status: self.state.status,
            });
        }

        self.object_store.verify(evidence.candidate_digest())?;
        if let Err(error) = verify_environment_object(&self.object_store, environment_digest) {
            return self.commit_verification_infrastructure(
                command_id,
                &command_digest,
                &error.to_string(),
            );
        }
        let evidence_object_digest = self.object_store.put(&evidence.object_bytes()?)?;
        if evidence_object_digest != evidence.evidence_digest() {
            return Err(ApplicationError::EvidenceDigestMismatch);
        }
        let record = VerificationRecord {
            candidate_digest: evidence.candidate_digest().to_owned(),
            contract_digest: evidence.contract_digest().to_owned(),
            oracle_digest: evidence.oracle_digest().to_owned(),
            environment_digest: environment_digest.to_owned(),
            evidence_digest: evidence.evidence_digest().to_owned(),
            decision: evidence.decision(),
        };
        let event = self.state.decide_verification(&record)?;
        let outcome = self.commit(command_id, command_digest, event)?;
        self.recorded_verifications
            .insert(identity, outcome.event.clone());
        Ok(outcome)
    }

    pub fn stored_verification(
        &self,
        candidate_digest: &str,
        contract_digest: &str,
        oracle_digest: &str,
        environment_digest: &str,
    ) -> Option<CommandOutcome> {
        self.recorded_verifications
            .get(&VerificationIdentity {
                candidate_digest: candidate_digest.to_owned(),
                contract_digest: contract_digest.to_owned(),
                oracle_digest: oracle_digest.to_owned(),
                environment_digest: environment_digest.to_owned(),
            })
            .cloned()
            .map(|event| CommandOutcome {
                event,
                replayed: true,
                status: self.state.status,
            })
    }

    fn commit_verification_infrastructure(
        &mut self,
        command_id: String,
        command_digest_source: &str,
        reason: &str,
    ) -> Result<CommandOutcome, ApplicationError> {
        validate_identifier("command_id", &command_id)?;
        let command_digest = ymp_domain::digest_bytes(command_digest_source.as_bytes());
        self.recover_projection()?;
        if let Some(outcome) = self.replay(&command_id, &command_digest)? {
            return Ok(outcome);
        }
        self.commit(
            command_id,
            command_digest,
            EventKind::RunFailed {
                reason: reason.chars().take(1024).collect(),
            },
        )
    }

    /// Captures a quiescent private attempt workspace and commits its immutable candidate.
    /// The caller must stop the producing runtime before invoking this use case.
    pub fn submit_workspace_candidate(
        &mut self,
        command_id: impl Into<String>,
        attempt_id: impl Into<String>,
        base_snapshot_digest: impl Into<String>,
        workspace: impl AsRef<Path>,
    ) -> Result<WorkspaceCandidateOutcome, ApplicationError> {
        self.submit_workspace_candidate_excluding(
            command_id,
            attempt_id,
            base_snapshot_digest,
            workspace,
            &[],
        )
    }

    pub fn submit_workspace_candidate_excluding(
        &mut self,
        command_id: impl Into<String>,
        attempt_id: impl Into<String>,
        base_snapshot_digest: impl Into<String>,
        workspace: impl AsRef<Path>,
        exclusions: &[&str],
    ) -> Result<WorkspaceCandidateOutcome, ApplicationError> {
        let base_snapshot_digest = base_snapshot_digest.into();
        let artifacts = self.artifact_store();
        let submission = artifacts.create_submission_excluding(
            base_snapshot_digest.clone(),
            workspace.as_ref(),
            exclusions,
        )?;
        let candidate =
            artifacts.build_candidate(&base_snapshot_digest, &submission.manifest_digest)?;
        let command = self.execute(
            command_id,
            Command::SubmitCandidate {
                attempt_id: attempt_id.into(),
                base_digest: base_snapshot_digest,
                object_digest: candidate.snapshot_digest.clone(),
            },
        )?;
        Ok(WorkspaceCandidateOutcome {
            submission,
            candidate,
            command,
        })
    }

    /// The construction of the candidate this run committed, or `None` while it has committed none.
    ///
    /// The change set is the difference between two snapshots the run's own record names: the base
    /// its attempt was started from, and the result its submission produced. A path both snapshots
    /// state identically is not a change and is left out; a path the result states differently, or
    /// states and the base does not, is the exact object the result puts there; a path the base
    /// states and the result does not is a deletion, which is a stated value like any other.
    ///
    /// Nothing here reads what any change means. Bytes are named by digest and compared by digest,
    /// and the objects themselves are the ones the store already holds.
    pub fn candidate_construction(
        &self,
    ) -> Result<Option<CandidateConstruction>, ApplicationError> {
        let Some(identity) = &self.candidate_identity else {
            return Ok(None);
        };
        let artifacts = self.artifact_store();
        let base = entries(artifacts.load_snapshot(&identity.base_digest)?.files);
        let result = entries(artifacts.load_snapshot(&identity.object_digest)?.files);
        let mut changes = Vec::new();
        for path in base.keys().chain(result.keys()).collect::<BTreeSet<_>>() {
            let change = match (base.get(path), result.get(path)) {
                (Some(before), Some(after)) if before == after => continue,
                (_, Some(after)) => PathChange::Upsert {
                    object_digest: after.object_digest.clone(),
                    executable: after.executable,
                },
                (Some(_), None) => PathChange::Delete,
                (None, None) => continue,
            };
            changes.push(BundleChange {
                path: (*path).clone(),
                change,
            });
        }
        Ok(Some(CandidateConstruction {
            base_digest: identity.base_digest.clone(),
            candidate_digest: identity.object_digest.clone(),
            changes,
        }))
    }

    pub fn export_evidence(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<EvidenceExportReport, ApplicationError> {
        let destination = destination.as_ref().to_path_buf();
        if destination.exists() {
            return Err(ApplicationError::ExportAlreadyExists(destination));
        }
        let candidate_digest = self
            .state
            .candidate_digest
            .clone()
            .ok_or(ApplicationError::NoCandidateForExport)?;
        let events = self.journal.read_committed()?;
        let evidence_digests: Vec<_> = events
            .iter()
            .filter_map(|event| match &event.event {
                EventKind::VerificationRecorded {
                    evidence_digest, ..
                } => Some(evidence_digest.clone()),
                _ => None,
            })
            .collect();
        let mut environment_digests = Vec::new();
        for evidence_digest in &evidence_digests {
            let bytes = self.object_store.read(evidence_digest)?;
            let evidence = StoredVerificationEvidence::from_object_bytes(&bytes, evidence_digest)
                .map_err(verification_evidence_error)?;
            let environment_digest = evidence
                .environment_digest()
                .ok_or(VerificationInfrastructureError::MissingEnvironmentBinding)?;
            verify_environment_object(&self.object_store, environment_digest)?;
            environment_digests.push(environment_digest.to_owned());
        }
        environment_digests.sort();
        environment_digests.dedup();
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".ymp-export-{}", Uuid::new_v4()));
        fs::create_dir(&temporary)?;
        let result = (|| -> Result<(), ApplicationError> {
            fs::write(
                temporary.join("state.json"),
                serde_json::to_vec_pretty(&self.state)?,
            )?;
            let mut journal = File::create(temporary.join("events.jsonl"))?;
            for event in &events {
                serde_json::to_writer(&mut journal, event)?;
                journal.write_all(b"\n")?;
            }
            journal.flush()?;
            journal.sync_all()?;

            fs::write(
                temporary.join("candidate-manifest.json"),
                self.object_store.read(&candidate_digest)?,
            )?;
            self.artifact_store()
                .materialize(&candidate_digest, temporary.join("candidate"))?;
            let evidence_directory = temporary.join("evidence");
            fs::create_dir(&evidence_directory)?;
            for digest in &evidence_digests {
                fs::write(
                    evidence_directory.join(format!("{digest}.json")),
                    self.object_store.read(digest)?,
                )?;
            }
            let environment_directory = temporary.join("environments");
            fs::create_dir(&environment_directory)?;
            for digest in &environment_digests {
                fs::write(
                    environment_directory.join(digest),
                    self.object_store.read(digest)?,
                )?;
            }
            let runtime_evidence_source = self.data_root.join("runtime-evidence");
            let has_runtime_evidence = runtime_evidence_source.is_dir();
            if has_runtime_evidence {
                copy_export_tree(
                    &runtime_evidence_source,
                    &temporary.join("runtime-evidence"),
                )?;
            }
            fs::write(
                temporary.join("manifest.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "schema_version": 1,
                    "run_id": self.state.run_id,
                    "status": self.state.status,
                    "candidate_digest": candidate_digest,
                    "evidence_digests": evidence_digests,
                    "environment_digests": environment_digests,
                    "runtime_evidence": has_runtime_evidence,
                    "event_count": events.len()
                }))?,
            )?;
            File::open(&temporary)?.sync_all()?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
        fs::rename(&temporary, &destination)?;
        File::open(parent)?.sync_all()?;
        Ok(EvidenceExportReport {
            destination,
            candidate_digest,
            evidence_digests,
            environment_digests,
            event_count: events.len(),
        })
    }

    /// Apply the accepted candidate's files into a project directory, and write nothing else.
    ///
    /// This is delivery rather than storage: what lands in the directory is the candidate tree
    /// exactly as the store holds it — no journal, no verifier evidence, no manifest — because a
    /// directory an operator works in is not a place to keep a record of the run. The evidence
    /// bundle of [`Application::export_evidence`] remains the default form of an export and keeps
    /// that record; this mode is chosen explicitly.
    ///
    /// Only an accepted candidate is applied. A rejected or unjudged candidate is refused by its
    /// run status, so an operator cannot put work into a project directory that nothing has
    /// approved.
    ///
    /// A file the project already holds is a refusal, not a silent replacement: every conflicting
    /// path is named and nothing at all is written, unless `overwrite` states that replacing them
    /// is intended. The whole tree is examined before the first file moves, and every path the
    /// candidate must pass through is examined too, not only the file at its end: a project file
    /// standing where the candidate needs a directory, or a directory standing where it names a
    /// file, or a symbolic link on the way, blocks the application before it begins rather than
    /// stopping it half-way.
    ///
    /// The files are materialized into one staging directory and renamed into place, so a file
    /// that appears in the project is complete and carries the candidate's own permissions. A
    /// condition the examination cannot see beforehand — a directory the operator may not write
    /// into, a filesystem that fills — can still stop the moves after some have happened, and
    /// [`ApplicationError::ApplyInterrupted`] then names the files that are already in the
    /// project rather than reporting that nothing was written.
    pub fn apply_candidate(
        &self,
        destination: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<CandidateApplyReport, ApplicationError> {
        let destination = destination.as_ref().to_path_buf();
        let candidate_digest = self
            .state
            .candidate_digest
            .clone()
            .ok_or(ApplicationError::NoCandidateForExport)?;
        if self.state.status != RunStatus::Accepted {
            return Err(ApplicationError::CandidateNotAccepted {
                actual: self.state.status,
            });
        }
        let manifest = self.artifact_store().load_snapshot(&candidate_digest)?;
        let mut targets = Vec::new();
        for entry in &manifest.files {
            let relative = checked_candidate_path(&entry.path)?;
            targets.push((entry.path.clone(), relative));
        }
        fs::create_dir_all(&destination)?;

        // What is already there decides whether anything is written at all. Every target and
        // every directory a target is reached through is examined before the first file moves,
        // so a refusal names every conflict at once and leaves the directory exactly as it was.
        let mut occupied = Vec::new();
        let mut blocked = Vec::new();
        for (path, relative) in &targets {
            blocked.extend(blocking_ancestors(&destination, relative));
            let Ok(metadata) = fs::symlink_metadata(destination.join(relative)) else {
                continue;
            };
            if metadata.is_dir() {
                blocked.push(format!(
                    "{path}: a directory stands where the candidate names a file"
                ));
            } else {
                occupied.push(path.clone());
            }
        }
        blocked.sort();
        blocked.dedup();
        occupied.sort();
        if !blocked.is_empty() {
            return Err(ApplicationError::ApplyBlocked(blocked));
        }
        if !occupied.is_empty() && !overwrite {
            return Err(ApplicationError::ApplyWouldOverwrite(occupied));
        }

        let staging = destination.join(format!(".ymp-apply-{}", Uuid::new_v4()));
        self.artifact_store()
            .materialize(&candidate_digest, &staging)?;
        let mut moved: Vec<String> = Vec::new();
        let result = (|| -> Result<(), ApplicationError> {
            for (path, relative) in &targets {
                let target = destination.join(relative);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(staging.join(relative), &target)?;
                moved.push(path.clone());
            }
            File::open(&destination)?.sync_all()?;
            Ok(())
        })();
        let _ = fs::remove_dir_all(&staging);
        if let Err(error) = result {
            // Some of the candidate may already be in the project. Saying that nothing was
            // written would be false, so what is there is named instead.
            moved.sort();
            return Err(ApplicationError::ApplyInterrupted {
                applied: moved,
                source: Box::new(error),
            });
        }

        let mut applied_paths = moved;
        applied_paths.sort();
        Ok(CandidateApplyReport {
            destination,
            candidate_digest,
            applied_paths,
            replaced_paths: occupied,
        })
    }

    fn replay(
        &self,
        command_id: &str,
        command_digest: &str,
    ) -> Result<Option<CommandOutcome>, ApplicationError> {
        let Some(result) = self.command_results.get(command_id) else {
            return Ok(None);
        };
        if result.event.command_digest != command_digest {
            return Err(ApplicationError::IdempotencyConflict {
                command_id: command_id.to_owned(),
            });
        }
        Ok(Some(CommandOutcome {
            event: result.event.clone(),
            replayed: true,
            status: result.status,
        }))
    }

    fn commit(
        &mut self,
        command_id: String,
        command_digest: String,
        event: EventKind,
    ) -> Result<CommandOutcome, ApplicationError> {
        let envelope = EventEnvelope::new(
            &self.state.run_id,
            self.state.last_sequence + 1,
            &command_id,
            command_digest,
            Some(self.state.last_event_digest.clone()),
            event,
        )?;
        if let Err(error) = self.journal.append(&envelope) {
            return self.fail_commit(error);
        }
        if let EventKind::CandidateSubmitted {
            base_digest,
            object_digest,
            ..
        } = &envelope.event
        {
            self.candidate_identity = Some(CandidateIdentity {
                base_digest: base_digest.clone(),
                object_digest: object_digest.clone(),
            });
        }
        self.fold_commitment(&envelope)?;
        self.state.apply(&envelope);
        self.command_results.insert(
            command_id,
            RecordedCommandResult {
                event: envelope.clone(),
                status: self.state.status,
            },
        );
        self.write_metadata()?;
        self.notify_subscribers(envelope.sequence);
        Ok(CommandOutcome {
            event: envelope,
            replayed: false,
            status: self.state.status,
        })
    }

    fn fail_commit(&mut self, error: JournalError) -> Result<CommandOutcome, ApplicationError> {
        // A run that has already ended keeps the ending it recorded. The reserve exists for the
        // one terminal a live run still owes, and a record the journal would not take from a run
        // that has ended is not a reason to write a second terminal over the first or to set one
        // in this projection. What the journal declined is reported instead.
        if self.state.status.is_terminal() {
            return Err(ApplicationError::Journal(error));
        }
        if matches!(
            &error,
            JournalError::CapacityExhausted { .. } | JournalError::EventTooLarge { .. }
        ) {
            let reason = "journal capacity exhausted".to_owned();
            let command_id = format!(
                "ymp.infrastructure.journal-capacity.{}",
                self.state.last_sequence + 1
            );
            let command = Command::FailInfrastructure {
                reason: reason.clone(),
            };
            let terminal = EventEnvelope::new(
                &self.state.run_id,
                self.state.last_sequence + 1,
                &command_id,
                command.digest()?,
                Some(self.state.last_event_digest.clone()),
                EventKind::RunFailed { reason },
            )?;
            if self.journal.append_terminal(&terminal).is_ok() {
                self.state.apply(&terminal);
                self.command_results.insert(
                    command_id,
                    RecordedCommandResult {
                        event: terminal.clone(),
                        status: self.state.status,
                    },
                );
                self.write_metadata()?;
                self.notify_subscribers(terminal.sequence);
                return Err(ApplicationError::Journal(error));
            }
        }

        self.state.status = RunStatus::InfrastructureError;
        self.state.active_attempts.clear();
        let _ = self.write_metadata();
        Err(ApplicationError::Journal(error))
    }

    pub fn state(&self) -> &RunState {
        &self.state
    }

    pub fn object_store(&self) -> &ObjectStore {
        &self.object_store
    }

    pub fn artifact_store(&self) -> ArtifactStore {
        ArtifactStore::new(self.object_store.clone())
    }

    pub fn events_after(&self, cursor: u64) -> Result<Vec<EventEnvelope>, ApplicationError> {
        Ok(self
            .journal
            .read_committed()?
            .into_iter()
            .filter(|event| event.sequence > cursor)
            .collect())
    }

    pub fn subscribe(&mut self, capacity: usize) -> Result<Receiver<u64>, ApplicationError> {
        if !(1..=1024).contains(&capacity) {
            return Err(ApplicationError::InvalidNotificationCapacity);
        }
        let (sender, receiver) = sync_channel(capacity);
        self.notification_senders.push(sender);
        Ok(receiver)
    }

    fn notify_subscribers(&mut self, sequence: u64) {
        self.notification_senders
            .retain(|sender| match sender.try_send(sequence) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
    }

    pub fn agent_session(&mut self, attempt_id: impl Into<String>) -> AgentSession<'_> {
        AgentSession {
            app: self,
            attempt_id: attempt_id.into(),
            workspace_submission: None,
        }
    }

    pub fn workspace_agent_session(
        &mut self,
        attempt_id: impl Into<String>,
        submission: WorkspaceSubmission,
    ) -> AgentSession<'_> {
        AgentSession {
            app: self,
            attempt_id: attempt_id.into(),
            workspace_submission: Some(submission),
        }
    }

    fn write_metadata(&self) -> Result<(), ApplicationError> {
        write_state_metadata(&self.data_root, &self.state)
    }
}

impl AgentToolHandler for AgentSession<'_> {
    fn call(&mut self, call: AgentToolCall) -> Result<serde_json::Value, AgentToolError> {
        match call {
            AgentToolCall::ReadControl => serde_json::to_value(self.app.state())
                .map_err(|_| AgentToolError::internal("control state serialization failed")),
            AgentToolCall::ReadEvents(arguments) => self.read_events(arguments),
            AgentToolCall::Submit(arguments) => self.submit(arguments),
            AgentToolCall::Yield(_) => Err(AgentToolError::rejected(
                "yield requires an invocation-bound controller endpoint",
            )),
        }
    }
}

impl AgentSession<'_> {
    fn read_events(
        &self,
        arguments: ReadEventsArguments,
    ) -> Result<serde_json::Value, AgentToolError> {
        if arguments.limit == 0 || arguments.limit > MAX_EVENT_PAGE {
            return Err(AgentToolError::invalid(format!(
                "limit must be between 1 and {MAX_EVENT_PAGE}"
            )));
        }
        let mut events = self
            .app
            .events_after(arguments.cursor)
            .map_err(|_| AgentToolError::internal("committed events could not be read"))?;
        let has_more = events.len() > usize::from(arguments.limit);
        events.truncate(usize::from(arguments.limit));
        let next_cursor = events
            .last()
            .map_or(arguments.cursor, |event| event.sequence);
        Ok(serde_json::json!({
            "events": events,
            "next_cursor": next_cursor,
            "has_more": has_more
        }))
    }

    fn submit(&mut self, arguments: SubmitArguments) -> Result<serde_json::Value, AgentToolError> {
        if arguments.command_id.is_empty() || arguments.command_id.chars().count() > 128 {
            return Err(AgentToolError::invalid(
                "command_id must contain between 1 and 128 characters",
            ));
        }
        let submission = self.workspace_submission.clone().ok_or_else(|| {
            AgentToolError::rejected("this attempt has no controller-bound workspace submission")
        })?;
        let exclusions: Vec<_> = submission
            .capture_exclusions
            .iter()
            .map(String::as_str)
            .collect();
        let outcome = self
            .app
            .submit_workspace_candidate_excluding(
                arguments.command_id,
                &self.attempt_id,
                &submission.base_digest,
                &submission.workspace,
                &exclusions,
            )
            .map_err(agent_application_error)?;
        serde_json::to_value(outcome)
            .map_err(|_| AgentToolError::internal("command outcome serialization failed"))
    }
}

/// A candidate path as it may be joined to a project directory: relative, and made of ordinary
/// names only, so nothing a manifest carries can address a file outside the directory applied to.
fn checked_candidate_path(path: &str) -> Result<PathBuf, ApplicationError> {
    let value = Path::new(path);
    if path.is_empty()
        || value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ApplicationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("the candidate names a path that cannot be applied: {path}"),
        )));
    }
    Ok(value.to_path_buf())
}

/// Everything on the way to a candidate file that stops it from being written there.
///
/// The candidate's path is relative and checked, but the directories it passes through belong to
/// the project. A file standing where the candidate needs a directory makes the write impossible,
/// and a symbolic link would carry it outside the directory the operator named; a path examined
/// only at its end sees neither, because the operating system reports the same condition for a
/// path that does not exist yet and for one whose parent is not a directory.
fn blocking_ancestors(destination: &Path, relative: &Path) -> Vec<String> {
    let mut blocking = Vec::new();
    let mut walked = destination.to_path_buf();
    let mut inside = PathBuf::new();
    let mut components: Vec<_> = relative.components().collect();
    components.pop();
    for component in components {
        walked.push(component);
        inside.push(component);
        let Ok(metadata) = fs::symlink_metadata(&walked) else {
            // Nothing exists here yet, so nothing below it can exist either: the application
            // creates the rest of the path itself.
            break;
        };
        let reached = inside.display();
        if metadata.file_type().is_symlink() {
            blocking.push(format!(
                "{reached}: the project reaches this path through a symbolic link"
            ));
            // Everything below a link stands outside the project; walking on would read
            // metadata beyond it and could name paths the project does not hold.
            break;
        } else if !metadata.is_dir() {
            blocking.push(format!(
                "{reached}: the project holds a file where the candidate needs a directory"
            ));
            break;
        }
    }
    blocking
}

fn copy_export_tree(source: &Path, destination: &Path) -> Result<(), ApplicationError> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(ApplicationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("runtime evidence contains a symlink: {}", source.display()),
        )));
    }
    if metadata.is_dir() {
        fs::create_dir(destination)?;
        let mut entries = fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            copy_export_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
        File::open(destination)?.sync_all()?;
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(ApplicationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "runtime evidence contains an unsupported file type: {}",
                source.display()
            ),
        )));
    }
    fs::copy(source, destination)?;
    File::open(destination)?.sync_all()?;
    Ok(())
}

/// Read the schema version of a store without opening it for writing.
///
/// Only the first journal record is read, and only its version field is used: the record is
/// immutable once written, so this needs neither the writer lock nor the digest chain.
fn store_compatibility(data_root: &Path) -> Result<(), ApplicationError> {
    let journal = data_root.join("events.jsonl");
    let Ok(bytes) = fs::read(&journal) else {
        return Ok(());
    };
    let Some(line) = bytes.split(|byte| *byte == b'\n').next() else {
        return Ok(());
    };
    #[derive(Deserialize)]
    struct Versioned {
        schema_version: u32,
    }
    let Ok(record) = serde_json::from_slice::<Versioned>(line) else {
        return Ok(());
    };
    if record.schema_version == ymp_domain::EVENT_SCHEMA_VERSION {
        return Ok(());
    }
    Err(ApplicationError::IncompatibleStore {
        actual: record.schema_version,
        expected: ymp_domain::EVENT_SCHEMA_VERSION,
    })
}

/// The ledger a genesis record states, built from that record and from nothing else.
fn ledger_from(event: &EventKind) -> Result<CommitmentLedger, ApplicationError> {
    let EventKind::CommitmentKernelOpened {
        root_participant,
        root_principal,
        root_obligation,
        budget,
    } = event
    else {
        return Err(ApplicationError::NoCommitmentKernel);
    };
    Ok(CommitmentLedger::new(
        root_participant,
        root_principal,
        root_obligation,
        *budget,
    )?)
}

/// Whether a commitment command begins something new in the run.
///
/// The distinction is what the run's ending means: after it, what already exists is wound down and
/// nothing further is begun, extended or bought. Creating is therefore read as authority — offering
/// work, consenting to it, forming a contract or an obligation under it, starting an attempt or a
/// process slice, admitting one back, and buying more lease for either.
///
/// Everything else is the accounting of what was already there. Withdrawing and settling an offer,
/// returning or cancelling a task contract, closing a slice, moving the clock and stopping the run
/// close what exists rather than open anything. Recording a result belongs here too: the result is
/// the work the run already did, and its bytes are in the run's record before any of this is
/// stated. So does a verdict: a protected query judges a result that exists, and the ending of a
/// finished run is derived from that verdict, so refusing it would leave the run with no ending to
/// derive at all.
///
/// The match names every command, so a command added to the protocol is classified here rather than
/// admitted by a wildcard.
const fn creates(command: &CommitmentCommand) -> bool {
    match command {
        CommitmentCommand::RegisterParticipant(_)
        | CommitmentCommand::Advertise(_)
        | CommitmentCommand::RecordBid(_)
        | CommitmentCommand::Award(_)
        | CommitmentCommand::AcceptOpen(_)
        | CommitmentCommand::StartAttempt(_)
        | CommitmentCommand::RenewLease(_)
        | CommitmentCommand::Reassign(_)
        | CommitmentCommand::StartInvocation(_)
        | CommitmentCommand::ResumeInvocation(_) => true,
        CommitmentCommand::WithdrawBid(_)
        | CommitmentCommand::WithdrawOffer(_)
        | CommitmentCommand::SettleOffer(_)
        | CommitmentCommand::SubmitResult(_)
        | CommitmentCommand::RecordObject(_)
        | CommitmentCommand::SubmitBundle(_)
        | CommitmentCommand::RecordConflict(_)
        | CommitmentCommand::ReturnObligation(_)
        | CommitmentCommand::CancelContract(_)
        | CommitmentCommand::AdvanceClock(_)
        | CommitmentCommand::YieldInvocation(_)
        | CommitmentCommand::CloseInvocation(_)
        | CommitmentCommand::RecordVerification(_)
        | CommitmentCommand::StopRun(_) => false,
    }
}

/// A snapshot's files keyed by path, which is the form the two snapshots are compared in.
fn entries(files: Vec<FileEntry>) -> BTreeMap<String, FileEntry> {
    files
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect()
}

fn journal_open_error(error: JournalError) -> ApplicationError {
    match error {
        JournalError::UnsupportedSchema { actual, .. } => ApplicationError::IncompatibleStore {
            actual,
            expected: ymp_domain::EVENT_SCHEMA_VERSION,
        },
        error => ApplicationError::Journal(error),
    }
}

fn validate_identifier(kind: &'static str, value: &str) -> Result<(), ApplicationError> {
    let length = value.chars().count();
    if (1..=MAX_IDENTIFIER_CHARS).contains(&length) {
        Ok(())
    } else {
        Err(ApplicationError::InvalidIdentifier { kind })
    }
}

fn ensure_candidate_identity(
    current: Option<&CandidateIdentity>,
    proposed: &CandidateIdentity,
) -> Result<(), ApplicationError> {
    if let Some(current) = current
        && current != proposed
    {
        return Err(ApplicationError::CandidateConflict {
            current: format!("{}:{}", current.base_digest, current.object_digest),
            proposed: format!("{}:{}", proposed.base_digest, proposed.object_digest),
        });
    }
    Ok(())
}

fn agent_application_error(error: ApplicationError) -> AgentToolError {
    match error {
        ApplicationError::Transition(_)
        | ApplicationError::IdempotencyConflict { .. }
        | ApplicationError::CandidateConflict { .. }
        | ApplicationError::ObjectStore(ObjectStoreError::Missing(_))
        | ApplicationError::ObjectStore(ObjectStoreError::DigestMismatch { .. }) => {
            AgentToolError::rejected(error.to_string())
        }
        _ => AgentToolError::internal("controller could not commit the tool call"),
    }
}

fn verification_evidence_error(error: VerifierError) -> ApplicationError {
    match error {
        VerifierError::AmbiguousLegacyEvidence => {
            VerificationInfrastructureError::AmbiguousLegacyEvidence.into()
        }
        VerifierError::MissingEnvironmentBinding => {
            VerificationInfrastructureError::MissingEnvironmentBinding.into()
        }
        error => VerificationInfrastructureError::InvalidEvidence(error.to_string()).into(),
    }
}

fn verify_environment_object(
    object_store: &ObjectStore,
    environment_digest: &str,
) -> Result<(), VerificationInfrastructureError> {
    match object_store.verify(environment_digest) {
        Ok(()) => Ok(()),
        Err(ObjectStoreError::Missing(_)) => {
            Err(VerificationInfrastructureError::EnvironmentObjectMissing(
                environment_digest.to_owned(),
            ))
        }
        Err(ObjectStoreError::DigestMismatch(_)) => {
            Err(VerificationInfrastructureError::EnvironmentDigestMismatch(
                environment_digest.to_owned(),
            ))
        }
        Err(error) => Err(VerificationInfrastructureError::InvalidEvidence(
            error.to_string(),
        )),
    }
}

fn mark_infrastructure_error(data_root: &Path) -> Result<(), ApplicationError> {
    let metadata_path = data_root.join("run.json");
    let mut state: RunState = serde_json::from_slice(&fs::read(metadata_path)?)?;
    state.status = RunStatus::InfrastructureError;
    state.active_attempts.clear();
    write_state_metadata(data_root, &state)
}

fn write_state_metadata(data_root: &Path, state: &RunState) -> Result<(), ApplicationError> {
    let target = data_root.join("run.json");
    let temporary = data_root.join(format!(".run.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut file, state)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, target)?;
        File::open(data_root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// What an application does with a fact its journal holds and its memory never saw.
///
/// The state under test is the one a panic leaves behind when it lands between the whole-fact
/// append and the in-memory apply, and only the crate itself can put an application there: the
/// window is inside a single method, and no caller can be interrupted in the middle of it. The
/// interruption is therefore reproduced from the same pieces the commit uses, stopping where the
/// panic stopped — the record complete, memory untouched.
#[cfg(test)]
mod interrupted_commit {
    use super::*;
    use tempfile::tempdir;

    /// Commit a command as far as the journal and no further, and answer with the fact the
    /// journal now holds.
    fn interrupt_after_append(
        application: &mut Application,
        command_id: &str,
        command: &Command,
    ) -> EventEnvelope {
        let event = application
            .state
            .decide(command)
            .expect("decide the interrupted command");
        let envelope = EventEnvelope::new(
            &application.state.run_id,
            application.state.last_sequence + 1,
            command_id,
            command.digest().expect("digest the interrupted command"),
            Some(application.state.last_event_digest.clone()),
            event,
        )
        .expect("build the interrupted event");
        application
            .journal
            .append(&envelope)
            .expect("append the interrupted fact");
        envelope
    }

    #[test]
    fn an_interrupted_apply_is_recovered_and_the_command_after_it_is_committed() {
        let temporary = tempdir().expect("temporary directory");
        let mut application = Application::create(temporary.path(), "run-1", Budget::new(2, 1))
            .expect("create application");
        let interrupted = interrupt_after_append(
            &mut application,
            "start-1",
            &Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        );
        assert_eq!(application.state().last_sequence, 1);
        assert_eq!(application.state().budget.attempts_remaining, 2);

        let next = application
            .execute(
                "start-2",
                Command::StartAttempt {
                    attempt_id: "attempt-2".to_owned(),
                },
            )
            .expect("commit the command that follows the interrupted one");

        assert!(!next.replayed);
        assert_eq!(next.event.sequence, 3);
        assert_eq!(application.state().last_sequence, 3);
        // One attempt for the recovered fact and one for the command that followed it: an
        // application that carried the recovered command out again would have spent more.
        assert_eq!(application.state().budget.attempts_remaining, 0);
        assert_eq!(
            application.state().active_attempts,
            vec!["attempt-1".to_owned(), "attempt-2".to_owned()]
        );

        let repeat = application
            .execute(
                "start-1",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("answer the repeated interrupted command");
        assert!(repeat.replayed);
        assert_eq!(repeat.event, interrupted);

        // The recovery read the journal and wrote nothing to it.
        let committed = application
            .journal
            .read_committed()
            .expect("read the committed facts");
        assert_eq!(committed.len(), 3);
        assert_eq!(committed[1], interrupted);

        let recovered = application.state().clone();
        drop(application);
        let reopened = Application::open(temporary.path()).expect("reopen the data root");
        assert_eq!(reopened.state(), &recovered);
    }

    #[test]
    fn a_recovered_projection_holds_the_candidate_its_journal_records() {
        let temporary = tempdir().expect("temporary directory");
        let mut application = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
            .expect("create application");
        let candidate = application
            .object_store()
            .put(b"candidate bytes")
            .expect("store the candidate");
        application
            .execute(
                "start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("start the attempt");
        let interrupted = interrupt_after_append(
            &mut application,
            "submit",
            &Command::SubmitCandidate {
                attempt_id: "attempt-1".to_owned(),
                base_digest: "0".repeat(64),
                object_digest: candidate.clone(),
            },
        );
        assert!(application.state().candidate_digest.is_none());

        // The recovery restores the whole projection and not the sequence alone: the candidate
        // the journal records is immutable, so a different one is refused rather than accepted
        // over it.
        let other = application
            .object_store()
            .put(b"a different candidate")
            .expect("store the second candidate");
        assert!(matches!(
            application.execute(
                "submit-other",
                Command::SubmitCandidate {
                    attempt_id: "attempt-1".to_owned(),
                    base_digest: "0".repeat(64),
                    object_digest: other,
                }
            ),
            Err(ApplicationError::CandidateConflict { .. })
        ));
        assert_eq!(
            application.state().candidate_digest.as_deref(),
            Some(candidate.as_str())
        );
        assert_eq!(
            application.state().last_sequence,
            interrupted.sequence,
            "the refused command left the recovered projection where the journal stands"
        );
        assert_eq!(
            application
                .journal
                .read_committed()
                .expect("read the committed facts")
                .len(),
            3
        );
    }
}
