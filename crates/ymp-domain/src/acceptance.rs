use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{AcceptanceContract, CriterionId, DomainError, required_text};

/// Returns the lowercase SHA-256 digest of an exact byte sequence.
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A deterministic method declared by the caller for checking criteria.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Check {
    criteria: Vec<CriterionId>,
    method: CheckMethod,
}

impl Check {
    pub fn command(
        criteria: Vec<CriterionId>,
        command: impl Into<String>,
    ) -> Result<Self, DomainError> {
        Self::new(
            criteria,
            CheckMethod::Command {
                command: required_text(command.into(), "check command")?,
            },
        )
    }

    pub fn exact_bytes(
        criteria: Vec<CriterionId>,
        path: impl Into<PathBuf>,
        expected: Vec<u8>,
    ) -> Result<Self, DomainError> {
        let path = path.into();
        validate_evidence_path(&path)?;
        Self::new(criteria, CheckMethod::ExactBytes { path, expected })
    }

    fn new(criteria: Vec<CriterionId>, method: CheckMethod) -> Result<Self, DomainError> {
        if criteria.is_empty() {
            return Err(DomainError::EmptyCheckCoverage);
        }
        let mut unique = HashSet::with_capacity(criteria.len());
        for criterion in &criteria {
            if !unique.insert(criterion) {
                return Err(DomainError::DuplicateCheckCriterion(criterion.clone()));
            }
        }
        Ok(Self { criteria, method })
    }

    pub fn criteria(&self) -> &[CriterionId] {
        &self.criteria
    }

    pub fn method(&self) -> &CheckMethod {
        &self.method
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckMethod {
    Command { command: String },
    ExactBytes { path: PathBuf, expected: Vec<u8> },
}

/// Exact bytes captured for evidence, including the digest stored with them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceFile {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
    sha256: Option<String>,
}

impl EvidenceFile {
    pub fn new(path: impl Into<PathBuf>, bytes: Option<Vec<u8>>) -> Result<Self, DomainError> {
        let path = path.into();
        validate_evidence_path(&path)?;
        let sha256 = bytes.as_deref().map(sha256);
        Ok(Self {
            path,
            bytes,
            sha256,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
    }
}

/// The immutable digest of a program used to execute a check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifierDigest {
    path: PathBuf,
    sha256: String,
}

impl VerifierDigest {
    pub fn new(path: impl Into<PathBuf>, sha256: impl Into<String>) -> Result<Self, DomainError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(DomainError::InvalidVerifierPath(path));
        }
        let sha256 = sha256.into();
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvalidSha256);
        }
        Ok(Self { path, sha256 })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceOutcome {
    Satisfied,
    Failed,
}

/// An observed check result. Constructors derive the result from the command
/// exit code or the captured bytes; callers cannot supply their own verdict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evidence {
    check: Check,
    workspace: String,
    outcome: EvidenceOutcome,
    exit_code: Option<i32>,
    elapsed_ms: u64,
    files: Vec<EvidenceFile>,
    verifiers: Vec<VerifierDigest>,
}

impl Evidence {
    pub fn command(
        check: Check,
        workspace: impl Into<String>,
        exit_code: i32,
        elapsed_ms: u64,
        verifiers: Vec<VerifierDigest>,
    ) -> Result<Self, DomainError> {
        if !matches!(check.method(), CheckMethod::Command { .. }) || verifiers.is_empty() {
            return Err(DomainError::EvidenceMethodMismatch);
        }
        Ok(Self {
            check,
            workspace: required_text(workspace.into(), "evidence workspace")?,
            outcome: if exit_code == 0 {
                EvidenceOutcome::Satisfied
            } else {
                EvidenceOutcome::Failed
            },
            exit_code: Some(exit_code),
            elapsed_ms,
            files: Vec::new(),
            verifiers,
        })
    }

    pub fn exact_bytes(
        check: Check,
        workspace: impl Into<String>,
        elapsed_ms: u64,
        actual: EvidenceFile,
    ) -> Result<Self, DomainError> {
        let CheckMethod::ExactBytes { path, expected } = check.method() else {
            return Err(DomainError::EvidenceMethodMismatch);
        };
        if actual.path() != path {
            return Err(DomainError::EvidenceMethodMismatch);
        }
        let outcome = if actual.bytes() == Some(expected.as_slice()) {
            EvidenceOutcome::Satisfied
        } else {
            EvidenceOutcome::Failed
        };
        Ok(Self {
            check,
            workspace: required_text(workspace.into(), "evidence workspace")?,
            outcome,
            exit_code: None,
            elapsed_ms,
            files: vec![actual],
            verifiers: Vec::new(),
        })
    }

    pub fn check(&self) -> &Check {
        &self.check
    }

    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    pub fn satisfied(&self) -> bool {
        self.outcome == EvidenceOutcome::Satisfied
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    pub fn files(&self) -> &[EvidenceFile] {
        &self.files
    }

    pub fn verifiers(&self) -> &[VerifierDigest] {
        &self.verifiers
    }
}

/// Criterion coverage derived from evidence. This is not the kernel's final
/// `Acceptance` decision, which requires a bound result version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriterionEvaluation {
    criteria: Vec<(CriterionId, CriterionStatus)>,
}

