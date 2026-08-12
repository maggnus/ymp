#![forbid(unsafe_code)]

use thiserror::Error;
use ymp_domain::{RunState, RunStatus};

#[derive(Debug, Error, Eq, PartialEq)]
pub enum InvariantViolation {
    #[error("terminal run still has active authority")]
    TerminalAuthority,
}

pub fn validate_state(state: &RunState) -> Result<(), InvariantViolation> {
    if state.status != RunStatus::Running && !state.active_attempts.is_empty() {
        return Err(InvariantViolation::TerminalAuthority);
    }
    Ok(())
}
