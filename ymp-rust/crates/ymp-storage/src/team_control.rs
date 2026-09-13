use super::*;
use anyhow::ensure;
use rusqlite::{Connection, Transaction};

fn key(session: &str) -> String {
    format!("owner_team:v1:{session}")
}
pub(super) fn state(db: &Connection, session: &str) -> Result<Option<OwnerTeamState>> {
    let raw: Option<String> = db
        .query_row("SELECT value FROM kv WHERE key=?", [key(session)], |r| {
            r.get(0)
        })
        .optional()?;
    raw.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
}
fn save(tx: &Transaction<'_>, owner: &OwnerTeamState, team: &TeamState) -> Result<()> {
    for (key, value) in [
        (key(&owner.session_id), serde_json::to_string(owner)?),
        (
            format!("team_state:v1:{}", owner.session_id),
            serde_json::to_string(team)?,
        ),
    ] {
        tx.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
    }
    Ok(())
}
pub(super) fn responsibilities(
    db: &Connection,
    session: &str,
) -> Result<Vec<ActiveResponsibility>> {
    let mut result = Vec::new();
    for assignment in provenance::records::<AssignmentRecord>(db, "assignments", session)? {
        if assignment.state == InvocationState::Running {
            result.push(ActiveResponsibility {
                agent_id: assignment.agent_id,
                kind: "invocation".into(),
                record_id: assignment.id,
                task: assignment.task,
            });
        }
    }
    for task in provenance::records::<Task>(db, "tasks", session)? {
        if task.state == TaskState::Running {
            if let Some(agent) = &task.assignee {
                result.push(ActiveResponsibility {
                    agent_id: agent.clone(),
                    kind: "task_execution".into(),
                    record_id: task.id.clone(),
                    task: Some(TaskAttemptRef::from(&task)),
                });
            }
        }
    }
    for task in board::snapshot(db, session)?.tasks {
        if let Some(commitment) = task.commitment {
            result.push(ActiveResponsibility {
                agent_id: commitment.agent_id,
                kind: "board_commitment".into(),
                record_id: task.task.id.clone(),
                task: Some(TaskAttemptRef::from(&task.task)),
            });
        }
    }
    let decisions = provenance::records::<DecisionRecord>(db, "decisions", session)?;
    for acquired in decisions
        .iter()
        .filter(|d| d.kind == "workspace_access_acquired")
    {
        if let (Some(actor), Some(access)) = (&acquired.actor, &acquired.links.workspace_access) {
            if !decisions.iter().any(|d| {
                d.kind == "workspace_access_released"
                    && d.links
                        .workspace_access
                        .as_ref()
                        .is_some_and(|a| a.reservation_id == access.reservation_id)
            }) {
                result.push(ActiveResponsibility {
                    agent_id: actor.clone(),
                    kind: "workspace_access".into(),
                    record_id: access.reservation_id.clone(),
                    task: acquired.links.task.clone(),
                });
            }
        }
    }
    Ok(result)
}
/// Checks on every fresh admission/claim, within its transaction. Current work keeps its authority.
pub(super) fn check_admission(db: &Connection, session: &str, agent: &str) -> Result<()> {
    if let Some(owner) = state(db, session)? {
        ensure!(
            owner.control == OwnerRunControl::Continue,
            "owner_paused: new work waits for explicit owner continuation"
        );
        ensure!(
            !owner.excluded_members.iter().any(|id| id == agent)
                && !owner.pending_departures.iter().any(|p| p.agent_id == agent),
            "pending_departure: owner removed this participant from fresh work"
        );
    }
    Ok(())
}
/// Settle only after invocation, task selection, board commitment and access ownership all end.
pub(super) fn settle(tx: &Transaction<'_>, session: &str) -> Result<()> {
    let Some(mut owner) = state(tx, session)? else {
        return Ok(());
    };
    if owner.pending_departures.is_empty() {
        return Ok(());
    }
    let busy = responsibilities(tx, session)?;
    let mut team =
        allocation::state(tx, session)?.context("Owner team is missing effective membership")?;
    let mut changed = false;
    owner.pending_departures.retain(|pending| {
        if busy.iter().any(|r| r.agent_id == pending.agent_id) {
            return true;
        }
        team.current_members.retain(|id| id != &pending.agent_id);
        if let Some(replacement) = &pending.replacement_id {
            if !team.current_members.contains(replacement) {
                team.current_members.push(replacement.clone());
            }
        }
        changed = true;
        false
    });
    if changed {
        team.revision += 1;
        team.updated_at = now();
        owner.updated_at = team.updated_at.clone();
        refresh_reviewer(tx, &mut team, &owner)?;
        save(tx, &owner, &team)?;
        tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'owner_team_settled',?,?)", params![session,serde_json::json!({"owner":owner,"effective":team}).to_string(),now()])?;
    }
    Ok(())
}
fn refresh_reviewer(db: &Connection, team: &mut TeamState, owner: &OwnerTeamState) -> Result<()> {
    let producers = provenance::records::<AssignmentRecord>(db, "assignments", &team.session_id)?
        .into_iter()
        .filter(|a| a.purpose == "execute")
        .map(|a| a.agent_id)
        .collect::<Vec<_>>();
    let allowed = |id: &String| {
        team.eligible_agents.contains(id)
            && !owner.excluded_members.contains(id)
            && !owner.pending_departures.iter().any(|p| &p.agent_id == id)
            && !producers.contains(id)
            && owner
                .constraints
                .fixed_roster
                .as_ref()
                .is_none_or(|r| r.contains(id))
    };
    if !team.reserved_final_reviewer.as_ref().is_some_and(allowed) {
        team.reserved_final_reviewer = team
            .current_members
            .iter()
            .rev()
            .find(|id| allowed(id))
            .cloned();
    }
    Ok(())
}
impl Store {
    pub fn owner_team_state(&self, session: &str) -> Result<Option<OwnerTeamState>> {
        state(&*self.db()?, session)
    }
    pub fn active_responsibilities(&self, session: &str) -> Result<Vec<ActiveResponsibility>> {
        responsibilities(&*self.db()?, session)
    }
    pub fn settle_owner_team(&self, session: &str) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        settle(&tx, session)?;
        tx.commit()?;
        Ok(())
    }
    pub fn owner_team_receipt(
        &self,
        command: &OwnerTeamCommand,
    ) -> Result<Option<OwnerTeamReceipt>> {
        let receipt = self
            .value(&receipt_key(command))?
            .map(serde_json::from_value::<OwnerTeamReceipt>)
            .transpose()?;
        if let Some(receipt) = &receipt {
            ensure!(
                receipt.command == *command,
                "owner_command_conflict: command ID has different content"
            );
        }
        Ok(receipt)
    }
    /// The runtime supplies only a candidate resolved from verified native metadata.
    /// The provider-facing authority surface has no route to this local owner ingress.
    pub fn commit_owner_team(
        &self,
        command: &OwnerTeamCommand,
        selected: Option<&VerifiedTeamCandidate>,
    ) -> Result<OwnerTeamReceipt> {
        ensure!(
            !command.command_id.trim().is_empty(),
            "owner_command: command ID required"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old: Option<String> = tx
            .query_row(
                "SELECT value FROM kv WHERE key=?",
                [receipt_key(command)],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            let receipt: OwnerTeamReceipt = serde_json::from_str(&old)?;
            ensure!(
                receipt.command == *command,
                "owner_command_conflict: command ID has different content"
            );
            return Ok(receipt);
        }
        let session_id = &command.session_id;
        let mut team = allocation::state(&tx, session_id)?
            .context("owner_command: effective team is not initialized")?;
        ensure!(
            team.revision == command.expected_revision,
            "stale_owner_command: effective team changed"
        );
        let mut session: Session = provenance::record(&tx, "sessions", session_id)?;
        let raw: String = tx.query_row(
            "SELECT data FROM session_policies WHERE session_id=?",
            [session_id],
            |r| r.get(0),
        )?;
        let policy: SessionPolicy = serde_json::from_str(&raw)?;
        let mut owner = state(&tx, session_id)?.unwrap_or(OwnerTeamState {
            schema_version: 1,
            session_id: session_id.clone(),
            policy_revision: 0,
            constraints: policy.team_constraints.unwrap_or_default(),
            desired_members: team.current_members.clone(),
            pending_departures: vec![],
            excluded_members: vec![],
            accepted_candidates: vec![],
            control: OwnerRunControl::Continue,
            updated_at: now(),
        });
        if owner.pending_departures.is_empty() {
            owner.desired_members = team.current_members.clone();
        }
        let (depart, join) = match &command.action {
            OwnerTeamAction::Add { agent_id } => (None, Some(agent_id)),
            OwnerTeamAction::Remove { agent_id } => (Some(agent_id), None),
            OwnerTeamAction::Replace {
                agent_id,
                replacement_id,
            } => (Some(agent_id), Some(replacement_id)),
            OwnerTeamAction::Pause => {
                owner.control = OwnerRunControl::Paused;
                (None, None)
            }
            OwnerTeamAction::Continue => {
                owner.control = OwnerRunControl::Continue;
                (None, None)
            }
            OwnerTeamAction::Wait { condition } => {
                ensure!(
                    !condition.trim().is_empty(),
                    "owner_command: waiting requires a condition"
                );
                owner.control = OwnerRunControl::Waiting {
                    condition: condition.clone(),
                };
                (None, None)
            }
        };
        if let Some(id) = depart {
            ensure!(
                owner.desired_members.contains(id) && team.current_members.contains(id),
                "owner_command: departing agent is not a current requested member"
            );
            owner.desired_members.retain(|m| m != id);
            if !owner.excluded_members.contains(id) {
                owner.excluded_members.push(id.clone());
            }
        }
        if let Some(id) = join {
            ensure!(
                !owner.desired_members.contains(id)
                    && !owner.pending_departures.iter().any(|p| &p.agent_id == id),
                "owner_command: addition is already a member or is draining"
            );
            let candidate =
                selected.context("owner_command: verified native candidate required")?;
            ensure!(
                &candidate.candidate.profile.id == id && candidate.candidate.exclusions.is_empty(),
                "owner_command: native candidate mismatch or exclusion"
            );
            ensure!(
                owner
                    .constraints
                    .eligible_agents
                    .as_ref()
                    .is_none_or(|allowed| allowed.contains(id)),
                "owner_command: candidate violates explicit eligibility restriction"
            );
            owner.desired_members.push(id.clone());
            owner.excluded_members.retain(|m| m != id);
            if !owner
                .accepted_candidates
                .iter()
                .any(|c| c.candidate.profile.id == *id)
            {
                owner.accepted_candidates.push(candidate.clone());
            }
            if !session.team.iter().any(|a| &a.id == id) {
                session.team.push(candidate.candidate.profile.clone());
            }
            if !team.eligible_agents.contains(id) {
                team.eligible_agents.push(id.clone());
            }
        }
        if depart.is_some() || join.is_some() {
            if owner.constraints.fixed_roster.is_some() {
                ensure!(
                    command.revise_pinned_roster,
                    "owner_command: pinned membership requires explicit owner revision"
                );
                owner.constraints.fixed_roster = Some(owner.desired_members.clone());
            }
            if depart.is_none() && join.is_some() {
                ensure!(team.current_members.len()<owner.constraints.max_members,"owner_command: active responsibilities still occupy all membership slots; wait for departure to settle");
            }
            owner.constraints.validate()?;
            ensure!(
                owner.desired_members.len() <= owner.constraints.max_members
                    && owner
                        .constraints
                        .fixed_size
                        .is_none_or(|size| size == owner.desired_members.len()),
                "owner_command: desired membership violates fixed size or ceiling"
            );
            // An unadmitted task selection owns no native effects. Withdraw it atomically
            // so the active engine may select another member instead of waiting on itself.
            if let Some(id) = depart {
                let assignments =
                    provenance::records::<AssignmentRecord>(&tx, "assignments", session_id)?;
                for mut task in provenance::records::<Task>(&tx, "tasks", session_id)? {
                    if task.state == TaskState::Running
                        && task.assignee.as_ref() == Some(id)
                        && !assignments.iter().any(|a| {
                            a.purpose == "execute"
                                && a.task.as_ref() == Some(&TaskAttemptRef::from(&task))
                        })
                    {
                        task.state = TaskState::Ready;
                        task.assignee = None;
                        task.result=Some("Owner withdrew selection before native admission; no execution effects were produced".into());
                        super::write_task(&tx, &task)?;
                        tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'owner_selection_withdrawn',?,?)",params![session_id,serde_json::json!({"command_id":command.command_id,"task":task}).to_string(),now()])?;
                    }
                }
            }
            let busy = responsibilities(&tx, session_id)?;
            if let Some(id) = depart.filter(|id| busy.iter().any(|r| &r.agent_id == *id)) {
                owner.pending_departures.push(PendingDeparture {
                    agent_id: id.clone(),
                    replacement_id: join.cloned(),
                    command_id: command.command_id.clone(),
                });
            } else {
                if let Some(id) = depart {
                    team.current_members.retain(|m| m != id);
                }
                if let Some(id) = join {
                    team.current_members.push(id.clone());
                }
            }
        }
        if let Some(departing) = depart {
            let mut q = tx.prepare("SELECT key,value FROM kv WHERE substr(key,1,?1)=?2")?;
            let prefix = format!("recovery:v1:{session_id}:");
            let rows = q
                .query_map(params![prefix.len(), prefix], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(q);
            for (key, raw) in rows {
                let mut stage: RecoveryStage = serde_json::from_str(&raw)?;
                if stage.selected_agent.as_ref() == Some(departing)
                    && matches!(
                        stage.status,
                        RecoveryStatus::Waiting
                            | RecoveryStatus::OwnerAction
                            | RecoveryStatus::Pending
                    )
                    && stage.failures.iter().all(|f| {
                        f.effective_access.is_read_only()
                            && f.termination == TerminationEvidence::BackendEnded
                    })
                {
                    stage.selected_agent = None;
                    stage.manual_permit = true;
                    stage.status = RecoveryStatus::Pending;
                    stage.condition=Some("Owner changed membership; pending review retains its exact result and objections".into());
                    stage.revision += 1;
                    stage.updated_at = now();
                    tx.execute(
                        "UPDATE kv SET value=? WHERE key=?",
                        params![serde_json::to_string(&stage)?, key],
                    )?;
                }
            }
        }
        owner.policy_revision += 1;
        owner.updated_at = now();
        team.revision += 1;
        team.updated_at = owner.updated_at.clone();
        refresh_reviewer(&tx, &mut team, &owner)?;
        save(&tx, &owner, &team)?;
        tx.execute(
            "UPDATE sessions SET data=? WHERE id=?",
            params![serde_json::to_string(&session)?, session_id],
        )?;
        let receipt = OwnerTeamReceipt {
            command: command.clone(),
            resulting_revision: team.revision,
            policy_revision: owner.policy_revision,
            pending_departures: owner.pending_departures.clone(),
        };
        tx.execute(
            "INSERT INTO kv(key,value) VALUES (?,?)",
            params![receipt_key(command), serde_json::to_string(&receipt)?],
        )?;
        tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'owner_team_command',?,?)",params![session_id,serde_json::json!({"receipt":receipt,"owner":owner,"effective":team}).to_string(),now()])?;
        tx.commit()?;
        Ok(receipt)
    }
}
fn receipt_key(command: &OwnerTeamCommand) -> String {
    format!(
        "owner_team_command:{}",
        content_digest(
            &serde_json::to_string(&(&command.session_id, &command.command_id))
                .expect("serializable command key")
        )
    )
}

