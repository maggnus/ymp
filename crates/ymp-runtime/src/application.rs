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

/// Recovery is an explicit trusted application command, never a participant notice.
pub enum RecoveryIntent {
    Observe,
    Continue,
    /// Restore owner capabilities for zero-call report completion only.
    Report,
}
pub struct RecoveredSession {
    pub control: SessionControl,
    pub budget: Option<ymp_kernel::treasury::BudgetControl>,
}
pub struct Application<J: Journal> {
    journal: Arc<J>,
    intake: Intake<J>,
    registry: Registry<J>,
    treasury: ymp_kernel::treasury::Treasury<J>,
}
impl<J: Journal> Application<J> {
    pub fn finalization<C: ymp_kernel::journal::ContentStore>(
        &self,
        content: Arc<C>,
    ) -> ymp_kernel::finalization::Finalization<J, C> {
        self.intake.finalization(content)
    }
    pub fn progress(&self) -> ymp_kernel::progress::Progress<J> {
        self.intake.progress()
    }
    pub fn plans(&self) -> ymp_kernel::plans::Plans<J> {
        self.intake.plans()
    }
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
            treasury: ymp_kernel::treasury::Treasury::new(journal.clone()),
            journal,
        }
    }

    pub fn recover_attempt<C: ymp_kernel::journal::ContentStore>(
        &self,
        control: &SessionControl,
        results: &ymp_kernel::results::Results<J, C>,
        id: &Id<ymp_domain::plan::Attempt>,
    ) -> Result<ymp_kernel::results::PreparedAttempt> {
        self.intake.recover_attempt(control, results, id)
    }
    pub fn configure(
        &self,
        owner: &SessionControl,
        at: u64,
        definition: ymp_kernel::session::SessionDefinition,
    ) -> Result<ymp_domain::Ref> {
        self.intake.session_control(
            owner,
            at,
            ymp_kernel::session::SessionChange::Configured(Box::new(definition)),
        )
    }
    pub fn treasury(&self) -> &ymp_kernel::treasury::Treasury<J> {
        &self.treasury
    }
    pub fn recover(
        &self,
        session: &Id,
        at: u64,
        intent: RecoveryIntent,
    ) -> Result<RecoveredSession> {
        let control = self.intake.recover(session)?;
        if matches!(intent, RecoveryIntent::Continue) {
            self.intake.session_control(
                &control,
                at,
                ymp_kernel::session::SessionChange::Continue,
            )?;
        }
        let control = self.intake.recover(session)?;
        if matches!(intent, RecoveryIntent::Observe | RecoveryIntent::Report) {
            control.stop_local();
        }
        let budget = if self.view(session, None)?.treasury().is_some() {
            Some(self.intake.recover_budget(&control, &self.treasury)?)
        } else {
            None
        };
        Ok(RecoveredSession { control, budget })
    }
    pub fn interrupt(&self, control: &SessionControl, at: u64) -> Result<ymp_domain::Ref> {
        self.intake
            .session_control(control, at, ymp_kernel::session::SessionChange::Stop)
    }
    pub fn recovery_consumed(
        &self,
        control: &SessionControl,
        at: u64,
        source: ymp_domain::Ref,
    ) -> Result<ymp_domain::Ref> {
        self.intake.session_control(
            control,
            at,
            ymp_kernel::session::SessionChange::RecoveryConsumed(source),
        )
    }
    pub fn phase(
        &self,
        control: &SessionControl,
        at: u64,
        phase: SessionStatus,
    ) -> Result<ymp_domain::Ref> {
        self.intake.session_control(
            control,
            at,
            ymp_kernel::session::SessionChange::Phase(phase),
        )
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

impl<J: Journal + 'static> Application<J> {
    pub fn execution<C: ymp_kernel::journal::ContentStore + 'static>(
        &self,
        admission: &crate::admission::AdmissionRuntime<J, C>,
        owner: Arc<SessionControl>,
        backend: Arc<dyn ymp_kernel::ports::execution::ExecutionBackend>,
        cost: Arc<dyn ymp_kernel::ports::resources::CostModel>,
        clock: Arc<dyn crate::clock::Clock>,
    ) -> Result<crate::execution_host::ExecutionHost<J, C>> {
        self.intake.owner_session(&owner)?;
        admission.require_journal(&self.journal)?;
        Ok(admission.execution(backend, cost, clock)?.controlled(owner))
    }
}
