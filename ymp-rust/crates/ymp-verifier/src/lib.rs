#![forbid(unsafe_code)]

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use ymp_domain::VerificationDecision;
use ymp_runtime_api::{configure_process_group, terminate_process_tree};

const EVIDENCE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DigestCheck {
    pub expected_digest: String,
    pub actual_digest: String,
    pub matched: bool,
}

/// Controller-owned verification evidence. Its fields are intentionally private, so callers
/// cannot manufacture an acceptance decision and commit it through `ymp-application`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifiedEvidence {
    schema_version: u32,
    candidate_digest: String,
    contract_digest: String,
    oracle_digest: String,
    verifier_profile: String,
    observation_digest: String,
    decision: VerificationDecision,
    evidence_digest: String,
}

impl VerifiedEvidence {
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }

    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }

    pub fn oracle_digest(&self) -> &str {
        &self.oracle_digest
    }

    pub fn decision(&self) -> VerificationDecision {
        self.decision
    }

    pub fn evidence_digest(&self) -> &str {
        &self.evidence_digest
    }

    pub fn object_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut body = self.clone();
        body.evidence_digest.clear();
        serde_json::to_vec(&body)
    }
}

#[derive(Clone, Debug)]
pub struct ExactDigestVerifier {
    contract_digest: String,
    oracle_digest: String,
    expected_digest: String,
}

impl ExactDigestVerifier {
    pub fn new(
        contract_digest: impl Into<String>,
        oracle_digest: impl Into<String>,
        expected_digest: impl Into<String>,
    ) -> Result<Self, VerifierError> {
        let verifier = Self {
            contract_digest: contract_digest.into(),
            oracle_digest: oracle_digest.into(),
            expected_digest: expected_digest.into(),
        };
        validate_digest("contract", &verifier.contract_digest)?;
        validate_digest("oracle", &verifier.oracle_digest)?;
        validate_digest("expected", &verifier.expected_digest)?;
        Ok(verifier)
    }

    pub fn verify_candidate(
        &self,
        path: impl AsRef<Path>,
        candidate_digest: impl Into<String>,
    ) -> Result<VerifiedEvidence, VerifierError> {
        let candidate_digest = candidate_digest.into();
        validate_digest("candidate", &candidate_digest)?;
        let check = check_exact_digest(path, &self.expected_digest)?;
        let decision = if check.matched && candidate_digest == self.expected_digest {
            VerificationDecision::Accept
        } else {
            VerificationDecision::Reject
        };
        let mut evidence = VerifiedEvidence {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            candidate_digest,
            contract_digest: self.contract_digest.clone(),
            oracle_digest: self.oracle_digest.clone(),
            verifier_profile: "exact_digest_v1".to_owned(),
            observation_digest: digest_serializable(&check)?,
            decision,
            evidence_digest: String::new(),
        };
        evidence.evidence_digest = ymp_domain::digest_bytes(&serde_json::to_vec(&evidence)?);
        Ok(evidence)
    }
}

#[derive(Clone, Debug)]
pub struct CommandVerifier {
    contract_digest: String,
    oracle_digest: String,
    program: PathBuf,
    arguments_before_subject: Vec<String>,
    wall_time_limit: Duration,
    output_limit_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
struct CommandObservation {
    success: bool,
    status_code: Option<i32>,
    stdout_digest: String,
    stderr_digest: String,
    stdout_bytes: u64,
    stderr_bytes: u64,
}

impl CommandVerifier {
    pub fn new(
        contract_digest: impl Into<String>,
        oracle_digest: impl Into<String>,
        program: impl Into<PathBuf>,
        arguments_before_subject: Vec<String>,
        wall_time_limit: Duration,
        output_limit_bytes: usize,
    ) -> Result<Self, VerifierError> {
        let verifier = Self {
            contract_digest: contract_digest.into(),
            oracle_digest: oracle_digest.into(),
            program: program.into(),
            arguments_before_subject,
            wall_time_limit,
            output_limit_bytes,
        };
        validate_digest("contract", &verifier.contract_digest)?;
        validate_digest("oracle", &verifier.oracle_digest)?;
        if !verifier.program.is_absolute() {
            return Err(VerifierError::InvalidProgram(verifier.program));
        }
        if verifier.wall_time_limit.is_zero() {
            return Err(VerifierError::InvalidLimit("wall time"));
        }
        if verifier.output_limit_bytes == 0 {
            return Err(VerifierError::InvalidLimit("output bytes"));
        }
        Ok(verifier)
    }

