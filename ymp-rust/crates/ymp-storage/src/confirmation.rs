use super::{
    provenance::{record, records},
    Store,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection};
use ymp_core::*;

fn root(db: &Connection, session: &str) -> Result<std::path::PathBuf> {
    let session: Session = record(db, "sessions", session)?;
    let project: Project = record(db, "projects", &session.project_id)?;
    Ok(project.path)
}

fn current_task(db: &Connection, result: &ResultVersion) -> Result<bool> {
    let Some(reference) = &result.task else {
        return Ok(false);
    };
    let task: Task = record(db, "tasks", &reference.task_id)?;
    let producer: AssignmentRecord = record(
        db,
        "assignments",
        result
            .producer_assignment_ids
            .first()
            .context("Missing result producer")?,
    )?;
    Ok(task.attempts == result.version
        && task.assignee.as_ref() == Some(&producer.agent_id)
        && producer.task == result.task
        && result.task_definition.as_ref() == Some(&TaskDefinition::from(&task))
        && task.result.as_deref() == Some(result.summary.as_str()))
}

fn current_aggregate(db: &Connection, session: &str, result: &ResultVersion) -> Result<bool> {
    if result.task.is_some() || result.task_definition.is_some() || result.component_ids.is_empty()
    {
        return Ok(false);
    }
    let tasks: Vec<Task> = records(db, "tasks", session)?;
    let components = result
        .component_ids
        .iter()
        .map(|id| record::<DecisionRecord>(db, "decisions", id))
        .collect::<Result<Vec<_>>>()?;
    if tasks.len() != components.len() {
        return Ok(false);
    }
    for task in tasks {
        if task.state != TaskState::Accepted {
            return Ok(false);
        }
        let Some(component) = components.iter().find(|d| {
            d.session_id == session
                && d.kind == "result_submitted"
                && d.links.task.as_ref() == Some(&TaskAttemptRef::from(&task))
        }) else {
            return Ok(false);
        };
        if !current_task(
            db,
            component
                .links
                .result
                .as_ref()
                .context("Missing component result")?,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn current_files(db: &Connection, session: &str, result: &ResultVersion) -> Result<bool> {
    let directory = root(db, session)?;
    if !result.artifacts.iter().all(|a| a.current(&directory)) {
        return Ok(false);
    }
    if let Some(id) = &result.contract_id {
        let contract: DecisionRecord = record(db, "decisions", id)?;
        if !contract
            .links
            .acceptance_contract
            .context("Missing captured contract")?
            .inputs
            .iter()
            .all(|a| a.current(&directory))
        {
            return Ok(false);
        }
    }
    for id in &result.component_ids {
        let component: DecisionRecord = record(db, "decisions", id)?;
        if !current_files(
            db,
            session,
            component
                .links
                .result
                .as_ref()
                .context("Missing component result")?,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn known_result(db: &Connection, session: &str, result: &ResultVersion) -> Result<()> {
    let decisions: Vec<DecisionRecord> = records(db, "decisions", session)?;
    ensure!(
        decisions.iter().any(|d| matches!(
            d.kind.as_str(),
            "result_submitted" | "result_aggregated"
        ) && d.links.result.as_ref() == Some(result)),
        "Cannot inspect an unknown or altered result version"
    );
    Ok(())
}

pub(super) fn validate_plain_task_write(db: &Connection, task: &Task) -> Result<()> {
    let tasks: Vec<Task> = records(db, "tasks", &task.session_id)?;
    ensure!(!tasks.iter().any(|old| old.id == task.id && old.state == TaskState::Accepted), "An accepted attempt cannot be reopened by a plain task write; submit a new task/result version");
    let decisions: Vec<DecisionRecord> = records(db, "decisions", &task.session_id)?;
    for result in decisions
        .iter()
        .filter(|d| {
            d.kind == "result_submitted"
                && d.links.task.as_ref() == Some(&TaskAttemptRef::from(task))
        })
        .filter_map(|d| d.links.result.as_ref())
    {
        ensure!(
            result.task_definition.as_ref() == Some(&TaskDefinition::from(task)),
            "Submitted task definition is immutable for this attempt"
        );
        if task.state == TaskState::Review {
            ensure!(
                task.result.as_deref() == Some(result.summary.as_str()),
                "Submitted result text is immutable for this attempt"
            );
        }
    }
    Ok(())
}

pub(super) fn grade(
    db: &Connection,
    session: &str,
    result: &ResultVersion,
) -> Result<(ConfirmationStatus, Vec<String>, bool)> {
    let all: Vec<DecisionRecord> = records(db, "decisions", session)?;
    let mut covered = std::collections::HashSet::new();
    let mut ids = Vec::new();
    let mut failed = false;
    for record in &all {
        if record.kind == "check_observed" && record.links.result.as_ref() == Some(result) {
            let check = record
                .links
                .check
                .as_ref()
                .context("Check record has no evidence")?;
            match check.outcome {
                CheckOutcome::Failed => failed = true,
                CheckOutcome::Passed => {
                    covered.extend(check.criterion_ids.iter().cloned());
                    ids.push(record.id.clone());
                }
                CheckOutcome::Inconclusive => {}
            }
        }
    }
    let mut complete =
        !result.criteria.is_empty() && result.criteria.iter().all(|c| covered.contains(&c.id));
    if !result.component_ids.is_empty() {
        complete = true;
        for id in &result.component_ids {
            let component: DecisionRecord = record(db, "decisions", id)?;
            let child = component
                .links
                .result
                .as_ref()
                .context("Component result is missing")?;
            let (status, child_ids, child_failed) = grade(db, session, child)?;
            complete &= status == ConfirmationStatus::Confirmed;
            failed |= child_failed;
            ids.extend(child_ids);
        }
    }
    let directory = root(db, session)?;
    complete &= result.artifacts.iter().all(|a| a.current(&directory));
    if let Some(id) = &result.contract_id {
        let contract: DecisionRecord = record(db, "decisions", id)?;
        complete &= contract
            .links
            .acceptance_contract
            .as_ref()
            .context("Missing contract")?
            .inputs
            .iter()
            .all(|s| s.current(&directory));
    }
    ids.sort();
    ids.dedup();
    Ok((
        if complete && !failed {
            ConfirmationStatus::Confirmed
        } else {
            ConfirmationStatus::Unconfirmed
        },
        ids,
        failed,
    ))
}

pub(super) fn validate(db: &Connection, value: &DecisionRecord) -> Result<()> {
    match value.kind.as_str() {
        "task_accepted" | "final_accepted" => ensure!(
            matches!(value.outcome, Some(DecisionOutcome::Accepted { .. })),
            "Accepted decision kind requires an accepted outcome"
        ),
        "task_rejected" | "final_rejected" => ensure!(
            value.outcome == Some(DecisionOutcome::Rejected),
            "Rejected decision kind requires a rejected outcome"
        ),
        "candidate_review" | "candidate_arbitration" | "final_review" => ensure!(
            matches!(
                value.outcome,
                Some(DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Unconfirmed
                }) | Some(DecisionOutcome::Rejected)
            ),
            "Review requires a single unconfirmed assessment outcome"
        ),
        _ => {}
    }
    if let Some(contract) = &value.links.acceptance_contract {
        ensure!(
            value.kind == "acceptance_contract_captured" && value.actor.is_none(),
            "Only runtime contract capture may declare trusted checks"
        );
        ensure!(
            contract.version == contract.digest()?,
            "Contract digest mismatch"
        );
        let fresh = CapturedAcceptanceContract::capture(
            contract.contract.clone(),
            &root(db, &value.session_id)?,
            contract.checker.clone(),
        )?;
        ensure!(
            fresh == *contract,
            "Contract does not match actual initial inputs and verifier code"
        );
        let all: Vec<DecisionRecord> = records(db, "decisions", &value.session_id)?;
        ensure!(
            !all.iter().any(|d| d
                .links
                .acceptance_contract
                .as_ref()
                .is_some_and(|c| c.contract.task_title == contract.contract.task_title)),
            "Task acceptance contract already captured"
        );
        let assignments: Vec<AssignmentRecord> = records(db, "assignments", &value.session_id)?;
        ensure!(
            assignments.is_empty(),
            "Trusted contracts must precede agent invocations"
        );
    }
    let requires_result = matches!(
        value.kind.as_str(),
        "result_submitted"
            | "result_aggregated"
            | "candidate_review"
            | "candidate_arbitration"
            | "final_review"
            | "task_accepted"
            | "task_rejected"
            | "final_accepted"
            | "final_rejected"
            | "check_observed"
    );
    ensure!(
        !requires_result || value.links.result.is_some(),
        "Result transition requires an immutable result version"
    );
    let Some(result) = &value.links.result else {
        return Ok(());
    };
    ensure!(
        !result.id.is_empty()
            && result.version > 0
            && !result.criteria.is_empty()
            && !result.producer_assignment_ids.is_empty(),
        "Incomplete result identity, criteria or producers"
    );
    ensure!(
        result.criteria_version == content_digest(&serde_json::to_string(&result.criteria)?),
        "Criteria version mismatch"
    );
    ensure!(
        result.artifacts.iter().all(FileSnapshot::valid),
        "Artifact digest mismatch"
    );
    ensure!(
        value.links.task == result.task,
        "Result and decision task mismatch"
    );
    let all: Vec<DecisionRecord> = records(db, "decisions", &value.session_id)?;
    for id in &value.links.evidence_ids {
        let evidence: DecisionRecord = record(db, "decisions", id)?;
        ensure!(
            evidence.session_id == value.session_id
                && evidence.kind == "check_observed"
                && evidence.links.result.as_ref() == Some(result),
            "Review evidence belongs to another result or source"
        );
    }
    let submissions = all
        .iter()
        .filter(|d| matches!(d.kind.as_str(), "result_submitted" | "result_aggregated"))
        .collect::<Vec<_>>();
    let existing = submissions.iter().find(|d| {
        d.links
            .result
            .as_ref()
            .is_some_and(|r| r.id == result.id && r.version == result.version)
    });
    if matches!(
        value.kind.as_str(),
        "final_review" | "final_accepted" | "final_rejected"
    ) {
        ensure!(
            current_aggregate(db, &value.session_id, result)?,
            "Final transition requires an aggregate covering every current accepted task"
        );
        ensure!(
            submissions
                .iter()
                .rev()
                .find(|d| d.kind == "result_aggregated")
                .is_some_and(|d| d.links.result.as_ref() == Some(result)),
            "Final transition requires the current recorded aggregate version"
        );
    }
    if matches!(
        value.kind.as_str(),
        "candidate_review" | "candidate_arbitration" | "task_accepted" | "task_rejected"
    ) {
        ensure!(
            result.task.is_some()
                && result.component_ids.is_empty()
                && result.task_definition.is_some(),
            "Candidate transition requires a leaf task result"
        );
    }
    let producing = matches!(
        value.kind.as_str(),
        "result_submitted" | "result_aggregated"
    );
    if producing {
        ensure!(existing.is_none(), "Result version already exists");
        if let Some(id) = &result.contract_id {
            let captured: DecisionRecord = record(db, "decisions", id)?;
            let contract = captured
                .links
                .acceptance_contract
                .context("Missing captured contract")?;
            let task: Task = record(
                db,
                "tasks",
                &result
                    .task
                    .as_ref()
                    .context("Contract needs a task")?
                    .task_id,
            )?;
            ensure!(
                captured.session_id == value.session_id
                    && task.title == contract.contract.task_title
                    && result.criteria == contract.contract.criteria,
                "Result does not match its trusted criteria"
            );
            ensure!(
                result
                    .artifacts
                    .iter()
                    .map(|s| &s.path)
                    .eq(&contract.contract.artifacts),
                "Result artifacts differ from the contract"
            );
        }
        ensure!(
            result
                .artifacts
                .iter()
                .all(|s| s.current(&root(db, &value.session_id).unwrap_or_default())),
            "Submitted artifact version is stale"
        );
    } else {
        ensure!(
            existing.is_some_and(|d| d.links.result.as_ref() == Some(result)),
            "Unknown or altered result version"
        );
    }
    let producers = result
        .producer_assignment_ids
        .iter()
        .map(|id| record::<AssignmentRecord>(db, "assignments", id))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        producers
            .iter()
            .all(|a| a.session_id == value.session_id && a.purpose == "execute"),
        "Result producer scope mismatch"
    );
    if value.kind == "result_submitted" {
        ensure!(
            current_task(db, result)?,
            "Submission requires the exact task definition and result text"
        );
        let task: Task = record(
            db,
            "tasks",
            &result
                .task
                .as_ref()
                .context("Leaf result needs a task")?
                .task_id,
        )?;
        ensure!(
            result.id == task.id
                && result.version == task.attempts
                && result.summary == task.result.clone().unwrap_or_default(),
            "Candidate identity or text differs from current task attempt"
        );
        ensure!(
            producers.len() == 1
                && producers[0].task == result.task
                && value.actor.as_ref() == Some(&producers[0].agent_id)
                && value.links.assignment_id.as_ref() == Some(&producers[0].id),
            "Submission must identify its actual producing assignment"
        );
        ensure!(
            result.component_ids.is_empty(),
            "Leaf submission cannot declare components"
        );
    }
    if value.kind == "result_aggregated" {
        ensure!(
            value.actor.is_none() && result.task.is_none() && result.contract_id.is_none(),
            "Aggregate is runtime-owned"
        );
        let mut expected_producers = Vec::new();
        let mut expected_artifacts = Vec::new();
        let mut expected_criteria = Vec::new();
        for id in &result.component_ids {
            let component: DecisionRecord = record(db, "decisions", id)?;
            ensure!(
                component.session_id == value.session_id && component.kind == "result_submitted",
                "Aggregate has an invalid component"
            );
            let child = component.links.result.context("Missing component result")?;
            ensure!(
                all.iter()
                    .any(|d| d.kind == "task_accepted" && d.links.result.as_ref() == Some(&child)),
                "Aggregate component has not been accepted"
            );
            expected_producers.extend(child.producer_assignment_ids);
            expected_artifacts.extend(child.artifacts);
            expected_criteria.extend(child.criteria.into_iter().map(|mut c| {
                c.id = format!("{}:{}", child.id, c.id);
                c
            }));
        }
        expected_producers.sort();
        expected_producers.dedup();
        ensure!(
            result.producer_assignment_ids == expected_producers
                && result.artifacts == expected_artifacts
                && result.criteria == expected_criteria,
            "Aggregate omits or changes component scope"
        );
        let tasks: Vec<Task> = records(db, "tasks", &value.session_id)?;
        ensure!(
            tasks.len() == result.component_ids.len()
                && tasks.iter().all(|t| t.state == TaskState::Accepted
                    && result
                        .component_ids
                        .iter()
                        .any(|id| all.iter().any(|d| &d.id == id
                            && d.links.task.as_ref() == Some(&TaskAttemptRef::from(t))))),
            "Final aggregate must include every current accepted task"
        );
    }
    if matches!(
        value.kind.as_str(),
        "candidate_review" | "candidate_arbitration" | "final_review"
    ) {
        let actor = value.actor.as_ref().context("Review needs a reviewer")?;
        ensure!(
            !all.iter().any(|d| matches!(
                d.kind.as_str(),
                "candidate_review" | "candidate_arbitration" | "final_review"
            ) && d.links.invocation_id == value.links.invocation_id),
            "A review invocation already has its immutable assessment"
        );
        ensure!(
            producers.iter().all(|p| &p.agent_id != actor),
            "Independent review excludes every result producer"
        );
        let invocation: InvocationRecord = record(
            db,
            "invocations",
            value
                .links
                .invocation_id
                .as_deref()
                .context("Review invocation missing")?,
        )?;
        let assignment: AssignmentRecord = record(db, "assignments", &invocation.assignment_id)?;
        ensure!(
            invocation.state == InvocationState::Completed
                && assignment.agent_id == *actor
                && assignment.purpose
                    == if value.kind == "final_review" {
                        "final_review"
                    } else {
                        "review"
                    }
                && assignment.task == result.task
                && value.links.assignment_id.as_ref() == Some(&assignment.id),
            "Review must identify its completed reviewer invocation"
        );
        let submission = existing.context("Review result submission missing")?;
        let digest = content_digest(&serde_json::to_string(&Some(result))?);
        ensure!(
            assignment
                .context
                .iter()
                .any(|c| c.kind == ContextKind::Result
                    && c.id == submission.id
                    && c.session_id.as_ref() == Some(&value.session_id)
                    && c.digest.as_ref() == Some(&digest)),
            "Review assignment was not bound to this exact result version"
        );
        ensure!(
            !matches!(
                value.outcome,
                Some(DecisionOutcome::Accepted {
                    confirmation: ConfirmationStatus::Confirmed
                })
            ),
            "Agent review is not confirmation"
        );
    }
    if value.kind == "check_observed" {
        ensure!(
            value.actor.is_none(),
            "Only runtime may observe confirmation"
        );
        let evidence = value
            .links
            .check
            .as_ref()
            .context("Missing observed check")?;
        ensure!(
            result.contract_id.as_ref() == Some(&evidence.contract_id),
            "Check contract mismatch"
        );
        let captured: DecisionRecord = record(db, "decisions", &evidence.contract_id)?;
        let contract = captured
            .links
            .acceptance_contract
            .context("Missing trusted check contract")?;
        ensure!(
            evidence.checker == contract.checker,
            "Check implementation differs from captured contract"
        );
        let check = contract
            .contract
            .checks
            .iter()
            .find(|c| c.id == evidence.check_id)
            .context("Undeclared check")?;
        ensure!(
            check.criterion_ids == evidence.criterion_ids
                && evidence.inputs.iter().all(FileSnapshot::valid)
                && evidence.artifacts_after.iter().all(FileSnapshot::valid),
            "Check coverage or capture mismatch"
        );
        ensure!(
            evidence
                .artifacts_after
                .iter()
                .map(|s| &s.path)
                .eq(result.artifacts.iter().map(|s| &s.path))
                && evidence
                    .inputs
                    .iter()
                    .map(|s| &s.path)
                    .eq(contract.inputs.iter().map(|s| &s.path)),
            "Check capture scope mismatch"
        );
        if evidence.outcome == CheckOutcome::Passed {
            let asserted = match &check.assertion {
                CheckAssertion::ExactBytes { artifact, expected } => {
                    result
                        .artifacts
                        .iter()
                        .find(|s| &s.path == artifact)
                        .and_then(|s| s.bytes.as_ref())
                        == Some(expected)
                }
                CheckAssertion::MatchesInput { artifact, input } => {
                    let actual = result
                        .artifacts
                        .iter()
                        .find(|s| &s.path == artifact)
                        .and_then(|s| s.bytes.as_ref());
                    actual.is_some()
                        && actual
                            == contract
                                .inputs
                                .iter()
                                .find(|s| &s.path == input)
                                .and_then(|s| s.bytes.as_ref())
                }
                CheckAssertion::Command { .. } => true,
            };
            ensure!(
                asserted,
                "Checker success contradicts captured typed assertion"
            );
            ensure!(
                evidence.inputs == contract.inputs && evidence.artifacts_after == result.artifacts,
                "Passed evidence refers to changed inputs or artifacts"
            );
            ensure!(
                evidence.artifacts_after.iter().all(|a| a.bytes.is_some())
                    && evidence.exit_code == Some(0),
                "Passed check needs existing artifacts and success"
            );
        }
    }
    if matches!(
        value.kind.as_str(),
        "task_accepted" | "task_rejected" | "final_accepted" | "final_rejected"
    ) {
        if value.kind.starts_with("task_") {
            let task: Task = record(
                db,
                "tasks",
                &result
                    .task
                    .as_ref()
                    .context("Task acceptance needs a task")?
                    .task_id,
            )?;
            ensure!(
                task.attempts == result.version
                    && ((value.kind == "task_accepted" && task.state == TaskState::Accepted)
                        || (value.kind == "task_rejected"
                            && matches!(task.state, TaskState::Ready | TaskState::Blocked))),
                "Acceptance decision must commit with the matching task state"
            );
        }
        if value.kind == "task_accepted" {
            ensure!(
                current_task(db, result)?,
                "Acceptance must preserve the reviewed task definition and result text"
            );
        }
        let review: DecisionRecord = record(
            db,
            "decisions",
            value
                .links
                .review_ids
                .last()
                .context("Acceptance requires independent review")?,
        )?;
        ensure!(
            review.session_id == value.session_id
                && review.links.result.as_ref() == Some(result)
                && if value.kind.starts_with("final_") {
                    review.kind == "final_review"
                } else {
                    matches!(
                        review.kind.as_str(),
                        "candidate_review" | "candidate_arbitration"
                    )
                }
                && value.actor == review.actor
                && value.links.invocation_id == review.links.invocation_id,
            "Acceptance review binding mismatch"
        );
        for id in &value.links.review_ids {
            let linked: DecisionRecord = record(db, "decisions", id)?;
            ensure!(
                linked.links.result.as_ref() == Some(result),
                "Review evidence belongs to another result version"
            );
        }
        let (status, ids, failed) = grade(db, &value.session_id, result)?;
        ensure!(
            value.links.confirmation_ids == ids,
            "Acceptance must link the exact applicable passing evidence"
        );
        if let Some(DecisionOutcome::Accepted { confirmation }) = value.outcome {
            ensure!(
                !failed && matches!(review.outcome, Some(DecisionOutcome::Accepted { .. })),
                "Failed applicable evidence or independent rejection prevents acceptance"
            );
            ensure!(
                confirmation == status,
                "Acceptance grade exceeds criterion coverage"
            );
            ensure!(
                result
                    .artifacts
                    .iter()
                    .all(|s| s.current(&root(db, &value.session_id).unwrap_or_default())),
                "Cannot accept a stale artifact version"
            );
        }
    }
    Ok(())
}

impl Store {
    pub fn confirmation_grade(
        &self,
        session: &str,
        result: &ResultVersion,
    ) -> Result<(ConfirmationStatus, Vec<String>, bool)> {
        let db = self.db()?;
        known_result(&db, session, result)?;
        grade(&db, session, result)
    }

    /// Check persistent artifact/input changes and current task bindings without
    /// turning qualitative acceptance into objective confirmation.
    pub fn result_is_current(&self, session: &str, result: &ResultVersion) -> Result<bool> {
        let db = self.db()?;
        known_result(&db, session, result)?;
        let definitions_current = if result.task.is_some() {
            current_task(&db, result)?
        } else {
            current_aggregate(&db, session, result)?
        };
        Ok(definitions_current && current_files(&db, session, result)?)
    }

    /// Credit a confirmed producing invocation once. Agreement and legacy
    /// observations never enter the qualified selection population.
    pub fn observe_confirmed(
        &self,
        observation: &Observation,
        acceptance_id: &str,
    ) -> Result<bool> {
        ensure!(
            observation.confirmation == ConfirmationStatus::Confirmed,
            "Qualified observation requires confirmed evidence status"
        );
        let mut db = self.db()?;
        let tx = db.transaction()?;
        let acceptance: DecisionRecord = record(&tx, "decisions", acceptance_id)?;
        ensure!(
            acceptance.kind == "task_accepted"
                && acceptance.outcome
                    == Some(DecisionOutcome::Accepted {
                        confirmation: ConfirmationStatus::Confirmed
                    }),
            "Observation requires confirmed task acceptance"
        );
        let result = acceptance
            .links
            .result
            .as_ref()
            .context("Missing observation result")?;
        ensure!(
            grade(&tx, &acceptance.session_id, result)?.0 == ConfirmationStatus::Confirmed,
            "Observation confirmation is stale"
        );
        ensure!(
            result.producer_assignment_ids.len() == 1 && observation.success,
            "Credit requires a single supported producer outcome"
        );
        let producer: AssignmentRecord =
            record(&tx, "assignments", &result.producer_assignment_ids[0])?;
        let invocations: Vec<InvocationRecord> =
            records(&tx, "invocations", &acceptance.session_id)?;
        let invocation = invocations
            .iter()
            .find(|i| i.assignment_id == producer.id)
            .context("Missing producing invocation")?;
        ensure!(
            invocation.state == InvocationState::Completed,
            "Interrupted work is not a competence observation"
        );
        let expected_version = content_digest(&serde_json::to_string(&(
            "effective-execution-v1",
            &producer.agent_config_version,
            &invocation.sent,
            &invocation.reported,
            &invocation.native_version,
        ))?)[..24]
            .to_owned();
        let task: Task = record(
            &tx,
            "tasks",
            &result.task.as_ref().context("Credit needs task")?.task_id,
        )?;
        ensure!(
            task.state == TaskState::Accepted && task.attempts == result.version,
            "Observation task is no longer the accepted current attempt"
        );
        let expected_id = format!(
            "result:{}:{}:{}",
            result.id, result.version, producer.agent_id
        );
        ensure!(
            observation.id == expected_id
                && observation.agent_version == expected_version
                && observation.competence == task.competence
                && observation.difficulty == task.difficulty,
            "Observation attribution mismatch"
        );
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM observations WHERE id=?)",
            [&observation.id],
            |r| r.get::<_, bool>(0),
        )? {
            return Ok(false);
        }
        let decision = DecisionRecord {
            id: new_id(),
            session_id: acceptance.session_id,
            kind: "reputation_observed".into(),
            actor: Some(producer.agent_id),
            reason: "Confirmed producing outcome".into(),
            outcome: None,
            links: RecordLinks {
                task: result.task.clone(),
                result: Some(result.clone()),
                assignment_id: Some(producer.id),
                invocation_id: Some(invocation.id.clone()),
                review_ids: vec![acceptance.id],
                confirmation_ids: acceptance.links.confirmation_ids,
                observation_id: Some(observation.id.clone()),
                ..Default::default()
            },
            created_at: now(),
        };
        super::provenance::decision(&tx, &decision)?;
        tx.execute("INSERT INTO observations(id,agent_version,competence,difficulty,success,data) VALUES (?,?,?,?,?,?)", params![observation.id, observation.agent_version, observation.competence, observation.difficulty, observation.success, serde_json::to_string(observation)?])?;
        tx.commit()?;
        Ok(true)
    }
}
