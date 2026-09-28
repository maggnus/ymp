//! Explicit work definitions and attempt outcomes, separate from execution I/O.
use crate::{
    Denial, Digest, Id, Ref, Result,
    assignment::Assignment,
    journal::Capability,
    result::ResultVersion,
    task::Criterion,
    workspace::{Workspace, WorkspacePath},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub id: Id<Plan>,
    pub session: Id,
    pub version: u32,
    pub items: BTreeSet<Id<WorkItem>>,
    pub rationale: String,
    pub author: Id<Assignment>,
}
impl Plan {
    pub fn validate(&self) -> Result<()> {
        crate::require_text(&self.rationale, 4096)?;
        if self.version == 0 || self.items.is_empty() || self.items.len() > 128 {
            return Err(Denial::new(
                "plan",
                "A versioned bounded work graph is required",
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkState {
    Open,
    Committed,
    Running,
    InReview,
    Accepted,
    Failed,
    Blocked,
    Superseded,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkItem {
    pub id: Id<WorkItem>,
    pub plan: Id<Plan>,
    pub title: String,
    pub targets: BTreeSet<Id<Criterion>>,
    pub deps: BTreeSet<Id<WorkItem>>,
    pub needs: BTreeSet<Capability>,
    pub writes: BTreeSet<WorkspacePath>,
    pub state: WorkState,
    pub attempts: Vec<Id<Attempt>>,
    pub accepted: Option<Id<ResultVersion>>,
    pub parent: Option<Id<WorkItem>>,
}
impl WorkItem {
    pub fn validate(&self) -> Result<()> {
        crate::require_text(&self.title, 1024)?;
        if self.targets.is_empty()
            || self.targets.len() > 128
            || self.deps.len() > 128
            || self.deps.contains(&self.id)
            || self.parent.as_ref() == Some(&self.id)
            || self.writes.len() > 128
            || self.attempts.len() > 4096
            || self.attempts.iter().collect::<BTreeSet<_>>().len() != self.attempts.len()
            || (!self.writes.is_empty() && !self.needs.contains(&Capability::WriteFiles))
            || (self.state == WorkState::Accepted) != self.accepted.is_some()
        {
            return Err(Denial::new(
                "work_item",
                "Work definition or lifecycle is invalid",
            ));
        }
        Ok(())
    }
    /// Definition identity is stable across attempts and lifecycle transitions.
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(&(
                &self.id,
                &self.plan,
                &self.title,
                &self.targets,
                &self.deps,
                &self.needs,
                &self.writes,
                &self.parent,
            ))?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptOutcome {
    Pending,
    Submitted,
    Accepted,
    Rejected(String),
    Abandoned,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub id: Id<Attempt>,
    pub item: Id<WorkItem>,
    pub assignment: Id<Assignment>,
    pub workspace: Id<Workspace>,
    pub result: Option<Id<ResultVersion>>,
    pub outcome: AttemptOutcome,
}
impl Attempt {
    pub fn validate(&self) -> Result<()> {
        if let AttemptOutcome::Rejected(reason) = &self.outcome {
            crate::require_text(reason, 1024)?;
        }
        if (self.outcome == AttemptOutcome::Pending && self.result.is_some())
            || (matches!(
                self.outcome,
                AttemptOutcome::Submitted | AttemptOutcome::Accepted | AttemptOutcome::Rejected(_)
            ) && self.result.is_none())
        {
            return Err(Denial::new(
                "attempt",
                "Attempt outcome and retained result disagree",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Membership {
    pub agent: Id<crate::identity::Agent>,
    pub joined: u64,
    pub left: Option<u64>,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Team {
    pub id: Id<Team>,
    pub session: Id,
    pub members: Vec<Membership>,
    pub revision: u64,
}