    pub fn verify_candidate(
        &self,
        candidate_directory: impl AsRef<Path>,
        negative_control_directory: impl AsRef<Path>,
        candidate_digest: impl Into<String>,
    ) -> Result<VerifiedEvidence, VerifierError> {
        let candidate_digest = candidate_digest.into();
        validate_digest("candidate", &candidate_digest)?;
        let negative = self.observe(negative_control_directory.as_ref())?;
        match negative.status_code {
            Some(1) => {}
            Some(0) => return Err(VerifierError::NegativeControlPassed),
            status => return Err(VerifierError::UnexpectedExit(status)),
        }

        let observation = self.observe(candidate_directory.as_ref())?;
        let decision = match observation.status_code {
            Some(0) => VerificationDecision::Accept,
            Some(1) => VerificationDecision::Reject,
            status => return Err(VerifierError::UnexpectedExit(status)),
        };
        let mut evidence = VerifiedEvidence {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            candidate_digest,
            contract_digest: self.contract_digest.clone(),
            oracle_digest: self.oracle_digest.clone(),
            verifier_profile: "bounded_command_v1".to_owned(),
            observation_digest: digest_serializable(&observation)?,
            decision,
            evidence_digest: String::new(),
        };
        evidence.evidence_digest = digest_serializable(&evidence)?;
        Ok(evidence)
    }

    fn observe(&self, subject: &Path) -> Result<CommandObservation, VerifierError> {
        if !subject.is_dir() {
            return Err(VerifierError::InvalidSubject(subject.to_path_buf()));
        }
        let mut command = Command::new(&self.program);
        command
            .args(&self.arguments_before_subject)
            .arg(subject)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_process_group(&mut command);
        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or(VerifierError::MissingProcessPipe)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(VerifierError::MissingProcessPipe)?;
        let exceeded = Arc::new(AtomicBool::new(false));
        let stdout_reader = digest_stream(stdout, self.output_limit_bytes, Arc::clone(&exceeded));
        let stderr_reader = digest_stream(stderr, self.output_limit_bytes, Arc::clone(&exceeded));
        let started = Instant::now();
        let status = loop {
            if exceeded.load(Ordering::Acquire) {
                terminate_process_tree(&mut child)?;
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(VerifierError::OutputLimitExceeded(self.output_limit_bytes));
            }
            if started.elapsed() >= self.wall_time_limit {
                terminate_process_tree(&mut child)?;
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(VerifierError::TimedOut(
                    self.wall_time_limit.as_millis() as u64
                ));
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| VerifierError::OutputReaderFailed)??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| VerifierError::OutputReaderFailed)??;
        if exceeded.load(Ordering::Acquire) {
            return Err(VerifierError::OutputLimitExceeded(self.output_limit_bytes));
        }
        Ok(CommandObservation {
            success: status.success(),
            status_code: status.code(),
            stdout_digest: stdout.digest,
            stderr_digest: stderr.digest,
            stdout_bytes: stdout.bytes,
            stderr_bytes: stderr.bytes,
        })
    }
}

struct StreamDigest {
    digest: String,
    bytes: u64,
}

fn digest_stream(
    mut stream: impl Read + Send + 'static,
    limit: usize,
    exceeded: Arc<AtomicBool>,
) -> thread::JoinHandle<Result<StreamDigest, std::io::Error>> {
    thread::spawn(move || {
        let mut digest = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 8192];
        loop {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.write_all(&buffer[..read])?;
            bytes = bytes.saturating_add(read as u64);
            if bytes > limit as u64 {
                exceeded.store(true, Ordering::Release);
            }
        }
        Ok(StreamDigest {
            digest: hex::encode(digest.finalize()),
            bytes,
        })
    })
}

#[derive(Debug, Error)]
pub enum VerifierError {
    #[error("verifier I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("verifier JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("verifier program path must be absolute: {0}")]
    InvalidProgram(PathBuf),
    #[error("verifier {0} limit must be greater than zero")]
    InvalidLimit(&'static str),
    #[error("verifier subject is not a directory: {0}")]
    InvalidSubject(PathBuf),
    #[error("verifier process did not expose a required output pipe")]
    MissingProcessPipe,
    #[error("verifier process output reader failed")]
    OutputReaderFailed,
    #[error("verifier process exceeded its {0}-byte output limit")]
    OutputLimitExceeded(usize),
    #[error("verifier process exceeded its {0}-millisecond wall-time limit")]
    TimedOut(u64),
    #[error("protected negative control passed")]
    NegativeControlPassed,
    #[error("verifier process returned an unclassified exit status: {0:?}")]
    UnexpectedExit(Option<i32>),
    #[error("{kind} digest is not a canonical lowercase SHA-256 digest: {value}")]
    InvalidDigest { kind: &'static str, value: String },
}

fn digest_serializable(value: &impl Serialize) -> Result<String, serde_json::Error> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value)?)))
}

pub fn check_exact_digest(
    path: impl AsRef<Path>,
    expected_digest: impl Into<String>,
) -> Result<DigestCheck, VerifierError> {
    let expected_digest = expected_digest.into();
    validate_digest("expected", &expected_digest)?;
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    let actual_digest = hex::encode(Sha256::digest(bytes));
    Ok(DigestCheck {
        matched: actual_digest == expected_digest,
        expected_digest,
        actual_digest,
    })
}

