//! Normalize native token reports to A13 without inventing unreported coverage.
use super::stream::refused;
use serde_json::Value;
use ymp_domain::{Result, resources::Usage};

fn count(value: &Value, key: &str) -> Result<u64> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(number) => number.as_u64().ok_or_else(|| refused("claude_usage")),
    }
}
/// Native input excludes cached tokens; A13 input includes them.
fn normalized(
    uncached: u64,
    read: u64,
    write: u64,
    output: u64,
    reasoning: Option<u64>,
) -> Result<Usage> {
    let usage = Usage {
        input: uncached
            .checked_add(read)
            .and_then(|sum| sum.checked_add(write))
            .ok_or_else(|| refused("claude_usage"))?,
        cache_read: read,
        cache_write: write,
        output,
        reasoning,
    };
    usage.validate().map_err(|_| refused("claude_usage"))?;
    Ok(usage)
}
/// Input side of one in-progress message. Its streamed output count is a
/// placeholder, so this is only a lower bound for partial coverage.
pub fn request(usage: &Value) -> Result<Usage> {
    normalized(
        usage["input_tokens"]
            .as_u64()
            .ok_or_else(|| refused("claude_usage"))?,
        count(usage, "cache_read_input_tokens")?,
        count(usage, "cache_creation_input_tokens")?,
        0,
        None,
    )
}
pub fn sum(left: &Usage, right: &Usage) -> Result<Usage> {
    let add = |a: u64, b: u64| a.checked_add(b).ok_or_else(|| refused("claude_usage"));
    Ok(Usage {
        input: add(left.input, right.input)?,
        cache_read: add(left.cache_read, right.cache_read)?,
        cache_write: add(left.cache_write, right.cache_write)?,
        output: add(left.output, right.output)?,
        reasoning: None,
    })
}
/// Final native totals and whether the per-model report confirms them for
/// exactly the requested model. Client cost estimates are not read.
pub fn settled(result: &Value, model: &str) -> Result<(Usage, bool)> {
    let total = &result["usage"];
    let uncached = total["input_tokens"]
        .as_u64()
        .ok_or_else(|| refused("claude_usage"))?;
    let read = count(total, "cache_read_input_tokens")?;
    let write = count(total, "cache_creation_input_tokens")?;
    let output = total["output_tokens"]
        .as_u64()
        .ok_or_else(|| refused("claude_usage"))?;
    let reasoning = match total["output_tokens_details"].get("thinking_tokens") {
        None | Some(Value::Null) => None,
        Some(number) => Some(number.as_u64().ok_or_else(|| refused("claude_usage"))?),
    };
    let usage = normalized(uncached, read, write, output, reasoning)?;
    let confirmed = result["modelUsage"].as_object().is_some_and(|models| {
        models.len() == 1
            && models.get(model).is_some_and(|own| {
                own["inputTokens"].as_u64() == Some(uncached)
                    && own["cacheReadInputTokens"].as_u64() == Some(read)
                    && own["cacheCreationInputTokens"].as_u64() == Some(write)
                    && own["outputTokens"].as_u64() == Some(output)
                    && own
                        .get("thinkingTokens")
                        .is_none_or(|thinking| thinking.as_u64() == reasoning)
            })
    });
    Ok((usage, confirmed))
}
