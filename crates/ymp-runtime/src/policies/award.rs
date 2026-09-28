//! FirstOffer: the approved deterministic alternative for the minimal W1 award flow.
use ymp_domain::{Denial, Proposal, Result, coordination::Award, journal::PolicySelection};
use ymp_kernel::{arbiter::AwardView, ports::organization::AwardPolicy};
pub struct FirstOffer {
    selection: PolicySelection,
}
impl FirstOffer {
    pub fn with_commitment_terms(terms: ymp_domain::coordination::CommitmentTerms) -> Result<Self> {
        terms.validate()?;
        Ok(Self {
            selection: PolicySelection::new(
                "AwardPolicy",
                "FirstOffer",
                "2",
                serde_json::to_value(terms).map_err(|_| {
                    Denial::new("commitment_terms", "Cannot encode commitment terms")
                })?,
            )?,
        })
    }
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
    fn commitment_terms(
        &self,
        view: &AwardView,
    ) -> Result<Proposal<ymp_domain::coordination::CommitmentTerms>> {
        let terms = serde_json::from_value(self.selection.parameters.clone()).map_err(|_| {
            Denial::new(
                "commitment_terms",
                "Select FirstOffer version 2 with explicit commitment terms",
            )
        })?;
        Ok(Proposal {
            value: terms,
            rationale: "Use the selected bounded commitment terms".into(),
            basis: vec![view.solicitation.reference.clone()],
            policy: self.selection.policy.clone(),
        })
    }
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
