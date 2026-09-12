use super::{provenance, Store};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use ymp_core::*;

fn key(session: &str) -> String {
    format!("team_state:v1:{session}")
}

pub(super) fn state(db: &rusqlite::Connection, session: &str) -> Result<Option<TeamState>> {
    let raw: Option<String> = db
        .query_row("SELECT value FROM kv WHERE key=?", [key(session)], |r| {
            r.get(0)
        })
        .optional()?;
    raw.map(|raw| Ok(serde_json::from_str(&raw)?)).transpose()
}

fn choice_key(session: &str, agent: &str, purpose: &str, task: Option<&str>) -> String {
    format!(
        "allocation_choice:{}",
        content_digest(
            &serde_json::to_string(&(session, agent, purpose, task))
                .expect("serializable choice key")
        )
    )
}

impl Store {
    pub fn allocation_settings(
        &self,
        session: &str,
        agent: &str,
        purpose: &str,
        task: Option<&str>,
    ) -> Result<Option<ModelEffort>> {
        self.value(&choice_key(session, agent, purpose, task))?
            .map(|v| Ok(serde_json::from_value(v)?))
            .transpose()
    }

    /// Refresh observed local eligibility without changing membership or historical identity.
    /// Subsequent public admission wrappers use this snapshot in their transaction.
    pub fn refresh_team_eligibility(&self, session: &str, eligible: &[String]) -> Result<()> {
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(mut current) = state(&tx, session)? {
            if current.eligible_agents != eligible {
                current.eligible_agents = eligible.to_vec();
                current.revision += 1;
                current.updated_at = now();
                tx.execute(
                    "UPDATE kv SET value=? WHERE key=?",
                    params![serde_json::to_string(&current)?, key(session)],
                )?;
                tx.execute("INSERT INTO events(session_id,kind,data,created_at) VALUES (?,'team_eligibility',?,?)", params![session, serde_json::to_string(&current)?, now()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn team_state(&self, session: &str) -> Result<Option<TeamState>> {
        state(&*self.db()?, session)
    }

    pub fn allocation_decisions(&self, session: &str) -> Result<Vec<AllocationDecision>> {
        Ok(
            provenance::records::<DecisionRecord>(&*self.db()?, "decisions", session)?
                .into_iter()
                .filter_map(|d| d.links.allocation.map(|a| *a))
                .collect(),
        )
    }

    /// Commit membership, append captured identities and record its decision atomically.
    /// A proposal never changes the immutable startup policy or historical profiles.
    pub fn commit_allocation(&self, decision: &DecisionRecord) -> Result<TeamState> {
        let allocation = decision
            .links
            .allocation
            .as_ref()
            .context("Missing allocation decision")?;
        ensure!(
            allocation.accepted,
            "Rejected allocation cannot change membership"
        );
        ensure!(
            allocation.input.session_id == decision.session_id,
            "Allocation session mismatch"
        );
        let mut db = self.db()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = commit(&tx, decision)?;
        tx.commit()?;
        Ok(current)
    }
}

/// Shared transaction with board commitment; no membership partial commit.
pub(super) fn commit(
    tx: &rusqlite::Transaction<'_>,
    decision: &DecisionRecord,
) -> Result<TeamState> {
    let allocation = decision
        .links
        .allocation
        .as_ref()
        .context("Missing allocation decision")?;
    let mut session: Session = provenance::record(tx, "sessions", &decision.session_id)?;
    let previous = state(tx, &session.id)?;
    ensure!(
        previous.as_ref().map(|s| s.revision)
            == allocation.input.current.as_ref().map(|s| s.revision),
        "stale_allocation: team changed since proposal"
    );
    let members = &allocation.proposal.members;
    let reviewer = allocation
        .proposal
        .reserved_final_reviewer
        .as_ref()
        .context(
            "no_independent_eligible_reviewer: membership requires a reserved final reviewer",
        )?;
    ensure!(
        allocation.input.eligible.iter().any(|a| &a.id == reviewer),
        "no_independent_eligible_reviewer: reserved final reviewer is not eligible"
    );
    for assignment in provenance::records::<AssignmentRecord>(tx, "assignments", &session.id)? {
        // Admission can advance after the policy's snapshot without changing
        // membership revision. Recheck producers under this same write lock.
        ensure!(
                assignment.purpose != "execute" || &assignment.agent_id != reviewer,
                "no_independent_eligible_reviewer: reserved final reviewer became a producer after allocation input was captured"
            );
        ensure!(
            assignment.state != InvocationState::Running || members.contains(&assignment.agent_id),
            "active_responsibility: cannot remove an active participant"
        );
    }
    for task in provenance::records::<Task>(tx, "tasks", &session.id)? {
        ensure!(
            task.state != TaskState::Running
                || task
                    .assignee
                    .as_ref()
                    .is_some_and(|id| members.contains(id)),
            "active_responsibility: cannot remove a committed task executor"
        );
    }
    for task in super::board::snapshot(tx, &session.id)?.tasks {
        if allocation.input.demand.task_id.as_ref() != Some(&task.task.id) {
            if let Some(commitment) = task.commitment {
                ensure!(members.contains(&commitment.agent_id), "active_responsibility: cannot remove a participant with a current board commitment without explicit reassignment");
            }
        }
    }
    for id in members {
        let profile = allocation
            .input
            .eligible
            .iter()
            .find(|a| &a.id == id)
            .context("ineligible_member: no current eligible profile")?;
        if !session.team.iter().any(|a| &a.id == id) {
            session.team.push(profile.clone());
        }
    }
    let current = TeamState {
        session_id: session.id.clone(),
        revision: previous.map_or(1, |s| s.revision + 1),
        current_members: members.clone(),
        eligible_agents: allocation
            .input
            .eligible
            .iter()
            .map(|a| a.id.clone())
            .collect(),
        reserved_final_reviewer: allocation.proposal.reserved_final_reviewer.clone(),
        method: allocation.proposal.method.clone(),
        updated_at: now(),
    };
    tx.execute(
        "UPDATE sessions SET data=? WHERE id=?",
        params![serde_json::to_string(&session)?, session.id],
    )?;
    tx.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key(&session.id),serde_json::to_string(&current)?])?;
    if let Some(choice) = &allocation.proposal.executor {
        tx.execute("INSERT INTO kv(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![choice_key(&session.id, &choice.agent_id, &allocation.input.demand.purpose, allocation.input.demand.task_id.as_deref()), serde_json::to_string(&choice.settings)?])?;
    }
    provenance::decision(tx, decision)?;
    Ok(current)
}
