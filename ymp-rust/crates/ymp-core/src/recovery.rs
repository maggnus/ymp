//! Versioned runtime recovery records. Policies propose actions; these records grant no authority.
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    TransientTransport,
    Authentication,
    QuotaExhausted,
    UnsupportedConfiguration,
    Timeout,
    Cancelled,
    MalformedResponse,
    Unknown,
}

/// Structured classification supplied by a trusted backend, or conservatively by runtime.
/// Native codes must be allowlisted; raw error messages never belong here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationFailure {
    pub class: FailureClass,
    pub native_code: Option<String>,
    pub assignment_id: String,
    pub invocation_id: String,
    pub agent_id: String,
    pub provider_id: String,
    pub effective_access: WorkspaceAccess,
    pub termination: TerminationEvidence,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationEvidence {
    /// The trusted backend future ended under its cleanup contract.
    BackendEnded,
    /// A restart alone does not establish termination of external execution.
    UnverifiedAfterRestart,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStatus {
    Pending,
    Running,
    Waiting,
    OwnerAction,
    Paused,
    Complete,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedResponse {
    pub text: String,
    pub assignment_id: String,
    pub invocation_id: String,
    pub agent_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryStage {
    pub schema_version: u32,
    pub session_id: String,
    /// Stable obligation identity, separate from the optimistic storage revision.
    pub id: String,
    pub revision: u64,
    pub purpose: String,
    pub task: Option<TaskAttemptRef>,
    pub plan: Option<PlanVersion>,
    pub result: Option<ResultVersion>,
    pub review_ids: Vec<String>,
    pub selected_agent: Option<String>,
    pub response: Option<SavedResponse>,
    pub active_invocation_id: Option<String>,
    pub failures: Vec<InvocationFailure>,
    /// Counts issued recovery actions, including interrupted delays; restart cannot reset it.
    pub recovery_attempts: u32,
    pub manual_permit: bool,
    pub admission_denial: Option<BudgetDenial>,
    pub status: RecoveryStatus,
    pub condition: Option<String>,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryInput {
    pub schema_version: u32,
    pub stage: RecoveryStage,
    pub failure: InvocationFailure,
    pub eligible: Vec<AgentProfile>,
    pub outstanding_assignments: Vec<String>,
    pub provider_failures: usize,
    pub team_revision: Option<u64>,
    pub owner_policy_revision: u64,
    pub budget: Option<SessionBudget>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RecoveryAction {
    ResumeReview,
    Retry { delay_ms: u64 },
    Reassign,
    InspectEffects,
    Wait { condition: String },
    RequestOwner { reason: String },
    Stop { reason: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryConfiguration {
    pub max_attempts: u32,
    pub max_provider_failures: usize,
    pub delay_ms: u64,
}
impl Default for RecoveryConfiguration {
    fn default() -> Self {
        Self {
            max_attempts: 2,
            max_provider_failures: 2,
            delay_ms: 250,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyProvenance {
    pub implementation: ExecutionBackendIdentity,
    pub configuration: serde_json::Value,
    pub originating_record_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryDecision {
    pub policy: PolicyProvenance,
    pub input: RecoveryInput,
    pub proposal: RecoveryAction,
    pub accepted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryControlCommand {
    pub session_id: String,
    pub stage_id: String,
    pub expected_revision: u64,
    pub command_id: String,
    pub action: RecoveryControl,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RecoveryControl {
    Retry,
    Continue,
    Wait { condition: String },
    Pause,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryControlReceipt {
    pub command: RecoveryControlCommand,
    pub resulting_revision: u64,
}
