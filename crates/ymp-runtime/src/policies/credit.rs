//! Selectable credit eligibility, without creating competence observations.
use ymp_domain::{Proposal, Result, journal::PolicySelection, verification::ConfirmationGrade};
use ymp_kernel::ports::experience::CreditPolicy;
pub struct ConfirmedOnly {
    selection: PolicySelection,
}
pub struct IncludeDiscriminated {
    selection: PolicySelection,
}
impl ConfirmedOnly {
    pub fn new() -> Result<Self> {
        Ok(Self {
            selection: PolicySelection::new(
                "CreditPolicy",
                "ConfirmedOnly",
                "1",
                serde_json::json!({}),
            )?,
        })
    }
}
impl IncludeDiscriminated {
    pub fn new() -> Result<Self> {
        Ok(Self {
            selection: PolicySelection::new(
                "CreditPolicy",
                "IncludeDiscriminated",
                "1",
                serde_json::json!({}),
            )?,
        })
    }
}
fn proposal(selection: &PolicySelection, value: bool) -> Proposal<bool> {
    Proposal {
        value,
        rationale: "Eligibility under the selected grade policy; no observation is created".into(),
        basis: vec![],
        policy: selection.policy.clone(),
    }
}
impl CreditPolicy for ConfirmedOnly {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn creditable(&self, grade: ConfirmationGrade) -> Result<Proposal<bool>> {
        Ok(proposal(
            &self.selection,
            matches!(grade, ConfirmationGrade::Confirmed(_)),
        ))
    }
}
impl CreditPolicy for IncludeDiscriminated {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn creditable(&self, grade: ConfirmationGrade) -> Result<Proposal<bool>> {
        Ok(proposal(
            &self.selection,
            grade.rank() >= ConfirmationGrade::Discriminated.rank(),
        ))
    }
}
