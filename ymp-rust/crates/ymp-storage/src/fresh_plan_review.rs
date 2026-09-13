use super::*;
use anyhow::ensure;
use rusqlite::{Connection, Transaction};

fn key(command: &FreshPlanReviewCommand) -> String {
    format!(
        "fresh_plan_review:{}",
        content_digest(
            &serde_json::to_string(&(&command.session_id, &command.command_id))
                .expect("serializable command")
        )
    )
}
fn stage(db: &Connection, session: &str, id: &str) -> Result<RecoveryStage> {
    let raw: String = db.query_row(
        "SELECT value FROM kv WHERE key=?",
        [super::recovery::key(session, id)],
        |r| r.get(0),
    )?;
    Ok(serde_json::from_str(&raw)?)
}
fn current_proposal(db: &Connection, session: &str, proposal: &PlanVersion) -> Result<()> {
    let decisions = provenance::records::<DecisionRecord>(db, "decisions", session)?;
    ensure!(
        decisions
            .iter()
            .any(|d| d.kind == "plan_proposed" && d.links.plan_proposal.as_ref() == Some(proposal))
            && !decisions
                .iter()
                .filter(|d| d.kind == "plan_proposed")
                .filter_map(|d| d.links.plan_proposal.as_ref())
                .any(|p| p.proposal_id == proposal.proposal_id && p.revision > proposal.revision),
        "stale_fresh_review: proposal is foreign, changed or superseded"
    );
    Ok(())
}
fn check(db: &Connection, command: &FreshPlanReviewCommand) -> Result<RecoveryStage> {
    let stage = stage(db, &command.session_id, &command.stage_id)?;
    ensure!(
        stage.revision == command.expected_revision
            && stage.plan.as_ref() == Some(&command.proposal)
            && stage.purpose == "review_plan"
            && stage.task.is_none()
            && stage.result.is_none(),
        "stale_fresh_review: expected stage/proposal does not match"
    );
    ensure!(
        !matches!(
            stage.status,
            RecoveryStatus::Complete | RecoveryStatus::Running | RecoveryStatus::Paused
        ) && stage.wait_reason != Some(RecoveryWaitReason::OwnerWait),
        "owner_hold: fresh review cannot lift a stage hold or active responsibility"
    );
    ensure!(
        !stage.failures.is_empty() && stage.fresh_plan_review.is_none(),
        "fresh_review_unavailable: no outstanding failed review"
    );
    current_proposal(db, &stage.session_id, &command.proposal)?;
    let assignments =
        provenance::records::<AssignmentRecord>(db, "assignments", &stage.session_id)?;
    ensure!(
        provenance::records::<Task>(db, "tasks", &stage.session_id)?.is_empty()
            && !assignments
                .iter()
                .any(|a| a.purpose == "execute" || a.state == InvocationState::Running),
        "active_responsibility: fresh saved-plan review is pre-execution only"
    );
    let decisions = provenance::records::<DecisionRecord>(db, "decisions", &stage.session_id)?;
    ensure!(
        !decisions.iter().any(|d| d.kind == "plan_review"
            && d.links.plan_proposal.as_ref() == Some(&command.proposal)),
        "existing_verdict: revision or consumption is required, not another reviewer"
    );
    ensure!(
        !stage
            .failures
            .iter()
            .any(|f| f.agent_id == command.reviewer_id)
            && !assignments
                .iter()
                .any(|a| a.id == command.proposal.producer_assignment_id
                    && a.agent_id == command.reviewer_id),
        "fresh_review_independence: author or failed participant cannot perform the new review"
    );
    for failure in &stage.failures {
        let (assignment, invocation) =
            super::recovery_inspection::terminal_failure(db, &stage, failure)?;
        let access = decisions
            .iter()
            .find(|d| {
                d.kind == "workspace_access_admitted"
                    && d.links.assignment_id.as_ref() == Some(&assignment.id)
                    && d.links.invocation_id.as_ref() == Some(&invocation.id)
            })
            .and_then(|d| d.links.workspace_access.as_ref())
            .context("unreleased_ownership: missing prior admitted access")?;
        ensure!(
            access.directory == assignment.cwd.canonicalize()?
                && invocation.execution_backend.as_ref() == Some(&access.backend)
                && access.effective_access == failure.effective_access
                && decisions
                    .iter()
                    .any(|d| d.kind == "workspace_access_released"
                        && d.links
                            .workspace_access
                            .as_ref()
                            .is_some_and(|a| a.reservation_id == access.reservation_id)),
            "unreleased_ownership: prior execution/access must be known ended and released"
        );
    }
    super::team_control::check_admission(db, &stage.session_id, &command.reviewer_id)?;
    Ok(stage)
}

