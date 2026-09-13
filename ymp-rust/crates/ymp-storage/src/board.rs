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
                board_release: Some(BoardCommitmentRelease {
                    command_id: command.command_id.clone(),
                    previous: BoardTaskRef { task_id: task.task.id, version: task.version },
                    commitment,
                }),
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
                    task.session_id == value.session_id && task.state == TaskState::Ready,
                    "Invalid board task session or state"
                );
                if let Some(previous) = resulting.iter_mut().find(|t| t.id == task.id) {
                    ensure!(previous.state == TaskState::Ready && previous.title == task.title && previous.access == task.access && previous.attempts == task.attempts && previous.checks.iter().all(|c| task.checks.contains(c)) && previous.dependencies.iter().all(|d| task.dependencies.contains(d)) && task.description.starts_with(&previous.description), "preserved_obligations: revisions cannot remove objectives, checks, dependencies, accepted work or authority restrictions");
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
