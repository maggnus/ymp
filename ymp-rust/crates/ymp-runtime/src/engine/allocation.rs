use super::*;
use anyhow::ensure;

impl Engine {
    pub(super) fn startup_budget(&self) -> SessionBudget {
        let mut limits = self.config.limits.clone();
        let resources = limits.resources.get_or_insert_with(ResourceLimits::default);
        let protected_review_invocations = resources.required_review_invocations;
        let reserved_tokens = resources.invocation_tokens.map(|_| 0);
        SessionBudget {
            limits,
            admitted_invocations: 0,
            in_flight_invocations: 0,
            startup_invocations: 0,
            protected_review_invocations,
            reserved_tokens,
            observed_usage: UsageTotals::default(),
            observed_token_overshoot: None,
            last_denial: None,
            strict_token_bound: false,
        }
    }

    pub fn with_allocation_policy(
        mut self,
        policy: Arc<dyn crate::AllocationPolicy>,
    ) -> Result<Self> {
        let identity = policy.identity();
        identity.validate()?;
        self.allocation_identity = identity;
        self.allocation_policy = policy;
        Ok(self)
    }
    pub fn with_resource_policy(
        mut self,
        policy: Arc<dyn crate::ResourceAllocationPolicy>,
    ) -> Result<Self> {
        let identity = policy.identity();
        identity.validate()?;
        self.resource_identity = identity;
        self.resource_policy = policy;
        Ok(self)
    }

    pub(super) fn team_constraints(&self, session: &str) -> Result<TeamConstraints> {
        Ok(self
            .store
            .session_policy(session)?
            .and_then(|p| p.team_constraints)
            .unwrap_or_else(|| self.config.team_constraints.clone()))
    }

    /// Metadata inspection only. Captured user restrictions survive config changes.
    pub fn eligible_agents(&self, session: &str) -> Result<Vec<AgentProfile>> {
        let constraints = self.team_constraints(session)?;
        let mut config = self.config.clone();
        config.team_constraints = constraints.clone();
        if let Some(policy) = self.store.session_policy(session)? {
            config.execution = policy.execution;
        }
        if let Ok(captured) = self.store.session(session) {
            for agent in &mut config.agents {
                if let Some(original) = captured.team.iter().find(|a| a.id == agent.id) {
                    let enabled = agent.enabled;
                    *agent = original.clone();
                    agent.enabled = enabled;
                }
            }
        }
        self.eligible_from(&config, &constraints)
    }

    pub fn refresh_team_eligibility(&self, session: &str) -> Result<Vec<AgentProfile>> {
        let eligible = self.eligible_agents(session)?;
        self.store.refresh_team_eligibility(
            session,
            &eligible.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
        )?;
        Ok(eligible)
    }

    pub(super) fn eligible_with(&self, constraints: &TeamConstraints) -> Result<Vec<AgentProfile>> {
        self.eligible_from(&self.config, constraints)
    }

