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

pub const PINNED_CODEX_VERSION: &str = "codex-cli 0.147.0";
pub const PINNED_CODEX_MODEL: &str = "gpt-5.6-sol";
pub const PINNED_CODEX_PROMPT_POLICY: &str = "ymp-codex-low-v1";
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_WALL_TIME_LIMIT_MS: u64 = 10 * 60 * 1000;
const HARNESS_INSTRUCTIONS: &str = "Execution policy: work without delegation or subagents. Do not send progress reports. Batch independent file reads and batch the final formatting, tests, lint, and diff checks. Use only the files and tools needed for the requested outcome. Do not commit. Stop immediately after a concise final report.";
const DISABLED_AMBIENT_FEATURES: [&str; 14] = [
    "apps",
    "browser_use",
    "browser_use_external",
    "browser_use_full_cdp_access",
    "computer_use",
    "goals",
    "image_generation",
    "multi_agent",
    "personality",
    "plugins",
    "skill_search",
    "tool_call_mcp_elicitation",
    "tool_suggest",
    "view_image",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexSandbox {
    ReadOnly,
    WorkspaceWrite,
}

impl CodexSandbox {
    fn as_arg(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexProfile {
    pub expected_version: String,
    pub model: String,
    pub reasoning_effort: String,
    pub approval_policy: String,
    pub prompt_policy: String,
    pub sandbox: CodexSandbox,
    pub output_limit_bytes: usize,
    pub wall_time_limit_ms: u64,
}

impl Default for CodexProfile {
    fn default() -> Self {
        Self {
            expected_version: PINNED_CODEX_VERSION.to_owned(),
            model: PINNED_CODEX_MODEL.to_owned(),
            reasoning_effort: "low".to_owned(),
            approval_policy: "never".to_owned(),
            prompt_policy: PINNED_CODEX_PROMPT_POLICY.to_owned(),
            sandbox: CodexSandbox::WorkspaceWrite,
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            wall_time_limit_ms: DEFAULT_WALL_TIME_LIMIT_MS,
        }
    }
}

impl CodexProfile {
    fn validate(&self) -> Result<(), RuntimeError> {
        if self.expected_version.trim().is_empty() {
            return Err(RuntimeError::InvalidProfile(
                "expected version must not be empty".to_owned(),
            ));
        }
        if self.model != PINNED_CODEX_MODEL {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex model {}",
                self.model
            )));
        }
        if self.reasoning_effort != "low" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex reasoning effort {}",
                self.reasoning_effort
            )));
        }
        if self.approval_policy != "never" {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex approval policy {}",
                self.approval_policy
            )));
        }
        if self.prompt_policy != PINNED_CODEX_PROMPT_POLICY {
            return Err(RuntimeError::InvalidProfile(format!(
                "unapproved Codex prompt policy {}",
                self.prompt_policy
            )));
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
pub struct CodexRuntime {
    executable: PathBuf,
    profile: CodexProfile,
}

impl Default for CodexRuntime {
    fn default() -> Self {
        Self::new("codex")
    }
}

impl CodexRuntime {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            profile: CodexProfile::default(),
        }
    }

    pub fn with_profile(executable: impl Into<PathBuf>, profile: CodexProfile) -> Self {
        Self {
            executable: executable.into(),
            profile,
        }
    }

    pub fn profile(&self) -> &CodexProfile {
        &self.profile
    }

    fn version_output(&self) -> Result<std::process::Output, RuntimeError> {
        Command::new(&self.executable)
            .arg("--version")
            .output()
            .map_err(RuntimeError::Process)
    }
}

