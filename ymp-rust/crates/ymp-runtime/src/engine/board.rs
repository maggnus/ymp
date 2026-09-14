use super::*;
use anyhow::ensure;

impl Engine {
    pub fn with_board_proposal_policy(
        mut self,
        policy: Arc<dyn crate::BoardProposalPolicy>,
    ) -> Result<Self> {
        let identity = policy.identity();
        identity.validate()?;
        self.board_identity = identity;
        self.board_policy = policy;
        Ok(self)
    }
    pub fn board(&self, session: &str) -> Result<BoardSnapshot> {
        self.store.board(session)
    }

    async fn observe_ordinary_check_set(
        &self,
        ctx: &RunContext,
        task: &Task,
        commands: &[String],
    ) -> Result<Vec<BoardCheckRunEvidence>> {
        let mut runs = Vec::with_capacity(commands.len());
        for command in commands {
            let (output, passed) = match self
                .checks(
                    ctx,
                    &ctx.workspace.directory,
                    std::slice::from_ref(command),
                    Some(TaskAttemptRef::from(task)),
                )
                .await
            {
                Ok(log) => (log, true),
                Err(error) => (format!("{error:#}\n"), false),
            };
            self.owner_boundary(&ctx.session.id)?;
            ensure!(!self.cancel.is_cancelled(), "Check cancelled");
            let mut output = output.chars().take(8_000).collect::<String>();
            if output.is_empty() {
                output.push_str("No command output was recorded.");
            }
            runs.push(BoardCheckRunEvidence {
                command: command.clone(),
                output,
                passed,
            });
        }
        Ok(runs)
    }

