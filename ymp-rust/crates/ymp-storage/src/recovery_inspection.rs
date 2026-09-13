use super::*;
use anyhow::ensure;
use rusqlite::{Connection, Transaction};

fn receipt_key(command: &RecoveryInspectionCommand) -> String {
    format!(
        "recovery_inspection:{}",
        content_digest(
            &serde_json::to_string(&(&command.session_id, &command.command_id))
                .expect("serializable inspection key")
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
fn check_stage(stage: &RecoveryStage, command: &RecoveryInspectionCommand) -> Result<()> {
    ensure!(
        stage.session_id == command.session_id
            && stage.id == command.stage_id
            && stage.revision == command.expected_revision,
        "stale_inspection: stage changed"
    );
    // An explicit inspection command can inspect a held stage without resuming
    // it. Session-wide owner holds still deny every native admission.
    ensure!(
        !matches!(
            stage.status,
            RecoveryStatus::Complete | RecoveryStatus::Running
        ),
        "active_responsibility: completed or running stages cannot start inspection"
    );
    ensure!(
        !stage.failures.is_empty(),
        "inspection_evidence: no failed invocation to inspect"
    );
    Ok(())
}
fn effects(
    db: &Connection,
    stage: &RecoveryStage,
    failures: &[InvocationFailure],
) -> Result<Vec<InspectedInvocationEffects>> {
    let decisions = provenance::records::<DecisionRecord>(db, "decisions", &stage.session_id)?;
    let policy_raw: String = db.query_row(
        "SELECT data FROM session_policies WHERE session_id=?",
        [&stage.session_id],
        |r| r.get(0),
    )?;
    let policy: SessionPolicy = serde_json::from_str(&policy_raw)?;
    let directory = policy.cwd.canonicalize()?;
    let mut total_bytes = 0usize;
    failures.iter().map(|failure| {
        ensure!(stage.failures.contains(failure) && failure.termination == TerminationEvidence::BackendEnded,
            "uncertain_effects: backend termination is unverified");
        let (assignment, invocation) = terminal_failure(db, stage, failure)?;
        let ended_at = invocation.ended_at.clone().context("uncertain_effects: missing termination record")?;
        let access_record = decisions.iter().find(|d| d.kind == "workspace_access_admitted"
            && d.links.assignment_id.as_ref() == Some(&assignment.id)
            && d.links.invocation_id.as_ref() == Some(&invocation.id))
            .context("uncertain_effects: missing admitted access record")?;
        let access = access_record.links.workspace_access.as_ref().context("Missing actual access")?;
        ensure!(access.directory == directory && assignment.cwd.canonicalize()? == directory
            && invocation.execution_backend.as_ref() == Some(&access.backend)
            && access.effective_access == failure.effective_access,
            "uncertain_effects: backend or workspace scope does not match execution");
        let files = if failure.effective_access.is_read_only() { vec![] } else {
            let scope = access.local_effect_scope.as_ref().context("uncertain_effects: complete local-only effect scope was not established by the execution backend")?;
            ensure!(scope.files.len() <= 64, "inspection_evidence: excessive file scope");
            scope.files.iter().map(|p| FileSnapshot::capture(&directory, p)).collect::<Result<Vec<_>>>()?
        };
        total_bytes += files.iter().filter_map(|f| f.bytes.as_ref()).map(Vec::len).sum::<usize>();
        ensure!(total_bytes <= 16 * 1024 * 1024, "inspection_evidence: snapshots exceed 16 MiB");
        Ok(InspectedInvocationEffects { failure: failure.clone(), access_record_id: access_record.id.clone(), ended_at, files })
    }).collect()
}

/// A completed malformed response is terminal, but a valid verdict is never a
/// failed review. Require the exact runtime classification and its original binding.
pub(super) fn terminal_failure(
    db: &Connection,
    stage: &RecoveryStage,
    failure: &InvocationFailure,
) -> Result<(AssignmentRecord, InvocationRecord)> {
    let invocation: InvocationRecord =
        provenance::record(db, "invocations", &failure.invocation_id)?;
    let assignment: AssignmentRecord =
        provenance::record(db, "assignments", &failure.assignment_id)?;
    let decisions = provenance::records::<DecisionRecord>(db, "decisions", &stage.session_id)?;
    let malformed = invocation.state == InvocationState::Completed
        && failure.class == FailureClass::MalformedResponse
        && decisions.iter().any(|d| {
            d.kind
                == if stage.purpose == "plan" {
                    "malformed_plan"
                } else {
                    "malformed_review"
                }
                && d.links.failure.as_ref() == Some(failure)
                && d.links.assignment_id.as_ref() == Some(&assignment.id)
                && d.links.invocation_id.as_ref() == Some(&invocation.id)
                && (stage.purpose == "plan"
                    || (d.links.plan_proposal == stage.plan
                        && d.links.result == stage.result
                        && d.links.task == stage.task))
        });
    ensure!(
        invocation.session_id == stage.session_id
            && assignment.session_id == stage.session_id
            && invocation.assignment_id == assignment.id
            && assignment.agent_id == failure.agent_id
            && assignment.provider_id == failure.provider_id
            && assignment.purpose == stage.purpose
            && (matches!(
                invocation.state,
                InvocationState::Failed | InvocationState::Cancelled
            ) || malformed)
            && assignment.state == invocation.state
            && assignment.ended_at.is_some()
            && invocation.ended_at.is_some()
            && failure.termination == TerminationEvidence::BackendEnded,
        "uncertain_effects: foreign, nonterminal or unclassified completed execution evidence"
    );
    ensure!(
        !decisions
            .iter()
            .any(|d| d.links.invocation_id.as_ref() == Some(&invocation.id)
                && matches!(
                    d.outcome,
                    Some(DecisionOutcome::Accepted { .. } | DecisionOutcome::Rejected)
                )),
        "existing_verdict: a completed verdict cannot be treated as malformed"
    );
    Ok((assignment, invocation))
}

pub(super) fn replay_safe(db: &Connection, stage: &RecoveryStage) -> Result<bool> {
    if stage.failures.iter().all(|f| {
        f.effective_access.is_read_only() && f.termination == TerminationEvidence::BackendEnded
    }) {
        return Ok(true);
    }
    if !stage.effects_resolved() {
        return Ok(false);
    }
    let Some(resolution) = &stage.effect_resolution else {
        return Ok(false);
    };
    let record: DecisionRecord = provenance::record(db, "decisions", &resolution.inspection_id)?;
    let Some(inspection) = record.links.recovery_inspection.as_ref() else {
        return Ok(false);
    };
    Ok(record.kind == "recovery_effects_inspected"
        && record.session_id == stage.session_id
        && matches!(
            record.outcome,
            Some(DecisionOutcome::Accepted {
                confirmation: ConfirmationStatus::Confirmed
            })
        )
        && inspection.command.session_id == stage.session_id
        && inspection.command.stage_id == stage.id
        && inspection.task == stage.task
        && inspection.plan == stage.plan
        && inspection.result == stage.result
        && inspection
            .effects
            .iter()
            .map(|e| &e.failure)
            .eq(resolution.failures.iter())
        && effects(db, stage, &resolution.failures)? == inspection.effects)
}

pub(super) fn check_admission(tx: &Transaction<'_>, assignment: &AssignmentRecord) -> Result<()> {
    let raw: Option<String> = tx
        .query_row(
            "SELECT value FROM kv WHERE key=?",
            [format!("recovery_inspection_admission:{}", assignment.id)],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(raw) = raw {
        let command: RecoveryInspectionCommand = serde_json::from_str(&raw)?;
        let stage = stage(tx, &command.session_id, &command.stage_id)?;
        check_stage(&stage, &command)?;
        ensure!(
            assignment.session_id == command.session_id && assignment.purpose == "review",
            "inspection_binding: foreign assignment"
        );
        independent(tx, &stage, &assignment.agent_id)?;
        effects(tx, &stage, &stage.failures)?;
    }
    Ok(())
}
fn independent(db: &Connection, stage: &RecoveryStage, agent: &str) -> Result<()> {
    ensure!(
        !stage.failures.iter().any(|f| f.agent_id == agent),
        "inspection_independence: failed actor cannot inspect its own effects"
    );
    let producers = stage
        .plan
        .iter()
        .map(|p| p.producer_assignment_id.clone())
        .chain(
            stage
                .result
                .iter()
                .flat_map(|r| r.producer_assignment_ids.clone()),
        )
        .collect::<Vec<_>>();
    ensure!(
        !provenance::records::<AssignmentRecord>(db, "assignments", &stage.session_id)?
            .iter()
            .any(|a| producers.contains(&a.id) && a.agent_id == agent),
        "inspection_independence: result producer cannot inspect its own recovery obligation"
    );
    Ok(())
}

impl Store {
    pub fn recovery_inspection_receipt(
        &self,
        command: &RecoveryInspectionCommand,
    ) -> Result<Option<RecoveryInspectionReceipt>> {
        let value = self
            .value(&receipt_key(command))?
            .map(serde_json::from_value::<RecoveryInspectionReceipt>)
            .transpose()?;
        if let Some(receipt) = &value {
            ensure!(
                receipt.command == *command,
                "owner_command_conflict: inspection command ID has different content"
            );
        }
        Ok(value)
    }
    pub fn recovery_inspection_effects(
        &self,
        command: &RecoveryInspectionCommand,
    ) -> Result<Vec<InspectedInvocationEffects>> {
        let db = self.db()?;
        let stage = stage(&db, &command.session_id, &command.stage_id)?;
        check_stage(&stage, command)?;
        effects(&db, &stage, &stage.failures)
    }
    pub fn bind_recovery_inspection_assignment(
        &self,
        assignment: &str,
        command: &RecoveryInspectionCommand,
    ) -> Result<()> {
        self.db()?.execute(
            "INSERT INTO kv(key,value) VALUES (?,?)",
            params![
                format!("recovery_inspection_admission:{assignment}"),
                serde_json::to_string(command)?
            ],
        )?;
        Ok(())
    }
    /// Atomically bind the actual independent read-only invocation and checked
    /// local state to the unchanged recovery obligation. No retry permit is issued.
    pub fn commit_recovery_inspection(
        &self,
        record: &DecisionRecord,
    ) -> Result<RecoveryInspectionReceipt> {
        let inspection = record
            .links
            .recovery_inspection
            .as_ref()
            .context("Missing inspection record")?;
        let command = &inspection.command;
        ensure!(
            !command.command_id.trim().is_empty()
                && record.session_id == command.session_id
                && record.kind == "recovery_effects_inspected",
            "inspection_binding: invalid command/record"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut stage = stage(&tx, &command.session_id, &command.stage_id)?;
        check_stage(&stage, command)?;
        ensure!(
            inspection.task == stage.task
                && inspection.plan == stage.plan
                && inspection.result == stage.result,
            "stale_inspection: saved result version changed"
        );
        ensure!(
            effects(&tx, &stage, &stage.failures)? == inspection.effects,
            "stale_inspection: observed effects changed"
        );
        let reviewer = &inspection.reviewer;
        independent(&tx, &stage, &reviewer.agent_id)?;
        super::team_control::check_admission(&tx, &stage.session_id, &reviewer.agent_id)?;
        let assignment: AssignmentRecord =
            provenance::record(&tx, "assignments", &reviewer.assignment_id)?;
        let invocation: InvocationRecord =
            provenance::record(&tx, "invocations", &reviewer.invocation_id)?;
        ensure!(
            assignment.session_id == stage.session_id
                && invocation.session_id == stage.session_id
                && invocation.assignment_id == assignment.id
                && assignment.agent_id == reviewer.agent_id
                && assignment.purpose == "review"
                && assignment.state == InvocationState::Completed
                && invocation.state == InvocationState::Completed,
            "inspection_binding: incomplete or foreign inspector"
        );
        let raw: String = tx.query_row(
            "SELECT value FROM kv WHERE key=?",
            [format!("recovery_inspection_admission:{}", assignment.id)],
            |r| r.get(0),
        )?;
        ensure!(
            serde_json::from_str::<RecoveryInspectionCommand>(&raw)? == *command,
            "inspection_binding: inspector belongs to another command"
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
            "inspection_access: inspector was not actually read-only"
        );
        ensure!(
            parse_response::<Review>(&reviewer.text)?.approved,
            "inspection_rejected: independent inspector did not approve continuation"
        );
        check_response(&tx, &stage.session_id, reviewer, "review")?;
        ensure!(
            record.actor.as_ref() == Some(&reviewer.agent_id)
                && record.outcome
                    == Some(DecisionOutcome::Accepted {
                        confirmation: ConfirmationStatus::Confirmed
                    }),
            "inspection_binding: invalid resolution outcome"
        );
        stage.effect_resolution = Some(RecoveryEffectResolution {
            inspection_id: record.id.clone(),
            failures: stage.failures.clone(),
        });
        if matches!(
            stage.wait_reason,
            Some(RecoveryWaitReason::UncertainEffects | RecoveryWaitReason::LegacyUnbound)
        ) {
            stage.wait_reason = Some(RecoveryWaitReason::EffectsInspected);
            stage.condition = Some("Independent inspection verified the bounded local effects; explicit continuation of the saved review remains required".into());
        }
        stage.revision += 1;
        stage.updated_at = now();
        provenance::decision(&tx, record)?;
        tx.execute(
            "UPDATE kv SET value=? WHERE key=?",
            params![
                serde_json::to_string(&stage)?,
                super::recovery::key(&stage.session_id, &stage.id)
            ],
        )?;
        let receipt = RecoveryInspectionReceipt {
            command: command.clone(),
            inspection_id: record.id.clone(),
            resulting_revision: stage.revision,
        };
        tx.execute(
            "INSERT INTO kv(key,value) VALUES (?,?)",
            params![receipt_key(command), serde_json::to_string(&receipt)?],
        )?;
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'recovery_stage',?,?)",
            params![stage.session_id, serde_json::to_string(&stage)?, now()],
        )?;
        tx.commit()?;
        Ok(receipt)
    }
}

pub(super) fn check_response(
    db: &Connection,
    session: &str,
    response: &SavedResponse,
    purpose: &str,
) -> Result<()> {
    let mut query =
        db.prepare("SELECT data FROM events WHERE session_id=? AND kind='message_invocation'")?;
    let origins = query
        .query_map([session], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let origins = origins
        .iter()
        .map(|raw| serde_json::from_str::<MessageOrigin>(raw))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut matched_response = false;
    for origin in origins.iter().filter(|o| {
        o.assignment_id == response.assignment_id
            && o.invocation_id == response.invocation_id
            && o.agent_id == response.agent_id
    }) {
        matched_response |= db.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE seq=? AND session_id=? AND author=? AND kind=? AND text=?)",
                params![origin.message_seq, session, response.agent_id, purpose, response.text], |r| r.get::<_, bool>(0))?;
    }
    ensure!(
        matched_response,
        "inspection_binding: reviewer text differs from its originating invocation"
    );
    Ok(())
}
