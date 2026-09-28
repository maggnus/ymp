//! Belief proposals over kernel-filtered, version-bound present-result evidence.
use serde::{Deserialize, Serialize};
use ymp_domain::{
    Digest, Id, Prob, Proposal, Ref, Result, journal::PolicySelection, task::Criterion,
    verification::*,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeliefView {
    pub journal: Digest,
    pub entry: LedgerEntry,
    pub evidence: Vec<Evidence>,
    pub criterion: Criterion,
    pub subject: Ref,
    pub prior_basis: Vec<PriorBasis>,
    pub rules: AssessmentRules,
    pub at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorBasis {
    pub source: Ref,
    pub polarity: Polarity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeliefPrior {
    pub criterion: Ref,
    pub subject: Ref,
    pub value: Prob,
    pub basis: Vec<Ref>,
    pub rationale: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeliefUpdate {
    pub entry: LedgerEntry,
    pub prior: BeliefPrior,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeliefResponse {
    pub input: Digest,
    pub proposal: Proposal<BeliefUpdate>,
}
pub trait BeliefModel {
    fn selection(&self) -> &PolicySelection;
    fn update(&self, view: &BeliefView) -> Result<Proposal<BeliefUpdate>>;
}
pub fn hard_contradiction(evidence: &Evidence) -> bool {
    evidence.polarity == Polarity::Contradicts
        && matches!(
            evidence.class,
            ymp_domain::task::EvidenceClass::Executed
                | ymp_domain::task::EvidenceClass::Browser
                | ymp_domain::task::EvidenceClass::ExternalData
        )
}
pub fn entry_for(view: &BeliefView, belief: Prob, thresholds: &BeliefThresholds) -> LedgerEntry {
    if view.evidence.iter().any(hard_contradiction) {
        return view.entry.updated(
            LedgerStatus::Contradicted,
            Prob::new(0.0).unwrap(),
            view.evidence.iter().map(|e| e.id.clone()).collect(),
            view.at,
        );
    }
    let supporting: Vec<_> = view
        .evidence
        .iter()
        .filter(|e| e.polarity == Polarity::Supports)
        .collect();
    let classes: std::collections::BTreeSet<_> =
        supporting.iter().map(|e| e.class.clone()).collect();
    let status = if belief >= thresholds.for_criterion(&view.criterion)
        && !supporting.is_empty()
        && view.criterion.needs_class.is_subset(&classes)
    {
        LedgerStatus::Satisfied
    } else if belief >= thresholds.support {
        LedgerStatus::Supported
    } else {
        LedgerStatus::Unmet
    };
    view.entry.updated(
        status,
        belief,
        supporting
            .iter()
            .map(|e| e.id.clone())
            .collect::<Vec<Id<Evidence>>>(),
        view.at,
    )
}

mod work;
pub use work::*;
