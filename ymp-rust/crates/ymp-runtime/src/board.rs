//! Replaceable proposal ordering; runtime validation owns every transition.
use anyhow::Result;
use ymp_core::*;

pub trait BoardProposalPolicy: Send + Sync {
    fn identity(&self) -> ExecutionBackendIdentity;
    /// Choose existing proposal IDs only. Returning a proposal ID confers no authority.
    fn propose(&self, board: &BoardSnapshot) -> Result<Vec<String>>;
}
pub struct OrderedBoardPolicy;
impl BoardProposalPolicy for OrderedBoardPolicy {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.ordered-board".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, board: &BoardSnapshot) -> Result<Vec<String>> {
        Ok(board
            .proposals
            .iter()
            .filter(|p| p.status == BoardProposalStatus::Pending)
            .map(|p| p.id.clone())
            .collect())
    }
}
