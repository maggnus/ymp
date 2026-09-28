//! Attributed context, final reviewer selection, narrative and claim audit ports.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use ymp_domain::{
    Digest, Id, Proposal, Ref, Result,
    assignment::{Assignment, Prompt},
    identity::{Agent, ExecutionProfile},
    journal::PolicySelection,
    report::*,
    task::Criterion,
    verification::{Check, CheckRun},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextParameters {
    pub history_limit: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributedText {
    pub author: Id<Agent>,
    pub text: String,
    pub untrusted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedContext {
    pub snapshot: Ref,
    pub path: ymp_domain::workspace::WorkspacePath,
    pub digest: Digest,
    pub bytes: Option<Vec<u8>>,
    pub untrusted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextInput {
    pub retained: Vec<RetainedContext>,
    pub journal: Digest,
    pub assignment: Assignment,
    pub instructions: AttributedText,
    pub goal: String,
    pub criteria: Vec<Criterion>,
    pub checks: Vec<Check>,
    pub snapshots: Vec<Ref>,
    pub evidence: Vec<Ref>,
    pub history: Vec<Ref>,
    pub purpose: serde_json::Value,
    pub basis: Vec<Ref>,
}
pub trait ContextComposer {
    fn selection(&self) -> &PolicySelection;
    fn prompt(&self, input: &ContextInput) -> Result<Proposal<Prompt>>;
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewerCandidate {
    pub profile: ExecutionProfile,
    pub prior_reviews: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewerInput {
    pub journal: Digest,
    pub aggregate: Ref,
    pub producers: BTreeSet<Id<Agent>>,
    pub candidates: Vec<ReviewerCandidate>,
}
pub trait ReviewerPolicy {
    fn selection(&self) -> &PolicySelection;
    fn pick(&self, input: &ReviewerInput) -> Result<Proposal<Option<Id<Agent>>>>;
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Assertion {
    Retained { result: Ref },
    Accepted { acceptance: Ref },
    Executed { run: Ref, program: bool },
    Browser { run: Ref },
    Causal { before: Ref, after: Ref },
    Scope { universal: bool },
    Recommendation { basis: Ref },
    Uncertainty,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftClaim {
    pub text: String,
    pub assertion: Assertion,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportDraft {
    pub claims: Vec<DraftClaim>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimInput {
    pub retained: Vec<Ref>,
    pub journal: Digest,
    pub claim: DraftClaim,
    pub aggregate: Option<FinalAggregate>,
    pub final_acceptance: Option<ymp_domain::verification::Acceptance>,
    pub runs: Vec<CheckRun>,
    pub checks: Vec<Check>,
    pub recommendations: Vec<Ref>,
}
pub trait ClaimAuditor {
    fn selection(&self) -> &PolicySelection;
    fn audit(&self, input: &ClaimInput) -> Result<Proposal<ClaimAudit>>;
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeInput {
    pub retained: Vec<Ref>,
    pub journal: Digest,
    pub report: Id<Report>,
    pub final_acceptance: Option<ymp_domain::verification::Acceptance>,
    pub aggregate: Option<FinalAggregate>,
    pub outcome: ymp_domain::task::SessionStatus,
    pub unmet: Vec<Criterion>,
    pub source: Option<String>,
    pub basis: Vec<Ref>,
}
pub trait NarrativeComposer {
    fn selection(&self) -> &PolicySelection;
    fn compose(&self, input: &NarrativeInput) -> Result<Proposal<ReportDraft>>;
}
