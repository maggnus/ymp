#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use ymp_domain::VerificationDecision;
use ymp_runtime_api::configure_process_group;
#[cfg(not(target_os = "linux"))]
use ymp_runtime_api::terminate_process_tree;

const EVIDENCE_SCHEMA_VERSION: u32 = 2;

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
    #[serde(flatten)]
    stored: StoredVerificationEvidence,
    #[serde(skip)]
    environment_object: Vec<u8>,
}

/// A validated read-only representation of evidence already stored by the application.
///
/// Unlike [`VerifiedEvidence`], this type carries no verifier-issued environment bytes and cannot
/// be passed to the application's verification-recording boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoredVerificationEvidence {
    schema_version: u32,
    candidate_digest: String,
    contract_digest: String,
    oracle_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    environment_digest: Option<String>,
    verifier_profile: String,
    observation_digest: String,
    decision: VerificationDecision,
    evidence_digest: String,
}

impl VerifiedEvidence {
    pub fn candidate_digest(&self) -> &str {
        self.stored.candidate_digest()
    }

    pub fn contract_digest(&self) -> &str {
        self.stored.contract_digest()
    }

    pub fn oracle_digest(&self) -> &str {
        self.stored.oracle_digest()
    }

    pub fn environment_digest(&self) -> Option<&str> {
        self.stored.environment_digest()
    }

    pub fn decision(&self) -> VerificationDecision {
        self.stored.decision()
    }

    pub fn evidence_digest(&self) -> &str {
        self.stored.evidence_digest()
    }

    pub fn object_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        self.stored.object_bytes()
    }

    pub fn environment_object(&self) -> &[u8] {
        &self.environment_object
    }

    /// Reads and validates a stored evidence object without recreating verifier-issued evidence.
    pub fn from_object_bytes(
        bytes: &[u8],
        evidence_digest: impl Into<String>,
    ) -> Result<StoredVerificationEvidence, VerifierError> {
        StoredVerificationEvidence::from_object_bytes(bytes, evidence_digest)
    }
}

impl StoredVerificationEvidence {
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }

    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }

    pub fn oracle_digest(&self) -> &str {
        &self.oracle_digest
    }

    pub fn environment_digest(&self) -> Option<&str> {
        self.environment_digest.as_deref()
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

    pub fn from_object_bytes(
        bytes: &[u8],
        evidence_digest: impl Into<String>,
    ) -> Result<Self, VerifierError> {
        let evidence_digest = evidence_digest.into();
        validate_digest("evidence", &evidence_digest)?;
        let mut evidence: Self = serde_json::from_slice(bytes)?;
        if evidence.schema_version == 1 {
            return Err(VerifierError::AmbiguousLegacyEvidence);
        }
        if evidence.schema_version != EVIDENCE_SCHEMA_VERSION {
            return Err(VerifierError::UnsupportedEvidenceSchema(
                evidence.schema_version,
            ));
        }
        if !evidence.evidence_digest.is_empty() {
            return Err(VerifierError::StoredEvidenceContainsDigest);
        }
        let actual_digest = ymp_domain::digest_bytes(bytes);
        if actual_digest != evidence_digest {
            return Err(VerifierError::EvidenceDigestMismatch {
                expected: evidence_digest,
                actual: actual_digest,
            });
        }
        validate_digest("candidate", &evidence.candidate_digest)?;
        validate_digest("contract", &evidence.contract_digest)?;
        validate_digest("oracle", &evidence.oracle_digest)?;
        let environment_digest = evidence
            .environment_digest
            .as_deref()
            .ok_or(VerifierError::MissingEnvironmentBinding)?;
        validate_digest("environment", environment_digest)?;
        evidence.evidence_digest = actual_digest;
        Ok(evidence)
    }

    fn bind_object_digest(&mut self) -> Result<(), serde_json::Error> {
        self.evidence_digest = ymp_domain::digest_bytes(&self.object_bytes()?);
        Ok(())
    }
}

