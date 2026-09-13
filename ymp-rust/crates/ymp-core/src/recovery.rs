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
/// Durable cause of a stopped stage. Display text never authorizes continuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryWaitReason {
    OwnerWait,
    OwnerPause,
    /// Reconsider through ordinary recovery/allocation after membership changes.
    ParticipantAvailability,
    /// Includes strategy waits/stops and exhausted policy/runtime recovery limits.
    RecoveryPolicy,
    Admission,
    UncertainEffects,
    Cancelled,
    LegacyUnbound,
    EffectsInspected,
    FreshPlanReviewRecorded,
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
    #[serde(default)]
    pub request_digest: Option<String>,
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
    /// Missing legacy metadata is unknown, never implicitly an availability wait.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_reason: Option<RecoveryWaitReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_resolution: Option<RecoveryEffectResolution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fresh_plan_review: Option<FreshPlanReviewState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_hold: Option<RecoveryHoldOrigin>,
    pub condition: Option<String>,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryInput {
    pub schema_version: u32,
    pub stage: RecoveryStage,
    pub failure: Option<InvocationFailure>,
    pub unavailable_agent: Option<String>,
    pub eligible: Vec<AgentProfile>,
    pub outstanding_assignments: Vec<String>,
    #[serde(default)]
    pub responsibilities: Option<Vec<ActiveResponsibility>>,
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
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
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
    ReleaseHold,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryControlReceipt {
    pub command: RecoveryControlCommand,
    pub resulting_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAdmission {
    pub stage_id: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryControlKind {
    Retry,
    Continue,
    Wait,
    Pause,
    ReleaseHold,
    /// Uses Engine::inspect_recovery, not the synchronous control_recovery API.
    InspectEffects,
    /// Uses Engine::review_saved_plan_fresh and never resolves old effects.
    ReviewSavedPlanFresh,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageManualActions {
    pub stage_id: String,
    pub expected_revision: u64,
    pub controls: Vec<RecoveryControlKind>,
}
impl RecoveryStage {
    pub fn effects_resolved(&self) -> bool {
        self.failures.iter().all(|f| {
            f.termination == TerminationEvidence::BackendEnded
                && (f.effective_access.is_read_only()
                    || self
                        .effect_resolution
                        .as_ref()
                        .is_some_and(|r| r.failures.contains(f)))
        })
    }
    pub fn manual_actions(&self) -> StageManualActions {
        let mut controls = Vec::new();
        if self.status != RecoveryStatus::Complete {
            controls.extend([RecoveryControlKind::Wait, RecoveryControlKind::Pause]);
            if matches!(
                self.wait_reason,
                Some(RecoveryWaitReason::OwnerWait | RecoveryWaitReason::OwnerPause)
            ) {
                controls.push(RecoveryControlKind::ReleaseHold);
            }
            if self.status != RecoveryStatus::Running
                && !self.failures.is_empty()
                && self
                    .failures
                    .iter()
                    .all(|f| f.termination == TerminationEvidence::BackendEnded)
            {
                controls.push(RecoveryControlKind::InspectEffects);
            }
            if self.status != RecoveryStatus::Running && self.effects_resolved() {
                controls.extend([RecoveryControlKind::Continue, RecoveryControlKind::Retry]);
            }
            if self.status != RecoveryStatus::Running
                && !self.effects_resolved()
                && self.fresh_plan_review.is_some()
            {
                controls.push(RecoveryControlKind::Continue);
            }
            if self.purpose == "review_plan"
                && self.fresh_plan_review.is_none()
                && !self.failures.is_empty()
                && self.status != RecoveryStatus::Running
            {
                controls.push(RecoveryControlKind::ReviewSavedPlanFresh);
            }
        }
        StageManualActions {
            stage_id: self.id.clone(),
            expected_revision: self.revision,
            controls,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryHoldOrigin {
    pub status: RecoveryStatus,
    pub wait_reason: Option<RecoveryWaitReason>,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshPlanReviewCommand {
    pub session_id: String,
    pub stage_id: String,
    pub expected_revision: u64,
    pub command_id: String,
    pub proposal: PlanVersion,
    pub reviewer_id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshPlanReviewNextAction {
    AwaitOwnerContinuation,
    ConsumeRecordedVerdict,
    VerdictConsumed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshPlanReviewState {
    pub review_record_id: String,
    pub next_action: FreshPlanReviewNextAction,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshPlanReviewRecord {
    pub command: FreshPlanReviewCommand,
    pub prior_failures: Vec<InvocationFailure>,
    pub response: SavedResponse,
    pub approved: bool,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshPlanReviewReceipt {
    pub record: FreshPlanReviewRecord,
    pub review_record_id: String,
    pub resulting_revision: u64,
    pub next_action: FreshPlanReviewNextAction,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedEffectDependencies {
    pub session_id: String,
    pub stage_id: String,
    pub proposal: PlanVersion,
    pub invocation_ids: Vec<String>,
}
impl std::fmt::Display for UnresolvedEffectDependencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unresolved_effect_dependencies: saved plan {} was independently reviewed, but dependencies on historical effects of {} remain unestablished; no production or revision is admitted", self.proposal.proposal_id, self.invocation_ids.join(", "))
    }
}
impl std::error::Error for UnresolvedEffectDependencies {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentFileVersion {
    pub path: std::path::PathBuf,
    pub sha256: Option<String>,
    pub length: Option<u64>,
    pub symlink_target: Option<std::path::PathBuf>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentFilesContext {
    pub session_id: String,
    pub stage_id: String,
    pub stage_revision: u64,
    pub plan: Option<PlanVersion>,
    pub result: Option<ResultVersion>,
    pub task: Option<TaskAttemptRef>,
    pub team_revision: u64,
    pub budget: SessionBudget,
    pub board_version: String,
    pub failures: Vec<InvocationFailure>,
    pub directory: std::path::PathBuf,
    pub files: Vec<CurrentFileVersion>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinueWithCurrentFilesCommand {
    pub command_id: String,
    pub context: CurrentFilesContext,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentFilesAuthorization {
    pub command: ContinueWithCurrentFilesCommand,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentFilesReceipt {
    pub command: ContinueWithCurrentFilesCommand,
    pub authorization_id: String,
    pub resulting_revision: u64,
}

/// Trusted local inspection ingress. It neither retries the failed call nor
/// releases an owner hold. Continue remains a separate versioned owner command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryInspectionCommand {
    pub session_id: String,
    pub stage_id: String,
    pub expected_revision: u64,
    pub command_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectedInvocationEffects {
    pub failure: InvocationFailure,
    pub access_record_id: String,
    pub ended_at: String,
    pub files: Vec<FileSnapshot>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryInspection {
    pub command: RecoveryInspectionCommand,
    pub task: Option<TaskAttemptRef>,
    pub plan: Option<PlanVersion>,
    pub result: Option<ResultVersion>,
    pub effects: Vec<InspectedInvocationEffects>,
    pub reviewer: SavedResponse,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryEffectResolution {
    pub inspection_id: String,
    pub failures: Vec<InvocationFailure>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryInspectionReceipt {
    pub command: RecoveryInspectionCommand,
    pub inspection_id: String,
    pub resulting_revision: u64,
}
