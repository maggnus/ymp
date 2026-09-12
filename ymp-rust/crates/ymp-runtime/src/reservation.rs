//! Owned workspace admission. A reservation precedes native authority and is
//! derived by the runtime from its installed backend and access policy.
use crate::{mcp::TeamServer, workspace_access::AccessLease};
use anyhow::{ensure, Result};
use std::sync::Arc;
use ymp_core::*;
use ymp_providers::TurnRequest;
use ymp_storage::Store;

pub enum WorkspaceAdmission {
    Acquired(Box<WorkspaceReservation>),
    Deferred(WorkspaceWait),
}
/// Drop closes any bound live capability before releasing filesystem ownership.
/// Trusted callers keep this value until native work and terminal accounting end.
pub struct WorkspaceReservation {
    pub(crate) lease: AccessLease,
    pub(crate) store: Store,
    pub(crate) session_id: String,
    pub(crate) request: TurnRequest,
    pub(crate) task: Option<TaskAttemptRef>,
    pub(crate) access: WorkspaceAccessDecision,
    pub(crate) authority: Option<(Arc<TeamServer>, String)>,
}
impl WorkspaceReservation {
    pub fn assignment_id(&self) -> &str {
        &self.lease.id
    }
    pub fn access(&self) -> &WorkspaceAccessDecision {
        &self.access
    }

    pub fn admit_reserved(
        &mut self,
        server: Arc<TeamServer>,
        assignment: &mut AssignmentRecord,
        invocation: &mut InvocationRecord,
        operations: Vec<TeamOperation>,
    ) -> Result<String> {
        ensure!(
            self.authority.is_none(),
            "workspace_reservation_spent: one reservation binds one invocation"
        );
        ensure!(
            assignment.id == self.lease.id
                && assignment.session_id == self.session_id
                && assignment.agent_id == self.request.profile.id
                && assignment.provider_id == self.request.provider.id
                && assignment.purpose == self.request.purpose
                && assignment.cwd.canonicalize()? == self.request.cwd.canonicalize()?
                && assignment.requested == self.request.settings
                && assignment.timeout_secs == self.request.timeout_secs
                && assignment.task == self.task
                && invocation.execution_backend.as_ref() == Some(&self.access.backend),
            "workspace_binding: assignment differs from the reserved request or execution backend"
        );
        let token = server.admit_reserved(assignment, invocation, operations)?;
        self.authority = Some((server, invocation.id.clone()));
        let recorded = self.store.record_decision(&DecisionRecord {
            id: new_id(),
            session_id: self.session_id.clone(),
            kind: "workspace_access_admitted".into(),
            actor: Some(assignment.agent_id.clone()),
            reason: self.access.rationale.clone(),
            outcome: None,
            links: RecordLinks {
                task: assignment.task.clone(),
                assignment_id: Some(assignment.id.clone()),
                invocation_id: Some(invocation.id.clone()),
                workspace_access: Some(self.access.clone()),
                ..Default::default()
            },
            created_at: now(),
        });
        if let Err(error) = recorded {
            if let Some((server, id)) = self.authority.take() {
                let _ = server.finish(
                    &id,
                    InvocationState::Interrupted,
                    Some("Workspace admission evidence could not be committed"),
                );
            }
            return Err(error);
        }
        Ok(token)
    }
}
impl Drop for WorkspaceReservation {
    fn drop(&mut self) {
        if let Some((server, id)) = &self.authority {
            let _ = server.finish(
                id,
                InvocationState::Interrupted,
                Some("Workspace reservation ended before terminal native accounting"),
            );
        }
    }
}
