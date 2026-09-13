use super::*;
use anyhow::ensure;
use rusqlite::{Connection, Transaction};

fn key(command: &ContinueWithCurrentFilesCommand) -> String {
    format!(
        "current_files_command:{}",
        content_digest(
            &serde_json::to_string(&(&command.context.session_id, &command.command_id))
                .expect("serializable owner key")
        )
    )
}
pub(super) fn unknown_failures(db: &Connection, session: &str) -> Result<Vec<InvocationFailure>> {
    let mut failures = provenance::records::<DecisionRecord>(db, "decisions", session)?
        .into_iter()
        .filter_map(|d| d.links.failure)
        .chain(
            super::recovery::stages(db, session)?
                .into_iter()
                .flat_map(|s| s.failures),
        )
        .filter(|f| {
            !f.effective_access.is_read_only() || f.termination != TerminationEvidence::BackendEnded
        })
        .collect::<Vec<_>>();
    failures.sort_by_cached_key(|f| serde_json::to_string(f).expect("serializable failure"));
    failures.dedup();
    for stage in super::recovery::stages(db, session)? {
        if stage.effect_resolution.is_some() && super::recovery_inspection::replay_safe(db, &stage)?
        {
            failures.retain(|f| !stage.failures.contains(f));
        }
    }
    Ok(failures)
}
pub(super) fn authorizations(
    db: &Connection,
    session: &str,
) -> Result<Vec<CurrentFilesAuthorization>> {
    Ok(
        provenance::records::<DecisionRecord>(db, "decisions", session)?
            .into_iter()
            .filter(|d| d.kind == "owner_current_files_authorized")
            .filter_map(|d| d.links.current_files_authorization.map(|a| *a))
            .collect(),
    )
}
pub(super) fn uncovered(db: &Connection, session: &str) -> Result<Vec<InvocationFailure>> {
    let authorizations = authorizations(db, session)?;
    Ok(unknown_failures(db, session)?
        .into_iter()
        .filter(|f| {
            !authorizations
                .iter()
                .any(|a| a.command.context.failures.contains(f))
        })
        .collect())
}
fn validate(db: &Connection, context: &CurrentFilesContext) -> Result<RecoveryStage> {
    let stages = super::recovery::stages(db, &context.session_id)?;
    let stage = stages
        .iter()
        .find(|s| s.id == context.stage_id)
        .context("current_files_context: unknown stage")?
        .clone();
    ensure!(
        stage.revision == context.stage_revision
            && stage.plan == context.plan
            && stage.result == context.result
            && stage.task == context.task,
        "stale_current_files: saved stage/result changed"
    );
    ensure!(
        stage.status != RecoveryStatus::Paused
            && !matches!(
                stage.wait_reason,
                Some(RecoveryWaitReason::OwnerWait | RecoveryWaitReason::OwnerPause)
            ),
        "owner_hold: explicitly release the owner hold before authorizing new work"
    );
    ensure!(
        super::team_control::state(db, &context.session_id)?
            .is_none_or(|s| s.control == OwnerRunControl::Continue),
        "owner_paused: release the session hold before authorizing new work"
    );
    let board = super::board::snapshot(db, &context.session_id)?;
    ensure!(
        super::budget::snapshot(db, &context.session_id)?.as_ref() == Some(&context.budget),
        "stale_current_files: captured usage or resource state changed"
    );
    ensure!(
        board
            .team
            .as_ref()
            .is_some_and(|t| t.revision == context.team_revision)
            && content_digest(&serde_json::to_string(&board)?) == context.board_version,
        "stale_current_files: board or membership changed"
    );
    let raw: String = db.query_row(
        "SELECT data FROM session_policies WHERE session_id=?",
        [&context.session_id],
        |r| r.get(0),
    )?;
    let policy: SessionPolicy = serde_json::from_str(&raw)?;
    ensure!(
        policy.cwd.canonicalize()? == context.directory,
        "current_files_context: foreign workspace"
    );
    ensure!(
        !context.failures.is_empty()
            && unknown_failures(db, &context.session_id)? == context.failures,
        "stale_current_files: acknowledged historical failures changed"
    );
    let decisions = provenance::records::<DecisionRecord>(db, "decisions", &context.session_id)?;
    let assignments =
        provenance::records::<AssignmentRecord>(db, "assignments", &context.session_id)?;
    ensure!(
        !assignments
            .iter()
            .any(|a| a.state == InvocationState::Running),
        "active_responsibility: prior local execution is still active"
    );
    for failure in &context.failures {
        let mut bound = stages
            .iter()
            .find(|s| s.failures.contains(failure))
            .unwrap_or(&stage)
            .clone();
        let assignment = assignments
            .iter()
            .find(|a| a.id == failure.assignment_id)
            .context("Missing historical assignment")?;
        bound.purpose = assignment.purpose.clone();
        bound.task = assignment.task.clone();
        let (_, invocation) = super::recovery_inspection::terminal_failure(db, &bound, failure)?;
        let access = decisions
            .iter()
            .find(|d| {
                d.kind == "workspace_access_admitted"
                    && d.links.invocation_id.as_ref() == Some(&invocation.id)
            })
            .and_then(|d| d.links.workspace_access.as_ref())
            .context("unreleased_ownership: missing prior access")?;
        ensure!(
            access.effective_access == failure.effective_access
                && access.directory == context.directory
                && invocation.execution_backend.as_ref() == Some(&access.backend)
                && decisions
                    .iter()
                    .any(|d| d.kind == "workspace_access_released"
                        && d.links
                            .workspace_access
                            .as_ref()
                            .is_some_and(|r| r.reservation_id == access.reservation_id)),
            "unreleased_ownership: local execution/access must end before authorization"
        );
    }
    Ok(stage)
}
pub(super) fn check_admission(
    tx: &Transaction<'_>,
    assignment: &AssignmentRecord,
    invocation: &InvocationRecord,
) -> Result<()> {
    let authorizations = authorizations(tx, &assignment.session_id)?;
    if authorizations.is_empty()
        && !super::recovery::stages(tx, &assignment.session_id)?
            .iter()
            .any(|s| s.fresh_plan_review.is_some())
    {
        return Ok(());
    }
    let fresh: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM kv WHERE key=? OR key=?)",
        params![
            format!("fresh_plan_review_admission:{}", assignment.id),
            format!("recovery_inspection_admission:{}", assignment.id)
        ],
        |r| r.get(0),
    )?;
    if !fresh && assignment.purpose != "conversation" {
        ensure!(uncovered(tx, &assignment.session_id)?.is_empty(), "unresolved_effect_dependencies: a later uncertain failure has no owner authorization for new work");
    }
    if authorizations.iter().any(|a| {
        a.command
            .context
            .failures
            .iter()
            .any(|f| f.agent_id == assignment.agent_id)
    }) {
        ensure!(
            invocation.resumed_from.is_none(),
            "current_files_context: new work cannot revive an uncertain actor's native context"
        );
    }
    Ok(())
}
impl Store {
    pub fn current_files_failures(&self, session: &str) -> Result<Vec<InvocationFailure>> {
        unknown_failures(&*self.db()?, session)
    }
    pub fn current_files_authorizations(
        &self,
        session: &str,
    ) -> Result<Vec<CurrentFilesAuthorization>> {
        authorizations(&*self.db()?, session)
    }
    pub fn current_files_receipt(
        &self,
        command: &ContinueWithCurrentFilesCommand,
    ) -> Result<Option<CurrentFilesReceipt>> {
        let receipt = self
            .value(&key(command))?
            .map(serde_json::from_value::<CurrentFilesReceipt>)
            .transpose()?;
        if let Some(receipt) = &receipt {
            ensure!(
                receipt.command == *command,
                "owner_command_conflict: current-files command ID has different content"
            );
        }
        Ok(receipt)
    }
    /// The runtime rechecks the reviewed file manifest under project/session locks.
    /// This transaction binds that context to exact state/failures and issues no call.
    pub fn authorize_current_files(
        &self,
        command: &ContinueWithCurrentFilesCommand,
    ) -> Result<CurrentFilesReceipt> {
        ensure!(
            !command.command_id.trim().is_empty(),
            "owner_command: current-files command ID required"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prior: Option<String> = tx
            .query_row("SELECT value FROM kv WHERE key=?", [key(command)], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(prior) = prior {
            let receipt: CurrentFilesReceipt = serde_json::from_str(&prior)?;
            ensure!(
                receipt.command == *command,
                "owner_command_conflict: current-files command ID has different content"
            );
            return Ok(receipt);
        }
        let mut stage = validate(&tx, &command.context)?;
        let id = new_id();
        let authorization = CurrentFilesAuthorization {
            command: command.clone(),
        };
        provenance::decision(&tx, &DecisionRecord { id: id.clone(), session_id: stage.session_id.clone(), kind: "owner_current_files_authorized".into(), actor: None,
            reason: "Owner explicitly authorized new work from the reviewed current files for these historical failures; effects remain unknown, retry and effect-resolution authority are unchanged".into(),
            outcome: None, links: RecordLinks { current_files_authorization: Some(Box::new(authorization)), ..Default::default() }, created_at: now() })?;
        if let Some(fresh) = &mut stage.fresh_plan_review {
            if fresh.next_action == FreshPlanReviewNextAction::AwaitOwnerContinuation {
                fresh.next_action = FreshPlanReviewNextAction::ConsumeRecordedVerdict;
                stage.status = RecoveryStatus::Pending;
                stage.wait_reason = None;
                stage.condition = None;
            }
        }
        stage.revision += 1;
        stage.updated_at = now();
        tx.execute(
            "UPDATE kv SET value=? WHERE key=?",
            params![
                serde_json::to_string(&stage)?,
                super::recovery::key(&stage.session_id, &stage.id)
            ],
        )?;
        let receipt = CurrentFilesReceipt {
            command: command.clone(),
            authorization_id: id,
            resulting_revision: stage.revision,
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
}
