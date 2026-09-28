//! Diagnosis-directed recovery and a conservative stop-on-uncertainty control.
use ymp_domain::{Proposal, Result, journal::PolicySelection};
use ymp_kernel::ports::progress::*;
pub struct DiagnosisFirstLadder {
    selection: PolicySelection,
    parameters: EscalationParameters,
}
pub struct StopOnUncertainty {
    selection: PolicySelection,
    parameters: EscalationParameters,
}
macro_rules! policy {
    ($name:ident,$stop:expr) => {
        impl $name {
            pub fn new(parameters: EscalationParameters) -> Result<Self> {
                parameters.validate()?;
                Ok(Self {
                    selection: PolicySelection::new(
                        "EscalationPolicy",
                        stringify!($name),
                        "1",
                        serde_json::json!(parameters),
                    )?,
                    parameters,
                })
            }
        }
        impl EscalationPolicy for $name {
            fn selection(&self) -> &PolicySelection {
                &self.selection
            }
            fn next(&self, input: &EscalationInput) -> Result<Proposal<EscalationPlan>> {
                Ok(Proposal {
                    value: escalation(input, &self.parameters, $stop)?,
                    rationale: concat!(
                        "Use ",
                        stringify!($name),
                        " within the method's recorded authority and finite limits"
                    )
                    .into(),
                    basis: input.basis.clone(),
                    policy: self.selection.policy.clone(),
                })
            }
        }
    };
}
policy!(DiagnosisFirstLadder, false);
policy!(StopOnUncertainty, true);
