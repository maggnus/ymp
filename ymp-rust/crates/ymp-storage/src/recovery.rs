use super::*;
fn key(session: &str, id: &str) -> String {
    format!("recovery:v1:{session}:{id}")
}
impl Store {
    pub fn recovery_stages(&self, session: &str) -> Result<Vec<RecoveryStage>> {
        let db = self.db()?;
        let prefix = format!("recovery:v1:{session}:");
        let mut q = db.prepare("SELECT value FROM kv WHERE substr(key,1,?1)=?2 ORDER BY key")?;
        let raw = q
            .query_map(params![prefix.len(), prefix], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        raw.into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    /// Compare-and-swap the stage and its proposal record under one write transaction.
    pub fn transition_recovery(
        &self,
        expected: u64,
        stage: &RecoveryStage,
        decision: Option<&DecisionRecord>,
    ) -> Result<()> {
        anyhow::ensure!(
            stage.schema_version == 1 && stage.revision == expected + 1,
            "stale_recovery: invalid stage version"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: Option<String> = tx
            .query_row(
                "SELECT value FROM kv WHERE key=?",
                [key(&stage.session_id, &stage.id)],
                |r| r.get(0),
            )
            .optional()?;
        let old = raw
            .map(|s| serde_json::from_str::<RecoveryStage>(&s))
            .transpose()?;
        anyhow::ensure!(
            old.as_ref().map_or(0, |s| s.revision) == expected,
            "stale_recovery: stage changed before transition"
        );
        if let Some(old) = &old {
            anyhow::ensure!(
                old.session_id == stage.session_id
                    && old.purpose == stage.purpose
                    && old.task == stage.task
                    && old.plan == stage.plan
                    && old.result == stage.result,
                "stale_recovery: saved result identity cannot change"
            );
        }
        if let Some(d) = decision {
            anyhow::ensure!(
                d.session_id == stage.session_id,
                "Recovery decision belongs to another session"
            );
            if let Some(recovery) = &d.links.recovery {
                anyhow::ensure!(
                    recovery.input.stage.revision == expected,
                    "stale_recovery: decision uses another input version"
                );
                anyhow::ensure!(
                    allocation::state(&tx, &stage.session_id)?
                        .as_ref()
                        .map(|s| s.revision)
                        == recovery.input.team_revision,
                    "stale_recovery: membership changed before decision"
                );
            }
            provenance::decision(&tx, d)?;
        }
        tx.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key(&stage.session_id, &stage.id), serde_json::to_string(stage)?])?;
        tx.execute(
            "INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'recovery_stage',?,?)",
            params![stage.session_id, serde_json::to_string(stage)?, now()],
        )?;
        tx.commit()?;
        Ok(())
    }
}

impl Store {
    /// Trusted local owner ingress; no grant or provider operation dispatches here.
    pub fn control_recovery(
        &self,
        command: &RecoveryControlCommand,
    ) -> Result<RecoveryControlReceipt> {
        anyhow::ensure!(
            !command.command_id.trim().is_empty(),
            "owner_command: command ID is required"
        );
        let receipt_key = format!(
            "recovery_command:{}",
            content_digest(&serde_json::to_string(&(
                &command.session_id,
                &command.command_id
            ))?)
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old: Option<String> = tx
            .query_row("SELECT value FROM kv WHERE key=?", [&receipt_key], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(old) = old {
            let receipt: RecoveryControlReceipt = serde_json::from_str(&old)?;
            anyhow::ensure!(
                receipt.command == *command,
                "owner_command_conflict: command ID reused with different content"
            );
            return Ok(receipt);
        }
        let raw: String = tx.query_row(
            "SELECT value FROM kv WHERE key=?",
            [key(&command.session_id, &command.stage_id)],
            |r| r.get(0),
        )?;
        let mut stage: RecoveryStage = serde_json::from_str(&raw)?;
        anyhow::ensure!(
            stage.revision == command.expected_revision,
            "stale_recovery: owner command uses an old revision"
        );
        anyhow::ensure!(
            stage.status != RecoveryStatus::Complete,
            "owner_command: stage is already complete"
        );
        match &command.action {
            RecoveryControl::Retry | RecoveryControl::Continue => {
                anyhow::ensure!(
                    stage
                        .failures
                        .iter()
                        .all(|f| f.effective_access.is_read_only()
                            && f.termination == TerminationEvidence::BackendEnded),
                    "uncertain_effects: inspect effects and establish termination before replay"
                );
                stage.manual_permit = true;
                stage.status = RecoveryStatus::Pending;
                stage.condition = None;
            }
            RecoveryControl::Wait { condition } => {
                anyhow::ensure!(
                    !condition.trim().is_empty(),
                    "owner_command: waiting requires a condition"
                );
                stage.manual_permit = false;
                stage.status = RecoveryStatus::Waiting;
                stage.condition = Some(condition.clone());
            }
            RecoveryControl::Pause => {
                stage.manual_permit = false;
                stage.status = RecoveryStatus::Paused;
                stage.condition = Some("Owner paused recovery".into());
            }
        }
        stage.revision += 1;
        stage.updated_at = now();
        let receipt = RecoveryControlReceipt {
            command: command.clone(),
            resulting_revision: stage.revision,
        };
        tx.execute(
            "UPDATE kv SET value=? WHERE key=?",
            params![
                serde_json::to_string(&stage)?,
                key(&stage.session_id, &stage.id)
            ],
        )?;
        tx.execute(
            "INSERT INTO kv(key,value) VALUES (?,?)",
            params![receipt_key, serde_json::to_string(&receipt)?],
        )?;
        tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'owner_recovery_command',?,?)", params![stage.session_id, serde_json::to_string(&receipt)?, now()])?;
        tx.commit()?;
        Ok(receipt)
    }
}

pub(super) fn bind_admission(
    tx: &rusqlite::Transaction<'_>,
    assignment: &AssignmentRecord,
    invocation: &InvocationRecord,
) -> Result<()> {
    for context in assignment
        .context
        .iter()
        .filter(|c| c.kind == ContextKind::RecoveryStage)
    {
        let raw: String = tx.query_row(
            "SELECT value FROM kv WHERE key=?",
            [key(&assignment.session_id, &context.id)],
            |r| r.get(0),
        )?;
        let mut stage: RecoveryStage = serde_json::from_str(&raw)?;
        anyhow::ensure!(
            context.digest.as_ref() == Some(&stage.revision.to_string())
                && stage.status == RecoveryStatus::Running
                && stage.purpose == assignment.purpose
                && stage.selected_agent.as_ref() == Some(&assignment.agent_id),
            "stale_recovery: admission lost its stage revision or owner permission"
        );
        stage.active_invocation_id = Some(invocation.id.clone());
        stage.revision += 1;
        stage.updated_at = now();
        tx.execute(
            "UPDATE kv SET value=? WHERE key=?",
            params![
                serde_json::to_string(&stage)?,
                key(&stage.session_id, &stage.id)
            ],
        )?;
    }
    Ok(())
}