    fn eligible_from(
        &self,
        config: &Config,
        constraints: &TeamConstraints,
    ) -> Result<Vec<AgentProfile>> {
        constraints.validate()?;
        self.validate_known_pins(config, constraints)?;
        let pool = ymp_providers::discovery::inspect_pool(config)?;
        let mut eligible = pool
            .eligible()
            .filter(|a| {
                constraints
                    .eligible_agents
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&a.profile.id))
            })
            .filter(|a| {
                constraints
                    .fixed_roster
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&a.profile.id))
            })
            .map(|a| a.profile.clone())
            .collect::<Vec<_>>();
        // A configured starting roster is a preference, never a hidden fixed pin.
        eligible.sort_by_key(|a| {
            self.config
                .team
                .iter()
                .position(|id| id == &a.id)
                .unwrap_or(usize::MAX)
        });
        Ok(eligible)
    }

    /// Only complete, applicable metadata can prove a pin contradictory. An
    /// unlisted model in an incomplete catalog or absent controls stay unknown.
    fn validate_known_pins(&self, config: &Config, constraints: &TeamConstraints) -> Result<()> {
        for agent in config.agents.iter().filter(|a| {
            a.enabled
                && config.provider(&a.provider).is_ok_and(|p| p.enabled)
                && constraints
                    .eligible_agents
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&a.id))
                && constraints
                    .fixed_roster
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&a.id))
        }) {
            let Some(policy) = config.execution.get(&agent.id) else {
                continue;
            };
            let Some(catalog) = config.capabilities.get(&agent.provider) else {
                continue;
            };
            let fixed = &policy.fixed;
            if let Some(model) = &fixed.model {
                ensure!(!catalog.models_complete || catalog.model(model).is_some(),
                    "unsupported_model: agent {} pins model {}, absent from the complete native catalog", agent.id, model);
            }
            let Some(effort) = &fixed.effort else {
                continue;
            };
            let models = if let Some(model) = &fixed.model {
                catalog.model(model).into_iter().collect::<Vec<_>>()
            } else if catalog.models_complete {
                catalog.models.iter().collect()
            } else {
                continue;
            };
            let possible = models.is_empty()
                || models.iter().any(|model| {
                    model.controls.as_ref().is_none_or(|controls| {
                        controls.iter().any(|control| {
                            ["effort", "thought_level"].contains(&control.id.as_str())
                                && control
                                    .values
                                    .contains(&NativeControlValue::Choice(effort.clone()))
                        })
                    })
                });
            ensure!(possible,
                "unsupported_effort: agent {} pins model {} and effort {}, unsupported by the applicable native model controls",
                agent.id, fixed.model.as_deref().unwrap_or("(adaptive model selection)"), effort);
        }
        Ok(())
    }

    fn validate_review_configuration(&self, input: &AllocationInput, reviewer: &str) -> Result<()> {
        let agent = input.eligible.iter().find(|a| a.id == reviewer).context(
            "no_independent_eligible_reviewer: reserved reviewer is not in the eligible pool",
        )?;
        let candidate_review = if input.demand.purpose == "plan" {
            "review_plan"
        } else {
            "review"
        };
        for purpose in [candidate_review, "final_review"] {
            let mut demand = input.demand.clone();
            demand.purpose = purpose.into();
            if purpose == "final_review" {
                demand.task_id = None;
            }
            if self
                .execution_options(&input.session_id, std::slice::from_ref(agent), &demand)?
                .is_empty()
            {
                let captured = self.store.session_policy(&input.session_id)?;
                let execution = captured
                    .as_ref()
                    .map(|p| &p.execution)
                    .unwrap_or(&self.config.execution);
                let rule = self.assignment_rule(
                    &input.session_id,
                    agent,
                    purpose,
                    demand.task_id.as_deref(),
                )?;
                let settings = execution
                    .get(&agent.id)
                    .cloned()
                    .unwrap_or_default()
                    .resolve(agent, rule.as_ref().unwrap_or(&ModelEffort::default()))?;
                self.validate_native_settings(agent, &settings)
                    .with_context(|| {
                        format!(
                            "Agent {} requires {purpose} with model {:?} and effort {:?}",
                            agent.id, settings.model, settings.effort
                        )
                    })?;
                bail!("no_independent_eligible_reviewer: agent {} has no usable {purpose} configuration under the captured settings", agent.id);
            }
        }
        Ok(())
    }

    pub(super) fn current_team(&self, session: &str) -> Result<Vec<AgentProfile>> {
        let captured = self.store.session(session)?;
        Ok(match self.store.team_state(session)? {
            Some(state) => captured
                .team
                .into_iter()
                .filter(|a| state.current_members.contains(&a.id))
                .collect(),
            None => captured.team,
        })
    }

    pub(super) fn demand(
        &self,
        session: &str,
        purpose: &str,
        task_id: Option<&str>,
        competence: &str,
        difficulty: &str,
    ) -> Result<AllocationDemand> {
        let tasks = self.store.tasks(session)?;
        let description = task_id
            .and_then(|id| tasks.iter().find(|t| t.id == id))
            .map(|t| format!("{} {}", t.title, t.description))
            .unwrap_or_else(|| {
                self.store
                    .session_policy(session)
                    .ok()
                    .flatten()
                    .map(|p| p.goal)
                    .unwrap_or_default()
            });
        let risk = task_risk(&description);
        Ok(AllocationDemand {
            purpose: purpose.into(),
            task_id: task_id.map(str::to_owned),
            competence: competence.into(),
            difficulty: difficulty.into(),
            risk,
            ready_work: if purpose == "plan" {
                1
            } else {
                tasks
                    .iter()
                    .filter(|t| {
                        t.state == TaskState::Ready
                            && t.dependencies.iter().all(|id| {
                                tasks
                                    .iter()
                                    .any(|d| &d.id == id && d.state == TaskState::Accepted)
                            })
                    })
                    .count()
                    .max(1)
            },
        })
    }

    pub(super) fn execution_options(
        &self,
        session: &str,
        eligible: &[AgentProfile],
        demand: &AllocationDemand,
    ) -> Result<Vec<ExecutionOption>> {
        let policy = self.store.session_policy(session)?;
        let configured = policy
            .as_ref()
            .map(|p| &p.execution)
            .unwrap_or(&self.config.execution);
        let mut options = Vec::new();
        for agent in eligible {
            let pins = configured.get(&agent.id).cloned().unwrap_or_default();
            let rule =
                self.assignment_rule(session, agent, &demand.purpose, demand.task_id.as_deref())?;
            let default = pins.resolve(agent, rule.as_ref().unwrap_or(&ModelEffort::default()))?;
            let mut choices = vec![ModelEffort {
                model: default.model,
                effort: default.effort,
            }];
            if rule.is_none() {
                if let Some(catalog) = self.config.capabilities.get(&agent.provider) {
                    for model in &catalog.models {
                        choices.push(ModelEffort {
                            model: Some(model.id.clone()),
                            effort: None,
                        });
                        for control in model
                            .controls
                            .iter()
                            .flatten()
                            .filter(|c| ["effort", "thought_level"].contains(&c.id.as_str()))
                        {
                            if let NativeControlValues::Choices { options } = &control.values {
                                for effort in options {
                                    choices.push(ModelEffort {
                                        model: Some(model.id.clone()),
                                        effort: Some(effort.clone()),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            let mut seen = HashSet::new();
            for choice in choices {
                let Ok(mut settings) = pins.resolve(agent, &choice) else {
                    continue;
                };
                if self.validate_native_settings(agent, &settings).is_err() {
                    continue;
                }
                settings.permission_mode = Some(
                    if demand.purpose == "execute" {
                        "write"
                    } else {
                        "read_only"
                    }
                    .into(),
                );
                let native = ModelEffort {
                    model: settings.model.clone(),
                    effort: settings.effort.clone(),
                };
                if !seen.insert(serde_json::to_string(&native)?) {
                    continue;
                }
                let key = self.backend_config_version(
                    agent,
                    self.config.provider(&agent.provider)?,
                    &settings,
                )?;
                let version = self
                    .store
                    .value(&format!("effective_execution:{key}"))?
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or(key);
                let experience = if self.adaptive {
                    self.store
                        .reputation(&version, &demand.competence, &demand.difficulty)?
                } else {
                    Reputation::default()
                };
                options.push(ExecutionOption {
                    agent_id: agent.id.clone(),
                    settings: native,
                    configuration_version: version,
                    experience,
                });
            }
        }
        Ok(options)
    }

    pub(super) fn validate_native_settings(
        &self,
        agent: &AgentProfile,
        settings: &ExecutionSettings,
    ) -> Result<()> {
        let Some(catalog) = self.config.capabilities.get(&agent.provider) else {
            return Ok(());
        };
        let model = settings.model.as_ref().or(catalog.default_model.as_ref());
        let advertised = model.and_then(|m| catalog.model(m));
        ensure!(
            !catalog.models_complete || advertised.is_some() || model.is_none(),
            "unsupported_model: model absent from complete capability catalog"
        );
        if let Some(effort) = &settings.effort {
            if let Some(controls) = advertised.and_then(|m| m.controls.as_ref()) {
                ensure!(
                    controls
                        .iter()
                        .any(|c| ["effort", "thought_level"].contains(&c.id.as_str())
                            && c.values
                                .contains(&NativeControlValue::Choice(effort.clone()))),
                    "unsupported_effort: native model does not advertise this effort"
                );
            }
        }
        Ok(())
    }

    pub(super) fn allocation_input(
        &self,
        session: &str,
        boundary: AllocationBoundary,
        demand: AllocationDemand,
        permitted: Option<&[String]>,
    ) -> Result<AllocationInput> {
        let policy = self.store.session_policy(session)?;
        let constraints = self.team_constraints(session)?;
        let eligible = self.refresh_team_eligibility(session)?;
        let mut candidates = self.execution_options(session, &eligible, &demand)?;
        if let Some(permitted) = permitted {
            candidates.retain(|c| permitted.contains(&c.agent_id));
        }
        let trace = self.store.trace(session)?;
        if demand.purpose == "review" {
            if let Some(task) = demand
                .task_id
                .as_ref()
                .and_then(|id| trace.tasks.iter().find(|t| &t.id == id))
            {
                candidates.retain(|c| task.assignee.as_ref() != Some(&c.agent_id));
            }
        }
        let mut producers = trace
            .assignments
            .iter()
            .filter(|a| a.purpose == "execute")
            .map(|a| a.agent_id.clone())
            .collect::<Vec<_>>();
        producers.sort();
        producers.dedup();
        let mut occupied_agent_ids = trace
            .assignments
            .iter()
            .filter(|a| a.state == InvocationState::Running)
            .map(|a| a.agent_id.clone())
            .chain(
                trace
                    .tasks
                    .iter()
                    .filter(|t| t.state == TaskState::Running)
                    .filter_map(|t| t.assignee.clone()),
            )
            .collect::<Vec<_>>();
        occupied_agent_ids.extend(
            self.store
                .board(session)?
                .tasks
                .into_iter()
                .filter(|t| demand.task_id.as_ref() != Some(&t.task.id))
                .filter_map(|t| t.commitment.map(|c| c.agent_id)),
        );
        occupied_agent_ids.sort();
        occupied_agent_ids.dedup();
        Ok(AllocationInput {
            occupied_agent_ids,
            session_id: session.into(),
            boundary,
            goal: policy.map(|p| p.goal).unwrap_or(trace.session.title),
            constraints,
            current: self.store.team_state(session)?,
            eligible,
            candidates,
            demand,
            budget: self.store.session_budget(session)?,
            producer_ids: producers,
            evidence: trace
                .decisions
                .iter()
                .rev()
                .filter(|d| d.outcome.is_some() || d.links.check.is_some())
                .take(12)
                .map(|d| AllocationEvidence {
                    record_id: d.id.clone(),
                    kind: d.kind.clone(),
                    reason: d.reason.clone(),
                })
                .chain(
                    trace
                        .invocations
                        .iter()
                        .rev()
                        .filter(|i| {
                            matches!(
                                i.state,
                                InvocationState::Failed | InvocationState::Interrupted
                            )
                        })
                        .take(4)
                        .map(|i| AllocationEvidence {
                            record_id: i.id.clone(),
                            kind: "invocation_failure".into(),
                            reason: i.terminal_reason.clone().unwrap_or_else(|| {
                                "Unknown failure cause; diagnose before retry".into()
                            }),
                        }),
                )
                .collect(),
            suggestions: self
                .store
                .messages(session, 0, 10000)?
                .into_iter()
                .filter(|m| m.kind == "proposal")
                .rev()
                .take(8)
                .map(|m| AllocationSuggestion {
                    message_seq: m.seq,
                    agent_id: m.author,
                    text: m.text.chars().take(2000).collect(),
                })
                .collect(),
        })
    }

    pub(super) fn validate_allocation(
        &self,
        input: &AllocationInput,
        proposal: &AllocationProposal,
    ) -> Result<()> {
        input.constraints.validate()?;
        ensure!(input.eligible.len() >= 2, "no_independent_eligible_reviewer: at least two eligible identities are required for independent review");
        let ids = &proposal.members;
        ensure!(
            input.occupied_agent_ids.iter().all(|id| ids.contains(id)),
            "active_responsibility: proposal cannot remove an active or committed participant"
        );
        let unique: HashSet<_> = ids.iter().collect();
        ensure!(
            !ids.is_empty() && unique.len() == ids.len(),
            "team_members: members must be nonempty and unique"
        );
        ensure!(
            ids.len() <= input.constraints.max_members,
            "membership_ceiling: proposal exceeds max_members"
        );
        ensure!(
            input.constraints.fixed_size.is_none_or(|n| n == ids.len()),
            "fixed_size: proposed membership changes pinned size"
        );
        ensure!(
            input
                .constraints
                .fixed_roster
                .as_ref()
                .is_none_or(|r| r.len() == ids.len() && r.iter().all(|id| ids.contains(id))),
            "fixed_roster: proposed membership changes pinned identities"
        );
        ensure!(
            ids.iter()
                .all(|id| input.eligible.iter().any(|a| &a.id == id)),
            "ineligible_member: proposal includes an unavailable or excluded participant"
        );
        ensure!(
            !proposal.method.trim().is_empty() && !proposal.rationale.trim().is_empty(),
            "allocation_reason: method and rationale are required"
        );
        let reviewer = proposal
            .reserved_final_reviewer
            .as_ref()
            .context("no_independent_eligible_reviewer: preserve a final reviewer")?;
        ensure!(
            input.eligible.iter().any(|a| &a.id == reviewer)
                && !input.producer_ids.contains(reviewer),
            "no_independent_eligible_reviewer: final reviewer must remain eligible and independent"
        );
        self.validate_review_configuration(input, reviewer)?;
        if let Some(choice) = &proposal.executor {
            ensure!(
                ids.contains(&choice.agent_id),
                "executor_membership: selected executor is not a current member"
            );
            ensure!(input.candidates.iter().any(|c| c.agent_id == choice.agent_id && c.settings == choice.settings), "invalid_execution_choice: agent/settings violate candidates, pins, or capabilities");
            ensure!(input.demand.purpose != "execute" || &choice.agent_id != reviewer, "no_independent_eligible_reviewer: execution would consume the reserved final reviewer");
            ensure!(
                input.demand.purpose != "final_review"
                    || !input.producer_ids.contains(&choice.agent_id),
                "self_review: final reviewer produced the result"
            );
        } else {
            ensure!(
                input.demand.ready_work == 0,
                "missing_executor: useful ready work needs an executor"
            );
        }
        Ok(())
    }

    pub(super) fn record_allocation(
        &self,
        input: AllocationInput,
        proposal: AllocationProposal,
    ) -> Result<AllocationProposal> {
        let validation = self.validate_allocation(&input, &proposal);
        let reason = validation
            .as_ref()
            .map(|_| proposal.rationale.clone())
            .unwrap_or_else(|e| e.to_string());
        let mut record = DecisionRecord {
            id: new_id(),
            session_id: input.session_id.clone(),
            kind: if validation.is_ok() {
                "allocation_committed"
            } else {
                "allocation_rejected"
            }
            .into(),
            actor: None,
            reason: reason.clone(),
            outcome: None,
            links: RecordLinks {
                allocation: Some(Box::new(AllocationDecision {
                    implementation: self.allocation_identity.clone(),
                    input,
                    proposal: proposal.clone(),
                    accepted: validation.is_ok(),
                    reason,
                })),
                ..Default::default()
            },
            created_at: now(),
        };
        if validation.is_ok() {
            if let Err(error) = self.store.commit_allocation(&record) {
                record.kind = "allocation_rejected".into();
                record.reason = error.to_string();
                let allocation = record.links.allocation.as_mut().expect("allocation record");
                allocation.accepted = false;
                allocation.reason = error.to_string();
                self.store.record_decision(&record)?;
                return Err(error);
            }
        } else {
            self.store.record_decision(&record)?;
        }
        validation?;
        Ok(proposal)
    }

    /// Reconsider only at a meaningful work/evidence boundary. This never invokes a model.
    /// The returned proposal has already passed runtime validation and durable commitment.
    pub fn reconsider_allocation(
        &self,
        session: &str,
        boundary: AllocationBoundary,
        demand: AllocationDemand,
    ) -> Result<AllocationProposal> {
        self.allocate(session, boundary, demand, None)
    }

    pub(super) fn allocate(
        &self,
        session: &str,
        boundary: AllocationBoundary,
        demand: AllocationDemand,
        permitted: Option<&[String]>,
    ) -> Result<AllocationProposal> {
        let input = self.allocation_input(session, boundary, demand, permitted)?;
        let proposal = match self.allocation_policy.propose(&input) {
            Ok(proposal) => proposal,
            Err(error) => {
                self.store.record_decision(&DecisionRecord {
                    id: new_id(),
                    session_id: session.into(),
                    kind: "allocation_unavailable".into(),
                    actor: None,
                    reason: error.to_string(),
                    outcome: None,
                    links: RecordLinks {
                        allocation: Some(Box::new(AllocationDecision {
                            implementation: self.allocation_identity.clone(),
                            input,
                            proposal: AllocationProposal {
                                members: vec![],
                                executor: None,
                                reserved_final_reviewer: None,
                                method: "unavailable".into(),
                                rationale: error.to_string(),
                            },
                            accepted: false,
                            reason: error.to_string(),
                        })),
                        ..Default::default()
                    },
                    created_at: now(),
                })?;
                return Err(error);
            }
        };
        self.record_allocation(input, proposal)
    }

    pub(super) fn resource_allowance(
        &self,
        ctx: &RunContext,
        purpose: &str,
        task: Option<&TaskAttemptRef>,
        agent: &AgentProfile,
        requested: &ExecutionSettings,
    ) -> Result<InvocationAllowance> {
        let task_id = task.map(|t| t.task_id.as_str());
        let tasks = self.store.tasks(&ctx.session.id)?;
        let difficulty = task_id
            .and_then(|id| tasks.iter().find(|t| t.id == id))
            .map(|t| t.difficulty.as_str())
            .unwrap_or("standard");
        self.store
            .capture_legacy_budget_limits(&ctx.session.id, &ctx.limits)?;
        let input = ResourceAllocationInput {
            agent_id: agent.id.clone(),
            requested: requested.clone(),
            demand: self.demand(&ctx.session.id, purpose, task_id, purpose, difficulty)?,
            budget: self
                .store
                .session_budget(&ctx.session.id)?
                .context("Missing captured budget")?,
        };
        let proposal = self.resource_policy.propose(&input)?;
        let limits = &input.budget.limits;
        let defaults = ResourceLimits::default();
        let resources = limits.resources.as_ref().unwrap_or(&defaults);
        let accepted = proposal.timeout_secs > 0
            && proposal.timeout_secs <= limits.turn_timeout_secs
            && proposal.native_max_turns > 0
            && proposal.native_max_turns <= resources.native_max_turns
            && proposal.max_output_chars > 0
            && proposal.max_output_chars <= resources.max_output_chars
            && !proposal.rationale.trim().is_empty();
        let reason = if accepted {
            proposal.rationale.clone()
        } else {
            "resource_ceiling: allocation exceeds captured invocation controls".into()
        };
        self.store.record_decision(&DecisionRecord {
            id: new_id(),
            session_id: ctx.session.id.clone(),
            kind: if accepted {
                "resource_allocation_committed"
            } else {
                "resource_allocation_rejected"
            }
            .into(),
            actor: None,
            reason: reason.clone(),
            outcome: None,
            links: RecordLinks {
                task: task.cloned(),
                resource_allocation: Some(Box::new(ResourceAllocationDecision {
                    implementation: self.resource_identity.clone(),
                    input,
                    proposal: proposal.clone(),
                    accepted,
                    reason: reason.clone(),
                })),
                ..Default::default()
            },
            created_at: now(),
        })?;
        ensure!(accepted, "{reason}");
        Ok(proposal)
    }
}

pub(super) fn task_risk(text: &str) -> TaskRisk {
    let lower = text.to_lowercase();
    if [
        "security",
        "credential",
        "payment",
        "migration",
        "delete",
        "publish",
        "production",
    ]
    .iter()
    .any(|s| lower.contains(s))
    {
        TaskRisk::Elevated
    } else {
        TaskRisk::Standard
    }
}
