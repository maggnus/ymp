use serde_json::Value;
use ymp_core::{TokenCounts, UsageSnapshot};

pub fn codex_counts(raw: &Value) -> TokenCounts {
    TokenCounts {
        input: raw["inputTokens"].as_u64(),
        output: raw["outputTokens"].as_u64(),
        cache_read: raw["cachedInputTokens"].as_u64(),
        cache_write: raw["cacheWriteInputTokens"].as_u64(),
        reasoning: raw["reasoningOutputTokens"].as_u64(),
    }
}

pub struct CodexUsage {
    baseline: Option<TokenCounts>,
    partial: bool,
    baseline_confirmed: bool,
    latest: Option<UsageSnapshot>,
}
impl CodexUsage {
    pub fn new(resumed: bool, baseline: Option<TokenCounts>) -> Self {
        Self {
            baseline: baseline.or_else(|| (!resumed).then(TokenCounts::zero)),
            partial: false,
            baseline_confirmed: !resumed,
            latest: None,
        }
    }
    pub fn restored(&mut self, raw: &Value) {
        if self.latest.is_none() {
            let counts = codex_counts(&raw["total"]);
            if counts.input.is_some() && counts.output.is_some() {
                self.baseline = Some(counts);
                self.baseline_confirmed = true;
            }
        }
    }
    pub fn update(&mut self, raw: &Value) -> Option<UsageSnapshot> {
        let total = codex_counts(&raw["total"]);
        if total.input.is_none() || total.output.is_none() {
            return None;
        }
        let last = codex_counts(&raw["last"]);
        if self.latest.is_none() && total.input == last.input && total.output == last.output {
            // Some CLI versions reset accounting when a native thread is resumed
            // in a new process. A first total equal to last proves a zero baseline.
            self.baseline = Some(TokenCounts::zero());
            self.baseline_confirmed = true;
        }
        if !self.baseline_confirmed {
            self.baseline = None;
        }

        let counts = match self
            .baseline
            .as_ref()
            .and_then(|b| total.checked_difference(b))
        {
            Some(counts) => counts,
            None => {
                // Missing/reset native baseline: account only the last known API
                // response, then use the new baseline for subsequent notifications.
                self.partial = true;
                self.baseline = total.checked_difference(&last);
                self.baseline_confirmed = true;
                last
            }
        };
        let snapshot = UsageSnapshot {
            counts,
            finalized: false,
            partial: self.partial,
            note: self
                .partial
                .then(|| "Native baseline unavailable or reset; partial accounting".into()),
            native_total: Some(total),
        };
        self.latest = Some(snapshot.clone());
        Some(snapshot)
    }
    pub fn finish(&self) -> Option<UsageSnapshot> {
        self.latest.clone().map(|mut s| {
            s.finalized = true;
            s
        })
    }
}

/// The installed GLM ACP implementation returns the last API response, not the
/// entire tool loop. Keep its known contribution, explicitly partial.
pub fn acp_usage(raw: &Value) -> Option<UsageSnapshot> {
    let counts = TokenCounts {
        input: raw["inputTokens"].as_u64(),
        output: raw["outputTokens"].as_u64(),
        cache_read: raw["cachedReadTokens"].as_u64(),
        cache_write: raw["cachedWriteTokens"].as_u64(),
        reasoning: raw["thoughtTokens"].as_u64(),
    };
    counts.known_total()?;
    Some(UsageSnapshot {
        counts,
        finalized: true,
        partial: true,
        note: Some("ACP reports the last model request only".into()),
        native_total: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn sample(input: u64, output: u64) -> Value {
        json!({"total":{"inputTokens":input,"outputTokens":output,"cachedInputTokens":0,"cacheWriteInputTokens":0,"reasoningOutputTokens":0},"last":{"inputTokens":100,"outputTokens":10}})
    }
    #[test]
    fn resumed_thread_history_is_subtracted_and_repeats_do_not_accumulate() {
        let baseline = TokenCounts {
            input: Some(1000),
            output: Some(100),
            ..TokenCounts::zero()
        };
        let mut tracker = CodexUsage::new(true, Some(baseline));
        tracker.restored(&sample(1000, 100));
        assert_eq!(
            tracker
                .update(&sample(1100, 110))
                .unwrap()
                .counts
                .known_total(),
            Some(110)
        );
        assert_eq!(
            tracker
                .update(&sample(1100, 110))
                .unwrap()
                .counts
                .known_total(),
            Some(110)
        );
        assert_eq!(
            tracker
                .update(&sample(1200, 130))
                .unwrap()
                .counts
                .known_total(),
            Some(230)
        );
        assert!(!tracker.finish().unwrap().partial);
    }
    #[test]
    fn missing_or_reset_baseline_is_partial_not_full_history() {
        let mut tracker = CodexUsage::new(true, None);
        let snapshot = tracker.update(&sample(10000, 1000)).unwrap();
        assert_eq!(snapshot.counts.known_total(), Some(110));
        assert!(snapshot.partial);
        assert_eq!(
            tracker
                .update(&sample(10200, 1020))
                .unwrap()
                .counts
                .known_total(),
            Some(330)
        );
        let reset = tracker.update(&sample(300, 30)).unwrap();
        assert_eq!(reset.counts.known_total(), Some(110));
        assert!(reset.partial);
    }
    #[test]
    fn a_new_process_can_reset_totals_even_if_its_new_numbers_are_larger() {
        let prior = TokenCounts {
            input: Some(50),
            output: Some(5),
            ..TokenCounts::zero()
        };
        let mut tracker = CodexUsage::new(true, Some(prior));
        let data = serde_json::json!({"total":{"inputTokens":100,"outputTokens":10},"last":{"inputTokens":100,"outputTokens":10}});
        let snapshot = tracker.update(&data).unwrap();
        assert_eq!(snapshot.counts.known_total(), Some(110));
        assert!(!snapshot.partial);
    }
    #[test]
    fn acp_usage_is_not_assumed_to_cover_a_whole_turn() {
        let snapshot = acp_usage(
            &json!({"inputTokens":100,"outputTokens":20,"cachedReadTokens":80,"thoughtTokens":10}),
        )
        .unwrap();
        assert_eq!(snapshot.counts.known_total(), Some(120));
        assert!(snapshot.partial);
        assert!(acp_usage(&json!({})).is_none());
    }
}
