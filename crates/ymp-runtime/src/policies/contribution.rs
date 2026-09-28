//! Ordinal criterion urgency and a fixed workflow experimental control.
use ymp_domain::{
    Proposal, Result, assignment::Contribution, journal::PolicySelection,
    resources::ContributionKind, verification::LedgerStatus,
};
use ymp_kernel::ports::planning::*;
pub struct OrdinalValue {
    selection: PolicySelection,
}
pub struct FixedWorkflow {
    selection: PolicySelection,
}
fn selection(name: &str, p: ContributionParameters) -> Result<PolicySelection> {
    PolicySelection::new("ContributionPolicy", name, "1", serde_json::json!(p))
}
impl OrdinalValue {
    pub fn new(p: ContributionParameters) -> Result<Self> {
        Ok(Self {
            selection: selection("OrdinalValue", p)?,
        })
    }
}
impl FixedWorkflow {
    pub fn new(p: ContributionParameters) -> Result<Self> {
        Ok(Self {
            selection: selection("FixedWorkflow", p)?,
        })
    }
}
fn kind(kind: ContributionKind) -> u8 {
    match kind {
        ContributionKind::Diagnose => 0,
        ContributionKind::Clarify => 1,
        ContributionKind::Verify => 2,
        ContributionKind::DesignChecks => 3,
        ContributionKind::Produce => 4,
        _ => 5,
    }
}
fn next(
    input: &ContributionView,
    selection: &PolicySelection,
    ordinal: bool,
) -> Result<Proposal<Vec<Contribution>>> {
    let mut candidates = input.candidates.clone();
    candidates.sort_by_key(|c| {
        if ordinal {
            (
                !c.required,
                match c.status {
                    LedgerStatus::Contradicted => 0,
                    LedgerStatus::Unmet => 1,
                    LedgerStatus::Supported => 2,
                    LedgerStatus::Satisfied => 3,
                },
                kind(c.contribution.kind),
            )
        } else {
            (false, kind(c.contribution.kind), 0)
        }
    });
    let mut value = vec![];
    let mut agents = std::collections::BTreeSet::new();
    let mut total = 0.0;
    for candidate in candidates {
        if value.len() >= input.slots {
            break;
        }
        let Some(agent) = candidate.eligible.iter().find(|a| !agents.contains(*a)) else {
            continue;
        };
        let c = candidate.contribution;
        let next_total = total + c.cost.p90.get();
        let remaining = if c.kind.purpose() == ymp_domain::resources::Purpose::Verification {
            input.verification_remaining
        } else {
            input.production_remaining
        };
        if next_total > remaining.get() {
            continue;
        }
        agents.insert(agent.clone());
        total = next_total;
        value.push(c);
    }
    Ok(Proposal{value,rationale:if ordinal{"Prioritize required criteria, contradiction and unmet evidence"}else{"Experimental fixed workflow orders diagnosis, verification and production regardless of criterion weight or requiredness"}.into(),basis:vec![],policy:selection.policy.clone()})
}
impl ContributionPolicy for OrdinalValue {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn next(&self, input: &ContributionView) -> Result<Proposal<Vec<Contribution>>> {
        next(input, &self.selection, true)
    }
}
impl ContributionPolicy for FixedWorkflow {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn next(&self, input: &ContributionView) -> Result<Proposal<Vec<Contribution>>> {
        next(input, &self.selection, false)
    }
}
