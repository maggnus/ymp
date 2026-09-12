//! Transport result limits, separate from automatic prompt-context allowances.
use anyhow::{ensure, Result};
use serde::Serialize;
use serde_json::{json, Value};

pub const MAX_PAGE_BYTES: usize = 48 * 1024;
pub const MAX_ROW_BYTES: usize = 8 * 1024;
pub const MAX_PAGE_ITEMS: usize = 25;

/// Bound every row before constructing a page. Snapshot bytes and trusted-check
/// expected bytes are never metadata; long strings/collections are explicitly marked.
pub fn metadata<T: Serialize>(value: &T) -> Result<Value> {
    let mut value = serde_json::to_value(value)?;
    let mut truncated = false;
    clip(&mut value, 0, &mut truncated);
    if serde_json::to_vec(&value)?.len() > MAX_ROW_BYTES {
        let retained = [
            "id",
            "session_id",
            "source_session",
            "result_id",
            "result_version",
            "version",
            "status",
            "kind",
            "confirmation",
        ];
        let mut identity = serde_json::Map::new();
        for key in retained {
            if let Some(v) = value.get(key) {
                let scalar = match v {
                    Value::String(s) => Value::String(s.chars().take(128).collect()),
                    Value::Number(_) | Value::Bool(_) | Value::Null => v.clone(),
                    _ => Value::Null,
                };
                identity.insert(key.into(), scalar);
            }
        }
        value = Value::Object(identity);
        truncated = true;
    }
    Ok(json!({"value":value,"truncated":truncated}))
}

fn clip(value: &mut Value, depth: usize, truncated: &mut bool) {
    if depth > 10 {
        *value = Value::Null;
        *truncated = true;
        return;
    }
    match value {
        Value::String(s) if s.chars().count() > 1024 => {
            *s = s.chars().take(1024).collect();
            *truncated = true;
        }
        Value::Array(items) => {
            if items.len() > 32 {
                items.truncate(32);
                *truncated = true;
            }
            for item in items {
                clip(item, depth + 1, truncated);
            }
        }
        Value::Object(fields) => {
            for key in ["bytes", "expected"] {
                if fields.remove(key).is_some_and(|v| !v.is_null()) {
                    *truncated = true;
                }
            }
            for item in fields.values_mut() {
                clip(item, depth + 1, truncated);
            }
        }
        _ => {}
    }
}

pub fn page<T: Serialize>(items: &[T], offset: usize, limit: usize) -> Result<Value> {
    ensure!(
        (1..=MAX_PAGE_ITEMS).contains(&limit),
        "limit must be between 1 and 25"
    );
    ensure!(
        offset <= items.len(),
        "cursor is beyond this result set; restart pagination"
    );
    let mut rows = Vec::new();
    let mut size = 0;
    for item in items.iter().skip(offset).take(limit) {
        let row = metadata(item)?;
        let bytes = serde_json::to_vec(&row)?.len();
        if size + bytes > MAX_PAGE_BYTES {
            break;
        }
        size += bytes;
        rows.push(row);
    }
    let next = offset + rows.len();
    Ok(
        json!({"items":rows,"next_cursor":(next < items.len()).then(|| next.to_string()),"total":items.len(),"schema_version":1}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_full_candidate_rows_and_snapshot_bytes() {
        let rows: Vec<_> = (0..25).map(|n| json!({"id":n,"content":"x".repeat(30_000),"snapshot":{"bytes":vec![65;50_000]}})).collect();
        let page = page(&rows, 0, 25).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() < MAX_PAGE_BYTES + 4096);
        assert_eq!(page["items"][0]["truncated"], true);
        assert!(page["items"][0]["value"]["snapshot"].get("bytes").is_none());
        assert_eq!(
            page["items"][0]["value"]["content"].as_str().unwrap().len(),
            1024
        );
        assert!(super::page(&rows, 0, 26).is_err());
    }
}
