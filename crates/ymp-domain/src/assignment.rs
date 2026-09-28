//! Contribution and bounded assignment values. Lifecycle authority belongs to the kernel.
pub use crate::resources::{ContributionKind, CostEstimate, Difficulty};
use crate::{
    Denial, Digest, Id, PolicyRef, Prob, Ref, Result,
    coordination::Commitment,
    identity::{Agent, ExecutionProfile, InvocationSettings, Provider},
    journal::Capability,
    require_text,
    resources::{Allowance, Receipt},
    task::{Criterion, Real},
    workspace::Workspace,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The variant preserves subject kind while its owning service resolves the ID.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ContributionSubject {
    WorkItem(Ref),
    ResultVersion(Ref),
    Objection(Ref),
}
impl ContributionSubject {
    pub fn reference(&self) -> &Ref {
        match self {
            Self::WorkItem(reference)
            | Self::ResultVersion(reference)
            | Self::Objection(reference) => reference,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContributionAuthor {
    Runtime,
    Agent(Id<Assignment>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForecastSource {
    Agent(ExecutionProfile),
    Model(PolicyRef),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forecast {
    pub p_success: Prob,
    pub delta_belief: BTreeMap<Id<Criterion>, Real>,
    pub source: ForecastSource,
}
impl Forecast {
    pub fn validate(&self) -> Result<()> {
        if self.delta_belief.len() > 1024 {
            return Err(Denial::new("forecast", "Too many criterion forecasts"));
        }
        match &self.source {
            ForecastSource::Agent(profile) => profile.validate(),
            ForecastSource::Model(policy) => policy.validate(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub id: Id<Contribution>,
    pub session: Id,
    pub kind: ContributionKind,
    pub targets: BTreeSet<Id<Criterion>>,
    pub subject: Option<ContributionSubject>,
    pub needs: BTreeSet<Capability>,
    pub forecast: Forecast,
    pub cost: CostEstimate,
    pub difficulty: Difficulty,
    pub proposed_by: ContributionAuthor,
    pub basis: Vec<Ref>,
}
impl Contribution {
    pub fn validate(&self) -> Result<()> {
        self.forecast.validate()?;
        self.cost.validate()?;
        if self.targets.len() > 1024
            || self.basis.len() > 128
            || self.basis.iter().collect::<BTreeSet<_>>().len() != self.basis.len()
            || self
                .forecast
                .delta_belief
                .keys()
                .any(|criterion| !self.targets.contains(criterion))
        {
            return Err(Denial::new(
                "contribution",
                "Contribution targets, forecast scope or basis are invalid",
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RoleKind {
    Planner,
    CheckDesigner,
    Producer,
    Verifier,
    Reviewer,
    FinalReviewer,
    Researcher,
    Curator,
    Narrator,
    Judge,
    Advocate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentState {
    Admitted,
    Running,
    Finished,
    Revoked,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub id: Id<Assignment>,
    pub session: Id,
    pub agent: Id<Agent>,
    pub profile: ExecutionProfile,
    pub contribution: Id<Contribution>,
    pub role: RoleKind,
    pub access: BTreeSet<Capability>,
    pub workspace: Id<Workspace>,
    pub allowance: Allowance,
    pub grant: Id<Grant>,
    pub commitment: Id<Commitment>,
    pub state: AssignmentState,
}
impl Assignment {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        self.allowance.validate()?;
        if self.profile.agent != self.agent {
            return Err(Denial::new(
                "assignment",
                "Assignment and profile name different agents",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TeamOperation {
    BoardRead,
    NoticePost,
    ContributionPropose,
    OfferSubmit,
    CommitmentRelease,
    CommitmentDelegate,
    ObjectionRaise,
    StatusNotify,
    CheckPropose,
    KnowledgePropose,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub id: Id<Grant>,
    pub assignment: Id<Assignment>,
    pub operations: BTreeSet<TeamOperation>,
    pub expires: u64,
    pub token_digest: Digest,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClass {
    Infrastructure,
    Environment,
    Protocol,
    Content,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationTerminal {
    Completed,
    Failed(ErrorClass),
    Cancelled,
    TimedOut,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invocation {
    pub id: Id<Invocation>,
    pub assignment: Id<Assignment>,
    pub provider: Id<Provider>,
    pub settings: InvocationSettings,
    pub native_session: Option<String>,
    pub started: u64,
    pub ended: Option<u64>,
    pub receipt: Option<Id<Receipt>>,
    pub terminal: Option<InvocationTerminal>,
}
/// Exact context retained before dispatch. Participant text carries no authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prompt {
    pub text: String,
    pub basis: Vec<Ref>,
}
impl Prompt {
    pub fn validate(&self) -> Result<()> {
        require_text(&self.text, 65_536)?;
        if self.basis.is_empty()
            || self.basis.len() > 128
            || self.basis.iter().collect::<BTreeSet<_>>().len() != self.basis.len()
        {
            return Err(Denial::new(
                "prompt_basis",
                "A bounded, distinct committed prompt basis is required",
            ));
        }
        Ok(())
    }
}
impl Invocation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        if let Some(session) = &self.native_session {
            require_text(session, 4096)?;
        }
        if self.ended.is_some() != self.terminal.is_some()
            || self.ended.is_some_and(|ended| ended < self.started)
        {
            return Err(Denial::new(
                "invocation",
                "Terminal status and end time must agree with the invocation lifetime",
            ));
        }
        Ok(())
    }
}