impl RuntimeDriver for CodexRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Codex
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        self.profile.validate()?;
        let output = match self.version_output() {
            Ok(output) => output,
            Err(RuntimeError::Process(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProbeReport {
                    kind: RuntimeKind::Codex,
                    executable: self.executable.display().to_string(),
                    version: None,
                    readiness: Readiness::NotInstalled,
                    detail: "executable not found".to_owned(),
                });
            }
            Err(error) => return Err(error),
        };
        if !output.status.success() {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
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
                kind: RuntimeKind::Codex,
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
            .args(["login", "status"])
            .output()?;
        if !auth.status.success() {
            return Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: Some(version),
                readiness: Readiness::Unauthenticated,
                detail: "Codex authentication is unavailable".to_owned(),
            });
        }
        Ok(ProbeReport {
            kind: RuntimeKind::Codex,
            executable: self.executable.display().to_string(),
            version: Some(version),
            readiness: Readiness::Ready,
            detail: format!(
                "pinned local profile ready: model={}, reasoning_effort={}, approval_policy={}, prompt_policy={}, sandbox={}, wall_time_limit_ms={}, output_limit_bytes={}",
                self.profile.model,
                self.profile.reasoning_effort,
                self.profile.approval_policy,
                self.profile.prompt_policy,
                self.profile.sandbox.as_arg(),
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
                "Codex profile is not ready: {}",
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
        let mut command = Command::new(&self.executable);
        command
            .arg("exec")
            .arg("--json")
            .arg("--ephemeral")
            .arg("--ignore-user-config")
            .arg("--ignore-rules")
            .args(["--sandbox", self.profile.sandbox.as_arg()])
            .args(["--model", &self.profile.model])
            .arg("-c")
            .arg(format!(
                "model_reasoning_effort=\"{}\"",
                self.profile.reasoning_effort
            ))
            .arg("-c")
            .arg(format!(
                "approval_policy=\"{}\"",
                self.profile.approval_policy
            ))
            .arg("-C")
            .arg(&request.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for feature in DISABLED_AMBIENT_FEATURES {
            command.arg("--disable").arg(feature);
        }
        if let Some(mcp) = &request.mcp {
            add_mcp_config(&mut command, mcp, &request.attempt_id)?;
        }
        command.arg("-");
        configure_process_group(&mut command);
        let mut child = command.spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stdin was not piped".to_owned()))?;
        stdin.write_all(request.prompt.as_bytes())?;
        stdin.write_all(b"\n\n")?;
        stdin.write_all(HARNESS_INSTRUCTIONS.as_bytes())?;
        stdin.flush()?;
        drop(stdin);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stdout was not piped".to_owned()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| RuntimeError::MalformedEvent("Codex stderr was not piped".to_owned()))?;
        let stderr_limit = self.profile.output_limit_bytes;
        let stderr_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr
                .take((stderr_limit.saturating_add(1)) as u64)
                .read_to_end(&mut bytes);
            bytes
        });
        Ok(Box::new(CodexSession {
            child,
            lines: read_bounded_lines(stdout, self.profile.output_limit_bytes),
            stderr_reader: Some(stderr_reader),
            invocation_id: request.invocation_id,
            sequence: 0,
            output_limit_bytes: self.profile.output_limit_bytes,
            wall_time_limit_ms: self.profile.wall_time_limit_ms,
            started_at: Instant::now(),
            cancellation: request.cancellation,
            completed: false,
            interrupted: false,
            interruption_emitted: false,
        }))
    }
}

struct CodexSession {
    child: Child,
    lines: Receiver<BoundedOutputLine>,
    stderr_reader: Option<JoinHandle<Vec<u8>>>,
    invocation_id: String,
    sequence: u64,
    output_limit_bytes: usize,
    wall_time_limit_ms: u64,
    started_at: Instant,
    cancellation: CancellationToken,
    completed: bool,
    interrupted: bool,
    interruption_emitted: bool,
}

impl CodexSession {
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
            "thread.started" => {
                let session_id = string_field(&event, "thread_id")?;
                Ok(Some(self.emit(RuntimeEventKind::Started {
                    opaque_session_id: session_id,
                })))
            }
            "item.completed" => {
                let Some(item) = event.get("item") else {
                    return Err(RuntimeError::MalformedEvent(
                        "item.completed has no item".to_owned(),
                    ));
                };
                match item.get("type").and_then(Value::as_str) {
                    Some("agent_message") => {
                        let text = string_field(item, "text")?;
                        Ok(Some(self.emit(RuntimeEventKind::Output { text })))
                    }
                    Some("mcp_tool_call") => {
                        let server = string_field(item, "server")?;
                        let tool = string_field(item, "tool")?;
                        let status = string_field(item, "status")?;
                        let arguments = item.get("arguments").cloned().unwrap_or(Value::Null);
                        let result = item.get("result").filter(|value| !value.is_null()).cloned();
                        let error = item.get("error").filter(|value| !value.is_null()).cloned();
                        Ok(Some(self.emit(RuntimeEventKind::McpToolCall {
                            server,
                            tool,
                            status,
                            arguments,
                            result,
                            error,
                        })))
                    }
                    _ => Ok(None),
                }
            }
            "turn.completed" => {
                let usage = event.get("usage").ok_or_else(|| {
                    RuntimeError::MalformedEvent("turn.completed has no usage".to_owned())
                })?;
                let usage = Usage {
                    input_tokens: u64_field(usage, "input_tokens")?,
                    cached_input_tokens: optional_u64_field(usage, "cached_input_tokens"),
                    output_tokens: u64_field(usage, "output_tokens")?,
                    reasoning_output_tokens: optional_u64_field(usage, "reasoning_output_tokens"),
                    cost_microusd: None,
                };
                let status = self.finish()?;
                if !status.success() {
                    return Err(self.unsuccessful(status));
                }
                Ok(Some(self.emit(RuntimeEventKind::Completed { usage })))
            }
            "turn.started" | "item.started" | "item.updated" => Ok(None),
            "error" | "turn.failed" => Err(RuntimeError::MalformedEvent(event.to_string())),
            other => Err(RuntimeError::MalformedEvent(format!(
                "unsupported Codex event type {other}"
            ))),
        }
    }
}