pub trait EnvironmentBoundVerifier {
    fn verify_candidate_in_environment(
        &self,
        candidate_path: &Path,
        candidate_digest: &str,
        environment_path: &Path,
        environment_digest: &str,
    ) -> Result<VerifiedEvidence, VerifierError>;
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
        let environment_object = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "verifier_profile": "exact_digest_v1"
        }))?;
        let environment_digest = ymp_domain::digest_bytes(&environment_object);
        self.verify_candidate_with_environment_bytes(
            path.as_ref(),
            candidate_digest.into(),
            environment_object,
            environment_digest,
        )
    }

    fn verify_candidate_with_environment_bytes(
        &self,
        path: &Path,
        candidate_digest: String,
        environment_object: Vec<u8>,
        environment_digest: String,
    ) -> Result<VerifiedEvidence, VerifierError> {
        validate_digest("candidate", &candidate_digest)?;
        let check = check_exact_digest(path, &self.expected_digest)?;
        let decision = if check.matched && candidate_digest == self.expected_digest {
            VerificationDecision::Accept
        } else {
            VerificationDecision::Reject
        };
        let mut stored = StoredVerificationEvidence {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            candidate_digest,
            contract_digest: self.contract_digest.clone(),
            oracle_digest: self.oracle_digest.clone(),
            environment_digest: Some(environment_digest),
            verifier_profile: "exact_digest_v1".to_owned(),
            observation_digest: digest_serializable(&check)?,
            decision,
            evidence_digest: String::new(),
        };
        stored.bind_object_digest()?;
        Ok(VerifiedEvidence {
            stored,
            environment_object,
        })
    }
}

