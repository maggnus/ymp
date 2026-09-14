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
#[serde(deny_unknown_fields)]
pub struct BoardTaskDefinitionRef {
    pub task_id: String,
    pub definition_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardCheckReplacement {
    pub old: String,
    pub new: String,
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
    /// Replace runtime-generated ordinary checks after independent review.
    /// Captured owner acceptance contracts use a separate immutable authority.
    ReplaceChecks {
        task: BoardTaskDefinitionRef,
        replacements: Vec<BoardCheckReplacement>,
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

    pub fn definition_task(&self) -> Option<&BoardTaskDefinitionRef> {
        match self {
            Self::ReplaceChecks { task, .. } => Some(task),
            _ => None,
        }
    }

    pub fn task_id(&self) -> Option<&str> {
        self.task()
            .map(|task| task.task_id.as_str())
            .or_else(|| self.definition_task().map(|task| task.task_id.as_str()))
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
pub struct BoardCheckRunEvidence {
    pub command: String,
    pub output: String,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCheckRevisionEvidence {
    pub proposal: BoardProposal,
    pub retained_checks: Vec<String>,
    pub proposed_checks: Vec<String>,
    pub retained_runs: Vec<BoardCheckRunEvidence>,
    pub proposed_runs: Vec<BoardCheckRunEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultCheckRevision {
    pub previous: ResultVersion,
    pub proposal: BoardProposal,
    pub review_id: String,
    pub attempt_before: usize,
    pub attempt_after: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CheckRevisionProvenance {
    Review(Box<BoardCheckRevisionEvidence>),
    Result(Box<ResultCheckRevision>),
}

pub fn revised_check_result_id(
    previous: &ResultVersion,
    proposal: &BoardProposal,
) -> anyhow::Result<String> {
    Ok(content_digest(&serde_json::to_string(&(
        "ordinary-check-revision-v1",
        &previous.id,
        previous.version,
        &proposal.id,
    ))?))
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
/// An explicit owner departure withdraws only responsibility not yet admitted.
/// The original board decision remains immutable and attributable to its proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCommitmentRelease {
    pub command_id: String,
    pub previous: BoardTaskRef,
    pub commitment: BoardCommitment,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardTask {
    pub task: Task,
    pub version: String,
    pub definition_version: String,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_ids: Vec<String>,
    pub commitment: Option<BoardCommitment>,
    pub resulting_plan_version: String,
}

pub fn board_task_version(task: &Task) -> anyhow::Result<String> {
    Ok(content_digest(&serde_json::to_string(task)?))
}

/// Definition-only binding for a running task proposal. Lifecycle bookkeeping
/// and result text cannot make this digest stale; source location and revision
/// remain part of the definition.
pub fn board_task_definition_version(task: &Task) -> anyhow::Result<String> {
    Ok(content_digest(&serde_json::to_string(
        &TaskDefinition::from(task),
    )?))
}

pub fn replace_ordinary_checks(
    checks: &[String],
    replacements: &[BoardCheckReplacement],
) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(
        !replacements.is_empty(),
        "check_replacement: at least one exact replacement is required"
    );
    let mut sources = std::collections::HashSet::new();
    for replacement in replacements {
        let old = replacement.old.trim();
        let new = replacement.new.trim();
        anyhow::ensure!(
            !old.is_empty() && !new.is_empty() && replacement.old != replacement.new,
            "check_replacement: commands must be nonempty and different"
        );
        anyhow::ensure!(
            !matches!(new, ":" | "true" | "/bin/true" | "exit 0" | "return 0"),
            "check_replacement: trivial success commands are forbidden"
        );
        anyhow::ensure!(
            sources.insert(replacement.old.as_str()),
            "check_replacement: each source command may be replaced once"
        );
        anyhow::ensure!(
            checks
                .iter()
                .filter(|check| *check == &replacement.old)
                .count()
                == 1,
            "check_replacement: source command is missing or ambiguous"
        );
    }
    let resulting = checks
        .iter()
        .map(|check| {
            replacements
                .iter()
                .find(|replacement| &replacement.old == check)
                .map_or_else(|| check.clone(), |replacement| replacement.new.clone())
        })
        .collect::<Vec<_>>();
    let unique = resulting.iter().collect::<std::collections::HashSet<_>>();
    anyhow::ensure!(
        unique.len() == resulting.len(),
        "check_replacement: resulting commands must remain distinct"
    );
    Ok(resulting)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> Task {
        serde_json::from_value(serde_json::json!({
            "id":"task",
            "session_id":"session",
            "title":"Verify output",
            "description":"Inspect the requested behavior",
            "competence":"verification",
            "difficulty":"standard",
            "dependencies":[],
            "checks":["test -f expected"],
            "state":"running",
            "assignee":"executor",
            "attempts":1,
            "result":null,
            "workspace":null
        }))
        .unwrap()
    }

    #[test]
    fn definition_binding_ignores_review_bookkeeping_but_detects_check_changes() {
        let running = task();
        let mut review = running.clone();
        review.state = TaskState::Review;
        review.result = Some("Candidate result".into());
        review.reviewer = Some("reviewer".into());
        assert_ne!(
            board_task_version(&running).unwrap(),
            board_task_version(&review).unwrap()
        );
        assert_eq!(
            board_task_definition_version(&running).unwrap(),
            board_task_definition_version(&review).unwrap()
        );
        review.base_commit = Some("changed-source".into());
        assert_ne!(
            board_task_definition_version(&running).unwrap(),
            board_task_definition_version(&review).unwrap()
        );
        review.base_commit = running.base_commit.clone();
        review.checks = vec!["test -f replacement".into()];
        assert_ne!(
            board_task_definition_version(&running).unwrap(),
            board_task_definition_version(&review).unwrap()
        );
    }
}
