#![forbid(unsafe_code)]

use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use ymp_runtime_api::{
    BoundedOutputLine, CancellationToken, InvocationRequest, McpBinding, ProbeReport, Readiness,
    RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeEventKind, RuntimeKind, RuntimeSession,
    Usage, configure_process_group, read_bounded_lines, terminate_process_tree,
};

pub const PINNED_CLAUDE_VERSION: &str = "2.1.227 (Claude Code)";
pub const PINNED_CLAUDE_MODEL: &str = "claude-opus-5";
pub const PINNED_CLAUDE_PROMPT_POLICY: &str = "ymp-claude-low-v1";
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_WALL_TIME_LIMIT_MS: u64 = 10 * 60 * 1000;
const DEFAULT_BUDGET_MICROUSD: u64 = 1_000_000;
const HARNESS_INSTRUCTIONS: &str = "Execution policy: work without delegation, agents, skills, or background tasks. Do not send progress reports. Batch independent file reads and batch the final formatting, tests, lint, and diff checks. Use only the files and tools needed for the requested outcome. Do not commit. Stop immediately after a concise final report.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeProfile {
    pub expected_version: String,
    pub model: String,
    pub effort: String,
    pub permission_mode: String,
    pub prompt_policy: String,
    pub max_budget_microusd: u64,
    pub output_limit_bytes: usize,
    pub wall_time_limit_ms: u64,
}

impl Default for ClaudeProfile {
    fn default() -> Self {
        Self {
            expected_version: PINNED_CLAUDE_VERSION.to_owned(),
            model: PINNED_CLAUDE_MODEL.to_owned(),
            effort: "low".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            prompt_policy: PINNED_CLAUDE_PROMPT_POLICY.to_owned(),
            max_budget_microusd: DEFAULT_BUDGET_MICROUSD,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            wall_time_limit_ms: DEFAULT_WALL_TIME_LIMIT_MS,
        }
    }
}

impl ClaudeProfile {
    fn validate(&self) -> Result<(), RuntimeError> {
        if self.expected_version.trim().is_empty() {
            return Err(RuntimeError::InvalidProfile(
                "expected version must not be empty".to_owned(),
            ));
        }
        if self.model != PINNED_CLAUDE_MODEL {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude model {}",
                self.model
            )));
        }
        if self.effort != "low" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude effort {}",
                self.effort
            )));
        }
        if self.permission_mode != "acceptEdits" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude permission mode {}",
                self.permission_mode
            )));
        }
        if self.prompt_policy != PINNED_CLAUDE_PROMPT_POLICY {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Claude prompt policy {}",
                self.prompt_policy
            )));
        }
        if self.max_budget_microusd == 0 {
            return Err(RuntimeError::InvalidProfile(
                "Claude budget must be positive".to_owned(),
            ));
        }
        if self.output_limit_bytes == 0 {
            return Err(RuntimeError::InvalidProfile(
                "output limit must be positive".to_owned(),
            ));
        }
        if self.wall_time_limit_ms == 0 {
            return Err(RuntimeError::InvalidProfile(
                "wall-time limit must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ClaudeRuntime {
    executable: PathBuf,
    profile: ClaudeProfile,
}

impl Default for ClaudeRuntime {
    fn default() -> Self {
        Self::new("claude")
    }
}

impl ClaudeRuntime {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            profile: ClaudeProfile::default(),
        }
    }

    pub fn with_profile(executable: impl Into<PathBuf>, profile: ClaudeProfile) -> Self {
        Self {
            executable: executable.into(),
            profile,
        }
    }

    pub fn profile(&self) -> &ClaudeProfile {
        &self.profile
    }
}

#[derive(Clone, Debug)]
struct ClaudeLaunch {
    executable: PathBuf,
    profile: ClaudeProfile,
    workspace: PathBuf,
    attempt_id: String,
    mcp: Option<McpBinding>,
}

struct ClaudeProcess {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: JoinHandle<Vec<u8>>,
    started_at: Instant,
}

