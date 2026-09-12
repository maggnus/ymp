//! Runtime-owned provenance. These records describe authority and evidence links;
//! they do not issue permissions or establish confirmation by themselves.
use crate::{AgentProfile, Limits, Plan, Session, SessionUsage, Task, UsageSnapshot};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub fn content_digest(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

/// Shared identity for producing observations and validating their attribution.
/// Historical invocations without backend metadata retain their original identity;
/// this does not infer which backend produced them.
pub fn effective_execution_version(
    config_version: &str,
    invocation: &InvocationRecord,
) -> anyhow::Result<String> {
    let content = if invocation.execution_backend.is_some() {
        serde_json::to_string(&(
            "effective-execution-v2",
            config_version,
            &invocation.execution_backend,
            &invocation.sent,
            &invocation.reported,
            &invocation.native_version,
        ))?
    } else {
        serde_json::to_string(&(
            "effective-execution-v1",
            config_version,
            &invocation.sent,
            &invocation.reported,
            &invocation.native_version,
        ))?
    };
    Ok(content_digest(&content)[..24].to_owned())
}

/// Captured exactly once for a new session. Historical sessions have no policy
/// rather than a policy invented from today's configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPolicy {
    pub session_id: String,
    pub goal: String,
    /// Separate structured constraints, when supplied; the complete goal remains
    /// authoritative even when constraints have not been extracted from its text.
    pub constraints: Option<Vec<String>>,
    pub cwd: PathBuf,
    pub limits: Limits,
    pub eligible_pool: Vec<AgentProfile>,
    pub captured_team: Vec<AgentProfile>,
    #[serde(default)]
    pub team_constraints: Option<crate::TeamConstraints>,
    #[serde(default)]
    pub execution: std::collections::BTreeMap<String, crate::AgentExecutionPolicy>,
    #[serde(default)]
    pub assignment_settings: Vec<crate::AssignmentSettingsRule>,
    pub parent_session_id: Option<String>,
    pub evaluation: Option<EvaluationReference>,
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationReference {
    pub run_id: String,
    pub scenario_id: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSettings {
    pub model: Option<String>,
    pub effort: Option<String>,
    /// A native mode or a requested restriction, never an inferred guarantee.
    pub permission_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAttemptRef {
    pub task_id: String,
    /// Zero is permitted for proposals before the first execution attempt.
    pub attempt: usize,
}
impl From<&Task> for TaskAttemptRef {
    fn from(task: &Task) -> Self {
        Self {
            task_id: task.id.clone(),
            attempt: task.attempts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationState {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl InvocationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentRecord {
    pub id: String,
    pub session_id: String,
    pub task: Option<TaskAttemptRef>,
    pub agent_id: String,
    /// Digest of the actual profile/provider configuration used for this work.
    /// Native authentication values are never copied into this record.
    pub agent_config_version: String,
    pub provider_id: String,
    pub purpose: String,
    pub reason: String,
    pub cwd: PathBuf,
    pub requested: ExecutionSettings,
    pub timeout_secs: u64,
    pub grant_ids: Vec<String>,
    pub context: Vec<ContextReference>,
    pub state: InvocationState,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextKind {
    Message,
    Memory,
    Task,
    Result,
    Session,
    Prompt,
    ProfileInstructions,
    NativeContinuation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReference {
    pub kind: ContextKind,
    pub id: String,
    pub session_id: Option<String>,
    pub digest: Option<String>,
    /// The number of characters included when a source was truncated.
    pub included_chars: Option<usize>,
}

/// Runtime-selected implementation, distinct from provider/model observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionBackendIdentity {
    pub id: String,
    pub version: String,
}

impl ExecutionBackendIdentity {
    pub fn validate(&self) -> anyhow::Result<()> {
        for value in [&self.id, &self.version] {
            anyhow::ensure!(
                !value.is_empty()
                    && value.len() <= 128
                    && value.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-+".contains(&c)),
                "Execution backend ID and version must be nonempty identifiers of at most 128 ASCII letters, digits, dots, underscores, hyphens or plus signs"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationRecord {
    pub id: String,
    pub session_id: String,
    pub assignment_id: String,
    /// Captured before execution; legacy records retain unknown implementation.
    #[serde(default)]
    pub execution_backend: Option<ExecutionBackendIdentity>,
    /// Existing per-session usage ordinal, retained for backward compatibility.
    pub turn: u64,
    pub requested: ExecutionSettings,
    pub sent: ExecutionSettings,
    pub reported: ExecutionSettings,
    pub resumed_from: Option<String>,
    pub native_session_id: Option<String>,
    pub native_turn_id: Option<String>,
    pub native_version: Option<String>,
    pub state: InvocationState,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub usage: Option<UsageSnapshot>,
    /// Runtime classification only; raw SDK errors can contain credentials.
    pub terminal_reason: Option<String>,
}

/// Allowlisted observations only. Do not pass raw provider payloads or reasoning
/// deltas. Each observation is retained as an event, including changed settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationObservation {
    /// Native tool/approval restrictions are not an OS containment guarantee.
    /// Retained in typed observation events, without native configuration secrets.
    #[serde(default)]
    pub permission_limitations: Vec<String>,
    pub sent: Option<ExecutionSettings>,
    pub reported: Option<ExecutionSettings>,
    pub native_session_id: Option<String>,
    pub native_turn_id: Option<String>,
    pub native_version: Option<String>,
    pub usage: Option<UsageSnapshot>,
}

/// An exact proposal version and the native invocation that produced it. Keeping
/// the structured plan here binds reviews and commitments to the same content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanVersion {
    pub proposal_id: String,
    pub revision: usize,
    pub producer_assignment_id: String,
    pub producer_invocation_id: String,
    pub plan: Plan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordLinks {
    #[serde(default)]
    pub workspace_access: Option<crate::WorkspaceAccessDecision>,
    #[serde(default)]
    pub workspace_wait: Option<crate::WorkspaceWait>,
    pub task: Option<TaskAttemptRef>,
    pub related_task_ids: Vec<String>,
    pub assignment_id: Option<String>,
    pub invocation_id: Option<String>,
    pub grant_ids: Vec<String>,
    pub review_ids: Vec<String>,
    pub confirmation_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub plan_proposal: Option<PlanVersion>,
    #[serde(default)]
    pub acceptance_contract: Option<crate::CapturedAcceptanceContract>,
    #[serde(default)]
    pub result: Option<crate::ResultVersion>,
    #[serde(default)]
    pub check: Option<crate::CheckEvidence>,
    #[serde(default)]
    pub observation_id: Option<String>,
    #[serde(default)]
    pub allocation: Option<Box<crate::AllocationDecision>>,
    #[serde(default)]
    pub resource_allocation: Option<Box<crate::ResourceAllocationDecision>>,
}

/// A concise runtime decision with explicit supporting record identities. Future
/// acceptance/grant implementations can append decisions without changing tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub actor: Option<String>,
    pub reason: String,
    /// The provenance layer does not grade evidence. Older or unclassified
    /// acceptances remain unknown until the acceptance runtime supplies a basis.
    pub outcome: Option<DecisionOutcome>,
    pub links: RecordLinks,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStatus {
    Unknown,
    Unconfirmed,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum DecisionOutcome {
    Accepted { confirmation: ConfirmationStatus },
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ProvenanceEvent {
    SessionCaptured {
        policy: Box<SessionPolicy>,
    },
    AssignmentStarted {
        assignment: Box<AssignmentRecord>,
        invocation: Box<InvocationRecord>,
    },
    InvocationObserved {
        invocation_id: String,
        observation: Box<InvocationObservation>,
    },
    InvocationFinished {
        invocation: Box<InvocationRecord>,
    },
    GrantIssued {
        grant: Box<crate::GrantRecord>,
    },
    GrantRevoked {
        grant: Box<crate::GrantRecord>,
    },
    TeamOperationCommitted {
        grant_id: String,
        assignment_id: String,
        invocation_id: String,
        agent_id: String,
        request_id: String,
        operation: crate::TeamOperation,
        message_seq: Option<i64>,
        memory_id: Option<String>,
    },
    DecisionRecorded {
        decision: Box<DecisionRecord>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub seq: i64,
    pub session_id: String,
    pub kind: String,
    pub data: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTrace {
    pub schema_version: u32,
    pub session: Session,
    pub policy: Option<SessionPolicy>,
    pub tasks: Vec<Task>,
    pub assignments: Vec<AssignmentRecord>,
    pub invocations: Vec<InvocationRecord>,
    pub decisions: Vec<DecisionRecord>,
    pub usage: SessionUsage,
    #[serde(default)]
    pub budget: Option<crate::SessionBudget>,
    #[serde(default)]
    pub team_state: Option<crate::TeamState>,
    pub history: Vec<HistoryEvent>,
}

#[cfg(test)]
mod execution_version_tests {
    use super::*;

    #[test]
    fn historical_observation_identity_survives_absent_backend_metadata() {
        let invocation: InvocationRecord = serde_json::from_value(serde_json::json!({
            "id":"invocation", "session_id":"session", "assignment_id":"assignment",
            "turn":1, "requested":{}, "sent":{}, "reported":{},
            "state":"completed", "started_at":"2026-09-12T00:00:00Z"
        }))
        .unwrap();
        assert!(invocation.execution_backend.is_none());
        assert_eq!(
            effective_execution_version("historical-config", &invocation).unwrap(),
            "ca91aef9422e5f35948eaf04"
        );
    }
}
