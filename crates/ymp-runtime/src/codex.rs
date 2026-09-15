//! Codex CLI provider adapter: the first real [`ExecutionBackend`].
//!
//! This adapter spawns the locally installed `codex` executable (verified on
//! this machine as `codex-cli 0.154.0`) as a child process through
//! `std::process` and maps its `codex exec --json` JSONL event stream into the
//! port's execution observations. Every return value stays an observation,
//! never authority; unknown metadata stays unknown and is never guessed.
//!
//! The verified `codex exec` flags used are:
//!
//! - `--json`: print events to stdout as JSONL (the only stream parsed);
//! - `--skip-git-repo-check`: allow running outside a Git repository, so the
//!   workspace root is not required to be a repository;
//! - `-m, --model <MODEL>`: passed through only when the recorded sent
//!   settings carry an admitted `model` setting.
//!
//! The child inherits the environment unchanged: authentication stays in the
//! native environment (`CODEX_HOME`), and no credential is read, copied or
//! stored. The process runs with its working directory set to the invocation's
//! workspace scope, interpreted as a directory path.
//!
//! Verified JSONL event kinds (from `codex exec --help` and the official
//! non-interactive-mode documentation of this Codex generation) and their
//! conservative mapping:
//!
//! | Codex JSONL event | Port observation |
//! | --- | --- |
//! | `thread.started` (field `thread_id`) | none; the thread id is kept adapter-side |
//! | `turn.started` | [`ExecutionObservation::UsageObserved`] with the running count of observed turn starts |
//! | `item.completed` with `item.type=agent_message` and a string `text` | [`ExecutionObservation::OutputObserved`] with the message's character count |
//! | other `item.started` / `item.updated` / `item.completed` kinds | none; not output text at the host-controlled boundary |
//! | `turn.completed` (field `usage`) | token totals kept adapter-side ([`CodexTokenUsage`]); the port's usage fields carry no tokens |
//! | `turn.failed` (fields undocumented) | failure signal; termination is classified `failed` with class `codex-turn-failed` once exit is observed |
//! | `error` (fields undocumented) | failure signal; class `codex-error-event`; a string `message` field, when present, is kept as the failure detail |
//! | process exit observed, code 0, no failure signal | [`Termination::Completed`] plus [`ExecutionObservation::WritesEnded`] |
//! | process exit observed, non-zero code | [`Termination::Failed`] with class `codex-nonzero-exit` (or the earlier signal's class) plus `WritesEnded` |
//! | exit by signal without a cancellation request | [`Termination::Failed`] with class `codex-signal-terminated` |
//! | exit observed after a cancellation request | [`Termination::Cancelled`] plus `WritesEnded` |
//! | malformed JSON line / unknown event type | no observation; counted adapter-side, never guessed |
//!
//! Honest limits of this adapter:
//!
//! - no model or effort negotiation: one default invocation shape, with an
//!   optional admitted `model` setting passed through to `-m`; any other sent
//!   setting key is refused at start with a typed confirmed never-started
//!   failure rather than silently dropped;
//! - no session resume: each invocation is a fresh `codex exec`;
//! - the port's [`Receipt`] carries no settings report because the verified
//!   event kinds include none: `reported` settings stay unknown;
//! - token usage does not fit the port's [`ObservedUsage`] fields (turns,
//!   output characters, wall clock); the adapter preserves it adapter-side;
//! - cancellation requests SIGTERM (through the POSIX `kill` utility, when
//!   present) and escalates to SIGKILL after a grace period, but claims a
//!   `cancelled` termination only when the child's exit was actually
//!   observed; an unobserved exit stays the kernel's `uncertain` path;
//! - the reader threads poll without an overall deadline: a child that never
//!   exits keeps its reader thread alive until the host cancels or the
//!   process dies.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use ymp_kernel::execution::{
    AgentId, BackendCancelRefused, BackendInvocation, BackendStartFailure, ErrorClass,
    ExclusionReason, ExecutionBackend, ExecutionObservation, InvocationId, ModelOffering,
    ObservedUsage, Pool, PoolEntry, Receipt, Registry, RegistryFailure, SettingKey, Settings,
    Termination,
};

use crate::clock::Clock;

/// How long the version probe waits before it kills the child and reports
/// the adapter not ready.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Default grace between SIGTERM and SIGKILL during cancellation.
const DEFAULT_CANCEL_GRACE: Duration = Duration::from_secs(3);