impl ClaudeLaunch {
    fn spawn(&self, session_id: Option<&str>, prompt: &str) -> Result<ClaudeProcess, RuntimeError> {
        let mcp_config = mcp_config(self.mcp.as_ref(), &self.attempt_id)?;
        let budget = format!(
            "{:.6}",
            self.profile.max_budget_microusd as f64 / 1_000_000.0
        );
        let mut command = Command::new(&self.executable);
        command
            .arg("--print")
            .args(["--input-format", "text"])
            .args(["--output-format", "stream-json"])
            .arg("--verbose")
            .args(["--setting-sources", ""])
            .arg("--disable-slash-commands")
            .arg("--strict-mcp-config")
            .args(["--mcp-config", &mcp_config])
            .args(["--model", &self.profile.model])
            .args(["--effort", &self.profile.effort])
            .args(["--permission-mode", &self.profile.permission_mode])
            .args(["--max-budget-usd", &budget])
            .args(["--tools", "Bash,Edit,Read,Write,Glob,Grep"])
            .current_dir(&self.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(session_id) = session_id {
            command.args(["--resume", session_id]);
        }
        configure_process_group(&mut command);
        let mut child = command.spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Claude stdin was not piped".to_owned()))?;
        stdin.write_all(prompt.as_bytes())?;
        stdin.write_all(b"\n\n")?;
        stdin.write_all(HARNESS_INSTRUCTIONS.as_bytes())?;
        stdin.flush()?;
        drop(stdin);
        let stdout = child.stdout.take().ok_or_else(|| {
            RuntimeError::MalformedEvent("Claude stdout was not piped".to_owned())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            RuntimeError::MalformedEvent("Claude stderr was not piped".to_owned())
        })?;
        let stderr_limit = self.profile.output_limit_bytes;
        let stderr_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr
                .take((stderr_limit.saturating_add(1)) as u64)
                .read_to_end(&mut bytes);
            bytes
        });
        Ok(ClaudeProcess {
            child,
            lines: read_bounded_lines(stdout, self.profile.output_limit_bytes),
            stderr_reader,
            started_at: Instant::now(),
        })
    }
}

impl RuntimeDriver for ClaudeRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::ClaudeCode
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        self.profile.validate()?;
        let output = match Command::new(&self.executable).arg("--version").output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProbeReport {
                    kind: RuntimeKind::ClaudeCode,
                    executable: self.executable.display().to_string(),
                    version: None,
                    readiness: Readiness::NotInstalled,
                    detail: "executable not found".to_owned(),
                });
            }
            Err(error) => return Err(RuntimeError::Process(error)),
        };
        if !output.status.success() {
            return Ok(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
                executable: self.executable.display().to_string(),
                version: None,
                readiness: Readiness::Unavailable,
                detail: format!("version probe exited with {}", output.status),
            });
        }
        let version = String::from_utf8(output.stdout)
            .map_err(|_| RuntimeError::NonUtf8Output)?
            .trim()
            .to_owned();
        if version != self.profile.expected_version {
            return Ok(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
                executable: self.executable.display().to_string(),
                version: Some(version.clone()),
                readiness: Readiness::Incompatible,
                detail: format!(
                    "profile requires {}, found {version}",
                    self.profile.expected_version
                ),
            });
        }
        let auth = Command::new(&self.executable)
            .args(["auth", "status"])
            .output()?;
        let authenticated = serde_json::from_slice::<Value>(&auth.stdout)
            .ok()
            .and_then(|value| value.get("loggedIn").and_then(Value::as_bool))
            .unwrap_or(false);
        if !auth.status.success() || !authenticated {
            return Ok(ProbeReport {
                kind: RuntimeKind::ClaudeCode,
                executable: self.executable.display().to_string(),
                version: Some(version),
                readiness: Readiness::Unauthenticated,
                detail: "Claude Code authentication is unavailable".to_owned(),
            });
        }
        Ok(ProbeReport {
            kind: RuntimeKind::ClaudeCode,
            executable: self.executable.display().to_string(),
            version: Some(version),
            readiness: Readiness::Ready,
            detail: format!(
                "pinned local profile ready: model={}, effort={}, prompt_policy={}, permission_mode={}, max_budget_usd={:.6}, wall_time_limit_ms={}, output_limit_bytes={}",
                self.profile.model,
                self.profile.effort,
                self.profile.prompt_policy,
                self.profile.permission_mode,
                self.profile.max_budget_microusd as f64 / 1_000_000.0,
                self.profile.wall_time_limit_ms,
                self.profile.output_limit_bytes
            ),
        })
    }

    fn start(&self, request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        self.profile.validate()?;
        let readiness = self.probe()?;
        if readiness.readiness != Readiness::Ready {
            return Err(RuntimeError::InvalidProfile(format!(
                "Claude profile is not ready: {}",
                readiness.detail
            )));
        }
        if !request.workspace.is_dir() {
            return Err(RuntimeError::InvalidProfile(format!(
                "workspace is not a directory: {}",
                request.workspace.display()
            )));
        }
        if request.attempt_id.is_empty() || request.attempt_id.len() > 128 {
            return Err(RuntimeError::InvalidProfile(
                "attempt identifier must contain between 1 and 128 bytes".to_owned(),
            ));
        }
        let launch = ClaudeLaunch {
            executable: self.executable.clone(),
            profile: self.profile.clone(),
            workspace: request.workspace,
            attempt_id: request.attempt_id,
            mcp: request.mcp,
        };
        let process = launch.spawn(None, &request.prompt)?;
        Ok(Box::new(ClaudeSession {
            child: process.child,
            lines: process.lines,
            stderr_reader: Some(process.stderr_reader),
            launch,
            session_id: None,
            invocation_id: request.invocation_id,
            sequence: 0,
            output_limit_bytes: self.profile.output_limit_bytes,
            wall_time_limit_ms: self.profile.wall_time_limit_ms,
            started_at: process.started_at,
            cancellation: request.cancellation,
            completed: false,
            terminal: false,
            recoverable: false,
            interrupted: false,
            interruption_emitted: false,
        }))
    }
}

