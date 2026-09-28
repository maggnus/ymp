//! Application boundary for explicit user tasks. Native execution is not started by intake.

use std::collections::BTreeSet;
use std::sync::Arc;
use ymp_domain::{Id, Result};
use ymp_domain::{
    identity::{
        Agent, ExecutionProfile, Pool, ProfileSettings, RegistryFacts, WorkspaceCapabilities,
    },
    journal::Capability,
};
pub use ymp_kernel::registry::{ReadinessResponse, ReadinessView, RegistryInput, readiness_views};
use ymp_kernel::{intake::Intake, journal::Journal, registry::Registry};

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
    registry: Registry<J>,
}
impl<J: Journal> Application<J> {
    pub fn acceptance<C: ymp_kernel::journal::ContentStore>(
        &self,
        content: Arc<C>,
    ) -> ymp_kernel::acceptance::AcceptanceAuthority<J, C> {
        self.intake.acceptance(content)
    }
    pub fn new(journal: Arc<J>) -> Self {
        Self {
            intake: Intake::new(journal.clone()),
            registry: Registry::new(journal.clone()),
            journal,
        }
    }

    /// Create a stable admission runtime with explicitly owned content storage.
    pub fn admission<C: ymp_kernel::journal::ContentStore>(
        &self,
        content: Arc<C>,
    ) -> crate::admission::AdmissionRuntime<J, C> {
        crate::admission::AdmissionRuntime::new(self.journal.clone(), content)
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
    pub fn registry_input(
        &self,
        session: &Id,
        facts: RegistryFacts,
        at: u64,
    ) -> Result<RegistryInput> {
        self.registry.prepare(session, facts, at)
    }
    pub fn record_pool(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        input: RegistryInput,
        effective: PolicySelection,
        responses: Vec<ReadinessResponse>,
    ) -> Result<u64> {
        self.registry
            .record(session, expected, at, input, effective, responses)
    }
    pub fn pool(&self, session: &Id) -> Result<Pool> {
        self.registry.pool(session)
    }
    pub fn profile(
        &self,
        session: &Id,
        agent: &Id<Agent>,
        settings: &ProfileSettings,
    ) -> Result<ExecutionProfile> {
        self.registry.profile(session, agent, settings)
    }
    pub fn capabilities(
        &self,
        session: &Id,
        profile: &ExecutionProfile,
        workspace: &WorkspaceCapabilities,
    ) -> Result<BTreeSet<Capability>> {
        self.registry.capabilities(session, profile, workspace)
    }
}
