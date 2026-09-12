//! Explicit client-owned acceptance contracts and runtime-captured evidence.
use crate::{content_digest, TaskAttemptRef};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

pub fn bytes_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub id: String,
    pub description: String,
}

/// Installed by the trusted client before a run, never parsed from model text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceContract {
    pub task_title: String,
    pub criteria: Vec<AcceptanceCriterion>,
    pub artifacts: Vec<PathBuf>,
    pub inputs: Vec<PathBuf>,
    pub checks: Vec<TrustedCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedCheck {
    pub id: String,
    pub criterion_ids: Vec<String>,
    pub assertion: CheckAssertion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckAssertion {
    ExactBytes {
        artifact: PathBuf,
        expected: Vec<u8>,
    },
    MatchesInput {
        artifact: PathBuf,
        input: PathBuf,
    },
    /// The trusted client declares the command's meaning and pins its code.
    /// Arguments are passed directly; `{workdir}` expands to the selected cwd.
    Command {
        program: PathBuf,
        args: Vec<String>,
        verifier_files: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: PathBuf,
    /// Missing output is evidence too. Inputs and verifier code must exist.
    pub bytes: Option<Vec<u8>>,
    pub sha256: Option<String>,
}
impl FileSnapshot {
    pub fn capture(root: &Path, path: &Path) -> Result<Self> {
        ensure!(
            !path.is_absolute() && path.components().all(|c| matches!(c, Component::Normal(_))),
            "Evidence paths must be relative and stay inside the working directory"
        );
        let root = root.canonicalize()?;
        let full = root.join(path);
        // Check the nearest existing ancestor as well as an existing file.
        let ancestor = full
            .ancestors()
            .find(|p| p.exists())
            .context("Missing evidence ancestor")?;
        ensure!(
            ancestor.canonicalize()?.starts_with(&root),
            "Evidence path escapes the working directory"
        );
        let bytes = if full.exists() {
            ensure!(full.is_file(), "Evidence path is not a file");
            ensure!(
                full.metadata()?.len() <= 4 * 1024 * 1024,
                "Evidence file exceeds 4 MiB capture limit"
            );
            Some(std::fs::read(full)?)
        } else {
            None
        };
        let sha256 = bytes.as_deref().map(bytes_digest);
        Ok(Self {
            path: path.into(),
            bytes,
            sha256,
        })
    }
    pub fn current(&self, root: &Path) -> bool {
        Self::capture(root, &self.path).is_ok_and(|snapshot| snapshot == *self)
    }
    pub fn valid(&self) -> bool {
        self.sha256 == self.bytes.as_deref().map(bytes_digest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckerIdentity {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedAcceptanceContract {
    pub checker: CheckerIdentity,
    pub contract: AcceptanceContract,
    pub version: String,
    pub inputs: Vec<FileSnapshot>,
    /// Absolute trusted code paths and their immutable initial byte digest.
    pub verifier_digests: Vec<(PathBuf, String)>,
}
impl CapturedAcceptanceContract {
    pub fn capture(
        contract: AcceptanceContract,
        root: &Path,
        checker: CheckerIdentity,
    ) -> Result<Self> {
        ensure!(
            !checker.id.is_empty() && !checker.version.is_empty(),
            "Checker identity and version are required"
        );
        ensure!(
            !contract.task_title.trim().is_empty() && !contract.criteria.is_empty(),
            "Acceptance contract requires a task and criteria"
        );
        let ids = contract
            .criteria
            .iter()
            .map(|c| &c.id)
            .collect::<std::collections::HashSet<_>>();
        ensure!(
            ids.len() == contract.criteria.len()
                && contract
                    .criteria
                    .iter()
                    .all(|c| !c.id.is_empty() && !c.description.trim().is_empty()),
            "Criteria need unique IDs and descriptions"
        );
        ensure!(
            !contract.artifacts.is_empty(),
            "An objective contract must identify artifacts"
        );
        for path in &contract.artifacts {
            FileSnapshot::capture(root, path)?;
        }
        let inputs = contract
            .inputs
            .iter()
            .map(|p| FileSnapshot::capture(root, p))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            inputs.iter().all(|s| s.bytes.is_some()),
            "Declared input is missing"
        );
        let mut verifier_digests = Vec::new();
        let mut check_ids = std::collections::HashSet::new();
        for check in &contract.checks {
            ensure!(
                !check.id.is_empty()
                    && check_ids.insert(&check.id)
                    && !check.criterion_ids.is_empty(),
                "Checks require unique IDs and coverage"
            );
            ensure!(
                check.criterion_ids.iter().all(|id| ids.contains(id)),
                "Check covers an undeclared criterion"
            );
            match &check.assertion {
                CheckAssertion::ExactBytes { artifact, .. } => ensure!(
                    contract.artifacts.contains(artifact),
                    "Assertion artifact is undeclared"
                ),
                CheckAssertion::MatchesInput { artifact, input } => ensure!(
                    contract.artifacts.contains(artifact) && contract.inputs.contains(input),
                    "Assertion input or artifact is undeclared"
                ),
                CheckAssertion::Command {
                    program,
                    verifier_files,
                    ..
                } => {
                    for path in std::iter::once(program).chain(verifier_files) {
                        ensure!(
                            path.is_absolute() && path.is_file(),
                            "Trusted verifier paths must be absolute files"
                        );
                        verifier_digests.push((path.clone(), bytes_digest(&std::fs::read(path)?)));
                    }
                }
            }
        }
        let mut captured = Self {
            checker,
            contract,
            version: String::new(),
            inputs,
            verifier_digests,
        };
        captured.version = captured.digest()?;
        Ok(captured)
    }
    pub fn digest(&self) -> Result<String> {
        Ok(content_digest(&serde_json::to_string(&(
            &self.checker,
            &self.contract,
            &self.inputs,
            &self.verifier_digests,
        ))?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultVersion {
    pub id: String,
    pub version: usize,
    pub task: Option<TaskAttemptRef>,
    pub summary: String,
    pub criteria: Vec<AcceptanceCriterion>,
    pub criteria_version: String,
    pub contract_id: Option<String>,
    pub producer_assignment_ids: Vec<String>,
    pub artifacts: Vec<FileSnapshot>,
    /// Exact component submission decisions, empty for a leaf result.
    pub component_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    Passed,
    Failed,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckEvidence {
    pub checker: CheckerIdentity,
    pub check_id: String,
    pub contract_id: String,
    pub criterion_ids: Vec<String>,
    pub outcome: CheckOutcome,
    pub inputs: Vec<FileSnapshot>,
    pub artifacts_after: Vec<FileSnapshot>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
}