struct ClaudeSession {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: Option<JoinHandle<Vec<u8>>>,
    launch: ClaudeLaunch,
    session_id: Option<String>,
    invocation_id: String,
    sequence: u64,
    output_limit_bytes: usize,
    wall_time_limit_ms: u64,
    started_at: Instant,
    cancellation: CancellationToken,
    completed: bool,
    terminal: bool,
    recoverable: bool,
    interrupted: bool,
    interruption_emitted: bool,
}

impl ClaudeSession {
    fn emit(&mut self, event: RuntimeEventKind) -> RuntimeEvent {
        self.sequence += 1;
        RuntimeEvent {
            sequence: self.sequence,
            event_id: format!("{}.event-{}", self.invocation_id, self.sequence),
            invocation_id: self.invocation_id.clone(),
            event,
        }
    }

    fn finish(&mut self) -> Result<ExitStatus, RuntimeError> {
        let status = self.child.wait()?;
        self.completed = true;
        Ok(status)
    }

    fn stderr(&mut self) -> String {
        let Some(reader) = self.stderr_reader.take() else {
            return String::new();
        };
        reader
            .join()
            .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
            .unwrap_or_else(|_| "stderr reader failed".to_owned())
    }

    fn unsuccessful(&mut self, status: ExitStatus) -> RuntimeError {
        RuntimeError::UnsuccessfulExit {
            status: status.to_string(),
            stderr: self.stderr(),
        }
    }

    fn install_process(&mut self, process: ClaudeProcess) {
        self.child = process.child;
        self.lines = process.lines;
        self.stderr_reader = Some(process.stderr_reader);
        self.started_at = process.started_at;
        self.completed = false;
        self.recoverable = false;
    }

