//! Application boundary for explicit user tasks. Native execution is not started by intake.

use std::sync::Arc;
use ymp_domain::{Id, Result};
use ymp_kernel::{intake::Intake, journal::Journal};

pub use ymp_domain::{
    journal::PolicySelection,
    task::{
        AcceptanceContract, Assumption, Clarification, Constraints, Criterion, CriterionKind,
        CriterionOrigin, EvidenceClass, Goal, Pins, Real, SessionStatus, Task,
    },
};
pub use ymp_kernel::{
    decision::SessionControl,
    intake::{IntakeNote, IntakeRefinement, IntakeRequest},
    view::SessionView,
};

pub struct Application<J: Journal> {
    journal: Arc<J>,
    intake: Intake<J>,
}
impl<J: Journal> Application<J> {
    pub fn new(journal: Arc<J>) -> Self {
        Self {
            intake: Intake::new(journal.clone()),
            journal,
        }
    }

    pub fn open(
        &self,
        session: Id,
        at: u64,
        request: IntakeRequest,
        selections: Vec<PolicySelection>,
    ) -> Result<SessionControl> {
        self.intake.open(session, at, request, selections)
    }
    pub fn refine(&self, control: &SessionControl, refinement: IntakeRefinement) -> Result<u64> {
        self.intake.refine(control, refinement)
    }
    pub fn view(&self, session: &Id, through: Option<u64>) -> Result<SessionView> {
        self.journal
            .read(session)?
            .view_with_schemas(session, through, self.journal.schemas())
    }
}
