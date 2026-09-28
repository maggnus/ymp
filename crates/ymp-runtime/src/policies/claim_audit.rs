//! Exact evidence-class rules and a conservative status-only audit control.
use ymp_domain::{Proposal, Result, journal::PolicySelection, report::ClaimAudit};
use ymp_kernel::ports::reporting::*;
pub struct EvidenceClassRules {
    selection: PolicySelection,
}
pub struct ConservativeAudit {
    selection: PolicySelection,
}
macro_rules! auditor {
    ($name:ident,$conservative:expr) => {
        impl $name {
            pub fn new() -> Result<Self> {
                Ok(Self {
                    selection: PolicySelection::new(
                        "ClaimAuditor",
                        stringify!($name),
                        "1",
                        serde_json::json!({}),
                    )?,
                })
            }
        }
        impl ClaimAuditor for $name {
            fn selection(&self) -> &PolicySelection {
                &self.selection
            }
            fn audit(&self, input: &ClaimInput) -> Result<Proposal<ClaimAudit>> {
                Ok(Proposal {
                    value: ymp_kernel::finalization::audit_claim(input, $conservative)?,
                    rationale: concat!("Audit exact applicable scope with ", stringify!($name))
                        .into(),
                    basis: vec![],
                    policy: self.selection.policy.clone(),
                })
            }
        }
    };
}
auditor!(EvidenceClassRules, false);
auditor!(ConservativeAudit, true);
