use super::Store;
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{de::DeserializeOwned, Serialize};
use ymp_core::*;

pub(super) fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(
        "CREATE TABLE session_policies(session_id TEXT PRIMARY KEY REFERENCES sessions(id),data TEXT NOT NULL);
         CREATE TABLE assignments(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id),data TEXT NOT NULL);
         CREATE INDEX assignments_session ON assignments(session_id);
         CREATE TABLE invocations(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id),assignment_id TEXT NOT NULL UNIQUE REFERENCES assignments(id),turn INTEGER NOT NULL,data TEXT NOT NULL,UNIQUE(session_id,turn));
         CREATE TABLE decisions(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id),data TEXT NOT NULL);
         CREATE INDEX decisions_session ON decisions(session_id);
         PRAGMA user_version=3;",
    )?;
    tx.commit()?;
    Ok(())
}

pub(super) fn event(tx: &Transaction<'_>, session: &str, value: &ProvenanceEvent) -> Result<()> {
    tx.execute(
        "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'provenance',?,?)",
        params![session, serde_json::to_string(value)?, now()],
    )?;
    Ok(())
}

pub(super) fn record<T: DeserializeOwned>(db: &Connection, table: &str, id: &str) -> Result<T> {
    let raw: String = db.query_row(&format!("SELECT data FROM {table} WHERE id=?"), [id], |r| {
        r.get(0)
    })?;
    Ok(serde_json::from_str(&raw)?)
}

pub(super) fn records<T: DeserializeOwned>(
    db: &Connection,
    table: &str,
    session: &str,
) -> Result<Vec<T>> {
    let mut query = db.prepare(&format!(
        "SELECT data FROM {table} WHERE session_id=? ORDER BY rowid"
    ))?;
    let rows = query
        .query_map([session], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|raw| Ok(serde_json::from_str(&raw)?))
        .collect()
}

fn update<T: Serialize>(tx: &Transaction<'_>, table: &str, id: &str, value: &T) -> Result<()> {
    ensure!(
        tx.execute(
            &format!("UPDATE {table} SET data=? WHERE id=?"),
            params![serde_json::to_string(value)?, id]
        )? == 1,
        "Missing provenance record"
    );
    Ok(())
}

pub(super) fn validate_task(
    db: &Connection,
    session: &str,
    reference: &TaskAttemptRef,
    current: bool,
) -> Result<()> {
    let task: Task = record(db, "tasks", &reference.task_id)?;
    ensure!(
        task.session_id == session,
        "Task belongs to another session"
    );
    ensure!(
        reference.attempt <= task.attempts && (!current || reference.attempt == task.attempts),
        "Task attempt is stale or unknown"
    );
    Ok(())
}

