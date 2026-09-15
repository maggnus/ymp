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
//! | `turn.failed` (fields undocumented) | failure signal; class `codex-turn-failed` once exit is observed; free-form fields are counted and dropped |
//! | `error` (fields undocumented) | failure signal; class `codex-error-event`; only an allowlisted protocol code survives (see [`allowlisted_error_code`]), free-form fields are counted and dropped |
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
//! - free-form provider text never becomes a persisted diagnosis: a failure
//!   event keeps only an allowlisted protocol code (or the stable generic
//!   marker) adapter-side, and the child's stderr is drained to a sink and
//!   discarded, because SDK diagnostics may contain credentials;
//! - no session resume: each invocation is a fresh `codex exec`;
//! - the port's [`Receipt`] carries no settings report because the verified
//!   event kinds include none: `reported` settings stay unknown;
//! - token usage does not fit the port's [`ObservedUsage`] fields (turns,
//!   output characters, wall clock); the adapter preserves it adapter-side;
//! - the Codex child owns a separate process group; cancellation requests
//!   SIGTERM for the whole group and escalates to SIGKILL after a grace
//!   period, but claims a `cancelled` termination only when the immediate
//!   child's exit was observed and the group has no live members; zombie
//!   entries cannot execute effects and do not delay that observation; an
//!   unobserved exit stays the kernel's `uncertain` path;
//! - the reader threads poll without an overall deadline: a child that never
//!   exits keeps its reader thread alive until the host cancels or the
//!   process dies.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
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

/// Protocol-defined error codes a failure event may carry in its
/// `codexErrorInfo` as a string naming the failure mode, enumerated from the
/// Codex CLI protocol's error types. Membership test only: these are the only
/// provider-side failure codes this adapter ever preserves.
const PROTOCOL_ERROR_CODE_STRINGS: &[&str] = &[
    "contextWindowExceeded",
    "sessionBudgetExceeded",
    "usageLimitExceeded",
    "rateLimitExceeded",
    "serverOverloaded",
    "cyberPolicy",
    "misalignmentPolicyViolation",
    "internalServerError",
    "unauthorized",
    "badRequest",
    "threadRollbackFailed",
    "sandboxError",
    "other",
];

/// Protocol-defined error codes a failure event may carry in its
/// `codexErrorInfo` as an object whose key names the failure mode; the nested
/// payload is never read. Membership test only, like the string codes.
const PROTOCOL_ERROR_CODE_OBJECT_KEYS: &[&str] = &[
    "httpConnectionFailed",
    "responseStreamConnectionFailed",
    "responseStreamDisconnected",
    "responseTooManyFailedAttempts",
    "activeTurnNotSteerable",
];

/// Stable generic marker kept when a failure event carries no allowlisted
/// protocol code; a code states a code, never a cause.
const FAILURE_CODE_UNSPECIFIED: &str = "codex-failure-unspecified";

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
    /// Free-form failure texts (an `error` or `turn.failed` message) that
    /// were observed and dropped: counted here, never stored or surfaced,
    /// because SDK diagnostics may contain credentials.
    pub dropped_failure_details: u64,
}