fn validate_digest(kind: &'static str, value: &str) -> Result<(), VerifierError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(VerifierError::InvalidDigest {
            kind,
            value: value.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandVerifier, ExactDigestVerifier, VerifierError, check_exact_digest};
    use sha2::{Digest, Sha256};
    use std::time::Duration;
    use tempfile::NamedTempFile;
    use ymp_domain::VerificationDecision;

    #[test]
    fn exact_digest_evidence_is_bound_to_candidate_contract_and_oracle() {
        let file = NamedTempFile::new().expect("temporary candidate");
        std::fs::write(file.path(), b"candidate").expect("write candidate");
        let digest = hex::encode(Sha256::digest(b"candidate"));
        let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &digest)
            .expect("valid verifier");

        let evidence = verifier
            .verify_candidate(file.path(), &digest)
            .expect("verify candidate");

        assert_eq!(evidence.decision(), VerificationDecision::Accept);
        assert_eq!(evidence.candidate_digest(), digest);
        assert_eq!(evidence.contract_digest(), "1".repeat(64));
        assert_eq!(evidence.oracle_digest(), "2".repeat(64));
        assert_eq!(evidence.evidence_digest().len(), 64);
    }

    #[test]
    fn mismatch_is_rejected_and_noncanonical_digests_are_invalid() {
        let file = NamedTempFile::new().expect("temporary candidate");
        std::fs::write(file.path(), b"candidate").expect("write candidate");
        let actual = hex::encode(Sha256::digest(b"candidate"));
        let expected = "f".repeat(64);
        let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &expected)
            .expect("valid verifier");
        let evidence = verifier
            .verify_candidate(file.path(), &actual)
            .expect("verify candidate");
        assert_eq!(evidence.decision(), VerificationDecision::Reject);
        assert!(
            !check_exact_digest(file.path(), expected)
                .expect("check digest")
                .matched
        );
        assert!(matches!(
            ExactDigestVerifier::new("invalid", "2".repeat(64), actual),
            Err(VerifierError::InvalidDigest { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_requires_a_failing_negative_control() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let program = temporary.path().join("oracle");
        std::fs::write(
            &program,
            "#!/bin/sh\nif [ -f \"$1/pass\" ]; then exit 0; else exit 1; fi\n",
        )
        .expect("write oracle");
        let mut permissions = std::fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&program, permissions).expect("make executable");
        let candidate = temporary.path().join("candidate");
        let negative = temporary.path().join("negative");
        std::fs::create_dir(&candidate).expect("candidate directory");
        std::fs::create_dir(&negative).expect("negative directory");
        std::fs::write(candidate.join("pass"), b"accepted\n").expect("candidate marker");
        let verifier = CommandVerifier::new(
            "1".repeat(64),
            "2".repeat(64),
            &program,
            Vec::new(),
            Duration::from_secs(5),
            1024,
        )
        .expect("command verifier");

        let evidence = verifier
            .verify_candidate(&candidate, &negative, "3".repeat(64))
            .expect("verified candidate");
        assert_eq!(evidence.decision(), VerificationDecision::Accept);

        std::fs::write(negative.join("pass"), b"invalid oracle\n").expect("negative marker");
        assert!(matches!(
            verifier.verify_candidate(&candidate, &negative, "3".repeat(64)),
            Err(VerifierError::NegativeControlPassed)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_stops_timeout_and_output_excess() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let subject = temporary.path().join("subject");
        std::fs::create_dir(&subject).expect("subject directory");
        let timeout_program = temporary.path().join("timeout-oracle");
        std::fs::write(&timeout_program, "#!/bin/sh\nsleep 30\n").expect("write timeout oracle");
        let output_program = temporary.path().join("output-oracle");
        std::fs::write(
            &output_program,
            "#!/bin/sh\nwhile :; do printf '0123456789'; done\n",
        )
        .expect("write output oracle");
        for program in [&timeout_program, &output_program] {
            let mut permissions = std::fs::metadata(program).expect("metadata").permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(program, permissions).expect("make executable");
        }

        let timeout = CommandVerifier::new(
            "1".repeat(64),
            "2".repeat(64),
            &timeout_program,
            Vec::new(),
            Duration::from_millis(50),
            1024,
        )
        .expect("timeout verifier");
        assert!(matches!(
            timeout.verify_candidate(&subject, &subject, "3".repeat(64)),
            Err(VerifierError::TimedOut(50))
        ));

        let output = CommandVerifier::new(
            "1".repeat(64),
            "2".repeat(64),
            &output_program,
            Vec::new(),
            Duration::from_secs(5),
            128,
        )
        .expect("output verifier");
        let error = output
            .verify_candidate(&subject, &subject, "3".repeat(64))
            .expect_err("reject excessive output");
        assert!(
            matches!(error, VerifierError::OutputLimitExceeded(128)),
            "unexpected error: {error:?}"
        );
    }
}
