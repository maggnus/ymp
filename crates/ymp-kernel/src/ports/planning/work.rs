//! Named planning ports over recorded, paid output and kernel-filtered candidates.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use ymp_domain::{
    Digest, Id, Proposal, Ref, Result,
    assignment::{Contribution, Invocation},
    journal::{Method, PolicySelection},
    plan::{Plan, WorkItem},
    task::{Criterion, Goal, Real},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningPrompt {
    pub contract: Ref,
    pub goal: Goal,
    pub criteria: Vec<Criterion>,
    pub method: Option<Method>,
    pub effective: PolicySelection,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaidPlanningView {
    pub journal: Digest,
    pub invocation: Id<Invocation>,
    pub assignment: Ref,
    pub admission: Ref,
    pub completion: Ref,
    pub receipt: Ref,
    pub prompt: PlanningPrompt,
    pub output: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub question: String,
    pub p_misinterpretation: ymp_domain::Prob,
    pub rework_cost: Real,
    pub assumption: String,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeOutput {
    pub criteria: Vec<Criterion>,
    pub questions: Vec<Question>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestionDecision {
    Ask(Question),
    Assume(Question),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeOutcome {
    pub criteria: Vec<Criterion>,
    pub questions: Vec<QuestionDecision>,
}
pub trait IntakePolicy {
    fn selection(&self) -> &PolicySelection;
    fn criteria(&self, input: &PaidPlanningView) -> Result<Proposal<IntakeOutcome>>;
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanDefinition {
    pub plan: Plan,
    pub items: Vec<WorkItem>,
}
pub trait Planner {
    fn selection(&self) -> &PolicySelection;
    fn plan(&self, input: &PaidPlanningView) -> Result<Proposal<PlanDefinition>>;
}
pub trait MethodRouter {
    fn selection(&self) -> &PolicySelection;
    fn choose(&self, view: &crate::view::SessionView) -> Result<Proposal<Method>>;
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContributionCandidate {
    pub contribution: Contribution,
    pub eligible: BTreeSet<Id<ymp_domain::identity::Agent>>,
    pub required: bool,
    pub status: ymp_domain::verification::LedgerStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContributionView {
    pub journal: Digest,
    pub candidates: Vec<ContributionCandidate>,
    pub slots: usize,
    pub production_remaining: Real,
    pub verification_remaining: Real,
    pub limitations: Vec<String>,
}
pub trait ContributionPolicy {
    fn selection(&self) -> &PolicySelection;
    fn next(&self, input: &ContributionView) -> Result<Proposal<Vec<Contribution>>>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeParameters {
    pub cost_interrupt: Real,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContributionParameters {
    pub expected: Real,
    pub p90: Real,
    pub p_success: ymp_domain::Prob,
    pub delta_belief: Real,
}
