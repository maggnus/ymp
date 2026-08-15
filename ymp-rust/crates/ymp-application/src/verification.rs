//! Judging the run's candidate with the verifier its approved contract names.
//!
//! One verification is three steps, and they are kept apart because the middle one runs a program
//! of the operator's choosing and waits for it: reading what the run is judged against and putting
//! the candidate somewhere the verifier can read it; running the verifier; recording what it
//! decided. Only the first and the third touch the store, so a caller that must keep drawing can
//! move the middle step to a thread of its own, and a caller whose process is the wait runs all
//! three inline.
//!
//! Nothing here chooses the acceptance condition. The program, its arguments, the negative control
//! it must reject and the limits it runs under are read from the contract object the run was
//! started against, so a verification cannot be made to judge a candidate by anything other than
//! what the operator authorized.

use std::path::{Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;
use ymp_domain::contract::ContractDocument;
use ymp_domain::{Command, ContractBinding};
use ymp_verifier::{CommandVerifier, VerifiedEvidence};

use crate::{Application, ApplicationError, CommandOutcome};

/// Everything one verification needs, holding nothing that keeps the store open.
///
/// It is `Send`, so the caller decides which thread runs the verifier.
#[derive(Clone, Debug)]
pub struct VerificationJob {
    contract_digest: String,
    oracle_digest: String,
    program: PathBuf,
    arguments: Vec<String>,
    negative_control: PathBuf,
    wall_time: Duration,
    output_limit_bytes: usize,
    candidate_digest: String,
    candidate: PathBuf,
}

/// What one verification produced.
#[derive(Debug)]
pub enum VerificationOutcome {
    /// The verifier decided. The evidence carries the decision and the environment it was bound
    /// to; whether that decision was acceptance or rejection is the evidence's to state.
    Judged(Box<VerifiedEvidence>),
    /// The verifier did not decide. This is an infrastructure condition and never a rejection:
    /// nothing about the candidate was established.
    Undecided { reason: String },
}

impl VerificationJob {
    /// The candidate this job judges.
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }

    /// What a caller says it is waiting for while this runs.
    pub fn waiting_for(&self) -> String {
        format!(
            "verifying candidate {} with {} · limit {} s",
            &self.candidate_digest[..self.candidate_digest.len().min(10)],
            file_name(&self.program),
            self.wall_time.as_secs().max(1)
        )
    }

    /// Run the verifier over the candidate and the negative control it must reject.
    ///
    /// A verifier that cannot be built, cannot be run, exceeds its limit or answers about
    /// something other than this candidate leaves the run undecided rather than rejected.
    pub fn run(self) -> VerificationOutcome {
        let verifier = match CommandVerifier::new(
            self.contract_digest,
            self.oracle_digest,
            self.program,
            self.arguments,
            self.wall_time,
            self.output_limit_bytes,
        ) {
            Ok(verifier) => verifier,
            Err(error) => {
                return VerificationOutcome::Undecided {
                    reason: format!("the verifier could not be prepared: {error}"),
                };
            }
        };
        match verifier.verify_candidate(
            &self.candidate,
            &self.negative_control,
            &self.candidate_digest,
        ) {
            Ok(evidence) => VerificationOutcome::Judged(Box::new(evidence)),
            Err(error) => VerificationOutcome::Undecided {
                reason: format!("the verifier failed: {error}"),
            },
        }
    }
}

impl Application {
    /// The contract this run was approved against, read back from the store as an object.
    ///
    /// The journal binds the run to a digest and the object store holds the bytes that carry it;
    /// both are read here, and a store whose object no longer matches its binding is refused
    /// rather than described. This is what a later process starts an attempt against: the
    /// authorization is in the store, not in the session that gave it.
    pub fn approved_contract(
        &self,
    ) -> Result<(ContractBinding, ContractDocument), ApplicationError> {
        let binding = self
            .contract()
            .ok_or(ApplicationError::NoApprovedContract)?
            .clone();
        let bytes = self
            .contract_bytes()?
            .ok_or(ApplicationError::NoApprovedContract)?;
        if ymp_domain::digest_bytes(&bytes) != binding.contract_digest {
            return Err(ApplicationError::ContractDigestMismatch);
        }
        let document = ContractDocument::parse(&bytes)
            .map_err(|error| ApplicationError::ContractUnreadable(error.to_string()))?
            .document;
        if document.verifier.oracle_digest != binding.oracle_digest {
            return Err(ApplicationError::ContractUnreadable(
                "the stored contract names a different oracle from the one the run was approved \
                 against"
                    .to_owned(),
            ));
        }
        Ok((binding, document))
    }

    /// Everything the run's committed candidate has to be judged by, with the candidate placed
    /// under `staging` where the verifier can read it.
    ///
    /// The store is read here and released; the work the job describes happens without it.
    pub fn verification_job(
        &self,
        staging: impl AsRef<Path>,
    ) -> Result<VerificationJob, ApplicationError> {
        let candidate_digest = self
            .state()
            .candidate_digest
            .clone()
            .ok_or(ApplicationError::NoCandidateForVerification)?;
        let (binding, document) = self.approved_contract()?;
        let candidate = staging
            .as_ref()
            .join(format!("{candidate_digest}-{}", Uuid::new_v4()));
        self.artifact_store()
            .materialize(&candidate_digest, &candidate)?;
        Ok(VerificationJob {
            contract_digest: binding.contract_digest,
            oracle_digest: document.verifier.oracle_digest,
            program: document.verifier.program,
            arguments: document.verifier.arguments,
            negative_control: document.verifier.negative_control,
            wall_time: Duration::from_millis(document.verifier.wall_time_ms),
            output_limit_bytes: document.verifier.output_limit_bytes,
            candidate_digest,
            candidate,
        })
    }

    /// Record what one verification decided.
    ///
    /// A decision becomes a verification record and moves the run to its terminal outcome. A
    /// verifier that did not decide becomes an infrastructure failure carrying the reason, so the
    /// run never reads as a candidate that was rejected on evidence nobody produced.
    pub fn record_verification_outcome(
        &mut self,
        command_id: impl Into<String>,
        outcome: VerificationOutcome,
    ) -> Result<CommandOutcome, ApplicationError> {
        match outcome {
            VerificationOutcome::Judged(evidence) => {
                self.record_verification(command_id, &evidence)
            }
            VerificationOutcome::Undecided { reason } => self.execute(
                command_id,
                Command::FailInfrastructure {
                    reason: reason.chars().take(ymp_domain::MAX_REASON_BYTES).collect(),
                },
            ),
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