    fn finish_after_error(&mut self, error: &RuntimeError) {
        if self.terminal || self.interrupted || self.cancellation.is_cancelled() {
            return;
        }
        if !self.completed {
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
        self.recoverable = self.session_id.is_some()
            && matches!(
                error,
                RuntimeError::Process(_) | RuntimeError::UnsuccessfulExit { .. }
            );
        self.terminal = self.session_id.is_some() && !self.recoverable;
    }

    fn next_line(&mut self) -> Result<Option<String>, RuntimeError> {
        let limit = Duration::from_millis(self.wall_time_limit_ms);
        loop {
            if self.cancellation.is_cancelled() {
                let _ = terminate_process_tree(&mut self.child);
                self.completed = true;
                self.interrupted = true;
                return Ok(None);
            }
            let Some(remaining) = limit.checked_sub(self.started_at.elapsed()) else {
                let _ = terminate_process_tree(&mut self.child);
                self.completed = true;
                return Err(RuntimeError::TimedOut {
                    limit_ms: self.wall_time_limit_ms,
                });
            };
            match self
                .lines
                .recv_timeout(remaining.min(Duration::from_millis(100)))
            {
                Ok(BoundedOutputLine::Line(line)) => return Ok(Some(line)),
                Ok(BoundedOutputLine::End) => return Ok(None),
                Ok(BoundedOutputLine::ReadFailed(error)) => {
                    return Err(RuntimeError::MalformedEvent(error));
                }
                Ok(BoundedOutputLine::LimitExceeded) => {
                    let _ = terminate_process_tree(&mut self.child);
                    self.completed = true;
                    return Err(RuntimeError::OutputLimitExceeded {
                        limit_bytes: self.output_limit_bytes,
                    });
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(None),
            }
        }
    }

    fn parse_event(&mut self, event: Value) -> Result<Option<RuntimeEvent>, RuntimeError> {
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::MalformedEvent("event has no type".to_owned()))?;
        match event_type {
            "system" if event.get("subtype").and_then(Value::as_str) == Some("init") => {
                let session_id = string_field(&event, "session_id")?;
                if let Some(expected) = &self.session_id {
                    if expected != &session_id {
                        let _ = terminate_process_tree(&mut self.child);
                        self.completed = true;
                        self.terminal = true;
                        return Err(RuntimeError::InvalidProfile(format!(
                            "resumed Claude session identifier changed from {expected} to {session_id}"
                        )));
                    }
                } else {
                    self.session_id = Some(session_id.clone());
                }
                Ok(Some(self.emit(RuntimeEventKind::Started {
                    opaque_session_id: session_id,
                })))
            }
            "assistant" => {
                let content = event
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        RuntimeError::MalformedEvent(
                            "Claude assistant event has no message content".to_owned(),
                        )
                    })?;
                let text = content
                    .iter()
                    .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    return Ok(None);
                }
                Ok(Some(self.emit(RuntimeEventKind::Output { text })))
            }
            "result" => {
                if event.get("is_error").and_then(Value::as_bool) == Some(true)
                    || event.get("subtype").and_then(Value::as_str) != Some("success")
                {
                    let detail = event
                        .get("result")
                        .and_then(Value::as_str)
                        .unwrap_or("Claude reported an unsuccessful result");
                    let _ = self.finish();
                    self.terminal = true;
                    return Err(RuntimeError::RuntimeReportedFailure(detail.to_owned()));
                }
                let raw_usage = event.get("usage").ok_or_else(|| {
                    RuntimeError::MalformedEvent("Claude result has no usage".to_owned())
                })?;
                let cost_microusd = event
                    .get("total_cost_usd")
                    .and_then(Value::as_f64)
                    .map(|cost| (cost * 1_000_000.0).round() as u64);
                let usage = Usage {
                    input_tokens: optional_u64_field(raw_usage, "input_tokens"),
                    cached_input_tokens: optional_u64_field(raw_usage, "cache_read_input_tokens"),
                    output_tokens: optional_u64_field(raw_usage, "output_tokens"),
                    reasoning_output_tokens: 0,
                    cost_microusd,
                };
                let status = self.finish()?;
                if !status.success() {
                    return Err(self.unsuccessful(status));
                }
                self.terminal = true;
                Ok(Some(self.emit(RuntimeEventKind::Completed { usage })))
            }
            "user" | "stream_event" | "rate_limit_event" => Ok(None),
            "system" => Ok(None),
            other => Err(RuntimeError::MalformedEvent(format!(
                "unsupported Claude event type {other}"
            ))),
        }
    }

    fn next_event_inner(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.cancellation.is_cancelled() && !self.completed {
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
            self.terminal = true;
            self.interrupted = true;
        }
        if self.interrupted {
            if self.interruption_emitted {
                return Ok(None);
            }
            self.interruption_emitted = true;
            return Ok(Some(self.emit(RuntimeEventKind::Interrupted)));
        }
        if self.completed {
            return Ok(None);
        }
        loop {
            let Some(line) = self.next_line()? else {
                if self.interrupted {
                    self.terminal = true;
                    self.interruption_emitted = true;
                    return Ok(Some(self.emit(RuntimeEventKind::Interrupted)));
                }
                let status = self.finish()?;
                if status.success() {
                    return Err(RuntimeError::MalformedEvent(
                        "Claude exited without a result event".to_owned(),
                    ));
                }
                return Err(self.unsuccessful(status));
            };
            let event: Value = serde_json::from_str(&line)
                .map_err(|error| RuntimeError::MalformedEvent(error.to_string()))?;
            if let Some(event) = self.parse_event(event)? {
                return Ok(Some(event));
            }
        }
    }
}

