//! Weighted evidence progress and an acceptance-only experimental monitor.
use ymp_domain::{Proposal, Result, journal::PolicySelection};
use ymp_kernel::ports::progress::*;
pub struct EvidenceDelta {
    selection: PolicySelection,
    parameters: MonitorParameters,
}
pub struct AcceptedOnlyProgress {
    selection: PolicySelection,
    parameters: MonitorParameters,
}
macro_rules! monitor {
    ($name:ident,$only:expr) => {
        impl $name {
            pub fn new(parameters: MonitorParameters) -> Result<Self> {
                parameters.validate()?;
                Ok(Self {
                    selection: PolicySelection::new(
                        "ProgressMonitor",
                        stringify!($name),
                        "1",
                        serde_json::json!(parameters),
                    )?,
                    parameters,
                })
            }
        }
        impl ProgressMonitor for $name {
            fn selection(&self) -> &PolicySelection {
                &self.selection
            }
            fn assess(&self, input: &ProgressInput) -> Result<Proposal<ProgressAssessment>> {
                Ok(Proposal {
                    value: assessment(input, &self.parameters, $only)?,
                    rationale: concat!(
                        "Assess the recorded work boundary with ",
                        stringify!($name)
                    )
                    .into(),
                    basis: input.basis.clone(),
                    policy: self.selection.policy.clone(),
                })
            }
        }
    };
}
monitor!(EvidenceDelta, false);
monitor!(AcceptedOnlyProgress, true);
