use super::*;
use anyhow::ensure;

impl Engine {
    pub fn with_recovery_policy(mut self, policy: Arc<dyn crate::RecoveryPolicy>) -> Result<Self> {
        let identity = policy.identity();
        identity.validate()?;
        self.recovery_identity = identity;
        self.recovery_configuration = policy.configuration();
        self.recovery_policy = policy;
        Ok(self)
    }
    /// Durable backend model for the local UI. Reading never discovers native models or invokes them.
    pub fn recovery_stages(&self, session: &str) -> Result<Vec<RecoveryStage>> {
        self.store.recovery_stages(session)
    }
    pub fn control_recovery(
        &self,
        command: &RecoveryControlCommand,
    ) -> Result<RecoveryControlReceipt> {
        self.store.control_recovery(command)
    }

    pub(super) fn last_failure_is_read_only(
        &self,
        session: &str,
        agent: &str,
        purpose: &str,
    ) -> Result<bool> {
        let trace = self.store.trace(session)?;
        Ok(trace
            .decisions
            .iter()
            .rev()
            .filter_map(|d| d.links.failure.as_ref())
            .find(|f| {
                f.agent_id == agent
                    && trace
                        .assignments
                        .iter()
                        .any(|a| a.id == f.assignment_id && a.purpose == purpose)
            })
            .is_some_and(|f| {
                f.effective_access.is_read_only()
                    && f.termination == TerminationEvidence::BackendEnded
            }))
    }
    fn save_stage(
        &self,
        stage: &mut RecoveryStage,
        decision: Option<&DecisionRecord>,
    ) -> Result<()> {
        let expected = stage.revision;
        stage.revision += 1;
        stage.updated_at = now();
        self.store.transition_recovery(expected, stage, decision)
    }
    fn stage_for_plan(&self, session: &str, proposal: &PlanVersion) -> Result<RecoveryStage> {
        let id = format!("plan-review-{}-{}", proposal.proposal_id, proposal.revision);
        if let Some(stage) = self
            .store
            .recovery_stages(session)?
            .into_iter()
            .find(|s| s.id == id)
        {
            return Ok(stage);
        }
        let mut stage = RecoveryStage {
            schema_version: 1,
            session_id: session.into(),
            id,
            revision: 0,
            purpose: "review_plan".into(),
            task: None,
            plan: Some(proposal.clone()),
            result: None,
            review_ids: vec![],
            selected_agent: None,
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
        Ok(stage)
    }
    fn wait_stage(
        &self,
        stage: &mut RecoveryStage,
        status: RecoveryStatus,
        reason: impl Into<String>,
    ) -> Result<()> {
        stage.status = status;
        stage.condition = Some(reason.into());
        self.save_stage(stage, None)
    }

    async fn recover_stage(
        &self,
        ctx: &RunContext,
        stage: &mut RecoveryStage,
        peers: &[AgentProfile],
    ) -> Result<()> {
        let failure = stage
            .failures
            .last()
            .context("Recovery requires failure evidence")?
            .clone();
        let trace = self.store.trace(&ctx.session.id)?;
        let input = RecoveryInput {
            schema_version: 1,
            stage: stage.clone(),
            failure: failure.clone(),
            eligible: peers.to_vec(),
            outstanding_assignments: trace
                .assignments
                .iter()
                .filter(|a| a.state == InvocationState::Running)
                .map(|a| a.id.clone())
                .collect(),
            provider_failures: trace
                .decisions
                .iter()
                .filter_map(|d| d.links.failure.as_ref())
                .filter(|f| f.provider_id == failure.provider_id)
                .count(),
            team_revision: self.store.team_state(&ctx.session.id)?.map(|s| s.revision),
            owner_policy_revision: 0,
            budget: self.store.session_budget(&ctx.session.id)?,
        };
        let proposal = self.recovery_policy.propose(&input)?;
        // These are runtime ceilings, independent of an injected strategy's own bounds.
        let effect = matches!(
            proposal,
            RecoveryAction::Retry { .. } | RecoveryAction::ResumeReview | RecoveryAction::Reassign
        );
        let safe = failure.effective_access.is_read_only()
            && failure.termination == TerminationEvidence::BackendEnded;
        let within = stage.recovery_attempts < 8 && !self.cancel.is_cancelled();
        let accepted = !effect
            || (safe
                && within
                && match proposal {
                    RecoveryAction::Retry { delay_ms } => {
                        delay_ms <= 30_000 && input.provider_failures <= 8
                    }
                    _ => true,
                });
        let reason = if accepted {
            format!("Validated recovery proposal: {proposal:?}")
        } else {
            "Recovery proposal rejected: uncertain effects, cancellation or runtime recovery ceiling".into()
        };
        let record = DecisionRecord {
            id: new_id(),
            session_id: ctx.session.id.clone(),
            kind: "recovery_proposed".into(),
            actor: None,
            reason: reason.clone(),
            outcome: None,
            links: RecordLinks {
                recovery: Some(Box::new(RecoveryDecision {
                    policy: PolicyProvenance {
                        implementation: self.recovery_identity.clone(),
                        configuration: self.recovery_configuration.clone(),
                        originating_record_ids: vec![failure.invocation_id.clone()],
                    },
                    input,
                    proposal: proposal.clone(),
                    accepted,
                    reason: reason.clone(),
                })),
                ..Default::default()
            },
            created_at: now(),
        };
        if effect && accepted {
            stage.recovery_attempts += 1;
        }
        stage.status = if accepted && effect {
            RecoveryStatus::Pending
        } else {
            RecoveryStatus::Waiting
        };
        stage.condition = Some(reason.clone());
        self.save_stage(stage, Some(&record))?;
        ensure!(accepted, "{reason}");
        match proposal {
            RecoveryAction::Retry { delay_ms } => {
                tokio::select! {
                    _ = self.cancel.cancelled() => { self.wait_stage(stage, RecoveryStatus::Paused, "Cancelled during recovery delay")?; bail!("Cancelled"); },
                    _ = tokio::time::sleep(std::time::Duration::from_millis(delay_ms)) => {}
                }
            }
            RecoveryAction::ResumeReview => {}
            RecoveryAction::Reassign => {
                let failed_providers = stage
                    .failures
                    .iter()
                    .map(|f| &f.provider_id)
                    .collect::<HashSet<_>>();
                let candidates = peers
                    .iter()
                    .filter(|a| !failed_providers.contains(&a.provider))
                    .cloned()
                    .collect::<Vec<_>>();
                if candidates.is_empty() {
                    self.wait_stage(stage, RecoveryStatus::Waiting, "No eligible independent replacement on an unaffected provider; owner action or provider recovery required")?;
                    bail!("{}", stage.condition.as_deref().unwrap());
                }
                let mut engine = self.clone();
                engine.allocation_origin = Some(PolicyProvenance {
                    implementation: self.recovery_identity.clone(),
                    configuration: self.recovery_configuration.clone(),
                    originating_record_ids: vec![record.id.clone()],
                });
                let next = engine.choose(
                    ctx,
                    &candidates,
                    "verification",
                    "standard",
                    "recover pending review",
                    &stage.purpose,
                    stage.task.as_ref().map(|t| t.task_id.as_str()),
                );
                match next {
                    Ok(agent) => {
                        stage.selected_agent = Some(agent.id);
                        self.save_stage(stage, None)?;
                    }
                    Err(error) => {
                        self.wait_stage(stage, RecoveryStatus::Waiting, error.to_string())?;
                        return Err(error);
                    }
                }
            }
            RecoveryAction::InspectEffects => {
                self.wait_stage(stage, RecoveryStatus::OwnerAction, "Uncertain execution effects: establish termination and inspect actual effects before replay; no rollback is available")?;
                bail!("{}", stage.condition.as_deref().unwrap());
            }
            RecoveryAction::Wait { condition } => {
                self.wait_stage(stage, RecoveryStatus::Waiting, &condition)?;
                bail!("{condition}");
            }
            RecoveryAction::RequestOwner { reason } | RecoveryAction::Stop { reason } => {
                self.wait_stage(stage, RecoveryStatus::OwnerAction, &reason)?;
                bail!("{reason}");
            }
        }
        Ok(())
    }

    pub(super) async fn review_saved_plan(
        &self,
        ctx: &RunContext,
        prompt: &str,
        proposal: &PlanVersion,
        peers: &[AgentProfile],
    ) -> Result<(RecordedResponse, Review, String)> {
        let mut stage = self.stage_for_plan(&ctx.session.id, proposal)?;
        // A durable verdict (including objections) must be consumed before any new review.
        let trace = self.store.trace(&ctx.session.id)?;
        if let Some(review) = trace
            .decisions
            .iter()
            .find(|d| d.kind == "plan_review" && d.links.plan_proposal.as_ref() == Some(proposal))
        {
            let response = RecordedResponse {
                _access: None,
                text: String::new(),
                assignment_id: review
                    .links
                    .assignment_id
                    .clone()
                    .context("Saved review assignment missing")?,
                invocation_id: review
                    .links
                    .invocation_id
                    .clone()
                    .context("Saved review invocation missing")?,
            };
            return Ok((
                response,
                Review {
                    approved: matches!(review.outcome, Some(DecisionOutcome::Accepted { .. })),
                    reason: review.reason.clone(),
                    lesson: None,
                },
                review.id.clone(),
            ));
        }
        if let Some(id) = &stage.active_invocation_id {
            if stage.response.is_none() && !stage.failures.iter().any(|f| &f.invocation_id == id) {
                if let Some(failure) = trace
                    .decisions
                    .iter()
                    .filter_map(|d| d.links.failure.as_ref())
                    .find(|f| &f.invocation_id == id)
                {
                    stage.failures.push(failure.clone());
                } else {
                    let invocation = self.store.invocation(&ctx.session.id, id)?;
                    let assignment = trace
                        .assignments
                        .iter()
                        .find(|a| a.id == invocation.assignment_id)
                        .context("Interrupted review assignment missing")?;
                    let access = trace
                        .decisions
                        .iter()
                        .find(|d| {
                            d.kind == "workspace_access_admitted"
                                && d.links.invocation_id.as_ref() == Some(id)
                        })
                        .and_then(|d| d.links.workspace_access.as_ref());
                    let message = self
                        .store
                        .messages(&ctx.session.id, 0, 10000)?
                        .into_iter()
                        .find(|m| {
                            trace
                                .message_attribution(m)
                                .is_some_and(|o| &o.invocation_id == id)
                        });
                    if invocation.state == InvocationState::Completed && message.is_some() {
                        stage.response = Some(SavedResponse {
                            text: message.unwrap().text,
                            assignment_id: assignment.id.clone(),
                            invocation_id: id.clone(),
                            agent_id: assignment.agent_id.clone(),
                        });
                    } else {
                        stage.failures.push(InvocationFailure {
                            class: FailureClass::Unknown,
                            native_code: None,
                            assignment_id: assignment.id.clone(),
                            invocation_id: id.clone(),
                            agent_id: assignment.agent_id.clone(),
                            provider_id: assignment.provider_id.clone(),
                            effective_access: access
                                .map_or(WorkspaceAccess::WriteAll, |a| a.effective_access.clone()),
                            termination: TerminationEvidence::UnverifiedAfterRestart,
                        });
                    }
                }
                self.save_stage(&mut stage, None)?;
            }
        }
        if let Some(denial) = &stage.admission_denial {
            return Err(denial.clone().into());
        }
        ensure!(
            stage.status != RecoveryStatus::Paused,
            "Owner paused recovery; explicit continuation required"
        );
        if matches!(
            stage.status,
            RecoveryStatus::Waiting | RecoveryStatus::OwnerAction
        ) && !stage.manual_permit
        {
            bail!(
                "{}",
                stage
                    .condition
                    .as_deref()
                    .unwrap_or("Recovery waits for owner action")
            );
        }
        let review_prompt = format!("Review this proposed plan against the user's request. Check completeness, meaningful acceptance checks, dependencies, and unnecessary work.\nRequest: {prompt}\nPlan: {}\nReturn ONLY JSON {{\"approved\":true|false,\"reason\":\"specific justification\"}}.", serde_json::to_string(&proposal.plan)?);
        loop {
            ensure!(!self.cancel.is_cancelled(), "Cancelled");
            if stage.response.is_none() {
                if !stage.failures.is_empty() && !stage.manual_permit {
                    self.recover_stage(ctx, &mut stage, peers).await?;
                }
                if stage.selected_agent.is_none() {
                    match self.choose(
                        ctx,
                        peers,
                        "verification",
                        "standard",
                        "saved plan review",
                        "review_plan",
                        None,
                    ) {
                        Ok(agent) => stage.selected_agent = Some(agent.id),
                        Err(error) => {
                            self.wait_stage(
                                &mut stage,
                                RecoveryStatus::Waiting,
                                error.to_string(),
                            )?;
                            return Err(error);
                        }
                    }
                }
                let selected = stage
                    .selected_agent
                    .as_ref()
                    .context("No reviewer selected")?;
                let Some(agent) = peers.iter().find(|a| &a.id == selected) else {
                    self.wait_stage(
                        &mut stage,
                        RecoveryStatus::Waiting,
                        "Selected independent reviewer is unavailable; owner replacement required",
                    )?;
                    bail!("{}", stage.condition.as_deref().unwrap());
                };
                stage.status = RecoveryStatus::Running;
                stage.manual_permit = false;
                self.save_stage(&mut stage, None)?;
                // Exactly one attempt; all repeats pass the same recorded recovery consumer.
                let mut engine = self.clone();
                engine.recovery_binding = Some((stage.id.clone(), stage.revision));
                let response = engine
                    .ask_scoped_once(
                        ctx,
                        agent,
                        &ctx.workspace.directory,
                        "review_plan",
                        &review_prompt,
                        true,
                        None,
                        &mut None,
                    )
                    .await;
                stage = self
                    .store
                    .recovery_stages(&ctx.session.id)?
                    .into_iter()
                    .find(|s| s.id == stage.id)
                    .context("Stage disappeared")?;
                match response {
                    Ok(response) => {
                        stage.response = Some(SavedResponse {
                            text: response.text,
                            assignment_id: response.assignment_id,
                            invocation_id: response.invocation_id,
                            agent_id: agent.id.clone(),
                        });
                        self.save_stage(&mut stage, None)?;
                    }
                    Err(error) => {
                        stage.admission_denial = error.downcast_ref::<BudgetDenial>().cloned();
                        let trace = self.store.trace(&ctx.session.id)?;
                        let failure = trace
                            .decisions
                            .iter()
                            .rev()
                            .filter_map(|d| d.links.failure.as_ref())
                            .find(|f| {
                                stage.active_invocation_id.as_ref() == Some(&f.invocation_id)
                                    && f.agent_id == agent.id
                                    && !stage
                                        .failures
                                        .iter()
                                        .any(|old| old.invocation_id == f.invocation_id)
                            })
                            .cloned();
                        if let Some(failure) = failure {
                            stage.failures.push(failure);
                            stage.status = RecoveryStatus::Pending;
                            self.save_stage(&mut stage, None)?;
                        } else {
                            self.wait_stage(
                                &mut stage,
                                RecoveryStatus::Waiting,
                                error.to_string(),
                            )?;
                            return Err(error);
                        }
                        if self.cancel.is_cancelled() {
                            self.wait_stage(
                                &mut stage,
                                RecoveryStatus::Paused,
                                "Cancelled; explicit continuation required",
                            )?;
                            return Err(error);
                        }
                        continue;
                    }
                }
            }
            let saved = stage
                .response
                .as_ref()
                .context("Saved review response missing")?
                .clone();
            let review = parse_response::<Review>(&saved.text);
            match review {
                Ok(review) => {
                    let response = RecordedResponse {
                        _access: None,
                        text: saved.text,
                        assignment_id: saved.assignment_id,
                        invocation_id: saved.invocation_id,
                    };
                    let review_id = self.record_review(
                        ctx,
                        &saved.agent_id,
                        &response,
                        &review,
                        RecordLinks {
                            plan_proposal: Some(proposal.clone()),
                            ..Default::default()
                        },
                        "plan_review",
                    )?;
                    stage.review_ids.push(review_id.clone());
                    stage.status = RecoveryStatus::Complete;
                    stage.condition = None;
                    self.save_stage(&mut stage, None)?;
                    return Ok((response, review, review_id));
                }
                Err(_) => {
                    let trace = self.store.trace(&ctx.session.id)?;
                    let assignment = trace
                        .assignments
                        .iter()
                        .find(|a| a.id == saved.assignment_id)
                        .context("Malformed review assignment missing")?;
                    let access = trace
                        .decisions
                        .iter()
                        .find(|d| {
                            d.kind == "workspace_access_admitted"
                                && d.links.invocation_id.as_ref() == Some(&saved.invocation_id)
                        })
                        .and_then(|d| d.links.workspace_access.as_ref())
                        .context("Malformed review access missing")?;
                    let failure = InvocationFailure {
                        class: FailureClass::MalformedResponse,
                        native_code: None,
                        assignment_id: saved.assignment_id.clone(),
                        invocation_id: saved.invocation_id.clone(),
                        agent_id: saved.agent_id.clone(),
                        provider_id: assignment.provider_id.clone(),
                        effective_access: access.effective_access.clone(),
                        termination: TerminationEvidence::BackendEnded,
                    };
                    self.store.record_decision(&DecisionRecord { id:new_id(), session_id:ctx.session.id.clone(), kind:"malformed_review".into(), actor:Some(saved.agent_id), reason:"Completed invocation did not contain a valid review; no verdict or acceptance inferred".into(), outcome:None, links:RecordLinks { failure:Some(failure.clone()), assignment_id:Some(saved.assignment_id), invocation_id:Some(saved.invocation_id), plan_proposal:Some(proposal.clone()), ..Default::default() }, created_at:now() })?;
                    stage.failures.push(failure);
                    stage.response = None;
                    stage.status = RecoveryStatus::Pending;
                    self.save_stage(&mut stage, None)?;
                }
            }
        }
    }
}