pub(super) fn check_allocation(
    db: &Connection,
    session: &str,
    decision: &AllocationDecision,
) -> Result<()> {
    if let Some(owner) = state(db, session)? {
        ensure!(
            owner.control == OwnerRunControl::Continue,
            "owner_paused: allocation waits for explicit continuation"
        );
        let team = allocation::state(db, session)?.context("Effective owner team missing")?;
        let mut constraints = owner.constraints.clone();
        if !owner.pending_departures.is_empty() && constraints.fixed_roster.is_some() {
            constraints.fixed_roster = Some(team.current_members.clone());
        }
        ensure!(
            decision.input.constraints == constraints,
            "stale_owner_policy: allocation uses superseded owner constraints"
        );
        ensure!(
            owner
                .pending_departures
                .iter()
                .all(|p| decision.proposal.members.contains(&p.agent_id)),
            "pending_departure: allocation cannot release live ownership"
        );
        ensure!(
            decision
                .proposal
                .members
                .iter()
                .all(|id| !owner.excluded_members.contains(id)
                    || owner.pending_departures.iter().any(|p| &p.agent_id == id)),
            "owner_excluded: allocation cannot reintroduce an explicitly removed member"
        );
        if let Some(executor) = &decision.proposal.executor {
            check_admission(db, session, &executor.agent_id)?;
        }
    }
    Ok(())
}
