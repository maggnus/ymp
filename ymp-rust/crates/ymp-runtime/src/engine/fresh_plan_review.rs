use super::*;
use anyhow::ensure;

impl Engine {
    /// New independent review of immutable data, not replay or effect certification.
    /// This owner action never calls run, commits tasks, or grants a retry permit.
    pub async fn review_saved_plan_fresh(
        &self,
        command: &FreshPlanReviewCommand,
    ) -> Result<FreshPlanReviewReceipt> {
        ensure!(
            !command.command_id.trim().is_empty(),
            "owner_command: fresh-review ID required"
        );
        if let Some(receipt) = self.store.fresh_plan_review_receipt(command)? {
            return Ok(receipt);
        }
        let session = self.store.session(&command.session_id)?;
        let policy = self
            .store
            .session_policy(&session.id)?
            .context("Fresh review requires captured policy")?;
        let project = self.store.project(&policy.cwd)?;
        ensure!(
            project.id == session.project_id,
            "fresh_review_binding: foreign project"
        );
        let owner = Arc::new(crate::WorkspaceOwner::acquire(&self.store, &project)?);
        let _lock = self.store.lock_session(&session)?;
        if let Some(receipt) = self.store.fresh_plan_review_receipt(command)? {
            return Ok(receipt);
        }
        self.owner_boundary(&session.id)?;
        let stage = self.store.validate_fresh_plan_review(command)?;
        let trace = self.store.trace(&session.id)?;
        let limits = trace
            .budget
            .as_ref()
            .context("Missing captured budget")?
            .limits
            .clone();
        let workspace = Workspace::open(
            &project.path,
            &self.store.session_dir(&session).join("workspace"),
        )?;
        let server =
            Arc::new(TeamServer::start(self.store.clone(), &session, self.events.clone()).await?);
        let ctx = RunContext {
            workspace_owner: owner,
            session: session.clone(),
            server,
            workspace,
            turns: Arc::new(AtomicUsize::new(usize::try_from(trace.usage.total.calls)?)),
            permits: Arc::new(Semaphore::new(limits.parallel)),
            limits,
        };
        let candidates = self
            .eligible_agents(&session.id)?
            .into_iter()
            .filter(|a| a.id == command.reviewer_id)
            .collect::<Vec<_>>();
        ensure!(
            !candidates.is_empty(),
            "fresh_review_ineligible: requested independent reviewer is unavailable"
        );
        let agent = self.choose(
            &ctx,
            &candidates,
            "verification",
            "standard",
            "fresh independent saved-plan review",
            "review_plan",
            None,
        )?;
        let mut engine = self.clone();
        engine.require_recovery_read_only = true;
        engine.fresh_plan_review_binding = Some(Arc::new(command.clone()));
        let prompt = format!("Perform a NEW independent read-only review of this exact saved proposal against the captured request. This is not a replay of the prior invocation. Its historical effects remain UNKNOWN; do not certify them, assume the workspace is unchanged, or execute any production task. Review the immutable proposal only and retain any objections.\nRequest: {}\nProposal/version: {}\nPrior failed invocation IDs: {}\nReturn ONLY JSON {{\"approved\":true|false,\"reason\":\"specific plan-review findings\"}}.", policy.goal, serde_json::to_string(&command.proposal)?, serde_json::to_string(&stage.failures.iter().map(|f| &f.invocation_id).collect::<Vec<_>>())?);
        let response = engine
            .ask_scoped_once(
                &ctx,
                &agent,
                &ctx.workspace.directory,
                "review_plan",
                &prompt,
                true,
                None,
                &mut None,
            )
            .await?;
        let review: Review = parse_response(&response.text)?;
        let fresh = FreshPlanReviewRecord {
            command: command.clone(),
            prior_failures: stage.failures,
            response: SavedResponse {
                text: response.text,
                assignment_id: response.assignment_id.clone(),
                invocation_id: response.invocation_id.clone(),
                agent_id: agent.id.clone(),
            },
            approved: review.approved,
            reason: review.reason.clone(),
        };
        self.store.commit_fresh_plan_review(&DecisionRecord {
            id: new_id(),
            session_id: session.id,
            kind: "plan_review".into(),
            actor: Some(agent.id),
            reason: review.reason,
            outcome: Some(if review.approved {
                DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Unconfirmed,
                }
            } else {
                DecisionOutcome::Rejected
            }),
            links: RecordLinks {
                plan_proposal: Some(command.proposal.clone()),
                fresh_plan_review: Some(Box::new(fresh)),
                assignment_id: Some(response.assignment_id),
                invocation_id: Some(response.invocation_id),
                ..Default::default()
            },
            created_at: now(),
        })
    }

    /// The task schema has no assertion about dependencies on old invocation
    /// effects. Conservatively stop production/revision instead of inventing one.
    pub(super) fn check_fresh_plan_dependencies(&self, session: &str) -> Result<()> {
        if let Some(block) = self.store.unresolved_fresh_plan_dependencies(session)? {
            if !self
                .store
                .decisions(session)?
                .iter()
                .any(|d| d.links.unresolved_effect_dependencies.as_deref() == Some(&block))
            {
                self.store.record_decision(&DecisionRecord {
                    id: new_id(),
                    session_id: session.into(),
                    kind: "saved_plan_execution_blocked".into(),
                    actor: None,
                    reason: block.to_string(),
                    outcome: None,
                    links: RecordLinks {
                        unresolved_effect_dependencies: Some(Box::new(block.clone())),
                        ..Default::default()
                    },
                    created_at: now(),
                })?;
            }
            return Err(block.into());
        }
        Ok(())
    }
}