/// Upper bound of child stderr kept adapter-side for failure detail.
const STDERR_TAIL_BYTES: usize = 8 * 1024;

/// The one sent-setting key this adapter can pass through to the CLI.
const MODEL_SETTING_KEY: &str = "model";

/// Error classes this adapter reports; a class states a class, not a cause.
const CLASS_SPAWN_FAILED: &str = "codex-spawn-failed";
const CLASS_SIGNAL_TERMINATED: &str = "codex-signal-terminated";
const CLASS_NONZERO_EXIT: &str = "codex-nonzero-exit";
const CLASS_ERROR_EVENT: &str = "codex-error-event";
const CLASS_TURN_FAILED: &str = "codex-turn-failed";
const CLASS_UNSUPPORTED_SETTING: &str = "unsupported-sent-setting";
const CLASS_WORKSPACE_MISSING: &str = "workspace-dir-missing";
const CLASS_MISSING_PROMPT: &str = "missing-prompt";
const CLASS_DUPLICATE_INVOCATION: &str = "duplicate-invocation";

/// The typed outcome of one discovery probe of the `codex` executable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodexProbe {
    /// The executable answered the version probe; the version line is kept
    /// exactly as printed.
    Ready { version: String },
    /// The executable cannot serve invocations now, with the typed reason.
    NotReady { detail: String },
}

impl CodexProbe {
    /// Whether the probe admits the adapter to serving.
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// The version line the probe observed, if any.
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Ready { version } => Some(version),
            Self::NotReady { .. } => None,
        }
    }

    /// The not-ready reason, if any.
    pub fn not_ready_detail(&self) -> Option<&str> {
        match self {
            Self::Ready { .. } => None,
            Self::NotReady { detail } => Some(detail),
        }
    }
}

/// A registry over the locally installed Codex CLI.
///
/// Discovery is an explicit probe: it spawns `<executable> --version` under a
/// deadline, keeps the printed version line verbatim, and reports the agent
/// with its explicitly configured offering. The registry invents no models
/// and no controls: the offering is operator configuration, and anything the
/// native environment did not report stays unknown. Authentication stays in
/// the native environment; the probe reads no credential. Reading the pool
/// never probes; only an explicit scan does.
pub struct CodexRegistry {
    agent: AgentId,
    offering: ModelOffering,
    executable: String,
    probe_timeout: Duration,
    last_scan: Option<Pool>,
    last_probe: Option<CodexProbe>,
}

impl CodexRegistry {
    /// A registry for one agent identity whose offering comes from explicit
    /// configuration, never from a hand-invented model table inside this
    /// adapter. `executable` is resolved through `PATH` unless it names a
    /// path.
    pub fn new(agent: AgentId, offering: ModelOffering, executable: impl Into<String>) -> Self {
        Self {
            agent,
            offering,
            executable: executable.into(),
            probe_timeout: PROBE_TIMEOUT,
            last_scan: None,
            last_probe: None,
        }
    }

    /// Sets how long the version probe may run before the adapter is
    /// reported not ready.
    pub fn with_probe_timeout(mut self, timeout: Duration) -> Self {
        self.probe_timeout = timeout;
        self
    }

    /// The probe of the last explicit scan, if one ran.
    pub fn probe(&self) -> Option<&CodexProbe> {
        self.last_probe.as_ref()
    }
}

impl Registry for CodexRegistry {
    fn scan(&mut self) -> Result<Pool, RegistryFailure> {
        let probe = probe_with_timeout(&self.executable, self.probe_timeout);
        self.last_probe = Some(probe.clone());
        let exclusion = (!probe.is_ready()).then(|| ExclusionReason::NotReady {
            detail: probe
                .not_ready_detail()
                .unwrap_or("the probe reported not ready")
                .to_owned(),
        });
        // Exclusion is a fact about serving, not a denial of identity: the
        // agent stays in the pool with its typed reason.
        let pool = Pool::from_scan(vec![PoolEntry::new(
            self.agent.clone(),
            self.offering.clone(),
            exclusion,
        )]);
        self.last_scan = Some(pool.clone());
        Ok(pool)
    }

    fn pool(&self) -> Pool {
        self.last_scan.clone().unwrap_or_else(Pool::unscanned)
    }
}

