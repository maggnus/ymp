#![forbid(unsafe_code)]

//! Acceptance: the agent bridge writes nothing into the workspace it is started in.
//!
//! The bridge is launched by the runtime with the agent's own workspace as its working directory,
//! and the candidate is that workspace captured. A bridge that addressed a store would create one
//! there, and the candidate an operator exports would carry a directory the agent never wrote —
//! which a live Claude Code run produced before this: `candidate/.ymp/projects/...` beside the
//! file the agent had been asked for.
//!
//! The check that must fail: resolve the store for every invocation again, and the bridge leaves
//! the root marker and the project directory below where it ran.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::TempDir;

#[test]
fn the_agent_bridge_addresses_no_store_where_it_runs() {
    let workspace = TempDir::new().expect("temporary workspace");
    fs::write(workspace.path().join("input.txt"), b"before\n").expect("workspace file");

    let mut bridge = Command::new(env!("CARGO_BIN_EXE_ymp"))
        .current_dir(workspace.path())
        .args(["internal", "agent-mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the agent bridge");
    // One well-formed request is enough: the store, if one were addressed, is addressed before any
    // command runs. The bridge has no controller-bound capability here, so what it answers is
    // beside the point.
    bridge
        .stdin
        .as_mut()
        .expect("bridge input")
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
        .expect("write one request");
    drop(bridge.stdin.take());
    let outcome = bridge.wait_with_output().expect("the bridge ends");

    let left: Vec<String> = fs::read_dir(workspace.path())
        .expect("read the workspace")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "input.txt")
        .collect();
    assert!(
        left.is_empty(),
        "the bridge wrote {left:?} into the workspace the candidate is captured from: {}",
        String::from_utf8_lossy(&outcome.stderr)
    );
}