impl EnvironmentBoundVerifier for ExactDigestVerifier {
    fn verify_candidate_in_environment(
        &self,
        candidate_path: &Path,
        candidate_digest: &str,
        environment_path: &Path,
        environment_digest: &str,
    ) -> Result<VerifiedEvidence, VerifierError> {
        let environment_object = read_environment_object(environment_path, environment_digest)?;
        self.verify_candidate_with_environment_bytes(
            candidate_path,
            candidate_digest.to_owned(),
            environment_object,
            environment_digest.to_owned(),
        )
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
        let (environment_object, program_digest) = self.environment_binding()?;
        let environment_digest = ymp_domain::digest_bytes(&environment_object);
        self.verify_candidate_with_environment_bytes(
            candidate_directory.as_ref(),
            negative_control_directory.as_ref(),
            candidate_digest.into(),
            environment_object,
            environment_digest,
            &program_digest,
        )
    }

    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }

    pub fn oracle_digest(&self) -> &str {
        &self.oracle_digest
    }

    pub fn environment_object(&self) -> Result<Vec<u8>, VerifierError> {
        self.environment_binding().map(|(object, _)| object)
    }

    pub fn environment_digest(&self) -> Result<String, VerifierError> {
        Ok(ymp_domain::digest_bytes(&self.environment_object()?))
    }

    fn environment_binding(&self) -> Result<(Vec<u8>, String), VerifierError> {
        let program_digest = ymp_domain::digest_bytes(&std::fs::read(&self.program)?);
        let environment_object = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "verifier_profile": "bounded_command_v3",
            "program": self.program,
            "program_digest": program_digest,
            "arguments_before_subject": self.arguments_before_subject,
            "wall_time_limit_millis": self.wall_time_limit.as_millis(),
            "output_limit_bytes": self.output_limit_bytes,
            "subject_isolation": command_isolation_profile()
        }))?;
        Ok((environment_object, program_digest))
    }

    fn verify_candidate_with_environment_bytes(
        &self,
        candidate_directory: &Path,
        negative_control_directory: &Path,
        candidate_digest: String,
        environment_object: Vec<u8>,
        environment_digest: String,
        program_digest: &str,
    ) -> Result<VerifiedEvidence, VerifierError> {
        validate_digest("candidate", &candidate_digest)?;
        let negative = self.observe(negative_control_directory, program_digest)?;
        match negative.status_code {
            Some(1) => {}
            Some(0) => return Err(VerifierError::NegativeControlPassed),
            status => return Err(VerifierError::UnexpectedExit(status)),
        }

        let observation = self.observe(candidate_directory, program_digest)?;
        let decision = match observation.status_code {
            Some(0) => VerificationDecision::Accept,
            Some(1) => VerificationDecision::Reject,
            status => return Err(VerifierError::UnexpectedExit(status)),
        };
        let mut stored = StoredVerificationEvidence {
            schema_version: EVIDENCE_SCHEMA_VERSION,
            candidate_digest,
            contract_digest: self.contract_digest.clone(),
            oracle_digest: self.oracle_digest.clone(),
            environment_digest: Some(environment_digest),
            verifier_profile: "bounded_command_v3".to_owned(),
            observation_digest: digest_serializable(&observation)?,
            decision,
            evidence_digest: String::new(),
        };
        stored.bind_object_digest()?;
        Ok(VerifiedEvidence {
            stored,
            environment_object,
        })
    }

    fn observe(
        &self,
        subject: &Path,
        expected_program_digest: &str,
    ) -> Result<CommandObservation, VerifierError> {
        let metadata = std::fs::symlink_metadata(subject)
            .map_err(|_| VerifierError::InvalidSubject(subject.to_path_buf()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(VerifierError::InvalidSubject(subject.to_path_buf()));
        }
        let isolated = tempfile::tempdir()?;
        let isolated_subject = isolated.path().join("subject");
        copy_subject_tree(subject, &isolated_subject)?;
        let isolated_temporary = isolated.path().join("scratch");
        std::fs::create_dir(&isolated_temporary)?;
        let isolated_home = isolated.path().join("home");
        std::fs::create_dir(&isolated_home)?;
        let isolated_program =
            prepare_oracle(&self.program, isolated.path(), expected_program_digest)?;
        #[cfg(target_os = "linux")]
        let _ = &isolated_program;
        #[cfg(target_os = "linux")]
        let mut command = linux_namespace_command(
            isolated.path(),
            &self.arguments_before_subject,
            &isolated_temporary,
            &isolated_home,
        );
        #[cfg(not(target_os = "linux"))]
        let mut command = {
            let mut command = Command::new(&isolated_program);
            command
                .args(&self.arguments_before_subject)
                .arg(&isolated_subject)
                .current_dir(&isolated_subject);
            command
        };
        command
            .env_clear()
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("PATH", "/usr/bin:/bin");
        #[cfg(target_os = "linux")]
        command
            .env("HOME", "/tmp/home")
            .env("TMPDIR", "/tmp/scratch")
            .env("TMP", "/tmp/scratch")
            .env("TEMP", "/tmp/scratch");
        #[cfg(not(target_os = "linux"))]
        command
            .env("HOME", &isolated_home)
            .env("TMPDIR", &isolated_temporary)
            .env("TMP", &isolated_temporary)
            .env("TEMP", &isolated_temporary);
        command
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
                terminate_observation(&mut child)?;
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(VerifierError::OutputLimitExceeded(self.output_limit_bytes));
            }
            if started.elapsed() >= self.wall_time_limit {
                terminate_observation(&mut child)?;
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

#[cfg(target_os = "linux")]
fn command_isolation_profile() -> &'static str {
    "linux_user_mount_pid_net_ipc_uts_namespaces_v1"
}

#[cfg(not(target_os = "linux"))]
fn command_isolation_profile() -> &'static str {
    "ephemeral_copy_per_observation_v2"
}

fn terminate_observation(child: &mut Child) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        if child.try_wait()?.is_none() {
            child.kill()?;
        }
        let _ = child.wait()?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        terminate_process_tree(child)
    }
}

