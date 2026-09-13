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
    pub(super) fn save_stage(
        &self,
        stage: &mut RecoveryStage,
        decision: Option<&DecisionRecord>,
    ) -> Result<()> {
        let expected = stage.revision;
        stage.revision += 1;
        stage.updated_at = now();
        self.store.transition_recovery(expected, stage, decision)
    }
    fn stage_for_review(
        &self,
        session: &str,
        purpose: &str,
        kind: &str,
        links: &RecordLinks,
    ) -> Result<RecoveryStage> {
        let id = if let Some(proposal) = &links.plan_proposal {
            format!("plan-review-{}-{}", proposal.proposal_id, proposal.revision)
        } else {
            format!(
                "{kind}-{}",
                content_digest(&serde_json::to_string(&(&links.result, &links.task))?)
            )
        };
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
            purpose: purpose.into(),
            request_digest: None,
            task: links.task.clone(),
            plan: links.plan_proposal.clone(),
            result: links.result.clone(),
            review_ids: links.review_ids.clone(),
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
        if purpose == "review_plan" {
            let trace = self.store.trace(session)?;
            let bound = self
                .store
                .recovery_stages(session)?
                .into_iter()
                .flat_map(|s| {
                    s.active_invocation_id
                        .into_iter()
                        .chain(s.response.map(|r| r.invocation_id))
                        .chain(s.failures.into_iter().map(|f| f.invocation_id))
                })
                .collect::<HashSet<_>>();
            for invocation in &trace.invocations {
                let Some(assignment) = trace
                    .assignments
                    .iter()
                    .find(|a| a.id == invocation.assignment_id && a.purpose == "review_plan")
                else {
                    continue;
                };
                if bound.contains(&invocation.id)
                    || trace.decisions.iter().any(|d| {
                        d.kind == "plan_review"
                            && d.links.invocation_id.as_ref() == Some(&invocation.id)
                    })
                {
                    continue;
                }
                // Legacy calls lack an exact stage binding. Keep their uncertainty;
                // neither an absent stage nor an unbound completed message is a verdict.
                let access = trace
                    .decisions
                    .iter()
                    .find(|d| {
                        d.kind == "workspace_access_admitted"
                            && d.links.invocation_id.as_ref() == Some(&invocation.id)
                    })
                    .and_then(|d| d.links.workspace_access.as_ref());
                stage.failures.push(InvocationFailure {
                    class: FailureClass::Unknown,
                    native_code: None,
                    assignment_id: assignment.id.clone(),
                    invocation_id: invocation.id.clone(),
                    agent_id: assignment.agent_id.clone(),
                    provider_id: assignment.provider_id.clone(),
                    effective_access: access
                        .map_or(WorkspaceAccess::WriteAll, |a| a.effective_access.clone()),
                    termination: if matches!(
                        invocation.state,
                        InvocationState::Failed | InvocationState::Cancelled
                    ) && invocation.ended_at.is_some()
                    {
                        TerminationEvidence::BackendEnded
                    } else {
                        TerminationEvidence::UnverifiedAfterRestart
                    },
                });
            }
            if !stage.failures.is_empty() {
                stage.status = RecoveryStatus::OwnerAction;
                stage.condition=Some("Unbound legacy plan review: inspect prior invocation effects and evidence before issuing a new review; no completion or acceptance was inferred".into());
            }
        }
        self.save_stage(&mut stage, None)?;
        Ok(stage)
    }
    pub(super) fn wait_stage(
        &self,
        stage: &mut RecoveryStage,
        status: RecoveryStatus,
        reason: impl Into<String>,
    ) -> Result<()> {
        stage.status = status;
        stage.condition = Some(reason.into());
        self.save_stage(stage, None)
    }

    pub(super) async fn recover_stage(
        &self,
        ctx: &RunContext,
        stage: &mut RecoveryStage,
        peers: &[AgentProfile],
    ) -> Result<()> {
        let failure = stage.failures.last().cloned();
        let unavailable = stage
            .selected_agent
            .as_ref()
            .filter(|id| !peers.iter().any(|a| &a.id == *id))
            .cloned();
        let trace = self.store.trace(&ctx.session.id)?;
        let input = RecoveryInput {
            schema_version: 1,
            stage: stage.clone(),
            failure: failure.clone(),
            unavailable_agent: unavailable.clone(),
            eligible: peers.to_vec(),
            responsibilities: Some(self.store.active_responsibilities(&ctx.session.id)?),
            outstanding_assignments: trace
                .assignments
                .iter()
                .filter(|a| a.state == InvocationState::Running)
                .map(|a| a.id.clone())
                .collect(),
            provider_failures: trace
                .invocations
                .iter()
                .filter(|i| {
                    matches!(
                        i.state,
                        InvocationState::Failed | InvocationState::Interrupted
                    ) && failure.as_ref().is_some_and(|f| {
                        trace
                            .assignments
                            .iter()
                            .any(|a| a.id == i.assignment_id && a.provider_id == f.provider_id)
                    })
                })
                .map(|i| i.id.clone())
                .chain(
                    trace
                        .decisions
                        .iter()
                        .filter_map(|d| d.links.failure.as_ref())
                        .filter(|f| {
                            failure
                                .as_ref()
                                .is_some_and(|current| current.provider_id == f.provider_id)
                        })
                        .map(|f| f.invocation_id.clone()),
                )
                .collect::<HashSet<_>>()
                .len(),
            team_revision: self.store.team_state(&ctx.session.id)?.map(|s| s.revision),
            owner_policy_revision: self
                .store
                .owner_team_state(&ctx.session.id)?
                .map_or(0, |s| s.policy_revision),
            budget: self.store.session_budget(&ctx.session.id)?,
        };
        let proposal = self.recovery_policy.propose(&input)?;
        // These are runtime ceilings, independent of an injected strategy's own bounds.
        let effect = matches!(
            proposal,
            RecoveryAction::Retry { .. } | RecoveryAction::ResumeReview | RecoveryAction::Reassign
        );
        let safe = failure
            .as_ref()
            .map_or(stage.active_invocation_id.is_none(), |f| {
                f.effective_access.is_read_only()
                    && f.termination == TerminationEvidence::BackendEnded
            });
        let within = stage.recovery_attempts < 8 && !self.cancel.is_cancelled();
        let accepted = !effect
            || (safe
                && within
                && match proposal {
                    RecoveryAction::Retry { delay_ms } => {
                        delay_ms <= 30_000 && input.provider_failures < 8
                    }
                    RecoveryAction::ResumeReview => input.provider_failures < 8,
                    _ => true,
                });
        let accepted = accepted
            && match &proposal {
                RecoveryAction::Wait { condition } => {
                    !condition.trim().is_empty() && condition.len() <= 4096
                }
                RecoveryAction::RequestOwner { reason } | RecoveryAction::Stop { reason } => {
                    !reason.trim().is_empty() && reason.len() <= 4096
                }
                _ => true,
            };
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
                        originating_record_ids: failure
                            .iter()
                            .map(|f| f.invocation_id.clone())
                            .collect(),
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
        let review_prompt = format!("Review this proposed plan against the user's request. Check completeness, meaningful acceptance checks, dependencies, and unnecessary work.\nRequest: {prompt}\nPlan: {}\nReturn ONLY JSON {{\"approved\":true|false,\"reason\":\"specific justification\"}}.", serde_json::to_string(&proposal.plan)?);
        let (_, response, review, id) = self
            .review_recovering(
                ctx,
                &review_prompt,
                RecordLinks {
                    plan_proposal: Some(proposal.clone()),
                    ..Default::default()
                },
                "plan_review",
                "review_plan",
                peers,
            )
            .await?;
        Ok((response, review, id))
    }

    pub(super) async fn review_recovering(
        &self,
        ctx: &RunContext,
        review_prompt: &str,
        links: RecordLinks,
        kind: &str,
        purpose: &str,
        peers: &[AgentProfile],
    ) -> Result<(String, RecordedResponse, Review, String)> {
        let mut stage = self.stage_for_review(&ctx.session.id, purpose, kind, &links)?;
        if let Some(denial) = &stage.admission_denial {
            return Err(denial.clone().into());
        }
        ensure!(
            stage.status != RecoveryStatus::Paused,
            "Owner paused recovery; explicit continuation required"
        );
        if !stage.manual_permit
            && matches!(
                stage.status,
                RecoveryStatus::Waiting | RecoveryStatus::OwnerAction
            )
        {
            bail!(
                "{}",
                stage
                    .condition
                    .as_deref()
                    .unwrap_or("Recovery requires owner action")
            );
        }
        // A durable verdict (including objections) must be consumed before any new review.
        let trace = self.store.trace(&ctx.session.id)?;
        if let Some(review) = trace.decisions.iter().find(|d| {
            d.kind == kind
                && d.links.plan_proposal == links.plan_proposal
                && d.links.result == links.result
                && d.links.task == links.task
        }) {
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
            if stage.status != RecoveryStatus::Complete {
                stage.status = RecoveryStatus::Complete;
                stage.condition = None;
                if !stage.review_ids.contains(&review.id) {
                    stage.review_ids.push(review.id.clone());
                }
                self.save_stage(&mut stage, None)?;
            }
            return Ok((
                review.actor.clone().context("Saved review actor missing")?,
                response,
                Review {
                    approved: matches!(review.outcome, Some(DecisionOutcome::Accepted { .. })),
                    reason: review.reason.clone(),
                    lesson: None,
                },
                review.id.clone(),
            ));
        }
        self.restore_stage_response(ctx, &mut stage)?;
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

        loop {
            self.owner_boundary(&ctx.session.id)?;
            ensure!(!self.cancel.is_cancelled(), "Cancelled");
            let current_trace = self.store.trace(&ctx.session.id)?;
            let mut producer_ids = links
                .result
                .as_ref()
                .map(|r| r.producer_assignment_ids.clone())
                .unwrap_or_default();
            if let Some(plan) = &links.plan_proposal {
                producer_ids.push(plan.producer_assignment_id.clone());
            }
            let producers = current_trace
                .assignments
                .iter()
                .filter(|a| producer_ids.contains(&a.id))
                .map(|a| &a.agent_id)
                .collect::<HashSet<_>>();
            let fresh_peers = self
                .eligible_agents(&ctx.session.id)?
                .into_iter()
                .filter(|a| {
                    !producers.contains(&a.id)
                        && (kind != "candidate_arbitration" || peers.iter().any(|p| p.id == a.id))
                })
                .collect::<Vec<_>>();
            let peers = fresh_peers.as_slice();
            if stage.response.is_none() {
                if (!stage.failures.is_empty()
                    || stage
                        .selected_agent
                        .as_ref()
                        .is_some_and(|id| !peers.iter().any(|a| &a.id == id)))
                    && !stage.manual_permit
                {
                    self.recover_stage(ctx, &mut stage, peers).await?;
                }
                if stage.selected_agent.is_none() {
                    match self.choose(
                        ctx,
                        peers,
                        "verification",
                        "standard",
                        "saved result review",
                        purpose,
                        links.task.as_ref().map(|t| t.task_id.as_str()),
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
                let mut requested = stage
                    .active_invocation_id
                    .as_ref()
                    .and_then(|id| self.store.invocation(&ctx.session.id, id).ok())
                    .filter(|i| {
                        current_trace
                            .assignments
                            .iter()
                            .any(|a| a.id == i.assignment_id && a.agent_id == agent.id)
                    })
                    .map(|i| i.requested);
                let response = engine
                    .ask_scoped_once(
                        ctx,
                        agent,
                        &ctx.workspace.directory,
                        purpose,
                        review_prompt,
                        true,
                        links.task.clone(),
                        &mut requested,
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
                        ensure!(
                            !matches!(
                                stage.status,
                                RecoveryStatus::Paused | RecoveryStatus::Waiting
                            ),
                            "Owner paused recovery; admitted response retained"
                        );
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
                        if stage.admission_denial.is_some() {
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
                        links.clone(),
                        kind,
                    )?;
                    stage.review_ids.push(review_id.clone());
                    stage.status = RecoveryStatus::Complete;
                    stage.condition = None;
                    self.save_stage(&mut stage, None)?;
                    return Ok((saved.agent_id, response, review, review_id));
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
                    self.store.record_decision(&DecisionRecord { id:new_id(), session_id:ctx.session.id.clone(), kind:"malformed_review".into(), actor:Some(saved.agent_id), reason:"Completed invocation did not contain a valid review; no verdict or acceptance inferred".into(), outcome:None, links:RecordLinks { failure:Some(failure.clone()), assignment_id:Some(saved.assignment_id), invocation_id:Some(saved.invocation_id), ..links.clone() }, created_at:now() })?;
                    stage.failures.push(failure);
                    stage.response = None;
                    stage.status = RecoveryStatus::Pending;
                    self.save_stage(&mut stage, None)?;
                }
            }
        }
    }
    pub(super) fn restore_stage_response(
        &self,
        ctx: &RunContext,
        stage: &mut RecoveryStage,
    ) -> Result<()> {
        let trace = self.store.trace(&ctx.session.id)?;
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
                    if let Some(message) =
                        message.filter(|_| invocation.state == InvocationState::Completed)
                    {
                        stage.response = Some(SavedResponse {
                            text: message.text,
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
                self.save_stage(stage, None)?;
            }
        }
        Ok(())
    }
}
