use super::{
    provenance::{decision, record, records},
    Store,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use ymp_core::*;

fn proposal_key(session: &str) -> String {
    format!("board:v1:{session}")
}
fn read_proposals(db: &Connection, session: &str) -> Result<Vec<BoardProposal>> {
    let raw: Option<String> = db
        .query_row(
            "SELECT value FROM kv WHERE key=?",
            [proposal_key(session)],
            |r| r.get(0),
        )
        .optional()?;
    raw.map(|v| Ok(serde_json::from_str(&v)?))
        .transpose()
        .map(Option::unwrap_or_default)
}
fn save_proposals(tx: &Transaction<'_>, session: &str, proposals: &[BoardProposal]) -> Result<()> {
    tx.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![proposal_key(session), serde_json::to_string(proposals)?])?;
    Ok(())
}
pub(super) fn snapshot(db: &Connection, session: &str) -> Result<BoardSnapshot> {
    let tasks = records::<Task>(db, "tasks", session)?;
    let plan_version = board_plan_version(&tasks)?;
    let decisions = records::<DecisionRecord>(db, "decisions", session)?;
    let tasks = tasks
        .into_iter()
        .map(|task| {
            let raw = board_task_version(&task)?;
            let latest = decisions
                .iter()
                .rev()
                .filter_map(|d| d.links.board.as_ref())
                .find_map(|d| {
                    d.commitment
                        .as_ref()
                        .filter(|c| c.task_id == task.id && d.accepted)
                });
            let release = latest.and_then(|commitment| {
                decisions.iter().rev().find(|d| {
                    d.links
                        .board_release
                        .as_ref()
                        .is_some_and(|r| r.commitment == *commitment)
                })
            });
            // Preserve pre-release versions exactly, including legacy snapshots.
            let mut version = content_digest(&serde_json::to_string(&(
                &raw,
                latest.map(|c| &c.proposal_id),
            ))?);
            if let Some(release) = release {
                version = content_digest(&serde_json::to_string(&(&version, &release.id))?);
            }
            let commitment = latest
                .filter(|c| {
                    release.is_none() && c.task_version == raw && task.state == TaskState::Ready
                })
                .cloned();
            Ok(BoardTask {
                definition_version: board_task_definition_version(&task)?,
                task,
                version,
                commitment,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(BoardSnapshot {
        session_id: session.into(),
        plan_version,
        tasks,
        proposals: read_proposals(db, session)?,
        team: super::allocation::state(db, session)?,
    })
}

/// Called inside the accepted owner command transaction, before responsibility settlement.
pub(super) fn release_unadmitted(
    tx: &Transaction<'_>,
    command: &OwnerTeamCommand,
    departing: &str,
) -> Result<()> {
    let assignments = records::<AssignmentRecord>(tx, "assignments", &command.session_id)?;
    for task in snapshot(tx, &command.session_id)?.tasks {
        let Some(commitment) = task.commitment.filter(|c| c.agent_id == departing) else {
            continue;
        };
        // Ready commitments are not invocation authority. A recorded admission for
        // this exact attempt is nevertheless a conservative exclusion at storage.
        if task.task.state != TaskState::Ready
            || assignments.iter().any(|a| {
                a.purpose == "execute" && a.task.as_ref() == Some(&TaskAttemptRef::from(&task.task))
            })
        {
            continue;
        }
        decision(tx, &DecisionRecord {
            id: new_id(),
            session_id: command.session_id.clone(),
            kind: "owner_board_commitment_released".into(),
            actor: None,
            reason: "Owner departure released ready responsibility before execution admission; task obligations and admitted ownership are preserved".into(),
            outcome: None,
            links: RecordLinks {
                task: Some(TaskAttemptRef::from(&task.task)),
                board_release: Some(Box::new(BoardCommitmentRelease {
                    command_id: command.command_id.clone(),
                    previous: BoardTaskRef { task_id: task.task.id, version: task.version },
                    commitment,
                })),
                ..Default::default()
            },
            created_at: now(),
        })?;
    }
    Ok(())
}
pub(super) fn propose(
    tx: &Transaction<'_>,
    grant: &GrantRecord,
    arguments: &serde_json::Value,
) -> Result<BoardProposal> {
    let board = snapshot(tx, &grant.session_id)?;
    ensure!(
        !board.tasks.is_empty(),
        "board_not_ready: initial reviewed plan has not been committed"
    );
    let plan_version = arguments["plan_version"]
        .as_str()
        .context("plan_version is required; read board_read first")?;
    ensure!(
        plan_version == board.plan_version,
        "stale_plan: read the current board before proposing"
    );
    let (change, rationale) = if let Some(change) = arguments.get("change") {
        ensure!(
            arguments.get("title").is_none() && arguments.get("description").is_none(),
            "Typed and legacy proposals cannot be mixed"
        );
        (
            serde_json::from_value::<BoardChange>(change.clone())?,
            arguments["rationale"]
                .as_str()
                .context("rationale is required")?
                .to_owned(),
        )
    } else {
        let title = arguments["title"].as_str().context("title required")?;
        let description = arguments["description"]
            .as_str()
            .context("description required")?;
        (
            BoardChange::AddTask {
                title: title.into(),
                description: description.into(),
                competence: "implementation".into(),
                difficulty: "standard".into(),
                access: TaskAccess::Write,
                dependencies: vec![],
                checks: vec![],
            },
            description.into(),
        )
    };
    ensure!(
        !rationale.trim().is_empty()
            && serde_json::to_string(&change)?.len() <= 32_000
            && rationale.len() <= 4000,
        "Invalid or oversized board proposal"
    );
    if let Some(reference) = change.task() {
        ensure!(
            board
                .tasks
                .iter()
                .any(|t| t.task.id == reference.task_id && t.version == reference.version),
            "stale_task: task version is no longer current or belongs to another session"
        );
    }
    if let Some(reference) = change.definition_task() {
        let target = board
            .tasks
            .iter()
            .find(|task| task.task.id == reference.task_id)
            .context("check_replacement: unknown or foreign task")?;
        ensure!(
            target.definition_version == reference.definition_version,
            "stale_definition: task definition changed before proposal"
        );
        let BoardChange::ReplaceChecks { replacements, .. } = &change else {
            unreachable!("definition-bound board changes are check replacements")
        };
        replace_ordinary_checks(&target.task.checks, replacements)?;
        let assignment: AssignmentRecord = record(tx, "assignments", &grant.assignment_id)?;
        ensure!(
            assignment.session_id == grant.session_id
                && assignment.agent_id == grant.agent_id
                && assignment.purpose == "execute"
                && assignment.task.as_ref() == Some(&TaskAttemptRef::from(&target.task))
                && target.task.state == TaskState::Running
                && target.task.assignee.as_ref() == Some(&grant.agent_id),
            "check_replacement_authority: only the active task executor may propose a replacement"
        );
    }
    let proposal = BoardProposal {
        id: new_id(),
        session_id: grant.session_id.clone(),
        agent_id: grant.agent_id.clone(),
        assignment_id: grant.assignment_id.clone(),
        invocation_id: grant.invocation_id.clone(),
        grant_id: grant.id.clone(),
        plan_version: plan_version.into(),
        team_version: board_team_version(board.team.as_ref())?,
        change,
        rationale,
        status: BoardProposalStatus::Pending,
        created_at: now(),
    };
    let mut proposals = board.proposals;
    ensure!(
        proposals.len() < 256,
        "board_limit: session proposal allowance exhausted"
    );
    proposals.push(proposal.clone());
    save_proposals(tx, &grant.session_id, &proposals)?;
    tx.execute(
        "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'board_proposed',?,?)",
        params![grant.session_id, serde_json::to_string(&proposal)?, now()],
    )?;
    Ok(proposal)
}

impl Store {
    /// This is preparation for independent review, never acceptance or replay.
    pub fn prepare_ended_execution_review(
        &self,
        session: &str,
        expected: &TaskAttemptRef,
    ) -> Result<Option<Task>> {
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut task: Task = record(&tx, "tasks", &expected.task_id)?;
        ensure!(
            task.session_id == session,
            "Execution review belongs to another session"
        );
        if task.state != TaskState::Running || TaskAttemptRef::from(&task) != *expected {
            return Ok(None);
        }
        if super::team_control::state(&tx, session)?
            .is_some_and(|s| s.control != OwnerRunControl::Continue)
        {
            return Ok(None);
        }
        let assignments = records::<AssignmentRecord>(&tx, "assignments", session)?;
        let decisions = records::<DecisionRecord>(&tx, "decisions", session)?;
        let Some(failure) = decisions
            .iter()
            .rev()
            .filter_map(|d| d.links.failure.as_ref())
            .find(|f| {
                f.class == FailureClass::TransientTransport
                    && f.termination == TerminationEvidence::BackendEnded
                    && f.effective_access.is_read_only()
                    && task.assignee.as_ref() == Some(&f.agent_id)
                    && assignments.iter().any(|a| {
                        a.id == f.assignment_id
                            && a.purpose == "execute"
                            && a.task.as_ref() == Some(expected)
                            && a.state == InvocationState::Failed
                            && a.ended_at.is_some()
                    })
            })
        else {
            return Ok(None);
        };
        if super::team_control::responsibilities(&tx, session)?
            .iter()
            .any(|r| {
                r.agent_id == failure.agent_id
                    && matches!(r.kind.as_str(), "invocation" | "workspace_access")
            })
        {
            return Ok(None);
        }
        let invocation: InvocationRecord = record(&tx, "invocations", &failure.invocation_id)?;
        ensure!(
            invocation.assignment_id == failure.assignment_id
                && invocation.state == InvocationState::Failed
                && invocation.ended_at.is_some(),
            "Execution inspection requires recorded backend termination"
        );
        task.state = TaskState::Review;
        task.interrupted = true;
        task.result = Some("Execution was interrupted. Inspect actual files and check results; do not assume completion or repeat external actions.".into());
        super::write_task(&tx, &task)?;
        decision(&tx, &DecisionRecord {
            id: new_id(), session_id: session.into(), kind: "execution_interruption_review_ready".into(),
            actor: None, reason: "Known-ended read-only transport failure retains its attempt and origin for independent inspection before rework".into(),
            outcome: None, links: RecordLinks {
                task: Some(expected.clone()), failure: Some(failure.clone()),
                assignment_id: Some(failure.assignment_id.clone()), invocation_id: Some(failure.invocation_id.clone()),
                ..Default::default()
            }, created_at: now(),
        })?;
        tx.commit()?;
        Ok(Some(task))
    }

    pub fn board(&self, session: &str) -> Result<BoardSnapshot> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let board = snapshot(&tx, session)?;
        tx.commit()?;
        Ok(board)
    }

    /// Runtime-only transaction. Rechecks the exact proposal, task, membership
    /// and invocation boundary before updating tasks, ownership and their journal.
    pub fn commit_board(
        &self,
        value: &DecisionRecord,
        updated: &[Task],
        allocation: Option<&DecisionRecord>,
        followups: &[DecisionRecord],
    ) -> Result<()> {
        let board_decision = value
            .links
            .board
            .as_ref()
            .context("Missing board decision")?;
        let proposal = &board_decision.proposal;
        ensure!(
            proposal.session_id == value.session_id && value.actor.is_none(),
            "Board decisions belong to the trusted runtime"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let board = snapshot(&tx, &value.session_id)?;
        let mut proposals = board.proposals;
        let saved = proposals
            .iter_mut()
            .find(|p| p.id == proposal.id)
            .context("Unknown board proposal")?;
        ensure!(
            saved == proposal && saved.status == BoardProposalStatus::Pending,
            "stale_proposal: proposal was already decided"
        );
        if board_decision.accepted {
            ensure!(
                proposal.plan_version == board.plan_version,
                "stale_plan: plan changed before board commitment"
            );
            ensure!(
                proposal.team_version == board_team_version(board.team.as_ref())?,
                "stale_membership: session membership or eligibility changed before commitment"
            );
            if let Some(reference) = proposal.change.task() {
                let current = board
                    .tasks
                    .iter()
                    .find(|t| t.task.id == reference.task_id)
                    .context("Unknown task")?;
                ensure!(
                    current.version == reference.version,
                    "stale_task: task or commitment changed before board commitment"
                );
                ensure!(current.task.state == TaskState::Ready, "work_boundary: only unresolved ready tasks can change; inspect uncertain effects first");
            } else if let Some(reference) = proposal.change.definition_task() {
                let current = board
                    .tasks
                    .iter()
                    .find(|task| task.task.id == reference.task_id)
                    .context("Unknown task")?;
                ensure!(
                    current.definition_version == reference.definition_version,
                    "stale_definition: task definition changed before board commitment"
                );
                ensure!(
                    current.task.state == TaskState::Review,
                    "check_replacement_boundary: source execution must be ended and awaiting review"
                );
            }
            for assignment in records::<AssignmentRecord>(&tx, "assignments", &value.session_id)? {
                ensure!(
                    assignment.state != InvocationState::Running,
                    "work_boundary: active invocations must finish before board commitment"
                );
            }
            let origin: AssignmentRecord = record(&tx, "assignments", &proposal.assignment_id)?;
            let invocation: InvocationRecord = record(&tx, "invocations", &proposal.invocation_id)?;
            ensure!(
                origin.session_id == value.session_id
                    && origin.agent_id == proposal.agent_id
                    && origin.state == InvocationState::Completed
                    && invocation.assignment_id == origin.id
                    && invocation.state == InvocationState::Completed
                    && origin.grant_ids.contains(&proposal.grant_id),
                "proposal_origin: incomplete or foreign source cannot commit changes"
            );
            if proposal.change.definition_task().is_some() {
                let BoardChange::ReplaceChecks { replacements, .. } = &proposal.change else {
                    unreachable!("definition-bound changes are check replacements")
                };
                if let Some(owner) = super::team_control::state(&tx, &value.session_id)? {
                    ensure!(
                        owner.control == OwnerRunControl::Continue,
                        "owner_paused: check replacement waits for explicit owner continuation"
                    );
                }
                let current = board
                    .tasks
                    .iter()
                    .find(|task| Some(task.task.id.as_str()) == proposal.change.task_id())
                    .context("Missing check replacement target")?;
                ensure!(
                    origin.purpose == "execute"
                        && origin.task.as_ref() == Some(&TaskAttemptRef::from(&current.task))
                        && current.task.assignee.as_ref() == Some(&proposal.agent_id),
                    "check_replacement_authority: proposal must come from the active target executor"
                );
                ensure!(
                    board_decision.review_ids.len() == 1
                        && value.links.review_ids == board_decision.review_ids,
                    "check_replacement_review: exactly one bound review is required"
                );
                let review: DecisionRecord =
                    record(&tx, "decisions", &board_decision.review_ids[0])?;
                let review_assignment: AssignmentRecord = record(
                    &tx,
                    "assignments",
                    review
                        .links
                        .assignment_id
                        .as_deref()
                        .context("Check replacement review assignment is missing")?,
                )?;
                ensure!(
                    review.kind == "board_check_revision_review"
                        && matches!(
                            review.outcome,
                            Some(DecisionOutcome::Accepted { .. })
                        )
                        && review.actor.as_ref() != Some(&proposal.agent_id)
                        && review.links.task.as_ref()
                            == Some(&TaskAttemptRef::from(&current.task))
                        && review
                            .links
                            .board_check_revision()
                            .is_some_and(|evidence| {
                                evidence.proposal == *proposal
                                    && evidence.retained_checks == current.task.checks
                                    && replacements.iter().all(|replacement| {
                                        evidence.retained_runs.iter().any(|run| {
                                            run.command == replacement.old && !run.passed
                                        })
                                    })
                                    && evidence.proposed_runs.iter().all(|run| run.passed)
                            })
                        && review_assignment.purpose == "review_check_revision"
                        && review_assignment.agent_id != proposal.agent_id
                        && review_assignment.state == InvocationState::Completed,
                    "check_replacement_review: accepted independent evidence is missing or mismatched"
                );
            }
            let budget = super::budget::snapshot(&tx, &value.session_id)?
                .context("Missing captured board budget")?;
            ensure!(
                budget.in_flight_invocations == 0
                    && budget.admitted_invocations < budget.limits.turns as u64,
                "budget_exhausted: board cannot reset or bypass current admission resources"
            );
            // The validated task replacement is conservative even at the storage seam.
            let mut resulting = board
                .tasks
                .iter()
                .map(|t| t.task.clone())
                .collect::<Vec<_>>();
            for task in updated {
                ensure!(
                    task.session_id == value.session_id
                        && (task.state == TaskState::Ready
                            || (matches!(proposal.change, BoardChange::ReplaceChecks { .. })
                                && task.state == TaskState::Review)),
                    "Invalid board task session or state"
                );
                if let Some(previous) = resulting.iter_mut().find(|t| t.id == task.id) {
                    if let BoardChange::ReplaceChecks { replacements, .. } = &proposal.change {
                        let expected_checks =
                            replace_ordinary_checks(&previous.checks, replacements)?;
                        ensure!(
                            previous.state == TaskState::Review
                                && task.state == TaskState::Review
                                && previous.id == task.id
                                && previous.session_id == task.session_id
                                && previous.title == task.title
                                && previous.description == task.description
                                && previous.competence == task.competence
                                && previous.difficulty == task.difficulty
                                && previous.access == task.access
                                && previous.dependencies == task.dependencies
                                && previous.attempts == task.attempts
                                && previous.assignee == task.assignee
                                && previous.reviewer == task.reviewer
                                && previous.workspace == task.workspace
                                && previous.base_commit == task.base_commit
                                && previous.interrupted == task.interrupted
                                && previous.result == task.result
                                && task.checks == expected_checks,
                            "preserved_obligations: check replacement changed fields outside ordinary checks and review lifecycle"
                        );
                    } else {
                        ensure!(previous.state == TaskState::Ready && previous.title == task.title && previous.access == task.access && previous.attempts == task.attempts && previous.checks.iter().all(|c| task.checks.contains(c)) && previous.dependencies.iter().all(|d| task.dependencies.contains(d)) && task.description.starts_with(&previous.description), "preserved_obligations: revisions cannot remove objectives, checks, dependencies, accepted work or authority restrictions");
                    }
                    *previous = task.clone();
                } else {
                    resulting.push(task.clone());
                }
            }
            ensure!(
                board_plan_version(&resulting)? == board_decision.resulting_plan_version,
                "Board result version mismatch"
            );
            if let Some(allocation) = allocation {
                super::allocation::commit(&tx, allocation)?;
            }
            for task in updated {
                super::write_task(&tx, task)?;
            }
            saved.status = BoardProposalStatus::Committed;
        } else {
            ensure!(
                updated.is_empty() && allocation.is_none(),
                "Rejected board proposal cannot mutate work"
            );
            saved.status = BoardProposalStatus::Rejected;
        }
        save_proposals(&tx, &value.session_id, &proposals)?;
        decision(&tx, value)?;
        for followup in followups {
            ensure!(
                followup.session_id == value.session_id,
                "Board follow-up belongs to another session"
            );
            decision(&tx, followup)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Competing schedulers cannot overwrite an admitted task or replay a stale wave.
    pub fn claim_board_task(&self, reference: &BoardTaskRef, task: &Task) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let board = snapshot(&tx, &task.session_id)?;
        let previous = board
            .tasks
            .iter()
            .find(|t| t.task.id == task.id)
            .context("Unknown task")?;
        ensure!(
            reference.task_id == task.id
                && reference.version == previous.version
                && previous.task.state == TaskState::Ready,
            "stale_claim: task version changed before admission"
        );
        ensure!(
            task.state == TaskState::Running && task.attempts == previous.task.attempts + 1,
            "Invalid task claim"
        );
        if let Some(commitment) = &previous.commitment {
            ensure!(
                task.assignee.as_ref() == Some(&commitment.agent_id),
                "commitment_owner: only the current responsible agent can execute"
            );
        }
        let agent = task.assignee.as_deref().context("Missing claim executor")?;
        super::team_control::check_admission(&tx, &task.session_id, agent)?;
        if let Some(team) = &board.team {
            ensure!(
                team.current_members.iter().any(|id| id == agent)
                    && team.eligible_agents.iter().any(|id| id == agent)
                    && team.reserved_final_reviewer.as_deref() != Some(agent),
                "claim_membership: executor became unavailable or reserved before commitment"
            );
        }
        ensure!(
            !board.tasks.iter().any(|t| t.task.id != task.id
                && t.task.state == TaskState::Running
                && t.task.assignee.as_deref() == Some(agent)),
            "claim_busy: another task already owns this executor"
        );
        ensure!(
            !records::<AssignmentRecord>(&tx, "assignments", &task.session_id)?
                .iter()
                .any(|a| a.agent_id == agent && a.state == InvocationState::Running),
            "claim_busy: executor has another active invocation"
        );
        let mut expected = previous.task.clone();
        let accepted = board
            .tasks
            .iter()
            .filter(|t| t.task.state == TaskState::Accepted)
            .map(|t| t.task.id.clone())
            .collect();
        expected.assign(
            task.assignee.as_deref().context("Missing claim executor")?,
            &accepted,
        )?;
        expected.workspace = task.workspace.clone();
        expected.base_commit = task.base_commit.clone();
        ensure!(
            serde_json::to_value(&expected)? == serde_json::to_value(task)?,
            "stale_claim: task definition changed before execution"
        );
        super::write_task(&tx, task)?;
        decision(&tx, &DecisionRecord { id: new_id(), session_id: task.session_id.clone(), kind: "task_claimed".into(), actor: None, reason: "Runtime selected the current task and responsibility at a work boundary; native admission still requires budget and grants".into(), outcome: None, links: RecordLinks { task: Some(TaskAttemptRef::from(task)), ..Default::default() }, created_at: now() })?;
        tx.commit()?;
        Ok(())
    }
}