fn prepare_oracle(
    source: &Path,
    isolation_root: &Path,
    expected_digest: &str,
) -> Result<PathBuf, VerifierError> {
    let metadata = std::fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(VerifierError::InvalidProgram(source.to_path_buf()));
    }
    let source_check = check_exact_digest(source, expected_digest)?;
    if !source_check.matched {
        return Err(VerifierError::ProgramDigestMismatch {
            expected: source_check.expected_digest,
            actual: source_check.actual_digest,
        });
    }
    let oracle_directory = isolation_root.join("oracle");
    std::fs::create_dir(&oracle_directory)?;
    let destination = oracle_directory.join("oracle");
    std::fs::copy(source, &destination)?;
    let copied_check = check_exact_digest(&destination, expected_digest)?;
    if !copied_check.matched {
        return Err(VerifierError::ProgramDigestMismatch {
            expected: copied_check.expected_digest,
            actual: copied_check.actual_digest,
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o555))?;
        std::fs::set_permissions(&oracle_directory, std::fs::Permissions::from_mode(0o555))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = std::fs::metadata(&destination)?.permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&destination, permissions)?;
    }
    Ok(destination)
}

#[cfg(target_os = "linux")]
fn linux_namespace_command(
    isolation_root: &Path,
    arguments_before_subject: &[String],
    _temporary: &Path,
    _home: &Path,
) -> Command {
    // This boundary prevents observations from sharing writable scratch mounts, processes,
    // networking, or verifier capabilities. It deliberately does not claim to contain hostile
    // code from the rest of the outer disposable host.
    const ISOLATION_SCRIPT: &str = r#"
root=$1
shift
/usr/bin/mount --make-rprivate /
/usr/bin/mount --bind "$root" /tmp
/usr/bin/mount --bind /tmp/oracle /tmp/oracle
/usr/bin/mount -o remount,bind,ro /tmp/oracle
/usr/bin/mount -t tmpfs -o nosuid,nodev,mode=1777 tmpfs /var/tmp
/usr/bin/mount -t tmpfs -o nosuid,nodev,mode=1777 tmpfs /dev/shm
/usr/bin/mount -t tmpfs -o nosuid,nodev,mode=755 tmpfs /run
cd /tmp/subject
exec /usr/bin/setpriv \
  --no-new-privs \
  --bounding-set=-all \
  --inh-caps=-all \
  --ambient-caps=-all \
  --securebits=+noroot,+noroot_locked,+no_setuid_fixup,+no_setuid_fixup_locked \
  /tmp/oracle/oracle "$@" /tmp/subject
"#;
    let mut command = Command::new("/usr/bin/unshare");
    command
        .args([
            "--user",
            "--map-root-user",
            "--mount",
            "--pid",
            "--net",
            "--ipc",
            "--uts",
            "--fork",
            "--kill-child",
            "--mount-proc",
            "/bin/sh",
            "-eu",
            "-c",
            ISOLATION_SCRIPT,
            "ymp-verifier-isolation",
        ])
        .arg(isolation_root)
        .args(arguments_before_subject)
        .current_dir("/");
    command
}

fn copy_subject_tree(source: &Path, destination: &Path) -> Result<(), VerifierError> {
    std::fs::create_dir(destination)?;
    let mut entries = std::fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            return Err(VerifierError::InvalidSubject(source_path));
        }
        if metadata.is_dir() {
            copy_subject_tree(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            std::fs::copy(&source_path, &destination_path)?;
        } else {
            return Err(VerifierError::InvalidSubject(source_path));
        }
    }
    Ok(())
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
    #[error(
        "verifier program changed after its environment was bound: expected {expected}, found {actual}"
    )]
    ProgramDigestMismatch { expected: String, actual: String },
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
    #[error("verification environment object is missing: {0}")]
    EnvironmentObjectMissing(PathBuf),
    #[error("verification environment digest mismatch: expected {expected}, found {actual}")]
    EnvironmentDigestMismatch { expected: String, actual: String },
    #[error("verification evidence schema version 1 has no unambiguous environment binding")]
    AmbiguousLegacyEvidence,
    #[error("unsupported verification evidence schema version {0}")]
    UnsupportedEvidenceSchema(u32),
    #[error("verification evidence object is missing its environment binding")]
    MissingEnvironmentBinding,
    #[error("stored verification evidence unexpectedly contains a self digest")]
    StoredEvidenceContainsDigest,
    #[error("verification evidence digest mismatch: expected {expected}, found {actual}")]
    EvidenceDigestMismatch { expected: String, actual: String },
    #[error("{kind} digest is not a canonical lowercase SHA-256 digest: {value}")]
    InvalidDigest { kind: &'static str, value: String },
}