fn validate_links(db: &Connection, decision: &DecisionRecord) -> Result<()> {
    let session: Session = record(db, "sessions", &decision.session_id)?;
    if let Some(actor) = &decision.actor {
        ensure!(
            session.team.iter().any(|agent| &agent.id == actor),
            "Decision actor is not in the session team"
        );
    }
    if let Some(task) = &decision.links.task {
        validate_task(db, &decision.session_id, task, false)?;
    }
    for id in &decision.links.related_task_ids {
        let task: Task = record(db, "tasks", id)?;
        ensure!(
            task.session_id == decision.session_id,
            "Related task belongs to another session"
        );
    }
    for id in &decision.links.review_ids {
        let review: DecisionRecord = record(db, "decisions", id)?;
        ensure!(
            review.session_id == decision.session_id,
            "Review belongs to another session"
        );
        if decision.links.task.is_some() {
            ensure!(
                review.links.task == decision.links.task,
                "Review belongs to another task attempt"
            );
        }
    }
    let assignment = decision
        .links
        .assignment_id
        .as_ref()
        .map(|id| record::<AssignmentRecord>(db, "assignments", id))
        .transpose()?;
    if let Some(assignment) = &assignment {
        ensure!(
            assignment.session_id == decision.session_id,
            "Assignment belongs to another session"
        );
        if decision.links.task.is_some() {
            ensure!(
                assignment.task == decision.links.task,
                "Decision task and assignment disagree"
            );
        }
    }
    if let Some(id) = &decision.links.invocation_id {
        let invocation: InvocationRecord = record(db, "invocations", id)?;
        ensure!(
            invocation.session_id == decision.session_id,
            "Invocation belongs to another session"
        );
        let linked: AssignmentRecord = record(db, "assignments", &invocation.assignment_id)?;
        if let Some(assignment) = assignment {
            ensure!(
                invocation.assignment_id == assignment.id,
                "Decision invocation and assignment disagree"
            );
        }
        if decision.links.task.is_some() {
            ensure!(
                linked.task == decision.links.task,
                "Decision invocation and task disagree"
            );
        }
    }
    for id in &decision.links.grant_ids {
        let grant = super::authority::grant(db, id)?;
        ensure!(
            grant.session_id == decision.session_id,
            "Decision grant belongs to another session"
        );
        ensure!(
            decision
                .links
                .assignment_id
                .as_ref()
                .is_none_or(|id| id == &grant.assignment_id)
                && decision
                    .links
                    .invocation_id
                    .as_ref()
                    .is_none_or(|id| id == &grant.invocation_id),
            "Decision grant belongs to another assignment or invocation"
        );
    }
    if let Some(version) = &decision.links.plan_proposal {
        validate_plan_version(db, decision, version)?;
    }
    Ok(())
}

fn validate_plan_version(
    db: &Connection,
    decision: &DecisionRecord,
    version: &PlanVersion,
) -> Result<()> {
    ensure!(
        !version.proposal_id.is_empty() && version.revision > 0,
        "Plan proposal needs an identity and revision"
    );
    version.plan.validate()?;
    let producer: AssignmentRecord = record(db, "assignments", &version.producer_assignment_id)?;
    let invocation: InvocationRecord = record(db, "invocations", &version.producer_invocation_id)?;
    ensure!(
        producer.session_id == decision.session_id
            && invocation.session_id == decision.session_id
            && invocation.assignment_id == producer.id,
        "Plan producer belongs to another assignment or session"
    );
    ensure!(
        producer.purpose == "plan" && invocation.state == InvocationState::Completed,
        "Plan producer must be a completed planning invocation"
    );
    let proposals = records::<DecisionRecord>(db, "decisions", &decision.session_id)?
        .into_iter()
        .filter(|d| d.kind == "plan_proposed")
        .filter_map(|d| d.links.plan_proposal)
        .filter(|p| p.proposal_id == version.proposal_id)
        .collect::<Vec<_>>();
    let existing = proposals.iter().find(|p| p.revision == version.revision);
    if decision.kind == "plan_proposed" {
        ensure!(existing.is_none(), "Plan proposal revision already exists");
        ensure!(
            decision.links.assignment_id.as_ref() == Some(&producer.id)
                && decision.links.invocation_id.as_ref() == Some(&invocation.id)
                && decision.actor.as_ref() == Some(&producer.agent_id),
            "Plan production decision must identify its producer"
        );
        if version.revision > 1 {
            ensure!(
                proposals.iter().any(|p| p.revision == version.revision - 1),
                "Previous plan proposal revision is missing"
            );
            ensure!(
                decision
                    .links
                    .review_ids
                    .iter()
                    .any(
                        |id| record::<DecisionRecord>(db, "decisions", id).is_ok_and(
                            |review| review.outcome == Some(DecisionOutcome::Rejected)
                                && review
                                    .links
                                    .plan_proposal
                                    .as_ref()
                                    .is_some_and(|prior| prior.proposal_id == version.proposal_id
                                        && prior.revision == version.revision - 1)
                        )
                    ),
                "Plan revision must link the rejected review of its predecessor"
            );
        }
    } else {
        ensure!(
            existing == Some(version),
            "Plan proposal content or producer differs from the recorded version"
        );
        if decision.kind == "plan_committed" {
            for id in &decision.links.review_ids {
                let review: DecisionRecord = record(db, "decisions", id)?;
                ensure!(
                    review.links.plan_proposal.as_ref() == Some(version),
                    "Plan commitment review refers to another proposal version"
                );
            }
        }
    }
    Ok(())
}

