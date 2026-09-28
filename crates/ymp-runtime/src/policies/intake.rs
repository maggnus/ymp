//! Atomic criteria from a paid Planner output; questions use recorded P6 inputs.
use ymp_domain::{Proposal, Result, journal::PolicySelection, task::Real};
use ymp_kernel::ports::planning::*;
pub struct VoiClarification {
    selection: PolicySelection,
    cost_interrupt: Real,
}
pub struct NoQuestions {
    selection: PolicySelection,
}
impl VoiClarification {
    pub fn new(cost_interrupt: Real) -> Result<Self> {
        if cost_interrupt.get() < 0.0 {
            return Err(ymp_domain::Denial::new(
                "intake_parameters",
                "Interruption cost cannot be negative",
            ));
        }
        Ok(Self {
            selection: PolicySelection::new(
                "IntakePolicy",
                "VoiClarification",
                "1",
                serde_json::json!({"cost_interrupt":cost_interrupt}),
            )?,
            cost_interrupt,
        })
    }
}
impl NoQuestions {
    pub fn new() -> Result<Self> {
        Ok(Self {
            selection: PolicySelection::new(
                "IntakePolicy",
                "NoQuestions",
                "1",
                serde_json::json!({"cost_interrupt":0.0}),
            )?,
        })
    }
}
pub(crate) fn basis(input: &PaidPlanningView) -> Vec<ymp_domain::Ref> {
    vec![
        input.admission.clone(),
        input.completion.clone(),
        input.receipt.clone(),
        input.prompt.contract.clone(),
    ]
}
fn criteria(
    input: &PaidPlanningView,
    selection: &PolicySelection,
    threshold: Option<Real>,
) -> Result<Proposal<IntakeOutcome>> {
    let output = ymp_kernel::plans::extracted(input)?;
    let mut criteria = input.prompt.criteria.clone();
    criteria.extend(output.criteria);
    let questions = output
        .questions
        .into_iter()
        .map(|q| {
            if threshold
                .is_some_and(|t| q.p_misinterpretation.get() * q.rework_cost.get() > t.get())
            {
                QuestionDecision::Ask(q)
            } else {
                QuestionDecision::Assume(q)
            }
        })
        .collect();
    Ok(Proposal{value:IntakeOutcome{criteria,questions},rationale:"Retain user criteria and attribute extracted requirements to the paid Planner; use P6 interruption cost".into(),basis:basis(input),policy:selection.policy.clone()})
}
impl IntakePolicy for VoiClarification {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn criteria(&self, input: &PaidPlanningView) -> Result<Proposal<IntakeOutcome>> {
        criteria(input, &self.selection, Some(self.cost_interrupt))
    }
}
impl IntakePolicy for NoQuestions {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn criteria(&self, input: &PaidPlanningView) -> Result<Proposal<IntakeOutcome>> {
        criteria(input, &self.selection, None)
    }
}
