//! Eligibility proposals; actual competence observations remain separate work.
use ymp_domain::{Proposal, Result, journal::PolicySelection, verification::ConfirmationGrade};
pub trait CreditPolicy {
    fn selection(&self) -> &PolicySelection;
    fn creditable(&self, grade: ConfirmationGrade) -> Result<Proposal<bool>>;
}