pub(super) fn decision(tx: &Transaction<'_>, value: &DecisionRecord) -> Result<()> {
    ensure!(
        !value.id.is_empty() && !value.kind.is_empty() && !value.reason.trim().is_empty(),
        "Decision needs an identity, kind and reason"
    );
    validate_links(tx, value)?;
    super::confirmation::validate(tx, value)?;
    tx.execute(
        "INSERT INTO decisions(id,session_id,data) VALUES (?,?,?)",
        params![value.id, value.session_id, serde_json::to_string(value)?],
    )?;
    event(
        tx,
        &value.session_id,
        &ProvenanceEvent::DecisionRecorded {
            decision: Box::new(value.clone()),
        },
    )
}

impl Store {
    /// Create a session and its immutable input capture in one transaction.
    pub fn create_session(&self, session: &Session, policy: &SessionPolicy) -> Result<()> {
        ensure!(
            session.id == policy.session_id,
            "Session policy identity mismatch"
        );
        ensure!(
            serde_json::to_value(&session.team)? == serde_json::to_value(&policy.captured_team)?,
            "Captured team differs from the session team"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        if let Some(parent) = &policy.parent_session_id {
            let prior: Session = record(&tx, "sessions", parent)?;
            ensure!(
                prior.project_id == session.project_id,
                "Parent session belongs to another project"
            );
        }
        tx.execute(
            "INSERT INTO sessions(id,project_id,data) VALUES (?,?,?)",
            params![
                session.id,
                session.project_id,
                serde_json::to_string(session)?
            ],
        )?;
        tx.execute(
            "INSERT INTO session_policies(session_id,data) VALUES (?,?)",
            params![session.id, serde_json::to_string(policy)?],
        )?;
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'session',?,?)",
            params![session.id, serde_json::to_string(session)?, now()],
        )?;
        event(
            &tx,
            &session.id,
            &ProvenanceEvent::SessionCaptured {
                policy: Box::new(policy.clone()),
            },
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn session_policy(&self, session: &str) -> Result<Option<SessionPolicy>> {
        let data: Option<String> = self
            .db()?
            .query_row(
                "SELECT data FROM session_policies WHERE session_id=?",
                [session],
                |r| r.get(0),
            )
            .optional()?;
        data.map(|raw| Ok(serde_json::from_str(&raw)?)).transpose()
    }

    /// Start one outer invocation and its assignment; no live permission is
    /// granted by this API. Identities must be fresh and the supplied ordinal
    /// must be exactly the next unspent one.
    pub fn begin_invocation(
        &self,
        assignment: &AssignmentRecord,
        invocation: &InvocationRecord,
    ) -> Result<()> {
        self.begin_invocation_with_grants(assignment, invocation, &[])
    }

    /// Commit explicit invocation identities and grants together. The ordinal
    /// must be exactly the next unspent one; this API does not import history.
    pub fn begin_invocation_with_grants(
        &self,
        assignment: &AssignmentRecord,
        invocation: &InvocationRecord,
        grants: &[GrantRecord],
    ) -> Result<()> {
        self.begin_invocation_inner(assignment, &mut invocation.clone(), false, grants)
    }

    pub fn admit_invocation(
        &self,
        assignment: &AssignmentRecord,
        invocation: InvocationRecord,
    ) -> Result<InvocationRecord> {
        self.admit_invocation_with_grants(assignment, invocation, &[])
    }

    /// Allocate an ordinal, reserve budget, and issue grants in one transaction.
    pub fn admit_invocation_with_grants(
        &self,
        assignment: &AssignmentRecord,
        mut invocation: InvocationRecord,
        grants: &[GrantRecord],
    ) -> Result<InvocationRecord> {
        self.begin_invocation_inner(assignment, &mut invocation, true, grants)?;
        Ok(invocation)
    }

    fn begin_invocation_inner(
        &self,
        assignment: &AssignmentRecord,
        invocation: &mut InvocationRecord,
        allocate_turn: bool,
        grants: &[GrantRecord],
    ) -> Result<()> {
        ensure!(
            assignment.grant_ids.iter().eq(grants.iter().map(|g| &g.id)),
            "Assignment grant identities disagree"
        );
        ensure!(
            !assignment.id.is_empty() && !invocation.id.is_empty() && invocation.turn > 0,
            "Missing invocation identity"
        );
        ensure!(
            assignment.id == invocation.assignment_id
                && assignment.session_id == invocation.session_id,
            "Invocation assignment scope mismatch"
        );
        ensure!(
            assignment.requested == invocation.requested,
            "Requested settings disagree"
        );
        ensure!(
            assignment.state == InvocationState::Running
                && invocation.state == InvocationState::Running
                && assignment.ended_at.is_none()
                && invocation.ended_at.is_none(),
            "New invocations must be running"
        );
        ensure!(
            invocation.usage.is_none() && invocation.terminal_reason.is_none(),
            "New invocation cannot contain terminal accounting"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let session: Session = record(&tx, "sessions", &assignment.session_id)?;
        ensure!(
            session
                .team
                .iter()
                .any(|agent| agent.id == assignment.agent_id
                    && agent.provider == assignment.provider_id),
            "Assignment agent/provider is not in the captured session team"
        );
        if let Some(team) = super::allocation::state(&tx, &assignment.session_id)? {
            ensure!(
                team.current_members.contains(&assignment.agent_id)
                    && team.eligible_agents.contains(&assignment.agent_id),
                "ineligible_member: assignment requires current admitted membership"
            );
        }
        if let Some(task) = &assignment.task {
            validate_task(&tx, &assignment.session_id, task, true)?;
        }
        if !grants.is_empty() {
            for current in records::<AssignmentRecord>(&tx, "assignments", &assignment.session_id)?
            {
                ensure!(
                    current.state != InvocationState::Running
                        || current.agent_id != assignment.agent_id,
                    "Agent already has an active assignment"
                );
                ensure!(
                    current.state != InvocationState::Running
                        || assignment.purpose != "execute"
                        || current.purpose != "execute"
                        || assignment.task != current.task
                        || assignment.task.is_none(),
                    "Task attempt already has an executor"
                );
            }
            if assignment.purpose == "execute" {
                let reference = assignment
                    .task
                    .as_ref()
                    .context("Execution assignment requires a task")?;
                let task: Task = record(&tx, "tasks", &reference.task_id)?;
                ensure!(
                    task.state == TaskState::Running
                        && task.assignee.as_ref() == Some(&assignment.agent_id),
                    "Execution claim is stale or belongs to another agent"
                );
            }
        }
        if let Some(denial) = super::budget::denial(&tx, assignment)? {
            super::budget::record_denial(&tx, &assignment.session_id, &denial)?;
            tx.commit()?;
            return Err(denial.into());
        }
        let next_turn =
            super::usage::invocation_count(&tx, &assignment.session_id, session.turns_used as u64)?
                .checked_add(1)
                .context("Invocation ordinal exhausted")?;
        if allocate_turn {
            invocation.turn = next_turn;
        } else {
            ensure!(
                invocation.turn == next_turn,
                "invocation_ordinal: explicit admission requires next unspent ordinal {next_turn}"
            );
        }
        for grant in grants {
            super::authority::issue(&tx, assignment, invocation, grant)?;
        }
        tx.execute(
            "INSERT INTO assignments(id,session_id,data) VALUES (?,?,?)",
            params![
                assignment.id,
                assignment.session_id,
                serde_json::to_string(assignment)?
            ],
        )?;
        tx.execute(
            "INSERT INTO invocations(id,session_id,assignment_id,turn,data) VALUES (?,?,?,?,?)",
            params![
                invocation.id,
                invocation.session_id,
                invocation.assignment_id,
                invocation.turn,
                serde_json::to_string(invocation)?
            ],
        )?;
        tx.execute(
            "INSERT INTO token_usage(session_id,turn,agent,status) VALUES (?,?,?,'running')",
            params![invocation.session_id, invocation.turn, assignment.agent_id],
        )?;
        // The ordinal and accounting start commit together, including after a
        // crash before the in-memory session's turn count can be saved.
        super::usage::advance_invocation_count(&tx, &invocation.session_id, invocation.turn)?;
        event(
            &tx,
            &assignment.session_id,
            &ProvenanceEvent::AssignmentStarted {
                assignment: Box::new(assignment.clone()),
                invocation: Box::new(invocation.clone()),
            },
        )?;
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'turn_started',?,?)",
            params![
                assignment.session_id,
                serde_json::json!({
                    "agent": assignment.agent_id, "purpose": assignment.purpose,
                    "turn": invocation.turn, "cwd": assignment.cwd, "task": assignment.task,
                    "assignment_id": assignment.id, "invocation_id": invocation.id,
                })
                .to_string(),
                invocation.started_at
            ],
        )?;
        super::budget::record_state(
            &tx,
            &assignment.session_id,
            &invocation.id,
            "budget_reserved",
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn invocation(&self, session: &str, id: &str) -> Result<InvocationRecord> {
        let value: InvocationRecord = record(&*self.db()?, "invocations", id)?;
        ensure!(
            value.session_id == session,
            "Invocation belongs to another session"
        );
        Ok(value)
    }

    pub fn observe_invocation(
        &self,
        session: &str,
        id: &str,
        observation: &InvocationObservation,
    ) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let mut invocation: InvocationRecord = record(&tx, "invocations", id)?;
        ensure!(
            invocation.session_id == session,
            "Invocation belongs to another session"
        );
        ensure!(
            invocation.state == InvocationState::Running,
            "Terminal invocation cannot be changed"
        );
        if let Some(settings) = &observation.sent {
            merge_settings(&mut invocation.sent, settings);
        }
        if let Some(settings) = &observation.reported {
            merge_settings(&mut invocation.reported, settings);
        }
        if let Some(id) = &observation.native_session_id {
            invocation.native_session_id = Some(id.clone());
        }
        if let Some(id) = &observation.native_turn_id {
            invocation.native_turn_id = Some(id.clone());
        }
        if let Some(version) = &observation.native_version {
            invocation.native_version = Some(version.clone());
        }
        if let Some(usage) = &observation.usage {
            if invocation
                .usage
                .as_ref()
                .is_some_and(|old| old.finalized && old != usage)
            {
                bail!("Finalized invocation usage cannot be replaced");
            }
            if let Some(previous) = &invocation.usage {
                for (old, new) in [
                    (previous.counts.input, usage.counts.input),
                    (previous.counts.output, usage.counts.output),
                ] {
                    ensure!(
                        old.is_none_or(|old| new.is_some_and(|new| new >= old)),
                        "Observed token spend cannot be retracted"
                    );
                }
            }
            invocation.usage = Some(usage.clone());
            tx.execute(
                "UPDATE token_usage SET snapshot=? WHERE session_id=? AND turn=?",
                params![serde_json::to_string(usage)?, session, invocation.turn],
            )?;
        }
        update(&tx, "invocations", id, &invocation)?;
        event(
            &tx,
            session,
            &ProvenanceEvent::InvocationObserved {
                invocation_id: id.into(),
                observation: Box::new(observation.clone()),
            },
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn finish_invocation(
        &self,
        session: &str,
        id: &str,
        state: InvocationState,
        reason: Option<&str>,
    ) -> Result<()> {
        ensure!(
            state != InvocationState::Running,
            "Invocation must finish in a terminal state"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        finish(&tx, session, id, state, reason)?;
        tx.commit()?;
        Ok(())
    }

    /// Call only while owning the session lock and after prior processes ended.
    /// Recovery never labels an uncertain native invocation as successful.
    pub fn interrupt_open_invocations(&self, session: &str) -> Result<usize> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let invocations: Vec<InvocationRecord> = records(&tx, "invocations", session)?;
        let mut count = 0;
        for invocation in invocations
            .into_iter()
            .filter(|i| i.state == InvocationState::Running)
        {
            finish(
                &tx,
                session,
                &invocation.id,
                InvocationState::Interrupted,
                Some("Runtime recovered an invocation without an observed terminal result"),
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn record_decision(&self, value: &DecisionRecord) -> Result<()> {
        ensure!(
            value.kind != "reputation_observed",
            "Reputation records require the qualified observation API"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        decision(&tx, value)?;
        tx.commit()?;
        Ok(())
    }

    pub fn save_plan_with_decision(&self, tasks: &[Task], value: &DecisionRecord) -> Result<()> {
        ensure!(
            value.kind == "plan_committed"
                && tasks
                    .iter()
                    .all(|t| t.state == TaskState::Ready && t.attempts == 0),
            "Plan commitment requires unexecuted ready tasks and a plan decision"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        ensure!(
            !tasks.is_empty() && tasks.iter().all(|t| t.session_id == value.session_id),
            "Plan tasks must share the decision session"
        );
        ensure!(
            tasks
                .iter()
                .map(|task| &task.id)
                .eq(&value.links.related_task_ids),
            "Plan decision must link every committed task in reviewed order"
        );
        let version = value
            .links
            .plan_proposal
            .as_ref()
            .context("Plan commitment requires the reviewed proposal version")?;
        version.plan.validate()?;
        ensure!(
            tasks.len() == version.plan.tasks.len(),
            "Committed task count differs from the reviewed plan"
        );
        for (index, (task, reviewed)) in tasks.iter().zip(&version.plan.tasks).enumerate() {
            ensure!(
                task.title == reviewed.title
                    && task.description == reviewed.description
                    && task.competence == reviewed.competence
                    && task.difficulty == reviewed.difficulty,
                "Committed task definition differs from reviewed task {index}"
            );
            ensure!(
                task.checks == reviewed.checks,
                "Committed checks differ from reviewed task {index}"
            );
            // Plan validation and the count check make every index valid. Map
            // through this exact ordered task list, including forward edges.
            ensure!(
                task.dependencies.iter().eq(reviewed
                    .dependencies
                    .iter()
                    .map(|&dependency| &tasks[dependency].id)),
                "Committed dependencies differ from reviewed task {index}"
            );
        }
        super::write_plan(&tx, tasks)?;
        decision(&tx, value)?;
        tx.commit()?;
        Ok(())
    }

    /// Commit submission or review against the current task attempt. This records
    /// a decision; acceptance and confirmation validation belongs to the runtime.
    pub fn save_task_with_decision(&self, task: &Task, value: &DecisionRecord) -> Result<()> {
        ensure!(
            value.session_id == task.session_id
                && value.links.task.as_ref() == Some(&TaskAttemptRef::from(task)),
            "Decision must reference the exact task attempt"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let previous: Task = record(&tx, "tasks", &task.id)?;
        ensure!(
            previous.session_id == task.session_id,
            "Task belongs to another session"
        );
        ensure!(
            previous.attempts == task.attempts && previous.assignee == task.assignee,
            "Task attempt or assignee changed before the decision was committed"
        );
        ensure!(
            matches!(
                (previous.state, task.state),
                (TaskState::Running, TaskState::Review)
                    | (
                        TaskState::Review,
                        TaskState::Accepted | TaskState::Ready | TaskState::Blocked
                    )
            ),
            "Task state changed before the decision was committed"
        );
        ensure!(
            matches!(
                (task.state, value.kind.as_str()),
                (TaskState::Review, "result_submitted")
                    | (TaskState::Accepted, "task_accepted")
                    | (TaskState::Ready | TaskState::Blocked, "task_rejected")
            ),
            "Task state and decision kind disagree"
        );
        ensure!(
            task.state != TaskState::Accepted
                || matches!(value.outcome, Some(DecisionOutcome::Accepted { .. })),
            "Accepted state requires accepted outcome"
        );
        ensure!(
            task.state != TaskState::Accepted || task.reviewer == value.actor,
            "Accepted state reviewer mismatch"
        );
        validate_links(&tx, value)?;
        super::write_task(&tx, task)?;
        decision(&tx, value)?;
        tx.commit()?;
        Ok(())
    }

    /// A consistent read-only snapshot of current records and their complete
    /// append-only history. Legacy events are retained in their original shape.
    pub fn trace(&self, session: &str) -> Result<SessionTrace> {
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let current: Session = record(&tx, "sessions", session)?;
        let policy: Option<String> = tx
            .query_row(
                "SELECT data FROM session_policies WHERE session_id=?",
                [session],
                |r| r.get(0),
            )
            .optional()?;
        let mut query = tx.prepare("SELECT seq,session_id,kind,data,created_at FROM events WHERE session_id=? ORDER BY seq")?;
        let mut history = Vec::new();
        let mut rows = query.query([session])?;
        while let Some(row) = rows.next()? {
            let raw: String = row.get(3)?;
            history.push(HistoryEvent {
                seq: row.get(0)?,
                session_id: row.get(1)?,
                kind: row.get(2)?,
                data: serde_json::from_str(&raw).context("Invalid historical event JSON")?,
                created_at: row.get(4)?,
            });
        }
        let trace = SessionTrace {
            schema_version: 1,
            session: current,
            policy: policy.map(|raw| serde_json::from_str(&raw)).transpose()?,
            tasks: records(&tx, "tasks", session)?,
            assignments: records(&tx, "assignments", session)?,
            invocations: records(&tx, "invocations", session)?,
            decisions: records(&tx, "decisions", session)?,
            usage: super::usage::session_usage(&tx, session)?,
            budget: super::budget::snapshot(&tx, session)?,
            team_state: super::allocation::state(&tx, session)?,
            history,
        };
        drop(rows);
        drop(query);
        tx.commit()?;
        Ok(trace)
    }
}

fn merge_settings(current: &mut ExecutionSettings, observed: &ExecutionSettings) {
    if observed.model.is_some() {
        current.model.clone_from(&observed.model);
    }
    if observed.effort.is_some() {
        current.effort.clone_from(&observed.effort);
    }
    if observed.permission_mode.is_some() {
        current
            .permission_mode
            .clone_from(&observed.permission_mode);
    }
}

pub(super) fn finish(
    tx: &Transaction<'_>,
    session: &str,
    id: &str,
    state: InvocationState,
    reason: Option<&str>,
) -> Result<()> {
    let mut invocation: InvocationRecord = record(tx, "invocations", id)?;
    ensure!(
        invocation.session_id == session,
        "Invocation belongs to another session"
    );
    ensure!(
        invocation.state == InvocationState::Running,
        "Invocation already ended"
    );
    let mut assignment: AssignmentRecord = record(tx, "assignments", &invocation.assignment_id)?;
    invocation.state = state;
    invocation.ended_at = Some(now());
    invocation.terminal_reason = reason.map(str::to_owned);
    // A native error can still have complete usage; absent/unfinalized reports
    // remain incomplete. Never turn a partial report into a complete one.
    if let Some(usage) = &mut invocation.usage {
        if !usage.finalized {
            usage.partial = true;
        }
    }
    super::authority::revoke(tx, &assignment, state.as_str())?;
    assignment.state = state;
    assignment.ended_at.clone_from(&invocation.ended_at);
    update(tx, "invocations", id, &invocation)?;
    update(tx, "assignments", &assignment.id, &assignment)?;
    tx.execute(
        "UPDATE token_usage SET status=?,snapshot=? WHERE session_id=? AND turn=?",
        params![
            state.as_str(),
            invocation
                .usage
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            session,
            invocation.turn
        ],
    )?;
    super::budget::record_state(tx, session, id, "budget_released")?;
    event(
        tx,
        session,
        &ProvenanceEvent::InvocationFinished {
            invocation: Box::new(invocation),
        },
    )?;
    Ok(())
}
