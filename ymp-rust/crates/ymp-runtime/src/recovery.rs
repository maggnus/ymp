//! A synchronous, inference-free proposal boundary. Admission and effects stay in Engine.
use anyhow::Result;
use ymp_core::*;

pub trait RecoveryPolicy: Send + Sync {
    fn identity(&self) -> ExecutionBackendIdentity;
    fn configuration(&self) -> serde_json::Value;
    fn propose(&self, input: &RecoveryInput) -> Result<RecoveryAction>;
}
#[derive(Debug, Default)]
pub struct BoundedRecoveryPolicy(pub RecoveryConfiguration);
impl RecoveryPolicy for BoundedRecoveryPolicy {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.bounded-recovery".into(),
            version: "1".into(),
        }
    }
    fn configuration(&self) -> serde_json::Value {
        serde_json::json!(self.0)
    }
    fn propose(&self, input: &RecoveryInput) -> Result<RecoveryAction> {
        let failure = &input.failure;
        if failure.termination != TerminationEvidence::BackendEnded
            || !failure.effective_access.is_read_only()
        {
            return Ok(RecoveryAction::InspectEffects);
        }
        if input.stage.recovery_attempts >= self.0.max_attempts {
            return Ok(RecoveryAction::Wait {
                condition: "Recovery attempt allowance exhausted; owner action required".into(),
            });
        }
        Ok(match failure.class {
            FailureClass::TransientTransport | FailureClass::MalformedResponse
                if input.provider_failures < self.0.max_provider_failures =>
            {
                RecoveryAction::Retry {
                    delay_ms: self.0.delay_ms,
                }
            }
            FailureClass::TransientTransport
            | FailureClass::Authentication
            | FailureClass::QuotaExhausted
            | FailureClass::UnsupportedConfiguration => RecoveryAction::Reassign,
            FailureClass::Cancelled => RecoveryAction::Wait {
                condition: "Cancellation requires explicit owner continuation".into(),
            },
            FailureClass::Timeout | FailureClass::MalformedResponse | FailureClass::Unknown => {
                RecoveryAction::RequestOwner {
                    reason: "Failure requires diagnosis or a changed condition before continuation"
                        .into(),
                }
            }
        })
    }
}
