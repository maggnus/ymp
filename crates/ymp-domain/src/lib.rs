#![forbid(unsafe_code)]

mod acceptance;

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

pub use acceptance::{
    Check, CheckMethod, CriterionEvaluation, CriterionStatus, Evidence, EvidenceFile,
    VerifierDigest, sha256,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        required_text(value.into(), "task ID").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        required_text(value.into(), "session ID").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CriterionId(String);

impl CriterionId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        required_text(value.into(), "criterion ID").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CriterionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Goal {
    request: String,
}

impl Goal {
    pub fn new(request: impl Into<String>) -> Result<Self, DomainError> {
        Ok(Self {
            request: required_text(request.into(), "goal request")?,
        })
    }

    pub fn request(&self) -> &str {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Criterion {
    id: CriterionId,
    description: String,
}

impl Criterion {
    pub fn new(id: CriterionId, description: impl Into<String>) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            description: required_text(description.into(), "criterion description")?,
        })
    }

    pub fn id(&self) -> &CriterionId {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceContract {
    criteria: Vec<Criterion>,
}

impl AcceptanceContract {
    pub fn new(criteria: Vec<Criterion>) -> Result<Self, DomainError> {
        if criteria.is_empty() {
            return Err(DomainError::EmptyAcceptanceContract);
        }

        let mut identifiers = HashSet::with_capacity(criteria.len());
        for criterion in &criteria {
            if !identifiers.insert(criterion.id()) {
                return Err(DomainError::DuplicateCriterionId(criterion.id().clone()));
            }
        }

        Ok(Self { criteria })
    }

    pub fn criteria(&self) -> &[Criterion] {
        &self.criteria
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Constraints {
    conditions: Vec<String>,
}

impl Constraints {
    pub fn new(conditions: Vec<String>) -> Result<Self, DomainError> {
        for condition in &conditions {
            if condition.trim().is_empty() {
                return Err(DomainError::BlankText {
                    field: "constraint condition",
                });
            }
        }

        Ok(Self { conditions })
    }

    pub fn conditions(&self) -> &[String] {
        &self.conditions
    }

    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    id: TaskId,
    goal: Goal,
    acceptance_contract: AcceptanceContract,
    constraints: Constraints,
}

impl Task {
    pub fn new(
        id: TaskId,
        goal: Goal,
        acceptance_contract: AcceptanceContract,
        constraints: Constraints,
    ) -> Self {
        Self {
            id,
            goal,
            acceptance_contract,
            constraints,
        }
    }

    pub fn id(&self) -> &TaskId {
        &self.id
    }

    pub fn goal(&self) -> &Goal {
        &self.goal
    }

    pub fn acceptance_contract(&self) -> &AcceptanceContract {
        &self.acceptance_contract
    }

    pub fn constraints(&self) -> &Constraints {
        &self.constraints
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    BlankText { field: &'static str },
    EmptyAcceptanceContract,
    DuplicateCriterionId(CriterionId),
    EmptyCheckCoverage,
    DuplicateCheckCriterion(CriterionId),
    UnsafeEvidencePath(PathBuf),
    InvalidVerifierPath(PathBuf),
    InvalidSha256,
    EvidenceMethodMismatch,
    UnknownEvidenceCriterion(CriterionId),
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankText { field } => write!(formatter, "{field} must not be blank"),
            Self::EmptyAcceptanceContract => {
                formatter.write_str("acceptance contract must contain at least one criterion")
            }
            Self::DuplicateCriterionId(id) => {
                write!(formatter, "criterion ID '{id}' must be unique")
            }
            Self::EmptyCheckCoverage => {
                formatter.write_str("check must cover at least one criterion")
            }
            Self::DuplicateCheckCriterion(id) => {
                write!(formatter, "check criterion ID '{id}' must be unique")
            }
            Self::UnsafeEvidencePath(path) => write!(
                formatter,
                "evidence path '{}' must be relative and stay inside the workspace",
                path.display()
            ),
            Self::InvalidVerifierPath(path) => write!(
                formatter,
                "check verifier path '{}' must be absolute",
                path.display()
            ),
            Self::InvalidSha256 => formatter.write_str(
                "SHA-256 digest must contain exactly 64 lowercase hexadecimal characters",
            ),
            Self::EvidenceMethodMismatch => {
                formatter.write_str("evidence does not match its check method")
            }
            Self::UnknownEvidenceCriterion(id) => write!(
                formatter,
                "evidence covers criterion ID '{id}' outside the acceptance contract"
            ),
        }
    }
}

impl Error for DomainError {}

fn required_text(value: String, field: &'static str) -> Result<String, DomainError> {
    if value.trim().is_empty() {
        Err(DomainError::BlankText { field })
    } else {
        Ok(value)
    }
}
