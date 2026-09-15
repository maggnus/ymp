//! Explicit task intake values. Validation performs no I/O or provider inference.

use crate::journal::Capability;
use crate::{Denial, Digest, Id, Ref, Result, require_text};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Finite domain real. NaN and infinities never enter a serialized task.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Real(f64);
impl Eq for Real {}
impl Real {
    pub fn new(value: f64) -> Result<Self> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(Denial::new("nonfinite", "A domain real must be finite"))
        }
    }
    pub fn get(self) -> f64 {
        self.0
    }
}
impl TryFrom<f64> for Real {
    type Error = Denial;
    fn try_from(value: f64) -> Result<Self> {
        Self::new(value)
    }
}
impl From<Real> for f64 {
    fn from(value: Real) -> Self {
        value.0
    }
}

pub type CostUnits = Real;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pins {
    pub team_size: Option<u32>,
    pub roster: Option<BTreeSet<Id>>,
    pub models: Option<BTreeSet<String>>,
    pub efforts: Option<BTreeSet<String>>,
}

impl Pins {
    pub fn unrestricted() -> Self {
        Self {
            team_size: None,
            roster: None,
            models: None,
            efforts: None,
        }
    }
    pub fn validate(&self, max_members: u32) -> Result<()> {
        if self
            .team_size
            .is_some_and(|size| size == 0 || size > max_members)
        {
            return Err(Denial::new(
                "pins",
                "Pinned team size must be positive and within the member limit",
            ));
        }
        if let Some(roster) = &self.roster
            && (roster.is_empty()
                || roster.len() as u64 > u64::from(max_members)
                || self
                    .team_size
                    .is_some_and(|size| u64::from(size) != roster.len() as u64))
        {
            return Err(Denial::new(
                "pins",
                "Pinned roster conflicts with team size or the member limit",
            ));
        }
        for values in [&self.models, &self.efforts].into_iter().flatten() {
            if values.is_empty() {
                return Err(Denial::new(
                    "pins",
                    "An explicit model or effort pin must name at least one value",
                ));
            }
            for value in values {
                require_text(value, 256)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    pub budget: CostUnits,
    pub verification_reserve: CostUnits,
    pub deadline: Option<u64>,
    pub pins: Pins,
    pub allowed: BTreeSet<Capability>,
    pub parallel_limit: u32,
    pub attempt_limit: u32,
    pub max_members: u32,
}
impl Constraints {
    pub fn validate(&self) -> Result<()> {
        if self.budget.get() < 0.0
            || self.verification_reserve.get() < 0.0
            || self.verification_reserve > self.budget
        {
            return Err(Denial::new(
                "budget",
                "Budget and verification reserve must be nonnegative, with reserve within budget",
            ));
        }
        if self.parallel_limit == 0 || self.attempt_limit == 0 || self.max_members == 0 {
            return Err(Denial::new(
                "limits",
                "Parallel, attempt and member limits must be positive",
            ));
        }
        self.pins.validate(self.max_members)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub request: String,
    pub assumptions: Vec<Assumption>,
    pub clarifications: Vec<Clarification>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assumption {
    pub text: String,
    pub criterion: Option<Id<Criterion>>,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clarification {
    pub question: String,
    pub answer: String,
    pub at: u64,
}
impl Assumption {
    pub fn validate(&self) -> Result<()> {
        require_text(&self.text, 16_384)?;
        require_text(&self.reason, 16_384)
    }
}
impl Clarification {
    pub fn validate(&self) -> Result<()> {
        require_text(&self.question, 16_384)?;
        require_text(&self.answer, 16_384)
    }
}
impl Goal {
    pub fn validate(&self) -> Result<()> {
        require_text(&self.request, 65_536)?;
        for assumption in &self.assumptions {
            assumption.validate()?;
        }
        for clarification in &self.clarifications {
            clarification.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriterionKind {
    NewBehavior,
    Preserve,
    ArtifactPresence,
    Quality,
    Constraint,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EvidenceClass {
    StaticRead,
    Executed,
    Browser,
    ExternalData,
    Inspection,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriterionOrigin {
    User,
    /// The exact, immutable assignment version; validating its admitted author is W1-0011.
    Derived(Ref),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    pub id: Id<Criterion>,
    pub text: String,
    pub kind: CriterionKind,
    pub weight: Real,
    pub required: bool,
    pub origin: CriterionOrigin,
    pub needs_class: BTreeSet<EvidenceClass>,
}
impl Criterion {
    pub fn validate(&self) -> Result<()> {
        require_text(&self.text, 16_384)?;
        if self.weight.get() < 0.0 {
            return Err(Denial::new(
                "weight",
                "Criterion weight must be nonnegative",
            ));
        }
        Ok(())
    }
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: Id<Task>,
    pub goal: Goal,
    pub contract: Id<AcceptanceContract>,
    pub constraints: Constraints,
}
impl Task {
    pub fn validate(&self) -> Result<()> {
        self.goal.validate()?;
        self.constraints.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceContract {
    pub id: Id<AcceptanceContract>,
    pub task: Id<Task>,
    pub criteria: Vec<Id<Criterion>>,
    pub checks: Vec<Id>,
    pub version: Digest,
}
impl AcceptanceContract {
    pub fn new(task: &Task, criteria: &[Criterion], checks: Vec<Id>) -> Result<Self> {
        task.validate()?;
        validate_criteria(&task.goal, criteria)?;
        let ids = criteria.iter().map(|c| c.id.clone()).collect();
        let version = Self::content_version(task, criteria, &checks)?;
        let contract = Self {
            id: task.contract.clone(),
            task: task.id.clone(),
            criteria: ids,
            checks,
            version,
        };
        contract.validate(task, criteria)?;
        Ok(contract)
    }
    pub fn reference(&self) -> Ref {
        Ref {
            id: self.id.erased(),
            version: self.version.clone(),
        }
    }
    pub fn content_version(task: &Task, criteria: &[Criterion], checks: &[Id]) -> Result<Digest> {
        Digest::of_value(&("AcceptanceContract", 1_u32, task, criteria, checks))
    }
    pub fn validate(&self, task: &Task, criteria: &[Criterion]) -> Result<()> {
        task.validate()?;
        validate_criteria(&task.goal, criteria)?;
        if self.id != task.contract
            || self.task != task.id
            || self.criteria != criteria.iter().map(|c| c.id.clone()).collect::<Vec<_>>()
        {
            return Err(Denial::new(
                "contract_refs",
                "Contract task or criterion references disagree with the supplied values",
            ));
        }
        if self.checks.iter().collect::<BTreeSet<_>>().len() != self.checks.len() {
            return Err(Denial::new(
                "contract_refs",
                "A contract cannot contain duplicate check references",
            ));
        }
        if self.version != Self::content_version(task, criteria, &self.checks)? {
            return Err(Denial::new(
                "contract_version",
                "Contract version does not match its criteria, goal and constraints",
            ));
        }
        Ok(())
    }
}

pub fn validate_criteria(goal: &Goal, criteria: &[Criterion]) -> Result<()> {
    if criteria.is_empty() {
        return Err(Denial::new(
            "criteria",
            "An explicit task requires criteria",
        ));
    }
    let ids: BTreeSet<_> = criteria.iter().map(|c| &c.id).collect();
    if ids.len() != criteria.len() {
        return Err(Denial::new(
            "criteria",
            "Criterion identities must be unique",
        ));
    }
    for criterion in criteria {
        criterion.validate()?;
    }
    for assumption in &goal.assumptions {
        if assumption
            .criterion
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            return Err(Denial::new(
                "criterion_ref",
                "An assumption refers to a missing criterion",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Intake,
    Running,
    Finalizing,
    Delivered,
    Blocked(String),
    Cancelled,
}
