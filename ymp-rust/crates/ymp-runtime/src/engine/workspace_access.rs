use super::*;
use crate::workspace_access::{coordinator, AccessLease};

impl Engine {
    pub(super) fn recover_workspace_access(&self, session: &str) -> Result<()> {
        let trace = self.store.trace(session)?;
        let released = trace
            .decisions
            .iter()
            .filter(|d| d.kind == "workspace_access_released")
            .filter_map(|d| {
                d.links
                    .workspace_access
                    .as_ref()
                    .map(|a| a.reservation_id.clone())
            })
            .collect::<HashSet<_>>();
        for decision in trace
            .decisions
            .iter()
            .filter(|d| d.kind == "workspace_access_acquired")
        {
            if let Some(access) = decision
                .links
                .workspace_access
                .as_ref()
                .filter(|a| !released.contains(&a.reservation_id))
            {
                self.store.record_decision(&DecisionRecord {
                    id:new_id(),session_id:session.into(),kind:"workspace_access_released".into(),actor:None,
                    reason:"Restart ended historical resource ownership; native continuation restores no active reservation".into(),outcome:None,
                    links:RecordLinks {workspace_access:Some(access.clone()),..Default::default()},created_at:now(),
                })?;
            }
        }
        Ok(())
    }

    pub fn with_workspace_access_policy(
        mut self,
        policy: Arc<dyn crate::WorkspaceAccessPolicy>,
    ) -> Result<Self> {
        let identity = policy.identity();
        identity.validate()?;
        self.workspace_policy_identity = identity;
        self.workspace_policy = policy;
        Ok(self)
    }

    pub(super) async fn acquire_workspace(
        &self,
        ctx: &RunContext,
        id: &str,
        agent: Option<&str>,
        task: Option<TaskAttemptRef>,
        access: &WorkspaceAccessDecision,
    ) -> Result<AccessLease> {
        let coordinator = coordinator();
        let mut previous = None;
        loop {
            let notified = coordinator.changed.notified();
            tokio::pin!(notified);
            // Register before testing the condition so release cannot be lost.
            notified.as_mut().enable();
            if self.cancel.is_cancelled() {
                bail!("Cancelled");
            }
            match coordinator.acquire(
                id,
                &ctx.session.id,
                agent,
                &access.directory,
                &access.effective_access,
                self.workspace_parent.as_deref(),
                ctx.limits.parallel,
            ) {
                Ok(mut lease) => {
                    lease.record =
                        Some((self.store.clone(), ctx.session.id.clone(), access.clone()));
                    self.store.record_decision(&DecisionRecord {
                        id: new_id(),
                        session_id: ctx.session.id.clone(),
                        kind: "workspace_access_acquired".into(),
                        actor: agent.map(str::to_owned),
                        reason: access.rationale.clone(),
                        outcome: None,
                        links: RecordLinks {
                            task,
                            workspace_access: Some(access.clone()),
                            ..Default::default()
                        },
                        created_at: now(),
                    })?;
                    return Ok(lease);
                }
                Err(wait) => {
                    if previous.as_ref() != Some(&wait) {
                        self.store.record_decision(&DecisionRecord {
                            id: new_id(),
                            session_id: ctx.session.id.clone(),
                            kind: "assignment_waiting".into(),
                            actor: agent.map(str::to_owned),
                            reason: wait.detail.clone(),
                            outcome: None,
                            links: RecordLinks {
                                task: task.clone(),
                                workspace_access: Some(access.clone()),
                                workspace_wait: Some(wait.clone()),
                                ..Default::default()
                            },
                            created_at: now(),
                        })?;
                        if let Some(agent) = agent {
                            let _ = self.events.send(UiEvent::AgentStatus {
                                agent: agent.into(),
                                status: format!("waiting: {}", wait.code),
                            });
                        }
                        previous = Some(wait);
                    }
                }
            }
            tokio::select! { _ = self.cancel.cancelled() => bail!("Cancelled"), _ = &mut notified => {} }
        }
    }

    pub(super) fn record_workspace_wait(
        &self,
        ctx: &RunContext,
        task: &Task,
        code: &str,
        detail: &str,
    ) -> Result<()> {
        self.store.record_decision(&DecisionRecord {
            id: new_id(),
            session_id: ctx.session.id.clone(),
            kind: "assignment_waiting".into(),
            actor: None,
            reason: detail.into(),
            outcome: None,
            links: RecordLinks {
                task: Some(TaskAttemptRef::from(task)),
                workspace_wait: Some(WorkspaceWait {
                    code: code.into(),
                    holder: None,
                    detail: detail.into(),
                }),
                ..Default::default()
            },
            created_at: now(),
        })
    }

    pub(super) async fn verify(
        &self,
        ctx: &RunContext,
        task: &mut Task,
        prompt: &str,
    ) -> Result<()> {
        // Arbitrary shell checks can write anywhere. Hold the whole directory
        // through snapshotting, checks, native inspection and final acceptance.
        let id = new_id();
        let access = WorkspaceAccessDecision {
            reservation_id: id.clone(),
            policy: self.workspace_policy_identity.clone(), backend: self.backend_identity.clone(), directory: ctx.workspace.directory.canonicalize()?,
            backend_access: WorkspaceAccess::WriteAll, effective_access: WorkspaceAccess::WriteAll,
            rationale: "Runtime verification protects the candidate and inputs through acceptance; arbitrary check commands require exclusive access".into(),
        };
        let lease = self
            .acquire_workspace(ctx, &id, None, Some(TaskAttemptRef::from(&*task)), &access)
            .await?;
        let mut engine = self.clone();
        engine.workspace_parent = Some(lease.id.clone());
        engine.verify_protected(ctx, task, prompt).await
    }
}