impl CriterionEvaluation {
    /// Coverage follows the prior proven rule: every criterion needs supporting
    /// evidence, and any applicable failed check keeps acceptance unsatisfied.
    pub fn evaluate(
        contract: &AcceptanceContract,
        evidence: &[Evidence],
    ) -> Result<Self, DomainError> {
        let declared = contract
            .criteria()
            .iter()
            .map(|criterion| criterion.id())
            .collect::<HashSet<_>>();
        for item in evidence {
            for criterion in item.check().criteria() {
                if !declared.contains(criterion) {
                    return Err(DomainError::UnknownEvidenceCriterion(criterion.clone()));
                }
            }
        }

        let criteria = contract
            .criteria()
            .iter()
            .map(|criterion| {
                let applicable = evidence
                    .iter()
                    .filter(|item| item.check().criteria().contains(criterion.id()))
                    .collect::<Vec<_>>();
                let status = if applicable.iter().any(|item| !item.satisfied()) {
                    CriterionStatus::Failed
                } else if applicable.iter().any(|item| item.satisfied()) {
                    CriterionStatus::Satisfied
                } else {
                    CriterionStatus::NotEvaluated
                };
                (criterion.id().clone(), status)
            })
            .collect();
        Ok(Self { criteria })
    }

    pub fn criteria(&self) -> impl Iterator<Item = (&CriterionId, CriterionStatus)> {
        self.criteria.iter().map(|(id, status)| (id, *status))
    }

    pub fn status(&self, criterion: &CriterionId) -> Option<CriterionStatus> {
        self.criteria
            .iter()
            .find_map(|(id, status)| (id == criterion).then_some(*status))
    }

    pub fn satisfied(&self) -> bool {
        !self.criteria.is_empty()
            && self
                .criteria
                .iter()
                .all(|(_, status)| *status == CriterionStatus::Satisfied)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CriterionStatus {
    Satisfied,
    Failed,
    NotEvaluated,
}

fn validate_evidence_path(path: &Path) -> Result<(), DomainError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        Err(DomainError::UnsafeEvidencePath(path.to_owned()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Criterion;

    fn criterion(id: &str) -> CriterionId {
        CriterionId::new(id).expect("valid criterion ID")
    }

    fn contract(ids: &[&str]) -> AcceptanceContract {
        AcceptanceContract::new(
            ids.iter()
                .map(|id| {
                    Criterion::new(criterion(id), format!("criterion {id}"))
                        .expect("valid criterion")
                })
                .collect(),
        )
        .expect("valid contract")
    }

    fn verifier() -> VerifierDigest {
        VerifierDigest::new("/bin/sh", "a".repeat(64)).expect("valid verifier digest")
    }

    #[test]
    fn command_exit_code_derives_evidence_outcome() {
        let satisfied = Evidence::command(
            Check::command(vec![criterion("one")], "echo ok").expect("valid check"),
            "/workspace",
            0,
            12,
            vec![verifier()],
        )
        .expect("valid evidence");
        let failed = Evidence::command(
            Check::command(vec![criterion("one")], "exit 1").expect("valid check"),
            "/workspace",
            1,
            7,
            vec![verifier()],
        )
        .expect("valid evidence");

        assert!(satisfied.satisfied());
        assert!(!failed.satisfied());
        assert_eq!(satisfied.exit_code(), Some(0));
        assert_eq!(failed.exit_code(), Some(1));
    }

    #[test]
    fn exact_bytes_preserve_the_observed_digest() {
        let check = Check::exact_bytes(vec![criterion("file")], "result.txt", b"abc".to_vec())
            .expect("valid check");
        let snapshot =
            EvidenceFile::new("result.txt", Some(b"abc".to_vec())).expect("valid snapshot");
        let evidence =
            Evidence::exact_bytes(check, "/workspace", 3, snapshot).expect("valid evidence");

        assert!(evidence.satisfied());
        assert_eq!(
            evidence.files()[0].sha256(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }

    #[test]
    fn acceptance_requires_all_criteria_and_no_failed_check() {
        let contract = contract(&["one", "two"]);
        let one = Evidence::command(
            Check::command(vec![criterion("one")], "true").expect("valid check"),
            "/workspace",
            0,
            1,
            vec![verifier()],
        )
        .expect("valid evidence");
        let two_failed = Evidence::command(
            Check::command(vec![criterion("two")], "false").expect("valid check"),
            "/workspace",
            1,
            1,
            vec![verifier()],
        )
        .expect("valid evidence");

        let incomplete = CriterionEvaluation::evaluate(&contract, std::slice::from_ref(&one))
            .expect("evidence is in contract");
        assert_eq!(
            incomplete.status(&criterion("two")),
            Some(CriterionStatus::NotEvaluated)
        );
        assert!(!incomplete.satisfied());

        let failed = CriterionEvaluation::evaluate(&contract, &[one, two_failed])
            .expect("evidence is in contract");
        assert_eq!(
            failed.status(&criterion("two")),
            Some(CriterionStatus::Failed)
        );
        assert!(!failed.satisfied());
    }

    #[test]
    fn checks_reject_unsafe_paths_and_duplicate_coverage() {
        assert_eq!(
            Check::exact_bytes(vec![criterion("one")], "../outside", Vec::new()),
            Err(DomainError::UnsafeEvidencePath(PathBuf::from("../outside")))
        );
        assert_eq!(
            Check::command(vec![criterion("one"), criterion("one")], "true"),
            Err(DomainError::DuplicateCheckCriterion(criterion("one")))
        );
    }
}
