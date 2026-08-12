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
use ymp_domain::digest_bytes;

pub fn evidence_digest(bytes: &[u8]) -> String {
    digest_bytes(bytes)
}

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LaunchEnvironmentVariable {
    pub name: String,
    pub value: Option<String>,
    pub value_digest: String,
    pub confidential: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LaunchDescriptor {
    pub schema_version: u32,
    pub invocation_id: String,
    pub attempt_id: String,
    pub executable: PathBuf,
    pub executable_digest: String,
    pub arguments: Vec<String>,
    pub environment: Vec<LaunchEnvironmentVariable>,
    pub working_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticSummary {
    pub digest: String,
    pub bytes: usize,
    pub truncated: bool,
}

impl DiagnosticSummary {
    pub fn from_bytes(bytes: &[u8], truncated: bool) -> Self {
        Self {
            digest: digest_bytes(bytes),
            bytes: bytes.len(),
            truncated,
        }
    }
}

impl std::fmt::Display for DiagnosticSummary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "digest={}, bytes={}, truncated={}",
            self.digest, self.bytes, self.truncated
        )
    }
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
    #[error("runtime exited unsuccessfully: {status}; diagnostic {diagnostic}")]
    SanitizedUnsuccessfulExit {
        status: String,
        diagnostic: DiagnosticSummary,
    },
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
#[serde(default)]
pub struct InFlightExcess {
    pub model_requests: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub cost_microusd: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub cost_microusd: Option<u64>,
    pub wall_time_ms: u64,
    pub protected_queries: u64,
    pub in_flight_excess: InFlightExcess,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFailureKind {
    ProcessExit,
    RuntimeReported,
    Protocol,
    OutputLimit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEventKind {
    Launch {
        descriptor: LaunchDescriptor,
    },
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
    Failed {
        kind: RuntimeFailureKind,
        usage: Usage,
        diagnostic: Option<DiagnosticSummary>,
    },
    TimedOut {
        limit_ms: u64,
        usage: Usage,
    },
    Cancelled {
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
    fn usage(&self) -> Usage {
        Usage::default()
    }
}

pub trait RuntimeDriver: Send + Sync {
    fn kind(&self) -> RuntimeKind;
    fn executable(&self) -> &Path;
    fn probe(&self) -> Result<ProbeReport, RuntimeError>;
    fn start(&self, request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError>;
    fn prepare_launch(
        &self,
        _request: &InvocationRequest,
    ) -> Result<Option<LaunchDescriptor>, RuntimeError> {
        Ok(None)
    }
    fn start_prepared(
        &self,
        request: InvocationRequest,
        descriptor: Option<&LaunchDescriptor>,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        if descriptor.is_some() {
            return Err(RuntimeError::Unsupported(
                "prepared launch descriptor for this runtime",
            ));
        }
        self.start(request)
    }
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
    let group = format!("-{}", child.id());
    let mut parent_reaped = child.try_wait()?.is_some();
    let _ = Command::new("/bin/kill")
        .args(["-TERM", &group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    for _ in 0..20 {
        if !process_group_exists(&group) {
            if !parent_reaped {
                let _ = child.wait()?;
            }
            return Ok(());
        }
        parent_reaped |= child.try_wait()?.is_some();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let status = Command::new("/bin/kill")
        .args(["-KILL", &group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !parent_reaped {
        let _ = child.wait()?;
    }
    for _ in 0..100 {
        if !process_group_exists(&group) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if status.success() {
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("process group {group} survived SIGKILL"),
        ))
    } else {
        Err(std::io::Error::other(format!(
            "failed to send SIGKILL to process group {group}: {status}"
        )))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{configure_process_group, terminate_process_tree};
    use std::fs;
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime};

    #[test]
    fn termination_reaches_descendant_after_group_parent_exits() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let pid_file = std::env::temp_dir().join(format!(
            "ymp-runtime-api-descendant-{}-{unique}.pid",
            std::process::id()
        ));
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & printf '%s\\n' \"$!\" > \"$YMP_DESCENDANT_PID_FILE\"; exit 17")
            .env("YMP_DESCENDANT_PID_FILE", &pid_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_process_group(&mut command);
        let mut child = command.spawn().expect("spawn group parent");
        let status = child.wait().expect("wait for group parent");
        assert_eq!(status.code(), Some(17));
        let descendant = fs::read_to_string(&pid_file).expect("descendant pid");
        terminate_process_tree(&mut child).expect("terminate orphaned process group");
        let mut alive = true;
        for _ in 0..20 {
            alive = Command::new("/bin/kill")
                .args(["-0", descendant.trim()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = fs::remove_file(&pid_file);
        assert!(!alive, "descendant survived after its group parent exited");
    }
}

#[cfg(unix)]
fn process_group_exists(group: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
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
