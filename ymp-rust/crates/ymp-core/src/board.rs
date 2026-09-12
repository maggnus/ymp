//! Assignment-bound proposals. Only the runtime commits board transitions.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardTaskRef {
    pub task_id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BoardChange {
    AcceptResponsibility {
        task: BoardTaskRef,
        settings: ModelEffort,
    },
    Assign {
        task: BoardTaskRef,
        agent_id: String,
        settings: ModelEffort,
    },
    /// Revisions add an approach and obligations; original objectives, checks,
    /// dependencies, authority and trusted contracts cannot be removed.
    Revise {
        task: BoardTaskRef,
        approach: String,
        dependencies: Vec<String>,
        checks: Vec<String>,
    },
    AddTask {
        title: String,
        description: String,
        competence: String,
        difficulty: String,
        access: TaskAccess,
        dependencies: Vec<String>,
        checks: Vec<String>,
    },
    Membership {
        members: Vec<String>,
    },
}
impl BoardChange {
    pub fn task(&self) -> Option<&BoardTaskRef> {
        match self {
            Self::AcceptResponsibility { task, .. }
            | Self::Assign { task, .. }
            | Self::Revise { task, .. } => Some(task),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardProposalStatus {
    Pending,
    Committed,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardProposal {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub assignment_id: String,
    pub invocation_id: String,
    pub grant_id: String,
    pub plan_version: String,
    pub team_version: String,
    pub change: BoardChange,
    pub rationale: String,
    pub status: BoardProposalStatus,
    pub created_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCommitment {
    pub proposal_id: String,
    pub task_id: String,
    /// Current task content/state at commitment, before the next admission.
    pub task_version: String,
    pub agent_id: String,
    pub settings: ModelEffort,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardTask {
    pub task: Task,
    pub version: String,
    pub commitment: Option<BoardCommitment>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardSnapshot {
    pub session_id: String,
    pub plan_version: String,
    pub tasks: Vec<BoardTask>,
    pub proposals: Vec<BoardProposal>,
    pub team: Option<TeamState>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardDecision {
    pub implementation: ExecutionBackendIdentity,
    pub proposal: BoardProposal,
    pub accepted: bool,
    pub reason: String,
    pub commitment: Option<BoardCommitment>,
    pub resulting_plan_version: String,
}

pub fn board_task_version(task: &Task) -> anyhow::Result<String> {
    Ok(content_digest(&serde_json::to_string(task)?))
}
/// Workspace location and lifecycle do not rewrite the plan's definition.
pub fn board_plan_version(tasks: &[Task]) -> anyhow::Result<String> {
    let definitions = tasks
        .iter()
        .map(|task| {
            let mut definition = TaskDefinition::from(task);
            definition.workspace = None;
            definition.base_commit = None;
            (&task.id, definition)
        })
        .collect::<Vec<_>>();
    Ok(content_digest(&serde_json::to_string(&definitions)?))
}

/// Repeating an unchanged allocation does not expire a proposal; changing actual
/// membership, eligibility or reviewer obligations does.
pub fn board_team_version(team: Option<&TeamState>) -> anyhow::Result<String> {
    let membership = team.map(|t| {
        let mut members = t.current_members.clone();
        let mut eligible = t.eligible_agents.clone();
        members.sort();
        eligible.sort();
        (members, eligible, &t.reserved_final_reviewer)
    });
    Ok(content_digest(&serde_json::to_string(&membership)?))
}
