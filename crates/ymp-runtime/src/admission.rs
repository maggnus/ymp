//! Runtime assembly for resource proposals and atomic admission. It starts no backend.
use crate::policies::resources::{allowance_response, estimate_response};
use std::{collections::BTreeSet, sync::Arc};
use ymp_domain::{
    Denial, Id, Ref, Result,
    assignment::{Assignment, Grant, RoleKind, TeamOperation},
    coordination::Lease,
    journal::Capability,
    resources::{Reservation, ResourceDemand},
    workspace::{LockMode, Workspace, WorkspacePath},
};
pub use ymp_kernel::gatekeeper::{AdmittedAssignment, GrantToken, PreparedAdmission};
use ymp_kernel::{
    gatekeeper::{AdmissionRequest, Gatekeeper},
    journal::{ContentStore, Journal},
    ports::{
        execution::WorkspaceProvider,
        resources::{CostModel, ResourcePolicy},
    },
    treasury::{allowance_view, estimate_view},
    workspace_guard::LockRequest,
};
pub struct AdmissionCommand {
    pub assignment: Id<Assignment>,
    pub award: Ref,
    pub role: RoleKind,
    pub workspace: Id<Workspace>,
    pub access: BTreeSet<Capability>,
    pub paths: Vec<(WorkspacePath, LockMode)>,
    pub reservation: Id<Reservation>,
    pub grant: Id<Grant>,
    pub operations: BTreeSet<TeamOperation>,
    pub lease: Lease,
}
pub struct AdmissionPolicies<'a> {
    pub cost: &'a dyn CostModel,
    pub resources: &'a dyn ResourcePolicy,
}
/// Retain this runtime for the lifetime of its prepared attempts and file handles.
/// Reopening a journal does not reconstruct their local secrets or completion rights.
pub struct AdmissionRuntime<J: Journal, C: ContentStore> {
    gatekeeper: Arc<Gatekeeper<J, C>>,
    journal: Arc<J>,
}
impl<J: Journal, C: ContentStore> AdmissionRuntime<J, C> {
    pub fn results(&self) -> Result<ymp_kernel::results::Results<J, C>> {
        ymp_kernel::results::Results::new(self.journal.clone(), self.gatekeeper.clone())
    }
    pub(crate) fn new(journal: Arc<J>, content: Arc<C>) -> Self {
        Self {
            gatekeeper: Arc::new(Gatekeeper::new(journal.clone(), content)),
            journal,
        }
    }
    pub fn prepare(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        command: AdmissionCommand,
        policies: AdmissionPolicies<'_>,
        provider: Option<Arc<dyn WorkspaceProvider>>,
    ) -> Result<PreparedAdmission> {
        let source = command.award.clone();
        let attempt = (|| -> Result<AdmissionRequest> {
            let view = self.gatekeeper.view(session)?;
            if view.revision() != expected {
                return Err(Denial::new("stale_revision", "Admission input changed"));
            }
            let award = view
                .coordination()
                .awards()
                .values()
                .find(|value| value.reference == command.award)
                .ok_or_else(|| {
                    Denial::new("award_missing", "No matching persisted award")
                        .with_ref(command.award.clone())
                })?;
            let offer = &view.coordination().offers()[&award.value.decision.outcome.offer].value;
            let contribution =
                &view.coordination().contributions()[&award.value.commitment.subject].value;
            let agent = view
                .registry()
                .and_then(|registry| {
                    registry
                        .input
                        .facts
                        .agents
                        .iter()
                        .find(|agent| agent.id == offer.agent)
                })
                .ok_or_else(|| Denial::new("agent_missing", "Awarded agent is unavailable"))?;
            let demand = ResourceDemand {
                contribution: contribution.id.erased(),
                kind: contribution.kind,
                difficulty: contribution.difficulty,
                provider: agent.provider.clone(),
                profile: offer.profile.clone(),
            };
            let estimate = estimate_response(policies.cost, &estimate_view(&view, &demand)?)?;
            let allowance = allowance_response(
                policies.resources,
                &allowance_view(&view, &demand, estimate.proposal.value.clone(), at)?,
            )?;
            let files = if command.paths.is_empty() {
                None
            } else {
                let provider = provider.ok_or_else(|| {
                    Denial::new(
                        "workspace_provider",
                        "File access requires a workspace provider",
                    )
                })?;
                Some(self.gatekeeper.workspace().prepare_mediation(
                    session,
                    expected,
                    at,
                    LockRequest {
                        assignment: command.assignment.erased(),
                        workspace: command.workspace.clone(),
                        profile: offer.profile.clone(),
                        paths: command.paths,
                    },
                    provider,
                )?)
            };
            Ok(AdmissionRequest {
                assignment: command.assignment,
                award: command.award,
                role: command.role,
                workspace: command.workspace,
                access: command.access,
                reservation: command.reservation,
                grant: command.grant,
                operations: command.operations,
                lease: command.lease,
                estimate,
                allowance,
                files,
            })
        })();
        match attempt {
            Ok(request) => self.gatekeeper.prepare(session, expected, at, request),
            Err(reason) => self
                .gatekeeper
                .reject(session, expected, at, source, reason),
        }
    }
    pub fn admit(&self, attempt: &mut PreparedAdmission) -> Result<AdmittedAssignment<J>> {
        self.gatekeeper.admit(attempt)
    }
    pub fn authorize(
        &self,
        token: &GrantToken,
        operation: TeamOperation,
        at: u64,
    ) -> Result<Assignment> {
        self.gatekeeper.authorize(token, operation, at)
    }
    pub fn revoke(
        &self,
        session: &Id,
        expected: u64,
        at: u64,
        assignment: &Id<Assignment>,
        reason: String,
    ) -> Result<u64> {
        self.gatekeeper
            .revoke(session, expected, at, assignment, reason)
    }
    pub fn release_files(
        &self,
        session: &Id,
        at: u64,
        files: &ymp_kernel::workspace_guard::MediatedAccess<J>,
    ) -> Result<u64> {
        let evidence = self.gatekeeper.workspace().withdraw_mediated(files)?;
        let revision = self.gatekeeper.view(session)?.revision();
        self.gatekeeper
            .workspace()
            .release(session, revision, at, &evidence)
    }
}
impl<J: Journal + 'static, C: ContentStore + 'static> AdmissionRuntime<J, C> {
    /// Shares the exact issuer of admitted grants and mediated file handles.
    pub fn execution(
        &self,
        backend: Arc<dyn ymp_kernel::ports::execution::ExecutionBackend>,
        cost: Arc<dyn CostModel>,
        clock: Arc<dyn crate::clock::Clock>,
    ) -> Result<crate::execution_host::ExecutionHost<J, C>> {
        crate::execution_host::ExecutionHost::new(
            self.journal.clone(),
            self.gatekeeper.clone(),
            backend,
            cost,
            clock,
        )
    }
}
