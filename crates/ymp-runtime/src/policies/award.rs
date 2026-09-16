//! FirstOffer: the approved deterministic alternative for the minimal W1 award flow.
use ymp_domain::{Denial, Proposal, Result, coordination::Award, journal::PolicySelection};
use ymp_kernel::{arbiter::AwardView, ports::organization::AwardPolicy};
pub struct FirstOffer {
    selection: PolicySelection,
}
impl FirstOffer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            selection: PolicySelection::new(
                "AwardPolicy",
                "FirstOffer",
                "1",
                serde_json::json!({}),
            )?,
        })
    }
}
impl AwardPolicy for FirstOffer {
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn award(&self, view: &AwardView) -> Result<Proposal<Award>> {
        let offer = view
            .offers
            .iter()
            .min_by(|a, b| (a.value.at, &a.value.id).cmp(&(b.value.at, &b.value.id)))
            .ok_or_else(|| Denial::new("no_offers", "No eligible recorded offer is available"))?;
        let rationale =
            "Select the earliest recorded offer, using its ID to break equal-time ties".to_owned();
        Ok(Proposal {
            value: Award {
                solicitation: view.solicitation.value.id.clone(),
                offer: offer.value.id.clone(),
                rationale: rationale.clone(),
            },
            rationale,
            basis: vec![view.solicitation.reference.clone(), offer.reference.clone()],
            policy: self.selection.policy.clone(),
        })
    }
}