    /// Review one current executor proposal before the candidate's ordinary
    /// acceptance review can consume the final task attempt.
    pub(super) async fn review_task_check_replacement(
        &self,
        ctx: &RunContext,
        task: &mut Task,
        original_prompt: &str,
    ) -> Result<bool> {
        let board = self.store.board(&ctx.session.id)?;
        let selected = self.board_policy.propose(&board)?;
        let mut seen = HashSet::new();
        let proposal = selected
            .into_iter()
            .map(|id| {
                ensure!(
                    seen.insert(id.clone()),
                    "duplicate_proposal: strategy repeated a proposal"
                );
                board
                    .proposals
                    .iter()
                    .find(|proposal| {
                        proposal.id == id && proposal.status == BoardProposalStatus::Pending
                    })
                    .cloned()
                    .context("unknown_proposal: strategy must select a pending, bound proposal")
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .find(|proposal| {
                proposal
                    .change
                    .definition_task()
                    .is_some_and(|reference| reference.task_id == task.id)
            });
        let Some(proposal) = proposal else {
            return Ok(false);
        };
        let BoardChange::ReplaceChecks {
            replacements,
            task: reference,
        } = &proposal.change
        else {
            unreachable!("definition-bound changes are check replacements")
        };
        ensure!(
            task.state == TaskState::Review
                && board_task_definition_version(task)? == reference.definition_version,
            "stale_definition: task definition changed before check replacement review"
        );
        let proposed_checks = replace_ordinary_checks(&task.checks, replacements)?;
        let retained_runs = self
            .observe_ordinary_check_set(ctx, task, &task.checks)
            .await?;
        let proposed_runs = self
            .observe_ordinary_check_set(ctx, task, &proposed_checks)
            .await?;
        let evidence = BoardCheckRevisionEvidence {
            proposal: proposal.clone(),
            retained_checks: task.checks.clone(),
            proposed_checks,
            retained_runs,
            proposed_runs,
        };
        let failed_sources = replacements.iter().all(|replacement| {
            evidence
                .retained_runs
                .iter()
                .any(|run| run.command == replacement.old && !run.passed)
        });
        let proposed_passed = evidence.proposed_runs.iter().all(|run| run.passed);
        if !failed_sources || !proposed_passed {
            let reason = if !failed_sources {
                "check_replacement_evidence: every replaced command must reproduce its reported failure"
            } else {
                "check_replacement_evidence: proposed checks did not pass"
            };
            let decision = BoardDecision {
                implementation: self.board_identity.clone(),
                proposal: proposal.clone(),
                accepted: false,
                reason: reason.into(),
                review_ids: vec![],
                commitment: None,
                resulting_plan_version: board.plan_version,
            };
            self.store.commit_board(
                &DecisionRecord {
                    id: new_id(),
                    session_id: ctx.session.id.clone(),
                    kind: "board_rejected".into(),
                    actor: None,
                    reason: reason.into(),
                    outcome: None,
                    links: RecordLinks {
                        task: Some(TaskAttemptRef::from(&*task)),
                        board: Some(Box::new(decision)),
                        check_revision: Some(Box::new(CheckRevisionProvenance::Review(Box::new(
                            evidence,
                        )))),
                        ..Default::default()
                    },
                    created_at: now(),
                },
                &[],
                None,
                &[],
            )?;
            return Ok(false);
        }
        let result = self.submitted_result(ctx, task)?;
        ensure!(
            result
                .artifacts
                .iter()
                .all(|artifact| artifact.current(&ctx.workspace.directory)),
            "Candidate artifact changed while check replacement evidence was collected"
        );
        let requirements = self.acceptance_requirements(&ctx.session.id)?;
        let review_prompt = format!(
            "Independently review a proposed replacement of runtime-generated ordinary task checks. Do not modify files. Do not remove or weaken a genuine requirement, and do not treat this review as task acceptance or confirmation. Judge whether each proposed command can fail when the covered task requirement is violated; command similarity, a zero exit status, or a nonempty output alone is not evidence of discriminating coverage.\nOriginal request: {original_prompt}\nTask: {}\n{}\nCaptured trusted requirements remain immutable: {requirements}\nExecutor rationale: {}\nExact replacements: {}\nRetained command runs:\n{}\nProposed command runs:\n{}\nReturn ONLY JSON {{\"approved\":true|false,\"reason\":\"specific evidence that the new commands preserve the acceptance meaning, can detect a violation, and fix the check cause\"}}.",
            task.title,
            task.description,
            proposal.rationale,
            serde_json::to_string(replacements)?,
            serde_json::to_string(&evidence.retained_runs)?,
            serde_json::to_string(&evidence.proposed_runs)?,
        );
        let peers = self
            .eligible_agents(&ctx.session.id)?
            .into_iter()
            .filter(|agent| agent.id != proposal.agent_id)
            .collect::<Vec<_>>();
        let (_, _, review, _) = self
            .review_recovering(
                ctx,
                &review_prompt,
                RecordLinks {
                    task: Some(TaskAttemptRef::from(&*task)),
                    result: Some(result.clone()),
                    check_revision: Some(Box::new(CheckRevisionProvenance::Review(Box::new(
                        evidence,
                    )))),
                    ..Default::default()
                },
                "board_check_revision_review",
                "review_check_revision",
                &peers,
            )
            .await?;
        let decisions = self.commit_board_proposals(&ctx.session.id)?;
        let decision = decisions
            .iter()
            .find(|decision| decision.proposal.id == proposal.id)
            .context("Check replacement proposal was not decided after review")?;
        if decision.accepted {
            let attempt_before = task.attempts;
            *task = self
                .store
                .tasks(&ctx.session.id)?
                .into_iter()
                .find(|current| current.id == task.id)
                .context("Revised task disappeared")?;
            ensure!(
                task.state == TaskState::Review && task.attempts == attempt_before,
                "check_replacement_attempt: replacement changed the saved execution attempt"
            );
            let _ = self.events.send(UiEvent::Task(task.clone()));
            return Ok(false);
        }
        ensure!(
            !review.approved,
            "Approved check replacement was rejected during commitment"
        );
        Ok(false)
    }

    /// Consume bound proposals only after active native work ends. Each proposal
    /// receives a durable decision; a rejected strategy output never mutates work.
    pub fn commit_board_proposals(&self, session: &str) -> Result<Vec<BoardDecision>> {
        let trace = self.store.trace(session)?;
        if trace
            .assignments
            .iter()
            .any(|a| a.state == InvocationState::Running)
        {
            return Ok(vec![]);
        }
        let snapshot = self.store.board(session)?;
        let choices = self.board_policy.propose(&snapshot)?;
        ensure!(
            choices.len() <= 256,
            "board_limit: strategy exceeded proposal allowance"
        );
        let mut seen = HashSet::new();
        let mut decisions = Vec::new();
        for id in choices {
            ensure!(
                seen.insert(id.clone()),
                "duplicate_proposal: strategy repeated a proposal"
            );
            let current = self.store.board(session)?;
            let proposal = current
                .proposals
                .iter()
                .find(|p| p.id == id && p.status == BoardProposalStatus::Pending)
                .context("unknown_proposal: strategy must select a pending, bound proposal")?
                .clone();
            let check_review = if matches!(proposal.change, BoardChange::ReplaceChecks { .. }) {
                self.store
                    .decisions(session)?
                    .into_iter()
                    .rev()
                    .find(|decision| {
                        decision.kind == "board_check_revision_review"
                            && decision
                                .links
                                .board_check_revision()
                                .is_some_and(|evidence| evidence.proposal == proposal)
                    })
            } else {
                None
            };
            if check_review.is_none()
                && proposal.change.definition_task().is_some_and(|reference| {
                    current.tasks.iter().any(|task| {
                        task.task.id == reference.task_id
                            && task.definition_version == reference.definition_version
                            && task.task.state == TaskState::Review
                    })
                })
            {
                // This proposal needs the dedicated independent review performed
                // while runtime verification owns the candidate workspace.
                continue;
            }
            let prepared = self.prepare_board_change(&current, &proposal);
            let (updated, allocation, commitment, result_version, reason, accepted) = match prepared
            {
                Ok((updated, allocation, commitment, result_version)) => (
                    updated,
                    allocation,
                    commitment,
                    result_version,
                    proposal.rationale.clone(),
                    true,
                ),
                Err(error) => (
                    vec![],
                    None,
                    None,
                    current.plan_version.clone(),
                    error.to_string(),
                    false,
                ),
            };
            let mut board = BoardDecision {
                implementation: self.board_identity.clone(),
                proposal,
                accepted,
                reason,
                review_ids: check_review
                    .iter()
                    .map(|review| review.id.clone())
                    .collect(),
                commitment,
                resulting_plan_version: result_version,
            };
            let followups = if board.accepted
                && matches!(board.proposal.change, BoardChange::ReplaceChecks { .. })
            {
                let before = current
                    .tasks
                    .iter()
                    .find(|task| Some(task.task.id.as_str()) == board.proposal.change.task_id())
                    .context("Missing check replacement source task")?;
                let after = updated
                    .iter()
                    .find(|task| task.id == before.task.id)
                    .context("Missing check replacement result task")?;
                ensure!(
                    before.task.attempts == after.attempts,
                    "check_replacement_attempt: board preparation changed the attempt counter"
                );
                let prior = self
                    .store
                    .decisions(session)?
                    .into_iter()
                    .rev()
                    .find(|decision| {
                        matches!(
                            decision.kind.as_str(),
                            "result_submitted" | "result_check_revised"
                        ) && decision.links.task.as_ref()
                            == Some(&TaskAttemptRef::from(&before.task))
                    })
                    .and_then(|decision| decision.links.result)
                    .context("Check replacement source result is missing")?;
                let mut revised = prior.clone();
                revised.id = revised_check_result_id(&prior, &board.proposal)?;
                revised.task_definition = Some(TaskDefinition::from(after));
                vec![
                    DecisionRecord {
                        id: new_id(),
                        session_id: session.into(),
                        kind: "result_invalidated".into(),
                        actor: None,
                        reason: "Ordinary check definition changed; prior check and review evidence remain historical and cannot accept the revised binding".into(),
                        outcome: None,
                        links: RecordLinks {
                            task: prior.task.clone(),
                            review_ids: board.review_ids.clone(),
                            result: Some(prior.clone()),
                            board: Some(Box::new(board.clone())),
                            ..Default::default()
                        },
                        created_at: now(),
                    },
                    DecisionRecord {
                        id: new_id(),
                        session_id: session.into(),
                        kind: "result_check_revised".into(),
                        actor: None,
                        reason: "Runtime rebound the existing candidate to an independently reviewed ordinary-check definition without another production attempt".into(),
                        outcome: None,
                        links: RecordLinks {
                            task: revised.task.clone(),
                            review_ids: board.review_ids.clone(),
                            result: Some(revised),
                            check_revision: Some(Box::new(CheckRevisionProvenance::Result(
                                Box::new(ResultCheckRevision {
                                    previous: prior,
                                    proposal: board.proposal.clone(),
                                    review_id: board.review_ids[0].clone(),
                                    attempt_before: before.task.attempts,
                                    attempt_after: after.attempts,
                                }),
                            ))),
                            ..Default::default()
                        },
                        created_at: now(),
                    },
                ]
            } else {
                vec![]
            };
            let make_record = |board: &BoardDecision| DecisionRecord {
                id: new_id(),
                session_id: session.into(),
                kind: if board.accepted {
                    "board_committed"
                } else {
                    "board_rejected"
                }
                .into(),
                actor: None,
                reason: board.reason.clone(),
                outcome: None,
                links: RecordLinks {
                    assignment_id: Some(board.proposal.assignment_id.clone()),
                    invocation_id: Some(board.proposal.invocation_id.clone()),
                    related_task_ids: if board.accepted {
                        updated
                            .iter()
                            .map(|task| task.id.clone())
                            .chain(board.proposal.change.task_id().map(str::to_owned))
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect()
                    } else {
                        vec![]
                    },
                    board: Some(Box::new(board.clone())),
                    review_ids: board.review_ids.clone(),
                    ..Default::default()
                },
                created_at: now(),
            };
            if let Err(error) = self.store.commit_board(
                &make_record(&board),
                &updated,
                allocation.as_ref(),
                &followups,
            ) {
                if error.to_string().contains("owner_paused:") {
                    return Err(error);
                }
                // A racing transition is an explicit rejection, never a partial
                // membership/task update. Another consumer may already own it.
                if !self
                    .store
                    .board(session)?
                    .proposals
                    .iter()
                    .any(|p| p.id == board.proposal.id && p.status == BoardProposalStatus::Pending)
                {
                    continue;
                }
                board.accepted = false;
                board.reason = error.to_string();
                board.commitment = None;
                board.resulting_plan_version = self.store.board(session)?.plan_version;
                self.store
                    .commit_board(&make_record(&board), &[], None, &[])?;
            }
            if board.accepted {
                for task in &updated {
                    let _ = self.events.send(UiEvent::Task(task.clone()));
                }
            }
            self.post(
                session,
                "ymp",
                if board.accepted {
                    "board_committed"
                } else {
                    "board_rejected"
                },
                &format!(
                    "Proposal {} by {}: {}",
                    board.proposal.id, board.proposal.agent_id, board.reason
                ),
            )?;
            decisions.push(board);
        }
        Ok(decisions)
    }

    fn board_allocation(
        &self,
        session: &str,
        task: Option<&Task>,
        agent: Option<(&str, &ModelEffort)>,
        members: Option<&[String]>,
    ) -> Result<DecisionRecord> {
        let demand = if let Some(task) = task {
            self.demand(
                session,
                "execute",
                Some(&task.id),
                &task.competence,
                &task.difficulty,
            )?
        } else {
            AllocationDemand {
                purpose: "execute".into(),
                task_id: None,
                competence: "implementation".into(),
                difficulty: "standard".into(),
                risk: TaskRisk::Standard,
                ready_work: 0,
            }
        };
        let mut input =
            self.allocation_input(session, AllocationBoundary::WorkReady, demand, None)?;
        if task.is_some() {
            input.demand.ready_work = 1;
        }
        let budget = input
            .budget
            .as_ref()
            .context("Missing captured board budget")?;
        ensure!(
            budget.in_flight_invocations == 0,
            "work_boundary: admission remains active"
        );
        ensure!(
            budget.admitted_invocations < budget.limits.turns as u64,
            "budget_exhausted: board cannot create fresh resources"
        );
        let mut proposal = self.allocation_policy.propose(&input)?;
        if let Some((id, settings)) = agent {
            settings.validate()?;
            let choice = input.candidates.iter().find(|c| c.agent_id == id && c.settings == *settings).context("invalid_execution_choice: proposed agent/model/effort violates captured rules or native capabilities")?.clone();
            if !proposal.members.contains(&choice.agent_id) {
                if let Some(index) = proposal.members.iter().position(|m| {
                    Some(m) != proposal.reserved_final_reviewer.as_ref()
                        && !input.occupied_agent_ids.contains(m)
                }) {
                    proposal.members[index] = choice.agent_id.clone();
                } else {
                    proposal.members.push(choice.agent_id.clone());
                }
            }
            proposal.executor = Some(choice);
        }
        if let Some(members) = members {
            proposal.members = members.to_vec();
            proposal.executor = None;
        }
        proposal.rationale = "Explicit shared-board decision validated at the next work boundary; captured resources and fixed settings remain authoritative".into();
        self.validate_allocation(&input, &proposal)?;
        Ok(DecisionRecord {
            id: new_id(),
            session_id: session.into(),
            kind: "allocation_committed".into(),
            actor: None,
            reason: proposal.rationale.clone(),
            outcome: None,
            links: RecordLinks {
                policy_chain: vec![
                    PolicyProvenance {
                        implementation: self.allocation_identity.clone(),
                        configuration: Value::Null,
                        originating_record_ids: vec![],
                    },
                    PolicyProvenance {
                        implementation: self.board_identity.clone(),
                        configuration: Value::Null,
                        originating_record_ids: vec![],
                    },
                ],
                allocation: Some(Box::new(AllocationDecision {
                    implementation: self.allocation_identity.clone(),
                    input,
                    proposal,
                    accepted: true,
                    reason: "Validated board allocation".into(),
                })),
                ..Default::default()
            },
            created_at: now(),
        })
    }

    #[allow(clippy::type_complexity)]
    fn prepare_board_change(
        &self,
        board: &BoardSnapshot,
        proposal: &BoardProposal,
    ) -> Result<(
        Vec<Task>,
        Option<DecisionRecord>,
        Option<BoardCommitment>,
        String,
    )> {
        ensure!(
            proposal.plan_version == board.plan_version,
            "stale_plan: proposal describes an older plan"
        );
        ensure!(
            proposal.team_version == board_team_version(board.team.as_ref())?,
            "stale_membership: session membership or eligibility changed before commitment"
        );
        let source = self
            .store
            .trace(&board.session_id)?
            .assignments
            .into_iter()
            .find(|a| a.id == proposal.assignment_id)
            .context("Missing proposal source")?;
        ensure!(
            source.state == InvocationState::Completed,
            "proposal_origin: source assignment did not complete"
        );
        let target = if let Some(reference) = proposal.change.task() {
            let task = board
                .tasks
                .iter()
                .find(|task| task.task.id == reference.task_id)
                .context("Unknown task")?;
            ensure!(
                task.version == reference.version,
                "stale_task: proposal describes an older task or commitment"
            );
            ensure!(
                task.task.state == TaskState::Ready,
                "work_boundary: inspect and review admitted effects before revision or reassignment"
            );
            Some(task.task.clone())
        } else if let Some(reference) = proposal.change.definition_task() {
            let task = board
                .tasks
                .iter()
                .find(|task| task.task.id == reference.task_id)
                .context("Unknown task")?;
            ensure!(
                task.definition_version == reference.definition_version,
                "stale_definition: task definition changed before check replacement"
            );
            ensure!(
                task.task.state == TaskState::Review,
                "check_replacement_boundary: replacement requires an ended execution awaiting review"
            );
            Some(task.task.clone())
        } else {
            None
        };
        let mut updated = vec![];
        let mut allocation = None;
        let mut commitment = None;
        match &proposal.change {
            BoardChange::AcceptResponsibility { settings, .. }
            | BoardChange::Assign { settings, .. } => {
                let task = target.context("Missing target")?;
                let agent = match &proposal.change {
                    BoardChange::Assign { agent_id, .. } => agent_id,
                    _ => &proposal.agent_id,
                };
                allocation = Some(self.board_allocation(
                    &board.session_id,
                    Some(&task),
                    Some((agent, settings)),
                    None,
                )?);
                commitment = Some(BoardCommitment {
                    proposal_id: proposal.id.clone(),
                    task_id: task.id.clone(),
                    task_version: board_task_version(&task)?,
                    agent_id: agent.clone(),
                    settings: settings.clone(),
                });
            }
            BoardChange::Membership { members } => {
                allocation =
                    Some(self.board_allocation(&board.session_id, None, None, Some(members))?)
            }
            BoardChange::Revise {
                approach,
                dependencies,
                checks,
                ..
            } => {
                ensure!(
                    !approach.trim().is_empty(),
                    "Revision requires an explicit approach and rationale"
                );
                let mut task = target.context("Missing target")?;
                task.description
                    .push_str(&format!("\n\nBoard approach: {approach}"));
                for dependency in dependencies {
                    if !task.dependencies.contains(dependency) {
                        task.dependencies.push(dependency.clone());
                    }
                }
                for check in checks {
                    if !task.checks.contains(check) {
                        task.checks.push(check.clone());
                    }
                }
                if let Some(previous) = board
                    .tasks
                    .iter()
                    .find(|t| t.task.id == task.id)
                    .and_then(|t| t.commitment.as_ref())
                {
                    allocation = Some(self.board_allocation(
                        &board.session_id,
                        Some(&task),
                        Some((&previous.agent_id, &previous.settings)),
                        None,
                    )?);
                    commitment = Some(BoardCommitment {
                        proposal_id: proposal.id.clone(),
                        task_id: task.id.clone(),
                        task_version: board_task_version(&task)?,
                        agent_id: previous.agent_id.clone(),
                        settings: previous.settings.clone(),
                    });
                }
                updated.push(task);
            }
            BoardChange::ReplaceChecks { replacements, .. } => {
                let mut task = target.context("Missing target")?;
                let review = self
                    .store
                    .decisions(&board.session_id)?
                    .into_iter()
                    .rev()
                    .find(|decision| {
                        decision.kind == "board_check_revision_review"
                            && decision.links.board_check_revision().is_some_and(
                                |evidence| evidence.proposal == *proposal,
                            )
                    })
                    .context(
                        "check_replacement_review: independent review is required before commitment",
                    )?;
                ensure!(
                    matches!(review.outcome, Some(DecisionOutcome::Accepted { .. }))
                        && review.actor.as_ref() != Some(&proposal.agent_id),
                    "check_replacement_review: replacement was not independently approved"
                );
                let evidence = review
                    .links
                    .board_check_revision()
                    .context("check_replacement_review: evidence is missing")?;
                ensure!(
                    replacements.iter().all(|replacement| {
                        evidence.retained_runs.iter().any(|run| {
                            run.command == replacement.old && !run.passed
                        })
                    }) && evidence.proposed_runs.iter().all(|run| run.passed),
                    "check_replacement_evidence: each replaced check must fail and all proposed checks must pass"
                );
                task.checks = replace_ordinary_checks(&task.checks, replacements)?;
                ensure!(
                    task.checks == evidence.proposed_checks,
                    "check_replacement_evidence: reviewed commands differ from the proposed result"
                );
                updated.push(task);
            }
            BoardChange::AddTask {
                title,
                description,
                competence,
                difficulty,
                access,
                dependencies,
                checks,
            } => {
                ensure!(
                    !board.tasks.iter().any(|t| t.task.title == *title),
                    "Duplicate task title would make configured contract binding ambiguous"
                );
                updated.push(Task {
                    id: new_id(),
                    session_id: board.session_id.clone(),
                    title: title.clone(),
                    description: description.clone(),
                    competence: competence.clone(),
                    difficulty: difficulty.clone(),
                    access: *access,
                    dependencies: dependencies.clone(),
                    checks: checks.clone(),
                    state: TaskState::Ready,
                    assignee: None,
                    reviewer: None,
                    attempts: 0,
                    result: None,
                    workspace: None,
                    base_commit: None,
                    interrupted: false,
                });
            }
        }
        let mut tasks = board
            .tasks
            .iter()
            .map(|t| t.task.clone())
            .collect::<Vec<_>>();
        for task in &updated {
            if let Some(previous) = tasks.iter_mut().find(|t| t.id == task.id) {
                *previous = task.clone();
            } else {
                tasks.push(task.clone());
            }
        }
        let plan = Plan {
            summary: proposal.rationale.clone(),
            tasks: tasks
                .iter()
                .map(|t| {
                    Ok(PlanTask {
                        title: t.title.clone(),
                        description: t.description.clone(),
                        competence: t.competence.clone(),
                        difficulty: t.difficulty.clone(),
                        access: t.access,
                        checks: t.checks.clone(),
                        dependencies: t
                            .dependencies
                            .iter()
                            .map(|id| {
                                tasks
                                    .iter()
                                    .position(|t| &t.id == id)
                                    .context("Unknown or foreign task dependency")
                            })
                            .collect::<Result<Vec<_>>>()?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        };
        plan.validate()?;
        self.validate_contract_bindings(&board.session_id, tasks.iter().map(|t| t.title.as_str()))?;
        if allocation.is_none() {
            let budget = self
                .store
                .session_budget(&board.session_id)?
                .context("Missing captured budget")?;
            ensure!(
                budget.admitted_invocations < budget.limits.turns as u64,
                "budget_exhausted: board revisions do not reset spend"
            );
        }
        Ok((updated, allocation, commitment, board_plan_version(&tasks)?))
    }

    pub(super) fn committed_executor(
        &self,
        session: &str,
        task: &Task,
        candidates: &[AgentProfile],
    ) -> Result<Option<AgentProfile>> {
        let Some(commitment) = self
            .store
            .board(session)?
            .tasks
            .into_iter()
            .find(|t| t.task.id == task.id)
            .and_then(|t| t.commitment)
        else {
            return Ok(None);
        };
        let agent = candidates.iter().find(|a| a.id == commitment.agent_id).context("commitment_unavailable: explicit reassignment is required for an unavailable responsible agent")?.clone();
        let record = self.board_allocation(
            session,
            Some(task),
            Some((&agent.id, &commitment.settings)),
            None,
        )?;
        self.store.commit_allocation(&record)?;
        Ok(Some(agent))
    }
}