/// Runs `<executable> --version` under a deadline, killing the child when the
/// deadline passes and reporting the typed not-ready reason.
fn probe_with_timeout(executable: &str, timeout: Duration) -> CodexProbe {
    let spawn = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawn {
        Ok(child) => child,
        Err(error) => {
            return CodexProbe::NotReady {
                detail: format!("cannot spawn '{executable}': {error}"),
            };
        }
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                if let Some(mut pipe) = child.stdout.take() {
                    let _ = pipe.read_to_string(&mut stdout);
                }
                let _ = child.wait();
                if !status.success() {
                    return CodexProbe::NotReady {
                        detail: format!(
                            "version probe of '{executable}' exited with status {status}"
                        ),
                    };
                }
                let version = stdout.lines().next().unwrap_or_default().trim();
                if version.is_empty() {
                    return CodexProbe::NotReady {
                        detail: format!("version probe of '{executable}' printed no version line"),
                    };
                }
                return CodexProbe::Ready {
                    version: version.to_owned(),
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return CodexProbe::NotReady {
                        detail: format!(
                            "version probe of '{executable}' did not answer within {} ms",
                            timeout.as_millis()
                        ),
                    };
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return CodexProbe::NotReady {
                    detail: format!("version probe of '{executable}' failed: {error}"),
                };
            }
        }
    }
}

/// Token totals one `turn.completed` event reported. A `None` component is a
/// field the event did not carry: it stays unknown, never zero. The port's
/// [`ObservedUsage`] has no token fields, so the adapter preserves these
/// totals adapter-side as evidence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CodexTokenUsage {
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_output_tokens: Option<u64>,
}

impl CodexTokenUsage {
    fn from_usage_value(usage: &Value) -> Self {
        let number = |name: &str| {
            usage
                .get(name)
                .and_then(Value::as_i64)
                .and_then(|value| u64::try_from(value).ok())
        };
        Self {
            input_tokens: number("input_tokens"),
            cached_input_tokens: number("cached_input_tokens"),
            output_tokens: number("output_tokens"),
            reasoning_output_tokens: number("reasoning_output_tokens"),
        }
    }
}

/// Conservative stream-parse statistics, kept adapter-side.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CodexStreamStats {
    /// Lines that were valid JSON but of an event kind or shape this adapter
    /// has not verified: mapped to no observation.
    pub unmapped_lines: u64,
    /// Lines that were not valid JSON at all.
    pub malformed_lines: u64,
}

/// The live record of one invocation, shared with the reader threads.
#[derive(Debug)]
struct CodexRun {
    child: Option<Child>,
    pid: u32,
    cancel_requested: bool,
    pending: VecDeque<ExecutionObservation>,
    receipt: Option<Receipt>,
    turns_observed: u64,
    output_chars_observed: u64,
    started_at: Duration,
    ended_at: Option<Duration>,
    thread_id: Option<String>,
    token_totals: Option<CodexTokenUsage>,
    failure_signal: Option<ErrorClass>,
    failure_detail: Option<String>,
    stderr_tail: Vec<u8>,
    stats: CodexStreamStats,
    exit_observed: bool,
}

impl CodexRun {
    fn new(child: Child, started_at: Duration) -> Self {
        let pid = child.id();
        Self {
            child: Some(child),
            pid,
            cancel_requested: false,
            pending: VecDeque::new(),
            receipt: None,
            turns_observed: 0,
            output_chars_observed: 0,
            started_at,
            ended_at: None,
            thread_id: None,
            token_totals: None,
            failure_signal: None,
            failure_detail: None,
            stderr_tail: Vec::new(),
            stats: CodexStreamStats::default(),
            exit_observed: false,
        }
    }

    /// Queues the exit-derived observations and assembles the receipt. Runs
    /// once, after the child's exit status was observed.
    fn finalize(&mut self, status: &ExitStatus, clock: &dyn Clock) {
        if self.receipt.is_some() {
            return;
        }
        self.ended_at = Some(clock.elapsed());
        let termination = if self.cancel_requested {
            Termination::Cancelled
        } else if let Some(class) = self.failure_signal.clone() {
            Termination::Failed { class }
        } else if status.success() {
            Termination::Completed
        } else if status.code().is_none() {
            Termination::Failed {
                class: ErrorClass::new(CLASS_SIGNAL_TERMINATED).expect("valid error class"),
            }
        } else {
            Termination::Failed {
                class: ErrorClass::new(CLASS_NONZERO_EXIT).expect("valid error class"),
            }
        };
        let wall_clock = clock.elapsed().saturating_sub(self.started_at);
        let usage = ObservedUsage::unknown()
            .with_turns(self.turns_observed)
            .with_output_chars(self.output_chars_observed)
            .with_wall_clock(wall_clock);
        self.pending.push_back(ExecutionObservation::Terminated {
            termination: termination.clone(),
        });
        // The backend observed this child's exit, so its writes to the held
        // scope have ended: an observation, not kernel proof.
        self.pending.push_back(ExecutionObservation::WritesEnded);
        // The verified event kinds include no settings report: `reported`
        // settings stay unknown.
        self.receipt = Some(Receipt::new(termination, None, usage));
    }