/// The live record of one invocation, shared with the reader threads.
#[derive(Debug)]
struct CodexRun {
    child: Option<Child>,
    pid: u32,
    cancel_requested: bool,
    pending: VecDeque<ExecutionObservation>,
    /// Index of the next observation the current scan will read.
    scanned: usize,
    /// Index up to which observations were made consumed by a committed
    /// journal append; a failed append rewinds `scanned` back here.
    consumed: usize,
    receipt: Option<Receipt>,
    turns_observed: u64,
    output_chars_observed: u64,
    started_at: Duration,
    ended_at: Option<Duration>,
    thread_id: Option<String>,
    token_totals: Option<CodexTokenUsage>,
    failure_signal: Option<ErrorClass>,
    /// An allowlisted protocol code or the stable generic marker; free-form
    /// provider text is never stored here or anywhere else.
    failure_code: Option<String>,
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
            scanned: 0,
            consumed: 0,
            receipt: None,
            turns_observed: 0,
            output_chars_observed: 0,
            started_at,
            ended_at: None,
            thread_id: None,
            token_totals: None,
            failure_signal: None,
            failure_code: None,
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
        self.receipt = Some(Receipt::new(termination, None, usage, true));
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
                    self.failure_code = Some(
                        allowlisted_error_code(&value)
                            .unwrap_or(FAILURE_CODE_UNSPECIFIED)
                            .to_owned(),
                    );
                }
                self.count_dropped_failure_detail(&value);
            }
            "error" => {
                if self.failure_signal.is_none() {
                    self.failure_signal =
                        Some(ErrorClass::new(CLASS_ERROR_EVENT).expect("valid error class"));
                    self.failure_code = Some(
                        allowlisted_error_code(&value)
                            .unwrap_or(FAILURE_CODE_UNSPECIFIED)
                            .to_owned(),
                    );
                }
                self.count_dropped_failure_detail(&value);
            }
            _ => {
                self.stats.unmapped_lines += 1;
            }
        }
    }

    /// Counts the free-form failure text a failure event carried beyond any
    /// allowlisted code: observable as a number in [`CodexStreamStats`],
    /// never as text. SDK diagnostics may contain credentials, so the text
    /// itself is neither stored nor surfaced.
    fn count_dropped_failure_detail(&mut self, value: &Value) {
        let carries_free_form_text = value.get("message").is_some()
            || value
                .get("error")
                .is_some_and(|error| error.get("message").is_some());
        if carries_free_form_text {
            self.stats.dropped_failure_details += 1;
        }
    }
}

