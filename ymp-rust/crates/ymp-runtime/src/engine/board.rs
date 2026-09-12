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
                commitment,
                resulting_plan_version: result_version,
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
                            .chain(
                                board
                                    .proposal
                                    .change
                                    .task()
                                    .map(|task| task.task_id.clone()),
                            )
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect()
                    } else {
                        vec![]
                    },
                    board: Some(Box::new(board.clone())),
                    ..Default::default()
                },
                created_at: now(),
            };
            if let Err(error) =
                self.store
                    .commit_board(&make_record(&board), &updated, allocation.as_ref())
            {
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
                self.store.commit_board(&make_record(&board), &[], None)?;
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
                allocation: Some(Box::new(AllocationDecision {
                    implementation: self.board_identity.clone(),
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
        let target = proposal.change.task().map(|reference| {
            let task = board.tasks.iter().find(|t| t.task.id == reference.task_id).context("Unknown task")?;
            ensure!(task.version == reference.version, "stale_task: proposal describes an older task or commitment");
            ensure!(task.task.state == TaskState::Ready, "work_boundary: inspect and review admitted effects before revision or reassignment");
            Ok(task.task.clone())
        }).transpose()?;
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