pub(super) fn check_admission(
    tx: &Transaction<'_>,
    assignment: &AssignmentRecord,
    invocation: &InvocationRecord,
) -> Result<()> {
    let raw: Option<String> = tx
        .query_row(
            "SELECT value FROM kv WHERE key=?",
            [format!("fresh_plan_review_admission:{}", assignment.id)],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(raw) = raw {
        let command: FreshPlanReviewCommand = serde_json::from_str(&raw)?;
        check(tx, &command)?;
        ensure!(
            assignment.session_id == command.session_id
                && assignment.purpose == "review_plan"
                && assignment.agent_id == command.reviewer_id
                && invocation.resumed_from.is_none(),
            "fresh_review_binding: invocation must be independent and use a fresh native context"
        );
        let attempt_key = format!("fresh_plan_review_attempt:{}", key(&command));
        let old: Option<String> = tx
            .query_row("SELECT value FROM kv WHERE key=?", [&attempt_key], |r| {
                r.get(0)
            })
            .optional()?;
        ensure!(
            old.is_none(),
            "fresh_review_already_attempted: command already owns a native invocation"
        );
        tx.execute(
            "INSERT INTO kv(key,value) VALUES (?,?)",
            params![
                attempt_key,
                serde_json::to_string(&(command, &invocation.id))?
            ],
        )?;
    }
    Ok(())
}
pub(super) fn saved_verdict(
    db: &Connection,
    stage: &RecoveryStage,
) -> Result<Option<DecisionRecord>> {
    let Some(saved) = &stage.fresh_plan_review else {
        return Ok(None);
    };
    let record: DecisionRecord = provenance::record(db, "decisions", &saved.review_record_id)?;
    let fresh = record
        .links
        .fresh_plan_review
        .as_ref()
        .context("Missing fresh-review origin")?;
    ensure!(
        record.session_id == stage.session_id
            && record.kind == "plan_review"
            && fresh.command.session_id == stage.session_id
            && fresh.command.stage_id == stage.id
            && stage.plan.as_ref() == Some(&fresh.command.proposal)
            && record.links.plan_proposal == stage.plan
            && fresh.prior_failures == stage.failures,
        "stale_fresh_review: recorded verdict no longer matches the saved obligation"
    );
    current_proposal(db, &stage.session_id, &fresh.command.proposal)?;
    Ok(Some(record))
}
impl Store {
    pub fn fresh_plan_review_receipt(
        &self,
        command: &FreshPlanReviewCommand,
    ) -> Result<Option<FreshPlanReviewReceipt>> {
        let receipt = self
            .value(&key(command))?
            .map(serde_json::from_value::<FreshPlanReviewReceipt>)
            .transpose()?;
        if let Some(receipt) = &receipt {
            ensure!(
                receipt.record.command == *command,
                "owner_command_conflict: fresh-review ID has different content"
            );
        } else if let Some(value) =
            self.value(&format!("fresh_plan_review_attempt:{}", key(command)))?
        {
            let (prior, invocation): (FreshPlanReviewCommand, String) =
                serde_json::from_value(value)?;
            ensure!(
                prior == *command,
                "owner_command_conflict: attempted fresh-review ID has different content"
            );
            bail!("fresh_review_already_attempted: command owns invocation {invocation}; its recorded outcome must be examined before another explicit command");
        }
        Ok(receipt)
    }
    pub fn validate_fresh_plan_review(
        &self,
        command: &FreshPlanReviewCommand,
    ) -> Result<RecoveryStage> {
        check(&*self.db()?, command)
    }
    pub fn bind_fresh_plan_review_assignment(
        &self,
        id: &str,
        command: &FreshPlanReviewCommand,
    ) -> Result<()> {
        self.db()?.execute(
            "INSERT INTO kv(key,value) VALUES (?,?)",
            params![
                format!("fresh_plan_review_admission:{id}"),
                serde_json::to_string(command)?
            ],
        )?;
        Ok(())
    }
    pub fn commit_fresh_plan_review(
        &self,
        record: &DecisionRecord,
    ) -> Result<FreshPlanReviewReceipt> {
        let fresh = record
            .links
            .fresh_plan_review
            .as_ref()
            .context("Missing fresh-review result")?;
        let command = &fresh.command;
        ensure!(
            !command.command_id.trim().is_empty(),
            "owner_command: fresh-review ID required"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut stage = check(&tx, command)?;
        let response = &fresh.response;
        ensure!(
            fresh.prior_failures == stage.failures
                && response.agent_id == command.reviewer_id
                && record.session_id == command.session_id
                && record.actor.as_ref() == Some(&command.reviewer_id)
                && record.kind == "plan_review"
                && record.links.plan_proposal.as_ref() == Some(&command.proposal),
            "fresh_review_binding: foreign or changed result"
        );
        let assignment: AssignmentRecord =
            provenance::record(&tx, "assignments", &response.assignment_id)?;
        let invocation: InvocationRecord =
            provenance::record(&tx, "invocations", &response.invocation_id)?;
        ensure!(
            assignment.session_id == stage.session_id
                && invocation.session_id == stage.session_id
                && invocation.assignment_id == assignment.id
                && assignment.agent_id == command.reviewer_id
                && assignment.purpose == "review_plan"
                && assignment.state == InvocationState::Completed
                && invocation.state == InvocationState::Completed
                && invocation.resumed_from.is_none(),
            "fresh_review_binding: incomplete, foreign or resumed native review"
        );
        let raw: String = tx.query_row(
            "SELECT value FROM kv WHERE key=?",
            [format!("fresh_plan_review_admission:{}", assignment.id)],
            |r| r.get(0),
        )?;
        ensure!(
            serde_json::from_str::<FreshPlanReviewCommand>(&raw)? == *command,
            "fresh_review_binding: admission belongs to another command"
        );
        let decisions = provenance::records::<DecisionRecord>(&tx, "decisions", &stage.session_id)?;
        ensure!(
            decisions
                .iter()
                .any(|d| d.kind == "workspace_access_admitted"
                    && d.links.invocation_id.as_ref() == Some(&invocation.id)
                    && d.links
                        .workspace_access
                        .as_ref()
                        .is_some_and(|a| a.effective_access.is_read_only())),
            "fresh_review_access: actual read-only access is required"
        );
        super::recovery_inspection::check_response(
            &tx,
            &stage.session_id,
            response,
            "review_plan",
        )?;
        let review = parse_response::<Review>(&response.text)?;
        ensure!(
            review.approved == fresh.approved
                && review.reason == fresh.reason
                && record.reason == fresh.reason
                && record.outcome
                    == Some(if review.approved {
                        DecisionOutcome::Accepted {
                            confirmation: ConfirmationStatus::Unconfirmed,
                        }
                    } else {
                        DecisionOutcome::Rejected
                    }),
            "fresh_review_binding: verdict differs from the native response"
        );
        provenance::decision(&tx, record)?;
        stage.fresh_plan_review = Some(FreshPlanReviewState {
            review_record_id: record.id.clone(),
            next_action: FreshPlanReviewNextAction::AwaitOwnerContinuation,
        });
        stage.manual_permit = false;
        stage.status = RecoveryStatus::Waiting;
        stage.wait_reason = Some(RecoveryWaitReason::FreshPlanReviewRecorded);
        stage.condition = Some("A fresh independent plan verdict is recorded; explicit continuation may consume it, while historical effects remain unresolved".into());
        stage.review_ids.push(record.id.clone());
        stage.revision += 1;
        stage.updated_at = now();
        tx.execute(
            "UPDATE kv SET value=? WHERE key=?",
            params![
                serde_json::to_string(&stage)?,
                super::recovery::key(&stage.session_id, &stage.id)
            ],
        )?;
        let receipt = FreshPlanReviewReceipt {
            record: *fresh.clone(),
            review_record_id: record.id.clone(),
            resulting_revision: stage.revision,
            next_action: FreshPlanReviewNextAction::AwaitOwnerContinuation,
        };
        tx.execute(
            "INSERT INTO kv(key,value) VALUES (?,?)",
            params![key(command), serde_json::to_string(&receipt)?],
        )?;
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'recovery_stage',?,?)",
            params![stage.session_id, serde_json::to_string(&stage)?, now()],
        )?;
        tx.commit()?;
        Ok(receipt)
    }
    pub fn unresolved_fresh_plan_dependencies(
        &self,
        session: &str,
    ) -> Result<Option<UnresolvedEffectDependencies>> {
        for stage in self.recovery_stages(session)? {
            if stage.fresh_plan_review.is_some()
                && !super::recovery_inspection::replay_safe(&*self.db()?, &stage)?
            {
                return Ok(Some(UnresolvedEffectDependencies {
                    session_id: session.into(),
                    stage_id: stage.id,
                    proposal: stage.plan.context("Fresh review has no saved proposal")?,
                    invocation_ids: stage
                        .failures
                        .iter()
                        .filter(|f| !f.effective_access.is_read_only())
                        .map(|f| f.invocation_id.clone())
                        .collect(),
                }));
            }
        }
        Ok(None)
    }
}
