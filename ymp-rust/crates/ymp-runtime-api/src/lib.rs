#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, sync_channel};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Fake,
    Codex,
    ClaudeCode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    Ready,
    NotInstalled,
    Unauthenticated,
    Incompatible,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProbeReport {
    pub kind: RuntimeKind,
    pub executable: String,
    pub version: Option<String>,
    pub readiness: Readiness,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime process failed: {0}")]
    Process(#[from] std::io::Error),
    #[error("runtime output is not valid UTF-8")]
    NonUtf8Output,
    #[error("runtime does not implement this lifecycle operation: {0}")]
    Unsupported(&'static str),
    #[error("runtime emitted a malformed event: {0}")]
    MalformedEvent(String),
    #[error("runtime profile is invalid: {0}")]
    InvalidProfile(String),
    #[error("runtime exited unsuccessfully: {status}; {stderr}")]
    UnsuccessfulExit { status: String, stderr: String },
    #[error("runtime reported failure: {0}")]
    RuntimeReportedFailure(String),
    #[error("runtime output exceeded its {limit_bytes}-byte limit")]
    OutputLimitExceeded { limit_bytes: usize },
    #[error("runtime exceeded its {limit_ms}-millisecond wall-time limit")]
    TimedOut { limit_ms: u64 },
    #[error("runtime session is not waiting for resume input")]
    NotYielded,
}

#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InvocationRequest {
    pub invocation_id: String,
    pub attempt_id: String,
    pub workspace: PathBuf,
    pub mcp: Option<McpBinding>,
    pub prompt: String,
    #[serde(skip)]
    pub cancellation: CancellationToken,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpBinding {
    pub executable: PathBuf,
    pub socket_path: PathBuf,
    pub token: String,
}

impl std::fmt::Debug for McpBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpBinding")
            .field("executable", &self.executable)
            .field("socket_path", &self.socket_path)
            .field("token", &"[redacted]")
            .finish()
    }
}

impl McpBinding {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if !self.executable.is_absolute() {
            return Err(RuntimeError::InvalidProfile(
                "MCP executable path must be absolute".to_owned(),
            ));
        }
        if !self.socket_path.is_absolute() {
            return Err(RuntimeError::InvalidProfile(
                "MCP socket path must be absolute".to_owned(),
            ));
        }
        if self.token.is_empty() || self.token.len() > 256 {
            return Err(RuntimeError::InvalidProfile(
                "MCP capability token must contain between 1 and 256 bytes".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub cost_microusd: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEventKind {
    Started {
        opaque_session_id: String,
    },
    Output {
        text: String,
    },
    McpToolCall {
        server: String,
        tool: String,
        status: String,
        arguments: Value,
        result: Option<Value>,
        error: Option<Value>,
    },
    Yielded {
        cursor: String,
    },
    Completed {
        usage: Usage,
    },
    Interrupted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeEvent {
    pub sequence: u64,
    pub event_id: String,
    pub invocation_id: String,
    pub event: RuntimeEventKind,
}

pub trait RuntimeSession: Send {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError>;
    fn resume(&mut self, input: String) -> Result<(), RuntimeError>;
    fn interrupt(&mut self) -> Result<(), RuntimeError>;
}

pub trait RuntimeDriver: Send + Sync {
    fn kind(&self) -> RuntimeKind;
    fn executable(&self) -> &Path;
    fn probe(&self) -> Result<ProbeReport, RuntimeError>;
    fn start(&self, request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError>;
}

#[doc(hidden)]
pub enum BoundedOutputLine {
    Line(String),
    End,
    ReadFailed(String),
    LimitExceeded,
}

#[doc(hidden)]
pub fn read_bounded_lines<R>(reader: R, limit_bytes: usize) -> Receiver<BoundedOutputLine>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = sync_channel(64);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader.take((limit_bytes.saturating_add(1)) as u64));
        let mut consumed = 0usize;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = sender.send(BoundedOutputLine::End);
                    return;
                }
                Ok(bytes) => {
                    consumed = consumed.saturating_add(bytes);
                    if consumed > limit_bytes {
                        let _ = sender.send(BoundedOutputLine::LimitExceeded);
                        return;
                    }
                    if line.ends_with('\n') {
                        line.pop();
                        if line.ends_with('\r') {
                            line.pop();
                        }
                    }
                    if sender.send(BoundedOutputLine::Line(line)).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.send(BoundedOutputLine::ReadFailed(error.to_string()));
                    return;
                }
            }
        }
    });
    receiver
}

#[doc(hidden)]
pub fn configure_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

#[doc(hidden)]
#[cfg(unix)]
pub fn terminate_process_tree(child: &mut Child) -> std::io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let group = format!("-{}", child.id());
    let _ = Command::new("/bin/kill")
        .args(["-TERM", &group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    for _ in 0..10 {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.wait()?;
    Ok(())
}

#[doc(hidden)]
#[cfg(not(unix))]
pub fn terminate_process_tree(child: &mut Child) -> std::io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    child.kill()?;
    let _ = child.wait()?;
    Ok(())
}