/// The allowlisted protocol error code a failure event carried, if any.
///
/// Membership test only: `codexErrorInfo` (directly on the event or nested
/// in its `error` object) is read either as the string naming the failure
/// mode or as an object whose key names it, and only codes in the fixed
/// [`PROTOCOL_ERROR_CODE_STRINGS`] / [`PROTOCOL_ERROR_CODE_OBJECT_KEYS`]
/// lists survive. Messages, nested payloads and every other free-form field
/// are dropped: they are never returned, stored or surfaced, because SDK
/// diagnostics may contain credentials.
fn allowlisted_error_code(value: &Value) -> Option<&'static str> {
    let info = value
        .get("codexErrorInfo")
        .or_else(|| value.pointer("/error/codexErrorInfo"))?;
    if let Some(reported) = info.as_str() {
        return PROTOCOL_ERROR_CODE_STRINGS
            .iter()
            .find(|code| **code == reported)
            .copied();
    }
    PROTOCOL_ERROR_CODE_OBJECT_KEYS
        .iter()
        .find(|code| info.get(*code).is_some())
        .copied()
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
    /// Resolved start outcomes, retained so retries return the first result
    /// without spawning another process or reclassifying a live invocation.
    start_outcomes: HashMap<InvocationId, Result<(), BackendStartFailure>>,
    /// Absolute native process-table executable used to verify that the
    /// invocation's process group can no longer perform effects. Keeping the
    /// path explicit prevents an invocation-controlled `PATH` from replacing
    /// this observation source.
    process_status_executable: PathBuf,
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
            start_outcomes: HashMap::new(),
            process_status_executable: PathBuf::from("/bin/ps"),
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

    /// The typed failure code preserved from a failure event: an allowlisted
    /// protocol code, or the stable generic marker when the event carried
    /// none. Free-form provider text is never stored or surfaced; its drop is
    /// counted in [`CodexStreamStats`].
    pub fn failure_code(&self, invocation: &InvocationId) -> Option<String> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .and_then(|run| run.failure_code.clone())
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

    fn start_run(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure> {
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
        let accesses = invocation.workspace_accesses();
        let workspace = if accesses.len() == 1 {
            PathBuf::from(accesses[0].scope().as_str())
        } else {
            return Err(Self::confirmed_failure(
                CLASS_WORKSPACE_MISSING,
                "this adapter runs in exactly one workspace scope",
            ));
        };
        if !workspace.is_dir() {
            return Err(Self::confirmed_failure(
                CLASS_WORKSPACE_MISSING,
                format!(
                    "workspace '{}' is not an existing directory",
                    workspace.display()
                ),
            ));
        }
        let mut command = Command::new(&self.executable);
        command
            .arg("exec")
            .arg("--json")
            .arg("--skip-git-repo-check")
            .process_group(0)
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
        command.arg("--").arg(prompt);

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

    /// Starts the stdout parser and the stderr drainer. The stdout parser
    /// holds the run lock only in short sections, so cancellation never
    /// blocks on it; the stderr drainer touches no run state at all.
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
            let process_status_executable = self.process_status_executable.clone();
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
                    if let Some(status) = status {
                        let process_group = run.lock().ok().map_or(0, |guard| guard.pid);
                        if process_group != 0
                            && Self::process_group_has_live_members(
                                &process_status_executable,
                                process_group,
                            )
                        {
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        if let Ok(mut guard) = run.lock() {
                            guard.exit_observed = true;
                            guard.finalize(&status, clock.as_ref());
                        }
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            });
        }

        if let Some(mut stderr) = stderr {
            thread::spawn(move || {
                // Drain stderr fully to a sink, retaining nothing: provider
                // diagnostics may contain credentials, so no byte of stderr
                // is stored, surfaced or persisted. The drain exists only so
                // the child cannot block on a full pipe.
                let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            });
        }
    }

    /// Reports whether the invocation group still has a process capable of
    /// executing effects. Zombie and dead entries cannot write and therefore
    /// do not delay the termination observation while their parent collects
    /// them. Failure to inspect the native process table stays conservative.
    fn process_group_has_live_members(executable: &Path, process_group: u32) -> bool {
        let output = Command::new(executable)
            .args(["-axo", "pgid=,stat="])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();
        let Ok(output) = output else {
            return true;
        };
        if !output.status.success() {
            return true;
        }
        let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
            return true;
        };
        let mut inspected_process = false;
        for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
            let mut fields = line.split_whitespace();
            let (Some(group), Some(state)) = (fields.next(), fields.next()) else {
                return true;
            };
            let Ok(group) = group.parse::<u32>() else {
                return true;
            };
            inspected_process = true;
            if group == process_group && !state.starts_with('Z') && !state.starts_with('X') {
                return true;
            }
        }
        // A successful but empty or wholly unusable response cannot prove
        // that the process group stopped writing.
        !inspected_process
    }

    /// Sends a signal to the complete process group owned by the invocation.
    fn signal_process_group(
        run: &Arc<Mutex<CodexRun>>,
        signal: Signal,
    ) -> Result<(), BackendCancelRefused> {
        let pid = run.lock().ok().map_or(0, |guard| guard.pid);
        if pid == 0 {
            return Err(BackendCancelRefused::new(
                "the invocation process group is unavailable",
            ));
        }
        let pid = i32::try_from(pid).map_err(|_| {
            BackendCancelRefused::new("the invocation process group ID is out of range")
        })?;
        match killpg(Pid::from_raw(pid), signal) {
            Ok(()) | Err(Errno::ESRCH) => Ok(()),
            Err(error) => Err(BackendCancelRefused::new(format!(
                "cannot signal the invocation process group: {error}"
            ))),
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
        // SIGTERM first, then SIGKILL for the complete invocation group.
        // Neither claims termination: the reader reports it only after the
        // immediate child exit and disappearance of the process group.
        Self::signal_process_group(&run, Signal::SIGTERM)?;
        let deadline = Instant::now() + self.cancel_grace;
        while Instant::now() < deadline {
            let observed = run.lock().map(|guard| guard.exit_observed).unwrap_or(true);
            if observed {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Self::signal_process_group(&run, Signal::SIGKILL)?;
        Ok(())
    }
}

impl ExecutionBackend for CodexBackend {
    fn start(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure> {
        if let Some(resolved) = self.start_outcomes.get(invocation.invocation()) {
            return resolved.clone();
        }
        let outcome = self.start_run(invocation);
        self.start_outcomes
            .insert(invocation.invocation().clone(), outcome.clone());
        outcome
    }

    fn cancel(&mut self, invocation: &InvocationId) -> Result<(), BackendCancelRefused> {
        self.cancel_invocation(invocation)
    }

    fn next_event(&mut self, invocation: &InvocationId) -> Option<ExecutionObservation> {
        let run = self.runs.get(invocation)?.lock().ok()?;
        let observation = run.pending.get(run.scanned).cloned()?;
        drop(run);
        if let Ok(mut run) = self.runs.get(invocation)?.lock() {
            run.scanned += 1;
        }
        Some(observation)
    }

    fn unread_last(&mut self, invocation: &InvocationId) {
        if let Some(handle) = self.runs.get(invocation)
            && let Ok(mut run) = handle.lock()
            && run.scanned > run.consumed
        {
            run.scanned -= 1;
        }
    }

    fn reset_scan(&mut self, invocation: &InvocationId) {
        if let Some(handle) = self.runs.get(invocation)
            && let Ok(mut run) = handle.lock()
        {
            run.scanned = run.consumed;
        }
    }

    fn commit_scan(&mut self, invocation: &InvocationId) {
        if let Some(handle) = self.runs.get(invocation)
            && let Ok(mut run) = handle.lock()
        {
            run.consumed = run.scanned;
        }
    }

    fn receipt(&mut self, invocation: &InvocationId) -> Option<Receipt> {
        self.runs
            .get(invocation)
            .and_then(|run| run.lock().ok())
            .and_then(|run| run.receipt.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AcceptanceContract, Allowance, AssignmentRequest, Constraints, Criterion, CriterionId,
        ExecutionScenario, Goal, InvocationLimits, InvocationStatus, ManualClock, MemoryJournal,
        ModelOffering, ObservationOutcome, OfferingId, ReservationPurpose, ResourceAmount,
        Revision, Role, ScriptedProvider, ScriptedRegistry, SessionId, Settings, Task, TaskId,
        UncertaintyCause, WorkspaceAccess, WorkspaceOperation, WorkspaceScope,
    };
    use std::os::unix::fs::PermissionsExt;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let unique = format!(
                "ymp-codex-process-probe-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock follows the Unix epoch")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            std::fs::create_dir(&path).expect("fixture directory is created");
            Self(path)
        }

        fn executable(&self, name: &str, body: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, body).expect("fixture executable is written");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("fixture executable permissions are set");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn failed_process_table_inspection_reaches_uncertain_without_releasing_workspace() {
        let directory = TestDir::new();
        let probe_attempted = directory.0.join("probe-attempted");
        let allow_probe = directory.0.join("allow-probe");
        let codex = directory.executable("codex", "#!/bin/sh\nexit 0\n");
        let process_status = directory.executable(
            "failing-ps",
            &format!(
                "#!/bin/sh\n\
                 if [ -f '{}' ]; then printf '1 S\\n'; exit 0; fi\n\
                 printf attempted > '{}'\n\
                 printf 'invalid process table\\n'\n\
                 exit 7\n",
                allow_probe.display(),
                probe_attempted.display()
            ),
        );
        assert!(CodexBackend::process_group_has_live_members(
            &directory.0.join("missing-ps"),
            u32::MAX
        ));

        let clock = ManualClock::new();
        let mut backend = CodexBackend::new(
            codex.to_string_lossy().into_owned(),
            Arc::new(clock.clone()),
        )
        .with_prompt("finish immediately");
        backend.process_status_executable = process_status;
        let scope = WorkspaceScope::new(directory.0.to_string_lossy().into_owned())
            .expect("valid workspace scope");
        let access = WorkspaceAccess::new(scope.clone(), [WorkspaceOperation::Write])
            .expect("valid workspace access");
        let agent = AgentId::new("codex").expect("valid agent ID");
        let offering = ModelOffering::new(
            OfferingId::new("codex-cli-local").expect("valid offering ID"),
            Vec::new(),
        )
        .expect("valid offering");
        let provider = ScriptedProvider::new(agent.clone(), offering)
            .with_effective_workspace_accesses([access.clone()]);
        let journal = MemoryJournal::new();
        let scenario = ExecutionScenario::over(
            journal,
            backend,
            [access.clone()],
            ScriptedRegistry::new([provider]),
            ResourceAmount::new(1),
            Arc::new(clock.clone()),
        );
        let session_id = SessionId::new("failed-process-inspection").expect("valid session ID");
        let task = Task::new(
            TaskId::new("failed-process-inspection").expect("valid task ID"),
            Goal::new("Keep the workspace held without process evidence").expect("valid goal"),
            AcceptanceContract::new(vec![
                Criterion::new(
                    CriterionId::new("hold-workspace").expect("valid criterion ID"),
                    "The workspace remains held until termination is observed.",
                )
                .expect("valid criterion"),
            ])
            .expect("valid acceptance contract"),
            Constraints::new(Vec::new()).expect("valid constraints"),
        );
        let limits = InvocationLimits::new(1, 1, Duration::from_millis(50))
            .expect("valid invocation limits");
        scenario
            .open_session(session_id.clone(), task)
            .expect("session opens");
        scenario.scan().expect("registry scan succeeds");
        let assignment = scenario
            .admit(
                &session_id,
                AssignmentRequest::new(
                    agent,
                    Role::new("implementer").expect("valid role"),
                    Settings::new(),
                    Allowance::new(
                        ResourceAmount::new(1),
                        ReservationPurpose::Production,
                        limits,
                    )
                    .expect("valid allowance"),
                    vec![access],
                )
                .expect("valid assignment request"),
                Revision::new(1),
            )
            .expect("assignment is admitted");
        scenario
            .invoke(&session_id, assignment.invocation(), Revision::new(2))
            .expect("invocation starts");

        let probe_deadline = Instant::now() + Duration::from_secs(5);
        while !probe_attempted.exists() && Instant::now() < probe_deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(probe_attempted.exists(), "the failing process probe ran");
        assert!(
            scenario.with_backend_mut(|backend| backend.receipt(assignment.invocation()).is_none()),
            "a failed process-table inspection must not fabricate a receipt"
        );

        clock.advance(limits.max_wall_clock());
        assert!(matches!(
            scenario
                .observe(&session_id, assignment.invocation(), Revision::new(4))
                .expect("the bounded observation resolves"),
            ObservationOutcome::UncertainAfterDeadline { .. }
        ));
        let invocation = scenario
            .execution_view(&session_id)
            .expect("execution history replays")
            .invocation(assignment.invocation())
            .expect("invocation remains recorded")
            .clone();
        assert_eq!(invocation.status(), InvocationStatus::Uncertain);
        assert_eq!(
            invocation.uncertainty_cause(),
            Some(UncertaintyCause::BoundedWaitExpired)
        );
        assert_eq!(
            scenario.workspace_hold_of(assignment.invocation()),
            Some(scope)
        );

        std::fs::write(&allow_probe, b"allow").expect("probe cleanup is enabled");
        let receipt_deadline = Instant::now() + Duration::from_secs(5);
        while scenario
            .with_backend_mut(|backend| backend.receipt(assignment.invocation()).is_none())
            && Instant::now() < receipt_deadline
        {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            scenario.with_backend_mut(|backend| backend.receipt(assignment.invocation()).is_some()),
            "the reader finishes after the conservative probe is restored"
        );
    }
}
