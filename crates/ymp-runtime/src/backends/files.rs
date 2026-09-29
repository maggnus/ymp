//! One file operation requested through a native tool call, shared by the process
//! backends. Tool names, schemas and reply envelopes stay with the provider; grants,
//! ownership and path scope stay with the host mediator.
use serde_json::Value;
use ymp_domain::{Denial, Result, workspace::WorkspacePath};
use ymp_kernel::ports::execution::InvocationFiles;

/// Largest text written or read by one operation, in bytes.
const BYTES: u64 = 65536;

/// Checks the two arguments of a read (`path`, `limit`) or a write (`path`,
/// `text`) and performs it once through the mediator.
pub(crate) fn operate(
    write: bool,
    arguments: &Value,
    files: Option<&dyn InvocationFiles>,
    refuse: fn(&str) -> Denial,
) -> Result<String> {
    let arguments = arguments
        .as_object()
        .filter(|arguments| arguments.len() == 2)
        .ok_or_else(|| refuse("tool"))?;
    let path = WorkspacePath::new(
        arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| refuse("tool"))?,
    )?;
    let files = files.ok_or_else(|| refuse("tool"))?;
    if write {
        let text = arguments
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| text.len() as u64 <= BYTES)
            .ok_or_else(|| refuse("tool"))?;
        files.write(&path, text.as_bytes())?;
        return Ok("Written by the host mediator".into());
    }
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .filter(|limit| (1..=BYTES).contains(limit))
        .ok_or_else(|| refuse("tool"))?;
    String::from_utf8(files.read(&path, limit as usize)?).map_err(|_| refuse("file_encoding"))
}

/// Whether the mediator refused for lack of authority rather than for content.
pub(crate) fn unauthorized(code: &str) -> bool {
    matches!(
        code,
        "workspace_path"
            | "access_scope"
            | "access_closed"
            | "path_conflict"
            | "invocation_authority"
    )
}

/// Text returned to the model for a refused operation; it names only the code.
pub(crate) fn refusal(code: &str) -> String {
    format!("Denied by host: {code}")
}
