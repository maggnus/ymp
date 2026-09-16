//! W1-0006 proposal/admission foundations; W1-0007 owns later commitment transitions.
//! Voluntary solicitation, reopening and participant transport remain W3 work.
use crate::{
    Denial, Id, Ref, Result,
    assignment::{Assignment, Contribution, CostEstimate, Forecast},
    identity::{Agent, ExecutionProfile},
    require_text,
    task::Real,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolicitationVisibility {
    Open,
    Sealed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolicitationState {
    Open,
    Awarded,
    Withdrawn,
    Expired,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Solicitation {
    pub id: Id<Solicitation>,
    pub contribution: Id<Contribution>,
    pub stimulus: Real,
    pub deadline: u64,
    pub eligible: BTreeSet<Id<Agent>>,
    pub visibility: SolicitationVisibility,
    pub reopened: u32,
    pub state: SolicitationState,
}
impl Solicitation {
    pub fn validate(&self) -> Result<()> {
        if self.stimulus.get() < 0.0 || self.eligible.len() > 1024 {
            return Err(Denial::new(
                "solicitation",
                "Solicitation needs bounded eligibility and nonnegative stimulus",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferSource {
    RuntimeProxy,
    InAssignment(Id<Assignment>),
    BidAssignment(Id<Assignment>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub id: Id<Offer>,
    pub solicitation: Id<Solicitation>,
    pub agent: Id<Agent>,
    pub profile: ExecutionProfile,
    pub forecast: Forecast,
    pub cost: CostEstimate,
    pub approach: String,
    pub source: OfferSource,
    pub at: u64,
}
impl Offer {
    pub fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        self.forecast.validate()?;
        self.cost.validate()?;
        require_text(&self.approach, 16_384)?;
        if self.agent != self.profile.agent {
            return Err(Denial::new(
                "offer",
                "Offer and profile name different agents",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Award {
    pub solicitation: Id<Solicitation>,
    pub offer: Id<Offer>,
    pub rationale: String,
}
impl Award {
    pub fn validate(&self) -> Result<()> {
        require_text(&self.rationale, 16_384)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Creditor {
    Team,
    Runtime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProgressSignal {
    EvidenceAdded,
    CheckRun,
    ResultSubmitted,
    Heartbeat,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub expires: u64,
    pub renew_on: BTreeSet<ProgressSignal>,
    pub renewals_left: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitmentState {
    Proposed,
    Active,
    Discharged,
    Released(String),
    Expired,
    Cancelled(String),
    Delegated(Id<Agent>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commitment {
    pub id: Id<Commitment>,
    pub debtor: Id<Agent>,
    pub creditor: Creditor,
    pub subject: Id<Contribution>,
    pub condition: Option<String>,
    pub lease: Lease,
    pub state: CommitmentState,
    pub history: Vec<Ref>,
}
impl Commitment {
    pub fn validate(&self) -> Result<()> {
        if let Some(condition) = &self.condition {
            require_text(condition, 4096)?;
        }
        if let CommitmentState::Released(reason) | CommitmentState::Cancelled(reason) = &self.state
        {
            require_text(reason, 4096)?;
        }
        if self.history.len() > 4096
            || self.history.iter().collect::<BTreeSet<_>>().len() != self.history.len()
        {
            return Err(Denial::new(
                "commitment",
                "Commitment history must be bounded and contain distinct references",
            ));
        }
        Ok(())
    }
}
