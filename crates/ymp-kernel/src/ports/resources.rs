//! Read-only inputs and proposal contracts for resource mechanisms.
use serde::{Deserialize, Serialize};
use ymp_domain::{
    Digest, PolicyRef, Proposal, Ref, Result, identity::Pool, journal::PolicySelection,
    resources::*, task::Task,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostHistory {
    pub demand: ResourceDemand,
    pub pricebook: Digest,
    pub policy: PolicyRef,
    pub cost: CostUnits,
    pub estimated: bool,
    pub basis: Ref,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EstimateView {
    pub journal: Digest,
    pub demand: ResourceDemand,
    pub pricebook: PriceBook,
    pub history: Vec<CostHistory>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowanceView {
    pub journal: Digest,
    pub demand: ResourceDemand,
    pub estimate: CostEstimate,
    pub remaining: CostUnits,
    pub deadline: Option<u64>,
    pub at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportingView {
    pub journal: Digest,
    pub task: Task,
    pub pool: Pool,
    pub at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostView {
    pub journal: Digest,
    pub demand: ResourceDemand,
    pub receipt: Receipt,
    pub pricebook: PriceBook,
    pub allowance: Allowance,
    pub unknown_usage: UnknownUsage,
    /// Supplied by Treasury only after validating complete-cost evidence.
    pub known_complete_cost: Option<CostUnits>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceResponse<T> {
    pub input: Digest,
    pub proposal: Proposal<T>,
}

pub trait CostModel {
    fn selection(&self) -> &PolicySelection;
    fn cost(&self, view: &CostView) -> Result<Proposal<ReceiptPrice>>;
    fn estimate(&self, view: &EstimateView) -> Result<Proposal<CostEstimate>>;
    /// Price the currently observed usage for a host limit decision. An estimate
    /// is not a complete bill and cannot settle partial usage under UnknownUsage::Stop.
    fn observed_cost(&self, view: &CostView) -> Result<Proposal<ReceiptPrice>> {
        if view.receipt.coverage == Coverage::Complete || view.known_complete_cost.is_some() {
            return self.cost(view);
        }
        Ok(Proposal {
            value: ReceiptPrice::Unknown,
            rationale: "No observation-price estimate supplied by this CostModel".into(),
            basis: vec![],
            policy: self.selection().policy.clone(),
        })
    }
}
pub trait ResourcePolicy {
    fn selection(&self) -> &PolicySelection;
    fn allowance(&self, view: &AllowanceView) -> Result<Proposal<Allowance>>;
    fn reporting_reserve(&self, view: &ReportingView) -> Result<Proposal<ReportingPlan>>;
}
