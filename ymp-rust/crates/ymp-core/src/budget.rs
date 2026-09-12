//! Supported admission units. Token reservations are allowances, not a promise
//! that an opaque native runtime cannot overshoot them.
use crate::{Limits, UsageTotals};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceLimits {
    pub startup_invocations: u64,
    pub required_review_invocations: u64,
    pub max_context_chars: u64,
    pub startup_context_chars: u64,
    pub max_output_chars: u64,
    pub native_max_turns: u64,
    /// Admission ceiling on reported raw input + output (cache already included).
    pub observed_tokens: Option<u64>,
    /// Default reservation and ceiling for smaller per-assignment allowances;
    /// never substituted for observed native use.
    pub invocation_tokens: Option<u64>,
    /// Combined protection for required review; None retains the legacy calculation.
    pub review_reserve_tokens: Option<u64>,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            startup_invocations: 6,
            required_review_invocations: 2,
            max_context_chars: 128_000,
            startup_context_chars: 32_000,
            max_output_chars: 64_000,
            native_max_turns: 16,
            observed_tokens: None,
            invocation_tokens: None,
            review_reserve_tokens: None,
        }
    }
}
impl ResourceLimits {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.startup_invocations >= 2
                && self.required_review_invocations > 0
                && self.max_context_chars > 0
                && self.startup_context_chars > 0
                && self.startup_context_chars <= self.max_context_chars
                && self.max_output_chars > 0
                && self.native_max_turns > 0,
            "Invalid session resource limits"
        );
        ensure!(matches!((self.observed_tokens, self.invocation_tokens), (None, None))
            || matches!((self.observed_tokens, self.invocation_tokens), (Some(total), Some(each)) if each > 0 && total >= each),
            "Observed token admission requires a positive per-invocation reservation within the total");
        ensure!(
            self.review_reserve_tokens
                .is_none_or(|reserve| reserve > 0
                    && self.observed_tokens.is_some_and(|total| reserve <= total)),
            "Review token reserve requires a positive captured token ceiling and cannot exceed it"
        );
        Ok(())
    }
    pub fn context_chars(&self, purpose: &str) -> u64 {
        if startup_purpose(purpose) {
            self.startup_context_chars
        } else {
            self.max_context_chars
        }
    }
}
pub fn startup_purpose(purpose: &str) -> bool {
    matches!(purpose, "plan" | "review_plan")
}
pub fn required_review_purpose(purpose: &str) -> bool {
    matches!(purpose, "review" | "final_review")
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeResourceControls {
    pub max_turns: Option<u64>,
    pub max_output_chars: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBudget {
    pub limits: Limits,
    pub admitted_invocations: u64,
    pub in_flight_invocations: u64,
    pub startup_invocations: u64,
    pub protected_review_invocations: u64,
    #[serde(default)]
    pub protected_review_tokens: Option<u64>,
    pub reserved_tokens: Option<u64>,
    pub observed_usage: UsageTotals,
    pub observed_token_overshoot: Option<u64>,
    pub last_denial: Option<BudgetDenial>,
    /// No installed native backend supports a proved hard whole-run token cap.
    pub strict_token_bound: bool,
}
impl SessionBudget {
    pub fn require_strict_token_bound(&self) -> Result<()> {
        ensure!(
            !self.observed_usage.is_partial(),
            "incomplete_native_accounting: strict equal-budget claim is unavailable"
        );
        ensure!(
            self.strict_token_bound,
            "opaque_native_token_bound: in-flight token overshoot has no finite proved bound"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetDenial {
    pub code: String,
    pub message: String,
    pub purpose: String,
    pub at: String,
}
impl std::fmt::Display for BudgetDenial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for BudgetDenial {}
