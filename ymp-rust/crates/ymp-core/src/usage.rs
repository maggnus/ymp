//! Provider-reported token accounting. Input includes cache reads/writes; output
//! includes reasoning. The cache and reasoning fields are subsets, not extra totals.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCounts {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub reasoning: Option<u64>,
}

impl TokenCounts {
    pub fn zero() -> Self {
        Self {
            input: Some(0),
            output: Some(0),
            cache_read: Some(0),
            cache_write: Some(0),
            reasoning: Some(0),
        }
    }
    pub fn known_total(&self) -> Option<u64> {
        add(self.input, self.output)
    }
    pub fn add_assign(&mut self, other: &Self) {
        self.input = add(self.input, other.input);
        self.output = add(self.output, other.output);
        self.cache_read = add(self.cache_read, other.cache_read);
        self.cache_write = add(self.cache_write, other.cache_write);
        self.reasoning = add(self.reasoning, other.reasoning);
    }
    pub fn checked_difference(&self, previous: &Self) -> Option<Self> {
        fn difference(a: Option<u64>, b: Option<u64>) -> Option<u64> {
            a.zip(b).and_then(|(a, b)| a.checked_sub(b))
        }
        let input = difference(self.input, previous.input)?;
        let output = difference(self.output, previous.output)?;
        Some(Self {
            input: Some(input),
            output: Some(output),
            cache_read: difference(self.cache_read, previous.cache_read),
            cache_write: difference(self.cache_write, previous.cache_write),
            reasoning: difference(self.reasoning, previous.reasoning),
        })
    }
}
fn add(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.saturating_add(b)),
        (a, b) => a.or(b),
    }
}

/// A replacement snapshot for one ymp agent invocation, never an additive delta.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub counts: TokenCounts,
    pub finalized: bool,
    pub partial: bool,
    #[serde(default)]
    pub note: Option<String>,
    /// Native cumulative counters, used only to establish the next Codex baseline.
    #[serde(default)]
    pub native_total: Option<TokenCounts>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageTotals {
    pub counts: TokenCounts,
    pub calls: u64,
    pub reported: u64,
    pub partial_calls: u64,
    pub open_calls: u64,
}
impl UsageTotals {
    pub fn known_total(&self) -> Option<u64> {
        if self.calls == 0 {
            Some(0)
        } else {
            self.counts.known_total()
        }
    }
    pub fn is_partial(&self) -> bool {
        self.reported < self.calls || self.partial_calls > 0 || self.open_calls > 0
    }
    pub fn include(&mut self, snapshot: Option<&UsageSnapshot>, open: bool) {
        self.calls += 1;
        self.open_calls += u64::from(open);
        if let Some(s) = snapshot {
            if s.counts.known_total().is_some() {
                self.reported += 1;
            }
            self.counts.add_assign(&s.counts);
            self.partial_calls += u64::from(
                s.partial || !s.finalized || s.counts.input.is_none() || s.counts.output.is_none(),
            );
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUsage {
    pub total: UsageTotals,
    /// Keys are agent/profile IDs within this session, never provider IDs.
    pub agents: BTreeMap<String, UsageTotals>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_and_reasoning_are_not_counted_twice() {
        let c = TokenCounts {
            input: Some(100),
            output: Some(40),
            cache_read: Some(80),
            cache_write: Some(10),
            reasoning: Some(30),
        };
        assert_eq!(c.known_total(), Some(140));
    }
    #[test]
    fn missing_usage_is_distinct_from_zero() {
        let mut total = UsageTotals::default();
        assert_eq!(total.known_total(), Some(0));
        total.include(None, false);
        assert_eq!(total.known_total(), None);
        assert!(total.is_partial());
        total.include(
            Some(&UsageSnapshot {
                counts: TokenCounts::zero(),
                finalized: true,
                ..Default::default()
            }),
            false,
        );
        assert_eq!(total.known_total(), Some(0));
        assert!(total.is_partial());
    }
}
