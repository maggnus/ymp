//! Codex App Server token accounting.

use serde_json::Value;
use ymp_kernel::execution::ObservedUsage;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TokenCounts {
    pub(super) input: Option<u64>,
    pub(super) output: Option<u64>,
    pub(super) cache_read: Option<u64>,
    pub(super) cache_write: Option<u64>,
    pub(super) reasoning: Option<u64>,
}

impl TokenCounts {
    pub(super) const fn zero() -> Self {
        Self {
            input: Some(0),
            output: Some(0),
            cache_read: Some(0),
            cache_write: Some(0),
            reasoning: Some(0),
        }
    }

    fn checked_difference(&self, previous: &Self) -> Option<Self> {
        fn difference(current: Option<u64>, previous: Option<u64>) -> Option<u64> {
            current
                .zip(previous)
                .and_then(|(current, previous)| current.checked_sub(previous))
        }
        Some(Self {
            input: Some(difference(self.input, previous.input)?),
            output: Some(difference(self.output, previous.output)?),
            cache_read: difference(self.cache_read, previous.cache_read),
            cache_write: difference(self.cache_write, previous.cache_write),
            reasoning: difference(self.reasoning, previous.reasoning),
        })
    }

    pub(super) fn into_usage(self, partial: bool) -> ObservedUsage {
        let mut usage = ObservedUsage::unknown().with_partial(partial);
        if let Some(value) = self.input {
            usage = usage.with_input_tokens(value);
        }
        if let Some(value) = self.output {
            usage = usage.with_output_tokens(value);
        }
        if let Some(value) = self.cache_read {
            usage = usage.with_cache_read_tokens(value);
        }
        if let Some(value) = self.cache_write {
            usage = usage.with_cache_write_tokens(value);
        }
        if let Some(value) = self.reasoning {
            usage = usage.with_reasoning_tokens(value);
        }
        usage
    }
}

pub(super) fn codex_counts(raw: &Value) -> TokenCounts {
    TokenCounts {
        input: raw.get("inputTokens").and_then(Value::as_u64),
        output: raw.get("outputTokens").and_then(Value::as_u64),
        cache_read: raw.get("cachedInputTokens").and_then(Value::as_u64),
        cache_write: raw.get("cacheWriteInputTokens").and_then(Value::as_u64),
        reasoning: raw.get("reasoningOutputTokens").and_then(Value::as_u64),
    }
}

/// Converts cumulative native counters to cumulative usage for this invocation.
pub(super) struct CodexUsage {
    baseline: Option<TokenCounts>,
    partial: bool,
    baseline_confirmed: bool,
    latest: Option<ObservedUsage>,
}

impl CodexUsage {
    pub(super) fn new(resumed: bool, baseline: Option<TokenCounts>) -> Self {
        Self {
            baseline: baseline.or_else(|| (!resumed).then(TokenCounts::zero)),
            partial: false,
            baseline_confirmed: !resumed,
            latest: None,
        }
    }

    /// Records the cumulative total restored before the resumed turn starts.
    pub(super) fn restored(&mut self, raw: &Value) {
        if self.latest.is_none() {
            let counts = codex_counts(raw.get("total").unwrap_or(&Value::Null));
            if counts.input.is_some() && counts.output.is_some() {
                self.baseline = Some(counts);
                self.baseline_confirmed = true;
            }
        }
    }

    pub(super) fn update(&mut self, raw: &Value) -> Option<ObservedUsage> {
        let total = codex_counts(raw.get("total").unwrap_or(&Value::Null));
        if total.input.is_none() || total.output.is_none() {
            return None;
        }
        let last = codex_counts(raw.get("last").unwrap_or(&Value::Null));
        if self.latest.is_none() && total.input == last.input && total.output == last.output {
            // Some Codex versions reset cumulative counters when a thread is
            // resumed in another process. Equality proves a zero baseline.
            self.baseline = Some(TokenCounts::zero());
            self.baseline_confirmed = true;
        }
        if !self.baseline_confirmed {
            self.baseline = None;
        }

        let counts = match self
            .baseline
            .as_ref()
            .and_then(|baseline| total.checked_difference(baseline))
        {
            Some(counts) => counts,
            None => {
                // With a missing or reset baseline only the last reported API
                // response is known. Subsequent totals use the inferred new
                // baseline while the partial flag remains sticky.
                self.partial = true;
                self.baseline = total.checked_difference(&last);
                self.baseline_confirmed = true;
                last
            }
        };
        let usage = counts.into_usage(self.partial);
        self.latest = Some(usage);
        Some(usage)
    }

    pub(super) const fn finish(&self) -> Option<ObservedUsage> {
        self.latest
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{CodexUsage, TokenCounts};

    fn sample(input: u64, output: u64) -> Value {
        json!({
            "total": {
                "inputTokens": input,
                "outputTokens": output,
                "cachedInputTokens": 0,
                "cacheWriteInputTokens": 0,
                "reasoningOutputTokens": 0
            },
            "last": {"inputTokens": 100, "outputTokens": 10}
        })
    }

    #[test]
    fn resumed_history_is_subtracted_and_repeated_reports_do_not_accumulate() {
        let baseline = TokenCounts {
            input: Some(1000),
            output: Some(100),
            ..TokenCounts::zero()
        };
        let mut tracker = CodexUsage::new(true, Some(baseline));
        tracker.restored(&sample(1000, 100));
        assert_eq!(
            tracker.update(&sample(1100, 110)).unwrap().input_tokens(),
            Some(100)
        );
        assert_eq!(
            tracker.update(&sample(1100, 110)).unwrap().input_tokens(),
            Some(100)
        );
        assert_eq!(
            tracker.update(&sample(1200, 130)).unwrap().output_tokens(),
            Some(30)
        );
        assert!(!tracker.finish().unwrap().is_partial());
    }

    #[test]
    fn missing_or_reset_baseline_is_partial_instead_of_full_history() {
        let mut tracker = CodexUsage::new(true, None);
        let first = tracker.update(&sample(10_000, 1_000)).unwrap();
        assert_eq!(first.input_tokens(), Some(100));
        assert_eq!(first.output_tokens(), Some(10));
        assert!(first.is_partial());
        assert_eq!(
            tracker
                .update(&sample(10_200, 1_020))
                .unwrap()
                .input_tokens(),
            Some(300)
        );
        let reset = tracker.update(&sample(300, 30)).unwrap();
        assert_eq!(reset.input_tokens(), Some(100));
        assert!(reset.is_partial());
    }

    #[test]
    fn equal_first_total_and_last_prove_a_zero_baseline_after_resume() {
        let prior = TokenCounts {
            input: Some(50),
            output: Some(5),
            ..TokenCounts::zero()
        };
        let mut tracker = CodexUsage::new(true, Some(prior));
        let data = json!({
            "total": {"inputTokens": 100, "outputTokens": 10},
            "last": {"inputTokens": 100, "outputTokens": 10}
        });
        let usage = tracker.update(&data).unwrap();
        assert_eq!(usage.input_tokens(), Some(100));
        assert_eq!(usage.output_tokens(), Some(10));
        assert!(!usage.is_partial());
    }
}