    /// Applies one JSONL line to the record; unknown shapes stay unmapped.
    fn apply_line(&mut self, line: &str) {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                self.stats.malformed_lines += 1;
                return;
            }
        };
        let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "thread.started" => {
                if self.thread_id.is_none()
                    && let Some(thread_id) = value.get("thread_id").and_then(Value::as_str)
                {
                    self.thread_id = Some(thread_id.to_owned());
                }
            }
            "turn.started" => {
                self.turns_observed += 1;
                self.pending.push_back(ExecutionObservation::UsageObserved {
                    usage: ObservedUsage::unknown().with_turns(self.turns_observed),
                });
            }
            "item.started" | "item.updated" => {
                // Verified event kinds that carry no output text at the
                // host-controlled boundary: no port observation.
            }
            "item.completed" => {
                let item = value.get("item");
                let is_agent_message = item
                    .and_then(|item| item.get("type"))
                    .and_then(Value::as_str)
                    == Some("agent_message");
                let text = item
                    .and_then(|item| item.get("text"))
                    .and_then(Value::as_str);
                if is_agent_message && let Some(text) = text {
                    let chars = text.chars().count() as u64;
                    self.output_chars_observed += chars;
                    self.pending
                        .push_back(ExecutionObservation::OutputObserved { chars });
                } else {
                    // A recognized container whose item kind or shape was not
                    // verified: no observation is guessed.
                    self.stats.unmapped_lines += 1;
                }
            }
            "turn.completed" => {
                if let Some(usage) = value.get("usage") {
                    self.token_totals = Some(CodexTokenUsage::from_usage_value(usage));
                }
            }
            "turn.failed" => {
                if self.failure_signal.is_none() {
                    self.failure_signal =
                        Some(ErrorClass::new(CLASS_TURN_FAILED).expect("valid error class"));
                    self.failure_detail = Some(truncated(line));
                }
            }
            "error" => {
                if self.failure_signal.is_none() {
                    self.failure_signal =
                        Some(ErrorClass::new(CLASS_ERROR_EVENT).expect("valid error class"));
                    let message = value.get("message").and_then(Value::as_str);
                    self.failure_detail = Some(message.map_or_else(|| truncated(line), truncated));
                }
            }
            _ => {
                self.stats.unmapped_lines += 1;
            }
        }
    }
}

fn truncated(text: &str) -> String {
    let mut truncated: String = text.chars().take(500).collect();
    if truncated.chars().count() < text.chars().count() {
        truncated.push('…');
    }
    truncated
}

/// The Codex CLI [`ExecutionBackend`]: spawn, stream-parse, cancel.
///
/// One configured instruction per backend instance: the port's
/// [`BackendInvocation`] carries no payload field, so the prompt is explicit
/// adapter configuration ([`CodexBackend::with_prompt`]) and a missing prompt
/// is a typed confirmed never-started start failure. Wall-clock usage is
/// measured with the injected [`Clock`]; the kernel's bounded-deadline
/// adjudication stays with the kernel, and the output bound is enforced at
/// the host-controlled drain boundary exactly as with the scripted backend.
pub struct CodexBackend {
    executable: String,
    clock: Arc<dyn Clock>,
    prompt: Option<String>,
    cancel_grace: Duration,
    extra_args: Vec<String>,
    runs: HashMap<InvocationId, Arc<Mutex<CodexRun>>>,
}

impl CodexBackend {
    /// A backend that spawns `executable` (resolved through `PATH` unless it
    /// names a path) and measures wall-clock usage with `clock`.
    pub fn new(executable: impl Into<String>, clock: Arc<dyn Clock>) -> Self {
        Self {
            executable: executable.into(),
            clock,
            prompt: None,
            cancel_grace: DEFAULT_CANCEL_GRACE,
            extra_args: Vec::new(),
            runs: HashMap::new(),
        }
    }

