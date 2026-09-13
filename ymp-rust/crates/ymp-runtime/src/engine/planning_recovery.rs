use super::*;
use anyhow::ensure;

impl Engine {
    /// Planning and explicit revision use the same finite recovery consumer as reviews.
    /// This is a stage-local adapter, not another workflow or policy interface.
    pub(super) async fn ask_plan_recovering(
        &self,
        ctx: &RunContext,
        original: &AgentProfile,
        cwd: &Path,
        prompt: &str,
    ) -> Result<RecordedResponse> {
        let trace = self.store.trace(&ctx.session.id)?;
        let predecessor = trace
            .decisions
            .iter()
            .rev()
            .find(|d| d.kind == "plan_review" && d.outcome == Some(DecisionOutcome::Rejected));
        let digest = content_digest(&serde_json::to_string(&(
            prompt,
            predecessor.map(|d| &d.id),
        ))?);
        let existing = self
            .store
            .recovery_stages(&ctx.session.id)?
            .into_iter()
            .find(|s| {
                s.purpose == "plan"
                    && s.request_digest.as_ref() == Some(&digest)
                    && (s.selected_agent.as_ref() == Some(&original.id)
                        || s.selected_agent.is_none())
            });
        let mut stage = if let Some(stage) = existing {
            stage
        } else {
            let mut stage = RecoveryStage {
                schema_version: 1,
                session_id: ctx.session.id.clone(),
                id: format!("planning-{}-{digest}", original.id),
                revision: 0,
                purpose: "plan".into(),
                request_digest: Some(digest),
                task: None,
                plan: predecessor.and_then(|d| d.links.plan_proposal.clone()),
                result: None,
                review_ids: predecessor.map(|d| vec![d.id.clone()]).unwrap_or_default(),
                selected_agent: Some(original.id.clone()),
                response: None,
                active_invocation_id: None,
                failures: vec![],
                recovery_attempts: 0,
                manual_permit: false,
                admission_denial: None,
                status: RecoveryStatus::Pending,
                condition: None,
                updated_at: now(),
            };
            self.save_stage(&mut stage, None)?;
            stage
        };
        self.restore_stage_response(ctx, &mut stage)?;
        if let Some(denial) = &stage.admission_denial {
            return Err(denial.clone().into());
        }
        ensure!(
            stage.status != RecoveryStatus::Paused,
            "Owner paused planning; explicit continuation required"
        );
        ensure!(
            stage.manual_permit
                || !matches!(
                    stage.status,
                    RecoveryStatus::Waiting | RecoveryStatus::OwnerAction
                ),
            "{}",
            stage
                .condition
                .as_deref()
                .unwrap_or("Planning requires owner continuation")
        );
        loop {
            self.owner_boundary(&ctx.session.id)?;
            ensure!(!self.cancel.is_cancelled(), "Cancelled");
            if let Some(saved) = &stage.response {
                return Ok(RecordedResponse {
                    _access: None,
                    text: saved.text.clone(),
                    assignment_id: saved.assignment_id.clone(),
                    invocation_id: saved.invocation_id.clone(),
                });
            }
            let peers = self.eligible_agents(&ctx.session.id)?;
            if (!stage.failures.is_empty()
                || stage
                    .selected_agent
                    .as_ref()
                    .is_some_and(|id| !peers.iter().any(|a| &a.id == id)))
                && !stage.manual_permit
            {
                self.recover_stage(ctx, &mut stage, &peers).await?;
            }
            if stage.selected_agent.is_none() {
                stage.selected_agent = Some(
                    self.choose(
                        ctx,
                        &peers,
                        "planning",
                        "standard",
                        "resume unfinished planning",
                        "plan",
                        None,
                    )?
                    .id,
                );
            }
            let agent = peers
                .iter()
                .find(|a| Some(&a.id) == stage.selected_agent.as_ref())
                .context("Planning participant unavailable after recovery")?
                .clone();
            stage.status = RecoveryStatus::Running;
            stage.manual_permit = false;
            self.save_stage(&mut stage, None)?;
            let mut engine = self.clone();
            engine.recovery_binding = Some((stage.id.clone(), stage.revision));
            let result = engine
                .ask_scoped_once(ctx, &agent, cwd, "plan", prompt, true, None, &mut None)
                .await;
            stage = self
                .store
                .recovery_stages(&ctx.session.id)?
                .into_iter()
                .find(|s| s.id == stage.id)
                .context("Planning stage disappeared")?;
            match result {
                Ok(response) => {
                    let valid = parse_response::<Plan>(&response.text)
                        .and_then(|p| p.validate().map(|_| p));
                    if valid.is_ok() {
                        stage.response = Some(SavedResponse {
                            text: response.text,
                            assignment_id: response.assignment_id,
                            invocation_id: response.invocation_id,
                            agent_id: agent.id.clone(),
                        });
                        if !matches!(
                            stage.status,
                            RecoveryStatus::Paused | RecoveryStatus::Waiting
                        ) {
                            stage.status = RecoveryStatus::Complete;
                        }
                        self.save_stage(&mut stage, None)?;
                        ensure!(
                            stage.status == RecoveryStatus::Complete,
                            "Owner paused planning; admitted result retained"
                        );
                    } else {
                        let trace = self.store.trace(&ctx.session.id)?;
                        let access = trace
                            .decisions
                            .iter()
                            .find(|d| {
                                d.kind == "workspace_access_admitted"
                                    && d.links.invocation_id.as_ref()
                                        == Some(&response.invocation_id)
                            })
                            .and_then(|d| d.links.workspace_access.as_ref())
                            .context("Planning access evidence missing")?;
                        let failure = InvocationFailure {
                            class: FailureClass::MalformedResponse,
                            native_code: None,
                            assignment_id: response.assignment_id.clone(),
                            invocation_id: response.invocation_id.clone(),
                            agent_id: agent.id.clone(),
                            provider_id: agent.provider.clone(),
                            effective_access: access.effective_access.clone(),
                            termination: TerminationEvidence::BackendEnded,
                        };
                        self.store.record_decision(&DecisionRecord {
                            id: new_id(),
                            session_id: ctx.session.id.clone(),
                            kind: "malformed_plan".into(),
                            actor: Some(agent.id.clone()),
                            reason: "Completed planning invocation did not produce a valid plan"
                                .into(),
                            outcome: None,
                            links: RecordLinks {
                                failure: Some(failure.clone()),
                                assignment_id: Some(response.assignment_id),
                                invocation_id: Some(response.invocation_id),
                                ..Default::default()
                            },
                            created_at: now(),
                        })?;
                        stage.failures.push(failure);
                        stage.status = RecoveryStatus::Pending;
                        self.save_stage(&mut stage, None)?;
                    }
                }
                Err(error) => {
                    self.post(
                        &ctx.session.id,
                        "ymp",
                        "notice",
                        &format!("Planning invocation failed: {error:#}"),
                    )?;
                    stage.admission_denial = error.downcast_ref::<BudgetDenial>().cloned();
                    let trace = self.store.trace(&ctx.session.id)?;
                    if let Some(failure) = trace
                        .decisions
                        .iter()
                        .rev()
                        .filter_map(|d| d.links.failure.as_ref())
                        .find(|f| {
                            stage.active_invocation_id.as_ref() == Some(&f.invocation_id)
                                && !stage
                                    .failures
                                    .iter()
                                    .any(|s| s.invocation_id == f.invocation_id)
                        })
                    {
                        stage.failures.push(failure.clone());
                        stage.status = RecoveryStatus::Pending;
                        self.save_stage(&mut stage, None)?;
                    } else {
                        self.wait_stage(&mut stage, RecoveryStatus::Waiting, error.to_string())?;
                        return Err(error);
                    }
                    if stage.admission_denial.is_some() {
                        return Err(error);
                    }
                    if self.cancel.is_cancelled() {
                        self.wait_stage(
                            &mut stage,
                            RecoveryStatus::Paused,
                            "Cancelled; explicit planning continuation required",
                        )?;
                        return Err(error);
                    }
                }
            }
        }
    }
}