fn read_environment_object(path: &Path, expected_digest: &str) -> Result<Vec<u8>, VerifierError> {
    validate_digest("environment", expected_digest)?;
    let bytes = std::fs::read(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            VerifierError::EnvironmentObjectMissing(path.to_path_buf())
        } else {
            VerifierError::Io(error)
        }
    })?;
    let actual_digest = ymp_domain::digest_bytes(&bytes);
    if actual_digest != expected_digest {
        return Err(VerifierError::EnvironmentDigestMismatch {
            expected: expected_digest.to_owned(),
            actual: actual_digest,
        });
    }
    Ok(bytes)
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
    use super::{
        CommandVerifier, EVIDENCE_SCHEMA_VERSION, EnvironmentBoundVerifier, ExactDigestVerifier,
        StoredVerificationEvidence, VerifiedEvidence, VerifierError, check_exact_digest,
    };
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
        assert!(evidence.environment_digest().is_some());
        assert_eq!(evidence.evidence_digest().len(), 64);
        let serialized = serde_json::to_value(&evidence).expect("serialize issued evidence");
        assert_eq!(serialized["schema_version"], EVIDENCE_SCHEMA_VERSION);
        assert!(serialized.get("stored").is_none());
    }

    #[test]
    fn explicit_environment_binding_survives_serialization_and_detects_substitution() {
        let candidate = NamedTempFile::new().expect("temporary candidate");
        std::fs::write(candidate.path(), b"candidate").expect("write candidate");
        let candidate_digest = hex::encode(Sha256::digest(b"candidate"));
        let environment = NamedTempFile::new().expect("temporary environment");
        std::fs::write(environment.path(), b"environment-v1").expect("write environment");
        let environment_digest = hex::encode(Sha256::digest(b"environment-v1"));
        let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &candidate_digest)
            .expect("valid verifier");

        let evidence = verifier
            .verify_candidate_in_environment(
                candidate.path(),
                &candidate_digest,
                environment.path(),
                &environment_digest,
            )
            .expect("verify in environment");
        let bytes = evidence.object_bytes().expect("serialize evidence");
        let restored: StoredVerificationEvidence =
            VerifiedEvidence::from_object_bytes(&bytes, evidence.evidence_digest())
                .expect("restore evidence");
        assert_eq!(
            restored.environment_digest(),
            Some(environment_digest.as_str())
        );

        std::fs::write(environment.path(), b"environment-v2").expect("substitute environment");
        assert!(matches!(
            verifier.verify_candidate_in_environment(
                candidate.path(),
                &candidate_digest,
                environment.path(),
                &environment_digest,
            ),
            Err(VerifierError::EnvironmentDigestMismatch { .. })
        ));
    }

    #[test]
    fn stored_evidence_reader_preserves_typed_legacy_and_invalid_data_errors() {
        let legacy = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "candidate_digest": "1".repeat(64),
            "contract_digest": "2".repeat(64),
            "oracle_digest": "3".repeat(64),
            "verifier_profile": "exact_digest_v1",
            "observation_digest": "4".repeat(64),
            "decision": "accept",
            "evidence_digest": ""
        }))
        .expect("serialize legacy evidence");
        assert!(matches!(
            VerifiedEvidence::from_object_bytes(&legacy, ymp_domain::digest_bytes(&legacy),),
            Err(VerifierError::AmbiguousLegacyEvidence)
        ));

        let malformed = b"{";
        assert!(matches!(
            VerifiedEvidence::from_object_bytes(malformed, ymp_domain::digest_bytes(malformed),),
            Err(VerifierError::Json(_))
        ));
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
            Duration::from_secs(15),
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
    fn negative_control_cannot_seed_candidate_observation_through_working_directory() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let program = temporary.path().join("oracle");
        let marker = format!(".ymp-verifier-negative-marker-{}", std::process::id());
        std::fs::write(
            &program,
            format!("#!/bin/sh\nif [ -f {marker} ]; then exit 0; fi\ntouch {marker}\nexit 1\n"),
        )
        .expect("write oracle");
        let mut permissions = std::fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&program, permissions).expect("make executable");
        let candidate = temporary.path().join("candidate");
        let negative = temporary.path().join("negative");
        std::fs::create_dir(&candidate).expect("candidate directory");
        std::fs::create_dir(&negative).expect("negative directory");
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
            .expect("verification result");

        assert_eq!(evidence.decision(), VerificationDecision::Reject);
        assert!(!negative.join(&marker).exists());
        assert!(!candidate.join(&marker).exists());
    }

    #[cfg(unix)]
    #[test]
    fn negative_control_cannot_modify_the_oracle_used_for_the_candidate() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let program = temporary.path().join("oracle");
        let original = b"#!/bin/sh\nprintf '# mutation' >> \"$0\" 2>/dev/null || :\nif [ -f \"$1/pass\" ]; then exit 0; else exit 1; fi\n";
        std::fs::write(&program, original).expect("write oracle");
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
            .expect("verification result");

        assert_eq!(evidence.decision(), VerificationDecision::Accept);
        assert_eq!(std::fs::read(program).expect("read oracle"), original);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn observations_use_disposable_mount_pid_and_network_namespaces() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let program = temporary.path().join("oracle");
        let marker = format!("/tmp/ymp-verifier-namespace-marker-{}", std::process::id());
        let parent_mount = std::fs::read_link("/proc/self/ns/mnt").expect("parent mount namespace");
        let parent_pid = std::fs::read_link("/proc/self/ns/pid").expect("parent pid namespace");
        let parent_network =
            std::fs::read_link("/proc/self/ns/net").expect("parent network namespace");
        std::fs::write(
            &program,
            "#!/bin/sh\n\
             [ -z \"$(env | grep -Ev '^(HOME|LANG|LC_ALL|OLDPWD|PATH|PWD|SHLVL|TEMP|TMP|TMPDIR|_)=')\" ] || exit 2\n\
             [ \"$(readlink /proc/self/ns/mnt)\" != \"$1\" ] || exit 2\n\
             [ \"$(readlink /proc/self/ns/pid)\" != \"$2\" ] || exit 2\n\
             [ \"$(readlink /proc/self/ns/net)\" != \"$3\" ] || exit 2\n\
             if [ -e \"$4\" ]; then exit 0; fi\n\
             touch \"$4\"\n\
             exit 1\n",
        )
        .expect("write oracle");
        let mut permissions = std::fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&program, permissions).expect("make executable");
        let candidate = temporary.path().join("candidate");
        let negative = temporary.path().join("negative");
        std::fs::create_dir(&candidate).expect("candidate directory");
        std::fs::create_dir(&negative).expect("negative directory");
        let verifier = CommandVerifier::new(
            "1".repeat(64),
            "2".repeat(64),
            &program,
            vec![
                parent_mount.to_string_lossy().into_owned(),
                parent_pid.to_string_lossy().into_owned(),
                parent_network.to_string_lossy().into_owned(),
                marker.clone(),
            ],
            Duration::from_secs(5),
            1024,
        )
        .expect("command verifier");

        let result = verifier.verify_candidate(&candidate, &negative, "3".repeat(64));
        let _ = std::fs::remove_file(&marker);
        let evidence = result.expect("isolated verification result");

        assert_eq!(evidence.decision(), VerificationDecision::Reject);
        assert!(!std::path::Path::new(&marker).exists());
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
