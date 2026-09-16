//! Award strategy boundary. Participant bidding and team strategy ports follow in W3.
use crate::arbiter::AwardView;
use ymp_domain::{Proposal, Result, coordination::Award, journal::PolicySelection};
pub trait AwardPolicy: Send + Sync {
    fn selection(&self) -> &PolicySelection;
    fn award(&self, view: &AwardView) -> Result<Proposal<Award>>;
}
