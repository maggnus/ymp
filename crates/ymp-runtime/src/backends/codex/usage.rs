//! Normalize thread cumulative usage without inventing unreported coverage.
use super::protocol::denied;
use serde_json::Value;
use ymp_domain::{Result, resources::Usage};
pub fn decode(value: &Value) -> Result<Usage> {
    let count = |key: &str| value[key].as_u64().ok_or_else(|| denied("codex_usage"));
    let usage = Usage {
        input: count("inputTokens")?,
        cache_read: count("cachedInputTokens")?,
        cache_write: match value.get("cacheWriteInputTokens") {
            None => 0,
            Some(n) => n.as_u64().ok_or_else(|| denied("codex_usage"))?,
        },
        output: count("outputTokens")?,
        reasoning: Some(count("reasoningOutputTokens")?),
    };
    usage.validate()?;
    if usage.input.checked_add(usage.output) != Some(count("totalTokens")?) {
        return Err(denied("codex_usage"));
    }
    Ok(usage)
}
pub(crate) fn zero() -> Usage {
    Usage {
        input: 0,
        cache_read: 0,
        cache_write: 0,
        output: 0,
        reasoning: Some(0),
    }
}