impl RuntimeSession for CodexSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if self.cancellation.is_cancelled() && !self.completed {
            let _ = terminate_process_tree(&mut self.child);
            self.completed = true;
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
                    self.interruption_emitted = true;
                    return Ok(Some(self.emit(RuntimeEventKind::Interrupted)));
                }
                let status = self.finish()?;
                if status.success() {
                    return Err(RuntimeError::MalformedEvent(
                        "Codex exited without turn.completed".to_owned(),
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

    fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported(
            "the pinned Codex process-per-turn profile does not support resume",
        ))
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        self.cancellation.cancel();
        if !self.completed && !self.interrupted {
            terminate_process_tree(&mut self.child)?;
            self.completed = true;
            self.interrupted = true;
        }
        Ok(())
    }
}

impl Drop for CodexSession {
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

fn u64_field(value: &Value, field: &str) -> Result<u64, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| RuntimeError::MalformedEvent(format!("missing integer field {field}")))
}

fn optional_u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn add_mcp_config(
    command: &mut Command,
    binding: &McpBinding,
    attempt_id: &str,
) -> Result<(), RuntimeError> {
    binding.validate()?;
    let executable = binding.executable.to_str().ok_or_else(|| {
        RuntimeError::InvalidProfile("MCP executable path must be UTF-8".to_owned())
    })?;
    let socket = binding
        .socket_path
        .to_str()
        .ok_or_else(|| RuntimeError::InvalidProfile("MCP socket path must be UTF-8".to_owned()))?;
    for setting in [
        "mcp_servers.ymp.required=true".to_owned(),
        "mcp_servers.ymp.enabled_tools=[\"read_control\",\"read_events\",\"submit\"]".to_owned(),
        "mcp_servers.ymp.default_tools_approval_mode=\"approve\"".to_owned(),
        format!(
            "mcp_servers.ymp.command={}",
            serde_json::to_string(executable).expect("string serialization cannot fail")
        ),
        "mcp_servers.ymp.args=[\"internal\",\"agent-mcp\"]".to_owned(),
        format!(
            "mcp_servers.ymp.env.YMP_AGENT_SOCKET={}",
            serde_json::to_string(socket).expect("string serialization cannot fail")
        ),
        format!(
            "mcp_servers.ymp.env.YMP_AGENT_TOKEN={}",
            serde_json::to_string(&binding.token).expect("string serialization cannot fail")
        ),
        format!(
            "mcp_servers.ymp.env.YMP_ATTEMPT_ID={}",
            serde_json::to_string(attempt_id).expect("string serialization cannot fail")
        ),
    ] {
        command.arg("-c").arg(setting);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CodexProfile, CodexRuntime, PINNED_CODEX_MODEL};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use ymp_runtime_api::{
        CancellationToken, InvocationRequest, RuntimeDriver, RuntimeError, RuntimeEventKind,
    };

    #[test]
    fn profile_rejects_unapproved_model_and_effort() {
        let profile = CodexProfile {
            model: "unapproved".to_owned(),
            ..CodexProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));

        let profile = CodexProfile {
            model: PINNED_CODEX_MODEL.to_owned(),
            reasoning_effort: "high".to_owned(),
            ..CodexProfile::default()
        };
        assert!(matches!(
            profile.validate(),
            Err(RuntimeError::InvalidProfile(_))
        ));
    }

    #[test]
    fn default_runtime_exposes_pinned_profile() {
        let runtime = CodexRuntime::default();
        assert_eq!(runtime.profile().model, PINNED_CODEX_MODEL);
        assert_eq!(runtime.profile().reasoning_effort, "low");
    }

    #[test]
    fn structured_process_stream_preserves_session_output_and_usage() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' "$@" > "$(dirname "$0")/invocation.args"
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-1"}'
  printf '%s\n' '{"type":"turn.started"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"read_control","status":"completed","arguments":{},"result":{"run_id":"run-1"},"error":null}}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"done"}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"cached_input_tokens":7,"output_tokens":3,"reasoning_output_tokens":2}}'
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-1".to_owned(),
                attempt_id: "attempt-1".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: Some(ymp_runtime_api::McpBinding {
                    executable: executable.clone(),
                    socket_path: directory.path().join("agent.sock"),
                    token: "fixture-token".to_owned(),
                }),
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        let started = session.next_event().expect("started").expect("event");
        assert!(matches!(
            started.event,
            RuntimeEventKind::Started { opaque_session_id } if opaque_session_id == "thread-1"
        ));
        let tool_call = session.next_event().expect("tool call").expect("event");
        assert!(matches!(
            tool_call.event,
            RuntimeEventKind::McpToolCall {
                server,
                tool,
                status,
                result: Some(result),
                error: None,
                ..
            } if server == "ymp"
                && tool == "read_control"
                && status == "completed"
                && result["run_id"] == "run-1"
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
                    && usage.reasoning_output_tokens == 2
        ));
        assert!(session.next_event().expect("terminal").is_none());
        let arguments = fs::read_to_string(directory.path().join("invocation.args"))
            .expect("captured invocation arguments");
        assert!(arguments.contains("mcp_servers.ymp.required=true"));
        assert!(arguments.contains(
            "mcp_servers.ymp.enabled_tools=[\"read_control\",\"read_events\",\"submit\"]"
        ));
        assert!(arguments.contains("mcp_servers.ymp.default_tools_approval_mode=\"approve\""));
    }

    #[test]
    fn timeout_terminates_the_runtime_process_group() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-timeout-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  sleep 30 &
  printf '%s\n' "$!" > descendant.pid
  wait
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let profile = CodexProfile {
            wall_time_limit_ms: 100,
            ..CodexProfile::default()
        };
        let runtime = CodexRuntime::with_profile(&executable, profile);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-timeout".to_owned(),
                attempt_id: "attempt-timeout".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: Default::default(),
            })
            .expect("start fixture");
        assert!(matches!(
            session.next_event(),
            Err(RuntimeError::TimedOut { limit_ms: 100 })
        ));

        let descendant =
            fs::read_to_string(directory.path().join("descendant.pid")).expect("descendant pid");
        let descendant = descendant.trim();
        let mut alive = true;
        for _ in 0..20 {
            alive = std::process::Command::new("/bin/kill")
                .args(["-0", descendant])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!alive, "descendant process {descendant} survived timeout");
    }

    #[test]
    fn cancellation_token_interrupts_a_blocked_runtime_tree() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let executable = directory.path().join("codex-cancel-fixture");
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cd "$(dirname "$0")"
  sleep 30 &
  printf '%s\n' "$!" > cancel-descendant.pid
  wait
fi
"##,
        )
        .expect("write fixture");
        let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make executable");

        let cancellation = CancellationToken::default();
        let runtime = CodexRuntime::new(&executable);
        let mut session = runtime
            .start(InvocationRequest {
                invocation_id: "invocation-cancel".to_owned(),
                attempt_id: "attempt-cancel".to_owned(),
                workspace: directory.path().to_owned(),
                mcp: None,
                prompt: "fixture".to_owned(),
                cancellation: cancellation.clone(),
            })
            .expect("start fixture");
        let worker = std::thread::spawn(move || session.next_event());
        let pid_path = directory.path().join("cancel-descendant.pid");
        for _ in 0..40 {
            if pid_path.is_file() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        cancellation.cancel();
        let event = worker
            .join()
            .expect("runtime thread")
            .expect("runtime result")
            .expect("interrupted event");
        assert!(matches!(event.event, RuntimeEventKind::Interrupted));

        let descendant = fs::read_to_string(pid_path).expect("descendant pid");
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", descendant.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        assert!(!alive, "descendant process survived cancellation");
    }
}
