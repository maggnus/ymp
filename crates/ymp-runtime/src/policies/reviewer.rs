//! Independent eligible reviewers, ordered by stable identity or prior review count.
use ymp_domain::{Id, Proposal, Result, identity::Agent, journal::PolicySelection};
use ymp_kernel::ports::reporting::*;
pub struct AnyNonProducer {
    selection: PolicySelection,
}
pub struct LeastUsedReviewer {
    selection: PolicySelection,
}
macro_rules! reviewer {
    ($name:ident,$least:expr) => {
        impl $name {
            pub fn new() -> Result<Self> {
                Ok(Self {
                    selection: PolicySelection::new(
                        "ReviewerPolicy",
                        stringify!($name),
                        "1",
                        serde_json::json!({}),
                    )?,
                })
            }
        }
        impl ReviewerPolicy for $name {
            fn selection(&self) -> &PolicySelection {
                &self.selection
            }
            fn pick(&self, input: &ReviewerInput) -> Result<Proposal<Option<Id<Agent>>>> {
                let mut candidates = input.candidates.clone();
                candidates.sort_by_key(|c| {
                    (
                        if $least { c.prior_reviews } else { 0 },
                        c.profile.agent.clone(),
                    )
                });
                Ok(Proposal {
                    value: candidates.first().map(|c| c.profile.agent.clone()),
                    rationale: concat!(
                        "Select an independent eligible reviewer with ",
                        stringify!($name)
                    )
                    .into(),
                    basis: vec![input.aggregate.clone()],
                    policy: self.selection.policy.clone(),
                })
            }
        }
    };
}
reviewer!(AnyNonProducer, false);
reviewer!(LeastUsedReviewer, true);
