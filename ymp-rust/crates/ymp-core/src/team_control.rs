//! Trusted local owner commands and the shared backend read model.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerTeamCommand {
    pub session_id: String,
    /// The TeamState revision shown by TeamControlView, including automatic changes.
    pub expected_revision: u64,
    pub command_id: String,
    /// Explicit consent to revise a pinned roster; fixed size and eligibility stay separate.
    pub revise_pinned_roster: bool,
    pub action: OwnerTeamAction,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum OwnerTeamAction {
    Add {
        agent_id: String,
    },
    Remove {
        agent_id: String,
    },
    Replace {
        agent_id: String,
        replacement_id: String,
    },
    Pause,
    Continue,
    Wait {
        condition: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum OwnerRunControl {
    Continue,
    Paused,
    Waiting { condition: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDeparture {
    pub agent_id: String,
    pub replacement_id: Option<String>,
    pub command_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedTeamCandidate {
    pub candidate: PoolAgent,
    pub provider_fingerprint: String,
    pub native_snapshot: Option<NativeProviderSnapshot>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerTeamState {
    pub schema_version: u32,
    pub session_id: String,
    pub policy_revision: u64,
    pub constraints: TeamConstraints,
    pub desired_members: Vec<String>,
    pub pending_departures: Vec<PendingDeparture>,
    /// Explicit departures remain excluded until the owner selects them again.
    pub excluded_members: Vec<String>,
    pub accepted_candidates: Vec<VerifiedTeamCandidate>,
    pub control: OwnerRunControl,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerTeamReceipt {
    pub command: OwnerTeamCommand,
    pub resulting_revision: u64,
    pub policy_revision: u64,
    pub pending_departures: Vec<PendingDeparture>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveResponsibility {
    pub agent_id: String,
    pub kind: String,
    pub record_id: String,
    pub task: Option<TaskAttemptRef>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamControlView {
    pub schema_version: u32,
    pub session_id: String,
    pub revision: u64,
    pub policy_revision: u64,
    pub startup_policy: Option<SessionPolicy>,
    pub effective: TeamState,
    pub desired_members: Vec<String>,
    pub constraints: TeamConstraints,
    pub pending_departures: Vec<PendingDeparture>,
    pub eligible_candidates: Vec<PoolAgent>,
    pub responsibilities: Vec<ActiveResponsibility>,
    /// Existing execution-task recovery retains its task/board version contract.
    pub board: BoardSnapshot,
    pub invocation_failures: Vec<InvocationFailure>,
    pub recovery_stages: Vec<RecoveryStage>,
    pub recovery_actions: Vec<StageManualActions>,
    pub control: OwnerRunControl,
    pub permitted_actions: Vec<OwnerTeamActionKind>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerTeamActionKind {
    Add,
    Remove,
    Replace,
    Pause,
    Continue,
    Wait,
}