    /// Sets the instruction sent as the `codex exec` prompt argument.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Sets the grace between SIGTERM and SIGKILL during cancellation.
    pub fn with_cancel_grace(mut self, grace: Duration) -> Self {
        self.cancel_grace = grace;
        self
    }

    /// Appends explicit extra CLI arguments (for example a sandbox policy);
    /// the adapter passes them through verbatim after its own flags.
    pub fn with_extra_args(mut self, args: impl IntoIterator<Item = String>) -> Self {
        self.extra_args.extend(args);
        self
    }

    /// Token totals the invocation's `turn.completed` event reported, kept
    /// adapter-side because the port's usage fields carry no tokens.
    pub fn token_totals(&self, invocation: &InvocationId) -> Option<CodexTokenUsage> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .and_then(|run| run.token_totals)
    }

    /// The codex thread id the invocation's `thread.started` event reported.
    pub fn thread_id(&self, invocation: &InvocationId) -> Option<String> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .and_then(|run| run.thread_id.clone())
    }

    /// The provider-side failure detail (an `error` or `turn.failed` message)
    /// preserved as evidence; a class states a class, not a cause.
    pub fn failure_detail(&self, invocation: &InvocationId) -> Option<String> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .and_then(|run| run.failure_detail.clone())
    }

    /// The last bounded slice of the child's stderr, kept for diagnosis.
    pub fn stderr_tail(&self, invocation: &InvocationId) -> Option<String> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .map(|run| String::from_utf8_lossy(&run.stderr_tail).into_owned())
    }

    /// Conservative stream-parse statistics for one invocation.
    pub fn stream_stats(&self, invocation: &InvocationId) -> Option<CodexStreamStats> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .map(|run| run.stats)
    }

    fn confirmed_failure(class: &str, detail: impl Into<String>) -> BackendStartFailure {
        BackendStartFailure::new(
            ErrorClass::new(class).expect("valid error class"),
            true,
            detail,
        )
    }

    fn unsupported_sent_settings(sent: &Settings) -> Vec<String> {
        sent.iter()
            .map(|(key, _)| key.to_string())
            .filter(|key| key != MODEL_SETTING_KEY)
            .collect()
    }

    fn start_invocation(
        &mut self,
        invocation: &BackendInvocation,
    ) -> Result<(), BackendStartFailure> {
        let prompt = self.prompt.as_deref().ok_or_else(|| {
            Self::confirmed_failure(
                CLASS_MISSING_PROMPT,
                "no prompt is configured for this codex backend",
            )
        })?;
        let unsupported = Self::unsupported_sent_settings(invocation.sent_settings());
        if !unsupported.is_empty() {
            return Err(Self::confirmed_failure(
                CLASS_UNSUPPORTED_SETTING,
                format!(
                    "this adapter passes only the '{MODEL_SETTING_KEY}' setting; unsupported \
                     sent settings: {}",
                    unsupported.join(", ")
                ),
            ));
        }
        let workspace = Path::new(invocation.workspace().as_str());
        if !workspace.is_dir() {
            return Err(Self::confirmed_failure(
                CLASS_WORKSPACE_MISSING,
                format!(
                    "workspace '{}' is not an existing directory",
                    invocation.workspace()
                ),
            ));
        }
        if self.runs.contains_key(invocation.invocation()) {
            return Err(Self::confirmed_failure(
                CLASS_DUPLICATE_INVOCATION,
                format!(
                    "invocation '{}' was already started on this backend",
                    invocation.invocation()
                ),
            ));
        }

        let mut command = Command::new(&self.executable);
        command
            .arg("exec")
            .arg("--json")
            .arg("--skip-git-repo-check")
            .current_dir(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(model) = invocation
            .sent_settings()
            .get(&SettingKey::new(MODEL_SETTING_KEY).expect("valid setting key"))
        {
            command.arg("-m").arg(model.as_str());
        }
        for arg in &self.extra_args {
            command.arg(arg);
        }
        command.arg(prompt);

        let started_at = self.clock.elapsed();
        let child = command.spawn().map_err(|error| {
            Self::confirmed_failure(
                CLASS_SPAWN_FAILED,
                format!("cannot spawn '{}': {error}", self.executable),
            )
        })?;
        let run = Arc::new(Mutex::new(CodexRun::new(child, started_at)));
        self.spawn_readers(&run);
        self.runs.insert(invocation.invocation().clone(), run);
        Ok(())
    }

    /// Starts the stdout parser and the stderr drainer. Both threads hold
    /// the run lock only in short sections, so cancellation never blocks on
    /// them.
    fn spawn_readers(&self, run: &Arc<Mutex<CodexRun>>) {
        let mut guard = match run.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let stdout = guard.child.as_mut().and_then(|child| child.stdout.take());
        let stderr = guard.child.as_mut().and_then(|child| child.stderr.take());
        drop(guard);

        if let Some(stdout) = stdout {
            let run = Arc::clone(run);
            let clock = Arc::clone(&self.clock);
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Ok(mut guard) = run.lock() {
                        guard.apply_line(&line);
                    }
                }
                // Stdout reached EOF; poll the child's exit without holding
                // the lock, then finalize under it. The poll has no overall
                // deadline: the kernel's bounded wait and cancellation own
                // that decision.
                loop {
                    let status = {
                        let Ok(mut guard) = run.lock() else {
                            return;
                        };
                        let Some(child) = guard.child.as_mut() else {
                            return;
                        };
                        match child.try_wait() {
                            Ok(status) => status,
                            Err(_) => return,
                        }
                    };
                    if let Some(status) = status
                        && let Ok(mut guard) = run.lock()
                    {
                        guard.exit_observed = true;
                        guard.finalize(&status, clock.as_ref());
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            });
        }

        if let Some(mut stderr) = stderr {
            let run = Arc::clone(run);
            thread::spawn(move || {
                let mut buffer = [0u8; 4096];
                loop {
                    match stderr.read(&mut buffer) {
                        Ok(0) | Err(_) => return,
                        Ok(read) => {
                            if let Ok(mut guard) = run.lock() {
                                guard.stderr_tail.extend_from_slice(&buffer[..read]);
                                let excess =
                                    guard.stderr_tail.len().saturating_sub(STDERR_TAIL_BYTES);
                                if excess > 0 {
                                    guard.stderr_tail.drain(0..excess);
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    /// Sends a signal to the run's child through the POSIX `kill` utility.
    /// Returns whether the utility ran successfully; `false` never asserts
    /// the child is dead.
    fn signal_child(run: &Arc<Mutex<CodexRun>>, signal_name: &str) -> bool {
        let pid = run.lock().ok().map_or(0, |guard| guard.pid);
        if pid == 0 {
            return false;
        }
        Command::new("kill")
            .arg("-s")
            .arg(signal_name)
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Escalates to SIGKILL: through the `kill` utility when present, else
    /// through `Child::kill` under a short lock.
    fn kill_child(run: &Arc<Mutex<CodexRun>>) {
        if !Self::signal_child(run, "KILL")
            && let Ok(mut guard) = run.lock()
            && let Some(child) = guard.child.as_mut()
        {
            let _ = child.kill();
        }
    }

    fn cancel_invocation(&mut self, invocation: &InvocationId) -> Result<(), BackendCancelRefused> {
        let run = self.runs.get(invocation).cloned().ok_or_else(|| {
            BackendCancelRefused::new("this codex backend does not know the invocation")
        })?;
        {
            let mut guard = run
                .lock()
                .map_err(|_| BackendCancelRefused::new("the invocation record is poisoned"))?;
            if guard.receipt.is_some() {
                // Exit already observed; nothing to signal.
                return Ok(());
            }
            guard.cancel_requested = true;
        }
        // SIGTERM first; escalate straight to SIGKILL when the utility is
        // unavailable. Neither claims termination: only the observed exit
        // does, and the reader thread reports it.
        if !Self::signal_child(&run, "TERM") {
            Self::kill_child(&run);
        }
        let deadline = Instant::now() + self.cancel_grace;
        while Instant::now() < deadline {
            let observed = run.lock().map(|guard| guard.exit_observed).unwrap_or(true);
            if observed {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Self::kill_child(&run);
        Ok(())
    }
}

impl ExecutionBackend for CodexBackend {
    fn start(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure> {
        self.start_invocation(invocation)
    }

    fn cancel(&mut self, invocation: &InvocationId) -> Result<(), BackendCancelRefused> {
        self.cancel_invocation(invocation)
    }

    fn events(&mut self, invocation: &InvocationId) -> Vec<ExecutionObservation> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .map(|mut run| run.pending.drain(..).collect())
            .unwrap_or_default()
    }

    fn receipt(&mut self, invocation: &InvocationId) -> Option<Receipt> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .and_then(|run| run.receipt.clone())
    }
}
