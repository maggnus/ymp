//! Initial paid scope versus a predeclared experimental work definition.
use ymp_domain::{
    Id, Proposal, Result,
    journal::{PolicySelection, decode},
};
use ymp_kernel::ports::planning::*;
pub struct AsNeededDecomposition {
    selection: PolicySelection,
}
pub struct ExplicitPlan {
    selection: PolicySelection,
    definition: PlanDefinition,
}
impl AsNeededDecomposition {
    pub fn new() -> Result<Self> {
        Ok(Self {
            selection: PolicySelection::new(
                "Planner",
                "AsNeededDecomposition",
                "1",
                serde_json::json!({}),
            )?,
        })
    }
}
impl ExplicitPlan {
    pub fn new(definition: PlanDefinition) -> Result<Self> {
        definition.plan.validate()?;
        for item in &definition.items {
            item.validate()?;
        }
        Ok(Self {
            selection: PolicySelection::new(
                "Planner",
                "ExplicitPlan",
                "1",
                serde_json::json!({"definition":definition}),
            )?,
            definition,
        })
    }
}
fn proposal(
    input: &PaidPlanningView,
    selection: &PolicySelection,
    mut definition: PlanDefinition,
    rationale: &str,
) -> Result<Proposal<PlanDefinition>> {
    definition.plan.author = Id::new(input.assignment.id.as_str())?;
    Ok(Proposal {
        value: definition,
        rationale: rationale.into(),
        basis: super::intake::basis(input),
        policy: selection.policy.clone(),
    })
}
impl Planner for AsNeededDecomposition {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn plan(&self, input: &PaidPlanningView) -> Result<Proposal<PlanDefinition>> {
        proposal(
            input,
            &self.selection,
            decode(input.output.as_bytes())?,
            "Use one paid proposed work scope; diagnosed decomposition is unavailable until W3",
        )
    }
}
impl Planner for ExplicitPlan {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn plan(&self, input: &PaidPlanningView) -> Result<Proposal<PlanDefinition>> {
        proposal(
            input,
            &self.selection,
            self.definition.clone(),
            "Experimental control uses the explicitly configured scope instead of the paid Planner's selected scope; invocation remains charged",
        )
    }
}
