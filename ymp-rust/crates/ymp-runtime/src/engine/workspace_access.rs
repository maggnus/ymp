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

    pub(super) fn workspace_access_decision(
        &self,
        session: &str,
        id: &str,
        request: &TurnRequest,
        task: Option<&TaskAttemptRef>,
    ) -> Result<WorkspaceAccessDecision> {
        let task_value = task
            .map(|reference| {
                self.store
                    .tasks(session)?
                    .into_iter()
                    .find(|t| t.id == reference.task_id && t.attempts == reference.attempt)
                    .context("Workspace policy task is missing or stale")
            })
            .transpose()?;
        let backend_access = self.execution_backend.workspace_access(request);
        let effective_access = self
            .workspace_policy
            .resolve(&crate::WorkspaceAccessInput {
                directory: &request.cwd,
                purpose: &request.purpose,
                task: task_value.as_ref(),
                backend_access: &backend_access,
            })?;
        anyhow::ensure!(
            effective_access.covers(&backend_access),
            "unsupported_workspace_guarantee: policy cannot narrow actual backend access"
        );
        crate::workspace_access::validate_access(&request.cwd, &backend_access)?;
        crate::workspace_access::validate_access(&request.cwd, &effective_access)?;
        Ok(WorkspaceAccessDecision { reservation_id:id.into(),policy:self.workspace_policy_identity.clone(),backend:self.backend_identity.clone(),directory:request.cwd.canonicalize()?,backend_access,effective_access,rationale:"Direct MVP execution; access comes from the trusted backend. Unbounded writers own the whole directory. No rollback or source isolation is provided.".into() })
    }

    /// One-shot trusted admission. A deferred result owns no resource, creates
    /// no assignment/grant/reservation spend, and installs no queued work.
    pub fn try_reserve_workspace(
        &self,
        session: &str,
        request: &TurnRequest,
        task: Option<TaskAttemptRef>,
    ) -> Result<crate::WorkspaceAdmission> {
        let policy = self
            .store
            .session_policy(session)?
            .context("Workspace admission needs captured session constraints")?;
        anyhow::ensure!(!self.cancel.is_cancelled(), "Cancelled");
        anyhow::ensure!(
            request.timeout_secs > 0 && request.timeout_secs <= policy.limits.turn_timeout_secs,
            "timeout_limit: request exceeds captured native timeout"
        );
        if let Some(resources) = &policy.limits.resources {
            anyhow::ensure!(
                request
                    .resource_controls
                    .max_turns
                    .is_some_and(|n| n > 0 && n <= resources.native_max_turns)
                    && request
                        .resource_controls
                        .max_output_chars
                        .is_some_and(|n| n > 0 && n <= resources.max_output_chars),
                "resource_ceiling: requested native controls exceed captured limits"
            );
        }

        anyhow::ensure!(
            policy.cwd.canonicalize()? == request.cwd.canonicalize()?,
            "workspace_binding: requested directory differs from the captured session"
        );
        let eligible = self.refresh_team_eligibility(session)?;
        let agent = eligible
            .iter()
            .find(|agent| agent.id == request.profile.id && agent.provider == request.provider.id)
            .context("ineligible_member: requested agent is unavailable")?;
        anyhow::ensure!(serde_json::to_value(&request.provider)? == serde_json::to_value(self.config.provider(&agent.provider)?)? && serde_json::to_value(&request.profile)? == serde_json::to_value(agent)?, "workspace_binding: native provider or agent differs from the captured execution configuration");
        if let Some(team) = self.store.team_state(session)? {
            anyhow::ensure!(
                team.current_members.contains(&agent.id),
                "ineligible_member: requested agent is not a current participant"
            );
        }
        let resolved = policy
            .execution
            .get(&agent.id)
            .cloned()
            .unwrap_or_default()
            .resolve(
                agent,
                &ModelEffort {
                    model: request.settings.model.clone(),
                    effort: request.settings.effort.clone(),
                },
            )?;
        anyhow::ensure!(
            resolved.model == request.settings.model && resolved.effort == request.settings.effort,
            "workspace_binding: unresolved native settings"
        );
        self.validate_native_settings(agent, &request.settings)?;
        let id = new_id();
        let access = self.workspace_access_decision(session, &id, request, task.as_ref())?;
        match self.try_acquire_access(
            session,
            &id,
            Some(&agent.id),
            task.clone(),
            &access,
            policy.limits.parallel,
        )? {
            Ok(lease) => Ok(crate::WorkspaceAdmission::Acquired(Box::new(
                crate::WorkspaceReservation {
                    lease,
                    store: self.store.clone(),
                    session_id: session.into(),
                    request: request.clone(),
                    task,
                    access,
                    authority: None,
                },
            ))),
            Err(wait) => {
                self.record_access_wait(session, Some(&agent.id), task, &access, &wait)?;
                Ok(crate::WorkspaceAdmission::Deferred(wait))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_acquire_access(
        &self,
        session: &str,
        id: &str,
        agent: Option<&str>,
        task: Option<TaskAttemptRef>,
        access: &WorkspaceAccessDecision,
        limit: usize,
    ) -> Result<std::result::Result<AccessLease, WorkspaceWait>> {
        let mut lease = match coordinator().acquire(
            id,
            session,
            agent,
            &access.directory,
            &access.effective_access,
            self.workspace_parent.as_deref(),
            limit,
        ) {
            Ok(lease) => lease,
            Err(wait) => return Ok(Err(wait)),
        };
        lease.record = Some((self.store.clone(), session.into(), access.clone()));
        self.store.record_decision(&DecisionRecord {
            id: new_id(),
            session_id: session.into(),
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
        Ok(Ok(lease))
    }

    fn record_access_wait(
        &self,
        session: &str,
        agent: Option<&str>,
        task: Option<TaskAttemptRef>,
        access: &WorkspaceAccessDecision,
        wait: &WorkspaceWait,
    ) -> Result<()> {
        self.store.record_decision(&DecisionRecord {
            id: new_id(),
            session_id: session.into(),
            kind: "assignment_waiting".into(),
            actor: agent.map(str::to_owned),
            reason: wait.detail.clone(),
            outcome: None,
            links: RecordLinks {
                task,
                workspace_access: Some(access.clone()),
                workspace_wait: Some(wait.clone()),
                ..Default::default()
            },
            created_at: now(),
        })
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
            match self.try_acquire_access(
                &ctx.session.id,
                id,
                agent,
                task.clone(),
                access,
                ctx.limits.parallel,
            )? {
                Ok(lease) => return Ok(lease),
                Err(wait) => {
                    if previous.as_ref() != Some(&wait) {
                        self.record_access_wait(
                            &ctx.session.id,
                            agent,
                            task.clone(),
                            access,
                            &wait,
                        )?;
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
