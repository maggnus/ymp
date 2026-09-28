//! Award strategy boundary. Participant bidding and team strategy ports follow in W3.
use crate::arbiter::AwardView;
use ymp_domain::{Proposal, Result, coordination::Award, journal::PolicySelection};
pub trait AwardPolicy: Send + Sync {
    fn selection(&self) -> &PolicySelection;
    fn award(&self, view: &AwardView) -> Result<Proposal<Award>>;
    fn commitment_terms(
        &self,
        _view: &AwardView,
    ) -> Result<Proposal<ymp_domain::coordination::CommitmentTerms>> {
        Err(ymp_domain::Denial::new(
            "commitment_terms",
            "This award strategy does not supply commitment terms",
        ))
    }
}