impl RuntimeSession for ClaudeSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        let result = self.next_event_inner();
        if let Err(error) = &result {
            self.finish_after_error(error);
        }
        result
    }

    fn resume(&mut self, input: String) -> Result<(), RuntimeError> {
        if self.terminal || self.interrupted || self.cancellation.is_cancelled() {
            return Err(RuntimeError::NotYielded);
        }
        let session_id = self.session_id.clone().ok_or_else(|| {
            RuntimeError::InvalidProfile(
                "managed Claude session identifier is unknown; refusing replacement start"
                    .to_owned(),
            )
        })?;
        if !self.recoverable {
            return Err(RuntimeError::NotYielded);
        }
        let process = self.launch.spawn(Some(&session_id), &input)?;
        self.install_process(process);
        Ok(())
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        self.cancellation.cancel();
        if !self.completed && !self.interrupted {
            terminate_process_tree(&mut self.child)?;
            self.completed = true;
            self.terminal = true;
            self.interrupted = true;
        }
        Ok(())
    }
}

impl Drop for ClaudeSession {
    fn drop(&mut self) {
        if !self.completed {
            let _ = terminate_process_tree(&mut self.child);
        }
    }
}

fn string_field(value: &Value, field: &str) -> Result<String, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| RuntimeError::MalformedEvent(format!("missing string field {field}")))
}

