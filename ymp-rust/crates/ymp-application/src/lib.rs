#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use thiserror::Error;
use uuid::Uuid;
use ymp_agent_api::{
    AgentToolCall, AgentToolError, AgentToolHandler, MAX_EVENT_PAGE, ReadEventsArguments,
    SubmitArguments,
};
use ymp_artifacts::{ArtifactError, ArtifactStore, CandidateRef, SubmissionRef};
use ymp_domain::{
    Budget, Command, EventEnvelope, EventKind, MAX_IDENTIFIER_CHARS, RunState, RunStatus,
    TransitionError, VerificationRecord,
};
use ymp_storage::{
    DataRootLock, Journal, JournalError, JournalLimits, ObjectStore, ObjectStoreError,
};
use ymp_verifier::{
    EnvironmentBoundVerifier, StoredVerificationEvidence, VerifiedEvidence, VerifierError,
};

pub mod commitment;
pub mod contract;
pub mod root;

pub use commitment::{CommitmentOutcome, CommitmentService, CommitmentServiceError, RecordedFact};
pub use contract::{
    AcceptanceCondition, ContractRequestError, DEFAULT_RUN_BUDGET, PreparedContract, RunRequest,
    load_contract_package, prepare_contract,
};

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
    #[error("evidence export destination already exists: {0}")]
    ExportAlreadyExists(PathBuf),
    #[error("a candidate must be submitted before evidence can be exported")]
    NoCandidateForExport,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceExportReport {
    pub destination: PathBuf,
    pub candidate_digest: String,
    pub evidence_digests: Vec<String>,
    pub environment_digests: Vec<String>,
    pub event_count: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ApplicationConfig {
    pub journal_limits: JournalLimits,
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
        let mut state = RunState::from_start(first).ok_or(ApplicationError::InvalidFirstEvent)?;
        let object_store = ObjectStore::open(data_root.join("objects/sha256"))?;
        let mut command_results = HashMap::new();
        command_results.insert(
            first.command_id.clone(),
            RecordedCommandResult {
                event: first.clone(),
                status: state.status,
            },
        );
        let mut candidate_identity = None;
        let mut recorded_verifications = HashMap::new();

        for event in events.iter().skip(1) {
            if let Some(result) = command_results.get(&event.command_id)
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
                    object_store.verify(object_digest)?;
                    let proposed = CandidateIdentity {
                        base_digest: base_digest.clone(),
                        object_digest: object_digest.clone(),
                    };
                    ensure_candidate_identity(candidate_identity.as_ref(), &proposed)?;
                    candidate_identity = Some(proposed);
                }
                EventKind::VerificationRecorded {
                    candidate_digest,
                    contract_digest,
                    oracle_digest,
                    evidence_digest,
                    accepted,
                } => {
                    let bytes = object_store.read(evidence_digest)?;
                    let evidence =
                        StoredVerificationEvidence::from_object_bytes(&bytes, evidence_digest)
                            .map_err(verification_evidence_error)?;
                    let environment_digest = evidence
                        .environment_digest()
                        .ok_or(VerificationInfrastructureError::MissingEnvironmentBinding)?;
                    verify_environment_object(&object_store, environment_digest)?;
                    if evidence.candidate_digest() != candidate_digest
                        || evidence.contract_digest() != contract_digest
                        || evidence.oracle_digest() != oracle_digest
                        || (evidence.decision() == ymp_domain::VerificationDecision::Accept)
                            != *accepted
                    {
                        return Err(VerificationInfrastructureError::EvidenceRecordMismatch.into());
                    }
                    recorded_verifications
                        .entry(VerificationIdentity {
                            candidate_digest: candidate_digest.clone(),
                            contract_digest: contract_digest.clone(),
                            oracle_digest: oracle_digest.clone(),
                            environment_digest: environment_digest.to_owned(),
                        })
                        .or_insert_with(|| event.clone());
                }
                _ => {}
            }
            state.apply(event);
            command_results
                .entry(event.command_id.clone())
                .or_insert_with(|| RecordedCommandResult {
                    event: event.clone(),
                    status: state.status,
                });
        }

        Ok(Self {
            data_root,
            _lock: lock,
            journal,
            object_store,
            state,
            command_results,
            candidate_identity,
            recorded_verifications,
            notification_senders: Vec::new(),
        })
    }

    pub fn execute(
        &mut self,
        command_id: impl Into<String>,
        command: Command,
    ) -> Result<CommandOutcome, ApplicationError> {
        let command_id = command_id.into();
        validate_identifier("command_id", &command_id)?;
        let command_digest = command.digest()?;
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
