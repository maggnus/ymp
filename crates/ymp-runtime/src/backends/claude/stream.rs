//! Bounded stream-json channel to one native Claude Code process. Native output
//! is observation and never grants task authority.
use super::super::process::{self, Process, Wire};
use serde_json::Value;
use std::{path::Path, process::Command, time::Duration};
use ymp_domain::{Denial, Result};

pub(crate) fn refused(code: &str) -> Denial {
    Denial::new(
        code,
        "Claude stream boundary was not established; no credentials or native payload are included",
    )
}

/// Ambient variables would select another endpoint, model, reasoning budget,
/// tool server or nested-session mode.
/// The installation's own stored login stays the only native authentication, so
/// the variable that locates it is kept.
fn detach(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(|name| {
            name != "CLAUDE_CONFIG_DIR"
                && (["CLAUDE", "ANTHROPIC", "MCP_"]
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
                    || ["MAX_THINKING_TOKENS", "NODE_OPTIONS"].contains(&name))
        }) {
            command.env_remove(&name);
        }
    }
    command.env("DISABLE_AUTOUPDATER", "1");
}
/// Refusal for a provider-neutral reason of the shared modules.
pub(crate) fn boundary(reason: &str) -> Denial {
    refused(&format!("claude_{reason}"))
}
pub(crate) fn wire(frame: usize) -> Wire {
    Wire {
        refuse: boundary,
        parse: |line| {
            (!line.iter().all(u8::is_ascii_whitespace))
                .then(|| serde_json::from_slice::<Value>(line).map_err(|_| refused("claude_line")))
        },
        frame,
    }
}
pub(crate) fn open(executable: &Path, arguments: &[String], frame: usize) -> Result<Process> {
    let mut command = Command::new(executable);
    detach(&mut command);
    command.args(arguments);
    Process::launch(command, "claude", wire(frame))
}

/// Observed release of the installed native tool; it starts no session.
pub(crate) fn installed(executable: &Path, timeout: Duration) -> Result<String> {
    let mut command = Command::new(executable);
    detach(&mut command);
    command.arg("--version");
    let (succeeded, text) = process::reported(command, timeout, boundary)?;
    match text.split_once(' ') {
        Some((version, "(Claude Code)"))
            if succeeded && ymp_kernel::ports::execution::claude_version(version) =>
        {
            Ok(version.to_owned())
        }
        _ => Err(refused("claude_version")),
    }
}
