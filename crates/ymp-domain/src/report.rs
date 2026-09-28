//! Immutable final scope and report statements; I/O and acceptance remain in the kernel.
use crate::{
    Digest, Id, Ref, Result,
    identity::Agent,
    task::{Assumption, Criterion},
    verification::{CheckEnvironment, ConfirmationGrade},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalSource {
    pub result: Ref,
    pub acceptance: Ref,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalCheck {
    pub check: Ref,
    pub environment: CheckEnvironment,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalAggregate {
    pub session: Id,
    pub snapshot: Ref,
    pub contract: Ref,
    pub criteria: BTreeSet<Ref>,
    pub checks: Vec<FinalCheck>,
    pub sources: Vec<FinalSource>,
    pub producers: BTreeSet<Id<Agent>>,
    pub baseline: Option<Ref>,
}
impl FinalAggregate {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: Id::new(format!("final-{}", Digest::of(self.session.as_str())))?,
            version: Digest::of_value(self)?,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimKind {
    Status,
    Causal,
    Scope,
    Recommendation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimAudit {
    Valid,
    Unsupported(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: Id<Claim>,
    pub report: Id<Report>,
    pub text: String,
    pub kind: ClaimKind,
    pub evidence: Vec<Ref>,
    pub audit: ClaimAudit,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub id: Id<Report>,
    pub session: Id,
    pub claims: Vec<Id<Claim>>,
    pub unmet: Vec<Id<Criterion>>,
    pub assumptions: Vec<Assumption>,
    pub grade: ConfirmationGrade,
}