fn optional_u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn mcp_config(binding: Option<&McpBinding>, attempt_id: &str) -> Result<String, RuntimeError> {
    let servers = if let Some(binding) = binding {
        binding.validate()?;
        serde_json::json!({
            "ymp": {
                "command": binding.executable,
                "args": ["internal", "agent-mcp"],
                "env": {
                    "YMP_AGENT_SOCKET": binding.socket_path,
                    "YMP_AGENT_TOKEN": binding.token,
                    "YMP_ATTEMPT_ID": attempt_id
                }
            }
        })
    } else {
        serde_json::json!({})
    };
    serde_json::to_string(&serde_json::json!({ "mcpServers": servers }))
        .map_err(|error| RuntimeError::InvalidProfile(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{ClaudeProfile, ClaudeRuntime, PINNED_CLAUDE_MODEL};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use ymp_runtime_api::{InvocationRequest, RuntimeDriver, RuntimeError, RuntimeEventKind};

    #[test]
    fn profile_rejects_unapproved_model_and_effort() {
        let profile = ClaudeProfile {
            model: "unapproved".to_owned(),
            ..ClaudeProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));

        let profile = ClaudeProfile {
            model: PINNED_CLAUDE_MODEL.to_owned(),
            effort: "high".to_owned(),
            ..ClaudeProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));
    }

    #[test]
    fn default_runtime_exposes_pinned_profile() {
        let runtime = ClaudeRuntime::default();
        assert_eq!(runtime.profile().model, PINNED_CLAUDE_MODEL);
        assert_eq!(runtime.profile().effort, "low");
        assert_eq!(runtime.profile().permission_mode, "acceptEdits");
    }

    #[test]
    fn structured_process_stream_preserves_session_output_usage_and_cost() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("claude-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{"loggedIn":true,"authMethod":"oauth"}'
  exit 0
else
  cat >/dev/null
  printf '%s\n' '{"type":"system","subtype":"init","session_id":"session-1"}'
  printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"done"}]}}'
  printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.125,"usage":{"input_tokens":11,"cache_read_input_tokens":7,"output_tokens":3}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = ClaudeRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-1".to_owned(),
                attempt_id: "attempt-1".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        let started = session.next_event().expect("started").expect("event");
        assert!(matches!(
            started.event,
            RuntimeEventKind::Started { opaque_session_id } if opaque_session_id == "session-1"
        ));
        let output = session.next_event().expect("output").expect("event");
        assert!(matches!(
            output.event,
            RuntimeEventKind::Output { text } if text == "done"
        ));
        let completed = session.next_event().expect("completed").expect("event");
        assert!(matches!(
            completed.event,
            RuntimeEventKind::Completed { usage }
                if usage.input_tokens == 11
                    && usage.cached_input_tokens == 7
                    && usage.output_tokens == 3
                    && usage.cost_microusd == Some(125_000)
        ));
        assert!(session.next_event().expect("terminal").is_none());
    }

    #[test]
    fn recoverable_process_failure_resumes_the_same_managed_session() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("claude-resume-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{"loggedIn":true,"authMethod":"oauth"}'
  exit 0
else
  cd "$(dirname "$0")"
  count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
  printf '%s\n' "$count" > invocation.count
  printf '%s\n' "$@" > "invocation-$count.args"
  cat > "invocation-$count.stdin"
  printf '%s\n' '{"type":"system","subtype":"init","session_id":"session-resume-1"}'
  if [ "$count" -eq 1 ]; then
    printf '%s\n' 'committed progress' > committed.progress
    printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"committed progress"}]}}'
    exit 17
  fi
  test "$(cat committed.progress)" = 'committed progress' || exit 29
  printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"resumed progress"}]}}'
  printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.125,"usage":{"input_tokens":11,"cache_read_input_tokens":7,"output_tokens":3}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = ClaudeRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-resume".to_owned(),
                attempt_id: "attempt-resume".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: Some(ymp_runtime_api::McpBinding {
                    executable: executable.clone(),
                    socket_path: directory.path().join("agent.sock"),
                    token: "fixture-token".to_owned(),
                }),
                prompt: "initial prompt".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");

        let started = session.next_event().expect("started").expect("event");
        assert_eq!(started.sequence, 1);
        assert!(matches!(
            started.event,
            RuntimeEventKind::Started { opaque_session_id }
                if opaque_session_id == "session-resume-1"
        ));
        assert!(matches!(
            session.next_event().expect("output").expect("event").event,
            RuntimeEventKind::Output { text } if text == "committed progress"
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::UnsuccessfulExit { .. })
        ));

        session
            .resume("continue after interruption".to_owned())
            .expect("resume fixture");
        let resumed = session.next_event().expect("resumed start").expect("event");
        assert_eq!(resumed.sequence, 3);
        assert!(matches!(
            resumed.event,
            RuntimeEventKind::Started { opaque_session_id }
                if opaque_session_id == "session-resume-1"
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("resumed output")
                .expect("event")
                .event,
            RuntimeEventKind::Output { text } if text == "resumed progress"
        ));
        assert!(matches!(
            session
                .next_event()
                .expect("completion")
                .expect("event")
                .event,
            RuntimeEventKind::Completed { .. }
        ));

        let resumed_arguments = fs::read_to_string(directory.path().join("invocation-2.args"))
            .expect("resumed arguments");
        let initial_arguments = fs::read_to_string(directory.path().join("invocation-1.args"))
            .expect("initial arguments");
        assert!(!initial_arguments.contains("--no-session-persistence"));
        assert!(resumed_arguments.contains("--resume\nsession-resume-1\n"));
        assert!(!resumed_arguments.contains("--no-session-persistence"));
        assert!(resumed_arguments.contains("YMP_ATTEMPT_ID"));
        assert!(resumed_arguments.contains("attempt-resume"));
        assert_eq!(
            fs::read_to_string(directory.path().join("invocation.count"))
                .expect("invocation count")
                .trim(),
            "2"
        );
    }

    #[test]
    fn mismatched_resumed_session_is_terminal_and_never_replaced() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("claude-mismatch-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '2.1.227 (Claude Code)'
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{"loggedIn":true,"authMethod":"oauth"}'
  exit 0
else
  cd "$(dirname "$0")"
  count=$(($(cat invocation.count 2>/dev/null || printf '0') + 1))
  printf '%s\n' "$count" > invocation.count
  cat >/dev/null
  if [ "$count" -eq 1 ]; then
    printf '%s\n' '{"type":"system","subtype":"init","session_id":"session-known"}'
    exit 17
  fi
  printf '%s\n' '{"type":"system","subtype":"init","session_id":"session-replacement"}'
  sleep 30
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = ClaudeRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-mismatch".to_owned(),
                attempt_id: "attempt-mismatch".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        assert!(matches!(
            session.next_event().expect("started").expect("event").event,
            RuntimeEventKind::Started { opaque_session_id }
                if opaque_session_id == "session-known"
        ));
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::UnsuccessfulExit { .. })
        ));
        session.resume("resume".to_owned()).expect("resume fixture");
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::InvalidProfile(_))
        ));
        assert!(matches!(
            session.resume("replacement".to_owned()),
            Err(RuntimeError::NotYielded)
        ));
        assert_eq!(
            fs::read_to_string(directory.path().join("invocation.count"))
                .expect("invocation count")
                .trim(),
            "2"
        );
    }
}
