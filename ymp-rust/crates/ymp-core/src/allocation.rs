//! Typed proposals and captured runtime decisions for bounded session allocation.
use crate::{AgentProfile, ExecutionBackendIdentity, ModelEffort, Reputation, SessionBudget};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TeamConstraints {
    pub fixed_size: Option<usize>,
    pub fixed_roster: Option<Vec<String>>,
    pub eligible_agents: Option<Vec<String>>,
    pub max_members: usize,
}
impl Default for TeamConstraints {
    fn default() -> Self {
        Self {
            fixed_size: None,
            fixed_roster: None,
            eligible_agents: None,
            max_members: 4,
        }
    }
}
impl TeamConstraints {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.max_members > 0,
            "membership_ceiling: max_members must be positive"
        );
        if let Some(size) = self.fixed_size {
            anyhow::ensure!(
                size > 0 && size <= self.max_members,
                "fixed_size: must be positive and within max_members"
            );
        }
        for ids in [&self.fixed_roster, &self.eligible_agents]
            .into_iter()
            .flatten()
        {
            let unique: std::collections::HashSet<_> = ids.iter().collect();
            anyhow::ensure!(
                !ids.is_empty()
                    && unique.len() == ids.len()
                    && ids.iter().all(|id| !id.trim().is_empty()),
                "team_constraints: identities must be nonempty and unique"
            );
        }
        if let Some(roster) = &self.fixed_roster {
            anyhow::ensure!(
                roster.len() <= self.max_members
                    && self.fixed_size.is_none_or(|n| n == roster.len()),
                "fixed_size: fixed roster and size disagree"
            );
            anyhow::ensure!(
                self.eligible_agents
                    .as_ref()
                    .is_none_or(|pool| roster.iter().all(|id| pool.contains(id))),
                "fixed_roster: participant is outside the eligible pool restriction"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamState {
    pub session_id: String,
    pub revision: u64,
    pub current_members: Vec<String>,
    /// Refreshed local admission eligibility; the immutable initial capture is separate.
    pub eligible_agents: Vec<String>,
    /// Eligibility kept available for future review; this grants no role or authority.
    pub reserved_final_reviewer: Option<String>,
    pub method: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationBoundary {
    Startup,
    WorkReady,
    ResultAvailable,
    CheckFailed,
    GoalChanged,
    ParticipantUnavailable,
    ResourcesChanged,
    Conversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRisk {
    Standard,
    Elevated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationDemand {
    pub purpose: String,
    pub task_id: Option<String>,
    pub competence: String,
    pub difficulty: String,
    pub risk: TaskRisk,
    pub ready_work: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionOption {
    pub agent_id: String,
    pub settings: ModelEffort,
    /// Configuration-scoped, confirmation-qualified observations only.
    pub configuration_version: String,
    pub experience: Reputation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationSuggestion {
    pub message_seq: i64,
    pub agent_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationEvidence {
    pub record_id: String,
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationInput {
    pub session_id: String,
    pub boundary: AllocationBoundary,
    pub goal: String,
    pub constraints: TeamConstraints,
    pub current: Option<TeamState>,
    pub eligible: Vec<AgentProfile>,
    pub candidates: Vec<ExecutionOption>,
    pub demand: AllocationDemand,
    pub budget: Option<SessionBudget>,
    /// Producers of any current result cannot become its final reviewer.
    pub producer_ids: Vec<String>,
    pub suggestions: Vec<AllocationSuggestion>,
    pub evidence: Vec<AllocationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationProposal {
    pub members: Vec<String>,
    pub executor: Option<ExecutionOption>,
    pub reserved_final_reviewer: Option<String>,
    pub method: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationDecision {
    pub implementation: ExecutionBackendIdentity,
    pub input: AllocationInput,
    pub proposal: AllocationProposal,
    pub accepted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAllocationInput {
    pub agent_id: String,
    pub requested: crate::ExecutionSettings,
    pub demand: AllocationDemand,
    pub budget: SessionBudget,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationAllowance {
    pub timeout_secs: u64,
    pub native_max_turns: u64,
    pub max_output_chars: u64,
    pub rationale: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceAllocationDecision {
    pub implementation: ExecutionBackendIdentity,
    pub input: ResourceAllocationInput,
    pub proposal: InvocationAllowance,
    pub accepted: bool,
    pub reason: String,
}
