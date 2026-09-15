//! Codex App Server adapter for the provider execution boundary.
//!
//! The adapter starts `codex app-server --stdio`, completes the App Server
//! handshake synchronously in [`ExecutionBackend::start`], and then maps only
//! the selected thread and turn into typed observations. Authentication and
//! unknown native metadata remain in the native Codex environment.

mod rpc;
mod settings;
mod usage;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use rpc::{ReaderEvent, RpcFailure, RpcProcess, SharedInput, spawn_reader};
use serde_json::{Map, Value, json};
use settings::{load_catalog, setting};
use usage::{CodexUsage, TokenCounts, codex_counts};
use ymp_kernel::execution::{
    AgentId, BackendCancelRefused, BackendInvocation, BackendStartFailure, ErrorClass,
    ExclusionReason, ExecutionBackend, ExecutionObservation, InvocationId, ModelOffering,
    ObservedUsage, Pool, PoolEntry, Receipt, Registry, RegistryFailure, Settings, Termination,
    WorkspaceOperation,
};

use crate::clock::{Clock, SystemClock};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_CANCEL_GRACE: Duration = Duration::from_secs(3);
const SUPERVISOR_POLL: Duration = Duration::from_millis(10);
const SUPERVISOR_SCRIPT: &str = r#"
trap '' TERM
trap 'kill -KILL -- -$$ 2>/dev/null' HUP INT EXIT
"$@" <&0 &
agent=$!
wait "$agent"
exit $?
"#;

const MODEL_SETTING_KEY: &str = "model";
const EFFORT_SETTING_KEY: &str = "effort";

const CLASS_SPAWN_FAILED: &str = "codex-spawn-failed";
const CLASS_PROTOCOL_FAILURE: &str = "codex-protocol-failure";
const CLASS_PROTOCOL_ENDED: &str = "codex-protocol-ended";
const CLASS_EMPTY_RESPONSE: &str = "codex-empty-response";
const CLASS_ERROR_EVENT: &str = "codex-error-event";
const CLASS_TURN_FAILED: &str = "codex-turn-failed";
const CLASS_UNSUPPORTED_SETTING: &str = "unsupported-sent-setting";
const CLASS_INVALID_ARGUMENT: &str = "unsupported-codex-argument";
const CLASS_WORKSPACE_MISSING: &str = "workspace-dir-missing";
const CLASS_MISSING_PROMPT: &str = "missing-prompt";
const FAILURE_CODE_UNSPECIFIED: &str = "codex-failure-unspecified";

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

const PROTOCOL_ERROR_CODE_OBJECT_KEYS: &[&str] = &[
    "httpConnectionFailed",
    "responseStreamConnectionFailed",
    "responseStreamDisconnected",
    "responseTooManyFailedAttempts",
    "activeTurnNotSteerable",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodexProbe {
    Ready { version: String },
    NotReady { detail: String },
}

impl CodexProbe {
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Ready { version } => Some(version),
            Self::NotReady { .. } => None,
        }
    }

    pub fn not_ready_detail(&self) -> Option<&str> {
        match self {
            Self::Ready { .. } => None,
            Self::NotReady { detail } => Some(detail),
        }
    }
}

/// Registry backed by a short-lived App Server `model/list` scan.
pub struct CodexRegistry {
    agent: AgentId,
    offering: ModelOffering,
    executable: String,
    probe_timeout: Duration,
    last_scan: Option<Pool>,
    last_probe: Option<CodexProbe>,
}

impl CodexRegistry {
    pub fn new(agent: AgentId, offering: ModelOffering, executable: impl Into<String>) -> Self {
        Self {
            agent,
            offering,
            executable: executable.into(),
            probe_timeout: HANDSHAKE_TIMEOUT,
            last_scan: None,
            last_probe: None,
        }
    }

    pub fn with_probe_timeout(mut self, timeout: Duration) -> Self {
        self.probe_timeout = timeout;
        self
    }

    pub fn probe(&self) -> Option<&CodexProbe> {
        self.last_probe.as_ref()
    }

    fn native_scan(&self) -> Result<(String, ModelOffering), String> {
        let workspace = std::env::current_dir()
            .map_err(|error| format!("cannot read the registry working directory: {error}"))?;
        let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
        let (mut process, handle) = spawn_app_server(&self.executable, &workspace, true, clock)
            .map_err(|error| format!("cannot spawn '{}': {error}", self.executable))?;
        let deadline = Instant::now() + self.probe_timeout;
        let result = (|| {
            let initialized = initialize(&mut process, deadline)?;
            let version = initialized
                .get("userAgent")
                .and_then(Value::as_str)
                .filter(|version| !version.trim().is_empty())
                .ok_or_else(|| RpcFailure::protocol("Codex initialize omitted userAgent"))?
                .to_owned();
            let catalog = load_catalog(&mut process, deadline)?;
            let offering = catalog
                .offering(self.offering.id().clone())
                .map_err(RpcFailure::protocol)?;
            Ok((version, offering))
        })();
        process.close_input();
        if !handle.wait_for_exit(deadline) {
            handle.kill("KILL");
        }
        result.map_err(|failure: RpcFailure| failure.to_string())
    }
}

impl Registry for CodexRegistry {
    fn scan(&mut self) -> Result<Pool, RegistryFailure> {
        let (probe, offering, exclusion) = match self.native_scan() {
            Ok((version, offering)) => (CodexProbe::Ready { version }, offering, None),
            Err(detail) => (
                CodexProbe::NotReady {
                    detail: detail.clone(),
                },
                self.offering.clone(),
                Some(ExclusionReason::NotReady { detail }),
            ),
        };
        self.last_probe = Some(probe);
        let pool = Pool::from_scan(vec![PoolEntry::new(
            self.agent.clone(),
            offering,
            exclusion,
        )]);
        self.last_scan = Some(pool.clone());
        Ok(pool)
    }

    fn pool(&self) -> Pool {
        self.last_scan.clone().unwrap_or_else(Pool::unscanned)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CodexTokenUsage {
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_output_tokens: Option<u64>,
}

impl From<TokenCounts> for CodexTokenUsage {
    fn from(counts: TokenCounts) -> Self {
        Self {
            input_tokens: counts.input,
            cached_input_tokens: counts.cache_read,
            output_tokens: counts.output,
            reasoning_output_tokens: counts.reasoning,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CodexStreamStats {
    pub unmapped_lines: u64,
    pub malformed_lines: u64,
    pub dropped_failure_details: u64,
}

#[derive(Debug)]
struct CodexRun {
    pid: u32,
    cancel_requested: bool,
    pending: VecDeque<ExecutionObservation>,
    scanned: usize,
    consumed: usize,
    receipt: Option<Receipt>,
    turns_observed: u64,
    output_chars_observed: u64,
    started_at: Duration,
    session: Option<String>,
    turn: Option<String>,
    reported_settings: Option<Settings>,
    usage: ObservedUsage,
    token_totals: Option<CodexTokenUsage>,
    turn_termination: Option<Termination>,
    failure_code: Option<String>,
    stats: CodexStreamStats,
    exit_observed: bool,
    stream_drained: bool,
}

impl CodexRun {
    fn new(pid: u32, started_at: Duration) -> Self {
        Self {
            pid,
            cancel_requested: false,
            pending: VecDeque::new(),
            scanned: 0,
            consumed: 0,
            receipt: None,
            turns_observed: 0,
            output_chars_observed: 0,
            started_at,
            session: None,
            turn: None,
            reported_settings: None,
            usage: ObservedUsage::unknown(),
            token_totals: None,
            turn_termination: None,
            failure_code: None,
            stats: CodexStreamStats::default(),
            exit_observed: false,
            stream_drained: false,
        }
    }

    fn fail(&mut self, class: &str, code: Option<String>) {
        if self.turn_termination.is_none() {
            self.turn_termination = Some(Termination::Failed {
                class: ErrorClass::new(class).expect("static error class is valid"),
            });
            self.failure_code = code.or_else(|| Some(FAILURE_CODE_UNSPECIFIED.to_owned()));
        }
    }

    fn maybe_finalize(&mut self, clock: &dyn Clock) {
        if self.receipt.is_some() || !self.stream_drained {
            return;
        }
        if !self.exit_observed {
            return;
        }
        let termination = if self.cancel_requested {
            Termination::Cancelled
        } else if let Some(termination) = self.turn_termination.clone() {
            termination
        } else {
            Termination::Failed {
                class: ErrorClass::new(CLASS_PROTOCOL_ENDED).expect("static error class is valid"),
            }
        };
        let usage = self
            .usage
            .with_turns(self.turns_observed)
            .with_output_chars(self.output_chars_observed)
            .with_wall_clock(clock.elapsed().saturating_sub(self.started_at));
        self.pending.push_back(ExecutionObservation::Terminated {
            termination: termination.clone(),
        });
        self.pending.push_back(ExecutionObservation::WritesEnded);
        let mut receipt = Receipt::new(termination, self.reported_settings.clone(), usage, true);
        if let Some(session) = &self.session {
            receipt = receipt.with_session(session.clone());
        }
        self.receipt = Some(receipt);
    }
}

#[derive(Clone)]
struct RunHandle {
    state: Arc<Mutex<CodexRun>>,
    input: SharedInput,
    kill_root: Arc<AtomicBool>,
}

impl RunHandle {
    fn close_input(&self) {
        if let Ok(mut input) = self.input.lock() {
            input.take();
        }
    }

    fn kill(&self, signal: &str) {
        let pid = self.state.lock().ok().map_or(0, |state| state.pid);
        if pid == 0 || !signal_process_group(pid, signal) {
            self.kill_root.store(true, Ordering::Release);
        }
    }

    fn wait_for_exit(&self, deadline: Instant) -> bool {
        loop {
            if self
                .state
                .lock()
                .map(|state| state.exit_observed)
                .unwrap_or(true)
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(SUPERVISOR_POLL);
        }
    }
}

fn signal_process_group(pid: u32, signal: &str) -> bool {
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;

        let signal = match signal {
            "TERM" => Signal::SIGTERM,
            "KILL" => Signal::SIGKILL,
            _ => return false,
        };
        i32::try_from(pid)
            .ok()
            .is_some_and(|group| killpg(Pid::from_raw(group), signal).is_ok())
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, signal);
        false
    }
}

fn spawn_supervisor(
    mut child: Child,
    handle: RunHandle,
    clock: Arc<dyn Clock>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
            if handle.kill_root.swap(false, Ordering::AcqRel) {
                let _ = child.kill();
            }
            match child.try_wait() {
                Ok(Some(_status)) => {
                    if let Ok(mut state) = handle.state.lock() {
                        state.exit_observed = true;
                        state.maybe_finalize(clock.as_ref());
                    }
                    // Descendants can retain stdout after the group leader
                    // exits. Cleanup starts from the independently observed
                    // process status and does not wait for EOF first.
                    let _ = signal_process_group(child.id(), "KILL");
                    return;
                }
                Ok(None) => thread::sleep(SUPERVISOR_POLL),
                Err(_) => {
                    let _ = child.kill();
                    return;
                }
            }
        }
    })
}

fn spawn_app_server(
    executable: &str,
    workspace: &Path,
    read_only: bool,
    clock: Arc<dyn Clock>,
) -> Result<(RpcProcess, RunHandle), String> {
    let executable = resolve_executable(executable, workspace)?;
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(SUPERVISOR_SCRIPT)
        .arg("ymp-codex-supervisor")
        .arg(executable)
        .arg("app-server")
        .arg("--stdio")
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let started_at = clock.elapsed();
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let pid = child.id();
    let input = child.stdin.take().ok_or_else(|| {
        let _ = child.kill();
        "App Server stdin was not piped".to_owned()
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        let _ = child.kill();
        "App Server stdout was not piped".to_owned()
    })?;
    if let Some(mut stderr) = child.stderr.take() {
        thread::spawn(move || {
            let _ = std::io::copy(&mut stderr, &mut std::io::sink());
        });
    }
    let input = Arc::new(Mutex::new(Some(input)));
    let output = spawn_reader(stdout);
    let state = Arc::new(Mutex::new(CodexRun::new(pid, started_at)));
    let handle = RunHandle {
        state,
        input: Arc::clone(&input),
        kill_root: Arc::new(AtomicBool::new(false)),
    };
    spawn_supervisor(child, handle.clone(), clock);
    Ok((RpcProcess::new(input, output, read_only), handle))
}

fn resolve_executable(executable: &str, workspace: &Path) -> Result<PathBuf, String> {
    let path = Path::new(executable);
    if path.components().count() > 1 {
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            workspace.join(path)
        };
        return path
            .is_file()
            .then_some(path)
            .ok_or_else(|| format!("executable '{executable}' is not a file"));
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(executable))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("executable '{executable}' was not found on PATH"))
}

fn initialize(process: &mut RpcProcess, deadline: Instant) -> Result<Value, RpcFailure> {
    let response = process.request(
        "initialize",
        json!({
            "clientInfo": {"name": "ymp", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {}
        }),
        deadline,
    )?;
    process.notify("initialized", json!({}))?;
    Ok(response)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AdapterOptions {
    ephemeral: bool,
    sandbox: Option<String>,
    workspace_write_config: Map<String, Value>,
}

impl AdapterOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut parsed = Self::default();
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--ephemeral" if !parsed.ephemeral => parsed.ephemeral = true,
                "--sandbox" | "-s" => {
                    index += 1;
                    let sandbox = arguments
                        .get(index)
                        .ok_or_else(|| "the sandbox argument has no value".to_owned())?;
                    if !matches!(sandbox.as_str(), "read-only" | "workspace-write") {
                        return Err(format!("sandbox mode '{sandbox}' is not allowlisted"));
                    }
                    if parsed.sandbox.replace(sandbox.clone()).is_some() {
                        return Err("the sandbox argument is repeated".to_owned());
                    }
                }
                "-c" | "--config" => {
                    index += 1;
                    let config = arguments
                        .get(index)
                        .ok_or_else(|| "the config argument has no value".to_owned())?;
                    match config.as_str() {
                        "approval_policy=\"never\"" => {}
                        "sandbox_workspace_write.writable_roots=[]" => {
                            parsed
                                .workspace_write_config
                                .insert("writable_roots".to_owned(), json!([]));
                        }
                        "sandbox_workspace_write.exclude_slash_tmp=true" => {
                            parsed
                                .workspace_write_config
                                .insert("exclude_slash_tmp".to_owned(), json!(true));
                        }
                        "sandbox_workspace_write.exclude_tmpdir_env_var=true" => {
                            parsed
                                .workspace_write_config
                                .insert("exclude_tmpdir_env_var".to_owned(), json!(true));
                        }
                        _ => return Err(format!("config override '{config}' is not allowlisted")),
                    }
                }
                argument => return Err(format!("argument '{argument}' is not allowlisted")),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn thread_config(&self) -> Value {
        let mut config = Map::new();
        if !self.workspace_write_config.is_empty() {
            config.insert(
                "sandbox_workspace_write".to_owned(),
                Value::Object(self.workspace_write_config.clone()),
            );
        }
        Value::Object(config)
    }
}

struct HandshakeFailure {
    source: RpcFailure,
    confirmed_never_started: bool,
}

impl HandshakeFailure {
    fn before_invocation(source: RpcFailure) -> Self {
        Self {
            source,
            confirmed_never_started: true,
        }
    }

    fn after_start_request(source: RpcFailure) -> Self {
        Self {
            source,
            confirmed_never_started: false,
        }
    }
}

pub struct CodexBackend {
    executable: String,
    clock: Arc<dyn Clock>,
    prompt: Option<String>,
    cancel_grace: Duration,
    extra_args: Vec<String>,
    runs: HashMap<InvocationId, RunHandle>,
    start_outcomes: HashMap<InvocationId, Result<(), BackendStartFailure>>,
}

impl CodexBackend {
    pub fn new(executable: impl Into<String>, clock: Arc<dyn Clock>) -> Self {
        Self {
            executable: executable.into(),
            clock,
            prompt: None,
            cancel_grace: DEFAULT_CANCEL_GRACE,
            extra_args: Vec::new(),
            runs: HashMap::new(),
            start_outcomes: HashMap::new(),
        }
    }

    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn with_cancel_grace(mut self, grace: Duration) -> Self {
        self.cancel_grace = grace;
        self
    }

    /// Accepts only the narrow argument vocabulary parsed by
    /// [`AdapterOptions`]; arbitrary arguments are rejected before spawn.
    pub fn with_extra_args(mut self, args: impl IntoIterator<Item = String>) -> Self {
        self.extra_args.extend(args);
        self
    }

    pub fn token_totals(&self, invocation: &InvocationId) -> Option<CodexTokenUsage> {
        self.runs
            .get(invocation)
            .and_then(|run| run.state.lock().ok())
            .and_then(|state| state.token_totals)
    }

    pub fn thread_id(&self, invocation: &InvocationId) -> Option<String> {
        self.runs
            .get(invocation)
            .and_then(|run| run.state.lock().ok())
            .and_then(|state| state.session.clone())
    }

    pub fn failure_code(&self, invocation: &InvocationId) -> Option<String> {
        self.runs
            .get(invocation)
            .and_then(|run| run.state.lock().ok())
            .and_then(|state| state.failure_code.clone())
    }

    pub fn stream_stats(&self, invocation: &InvocationId) -> Option<CodexStreamStats> {
        self.runs
            .get(invocation)
            .and_then(|run| run.state.lock().ok())
            .map(|state| state.stats)
    }

    fn start_failure(
        class: &str,
        confirmed_never_started: bool,
        detail: impl Into<String>,
    ) -> BackendStartFailure {
        BackendStartFailure::new(
            ErrorClass::new(class).expect("static error class is valid"),
            confirmed_never_started,
            detail,
        )
    }

    fn validate_start(
        &self,
        invocation: &BackendInvocation,
    ) -> Result<(PathBuf, bool, AdapterOptions), BackendStartFailure> {
        if self.prompt.as_ref().is_none_or(|prompt| prompt.is_empty()) {
            return Err(Self::start_failure(
                CLASS_MISSING_PROMPT,
                true,
                "no prompt is configured for this Codex backend",
            ));
        }
        let unsupported = invocation
            .sent_settings()
            .iter()
            .map(|(key, _)| key.as_str())
            .filter(|key| !matches!(*key, MODEL_SETTING_KEY | EFFORT_SETTING_KEY))
            .collect::<Vec<_>>();
        if !unsupported.is_empty() {
            return Err(Self::start_failure(
                CLASS_UNSUPPORTED_SETTING,
                true,
                format!("unsupported sent settings: {}", unsupported.join(", ")),
            ));
        }
        let options = AdapterOptions::parse(&self.extra_args)
            .map_err(|detail| Self::start_failure(CLASS_INVALID_ARGUMENT, true, detail))?;
        if invocation.session().is_some() && options.ephemeral {
            return Err(Self::start_failure(
                CLASS_INVALID_ARGUMENT,
                true,
                "an ephemeral Codex thread cannot be resumed",
            ));
        }
        let accesses = invocation.workspace_accesses();
        let workspace = if accesses.len() == 1 {
            PathBuf::from(accesses[0].scope().as_str())
        } else {
            return Err(Self::start_failure(
                CLASS_WORKSPACE_MISSING,
                true,
                "this adapter runs in exactly one workspace scope",
            ));
        };
        if !workspace.is_dir() {
            return Err(Self::start_failure(
                CLASS_WORKSPACE_MISSING,
                true,
                format!(
                    "workspace '{}' is not an existing directory",
                    workspace.display()
                ),
            ));
        }
        let writable = accesses[0]
            .operations()
            .contains(&WorkspaceOperation::Write);
        if !writable && options.sandbox.as_deref() == Some("workspace-write") {
            return Err(Self::start_failure(
                CLASS_INVALID_ARGUMENT,
                true,
                "workspace-write exceeds the admitted read-only workspace access",
            ));
        }
        Ok((workspace, !writable, options))
    }

    fn start_run(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure> {
        let (workspace, read_only, options) = self.validate_start(invocation)?;
        let (mut process, handle) = spawn_app_server(
            &self.executable,
            &workspace,
            read_only,
            Arc::clone(&self.clock),
        )
        .map_err(|error| {
            Self::start_failure(
                CLASS_SPAWN_FAILED,
                true,
                format!("cannot spawn '{}': {error}", self.executable),
            )
        })?;
        let timeout = HANDSHAKE_TIMEOUT.min(invocation.limits().max_wall_clock());
        let deadline = Instant::now() + timeout;
        let started = self.complete_handshake(
            invocation,
            &workspace,
            &options,
            &mut process,
            &handle,
            deadline,
        );
        match started {
            Ok((session, turn)) => {
                self.spawn_event_worker(
                    process,
                    handle.clone(),
                    session,
                    turn,
                    invocation.session().is_some(),
                );
                self.runs.insert(invocation.invocation().clone(), handle);
                Ok(())
            }
            Err(failure) => {
                process.close_input();
                handle.kill("KILL");
                Err(Self::start_failure(
                    CLASS_PROTOCOL_FAILURE,
                    failure.confirmed_never_started,
                    failure.source.detail(),
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_handshake(
        &self,
        invocation: &BackendInvocation,
        workspace: &Path,
        options: &AdapterOptions,
        process: &mut RpcProcess,
        handle: &RunHandle,
        deadline: Instant,
    ) -> Result<(String, String), HandshakeFailure> {
        initialize(process, deadline).map_err(HandshakeFailure::before_invocation)?;
        let catalog =
            load_catalog(process, deadline).map_err(HandshakeFailure::before_invocation)?;
        catalog
            .validate(invocation.sent_settings())
            .map_err(RpcFailure::protocol)
            .map_err(HandshakeFailure::before_invocation)?;

        let sandbox = options.sandbox.as_deref().unwrap_or(
            if invocation.workspace_accesses()[0]
                .operations()
                .contains(&WorkspaceOperation::Write)
            {
                "workspace-write"
            } else {
                "read-only"
            },
        );
        let mut thread_params = json!({
            "cwd": workspace,
            "approvalPolicy": "never",
            "sandbox": sandbox,
            "config": options.thread_config()
        });
        if let Some(model) = setting(invocation.sent_settings(), MODEL_SETTING_KEY) {
            thread_params["model"] = json!(model);
        }
        if let Some(effort) = setting(invocation.sent_settings(), EFFORT_SETTING_KEY) {
            thread_params["config"]["model_reasoning_effort"] = json!(effort);
        }
        let thread_response = if let Some(session) = invocation.session() {
            thread_params["threadId"] = json!(session);
            process
                .request("thread/resume", thread_params, deadline)
                .map_err(HandshakeFailure::after_start_request)?
        } else {
            thread_params["ephemeral"] = json!(options.ephemeral);
            process
                .request("thread/start", thread_params, deadline)
                .map_err(HandshakeFailure::after_start_request)?
        };
        catalog
            .verify_acknowledgement(invocation.sent_settings(), &thread_response)
            .map_err(RpcFailure::protocol)
            .map_err(HandshakeFailure::after_start_request)?;
        let session = thread_response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| RpcFailure::protocol("Codex did not return a thread ID"))
            .map_err(HandshakeFailure::after_start_request)?
            .to_owned();

        let prompt = self
            .prompt
            .as_deref()
            .expect("prompt validated before spawn");
        let mut turn_params = json!({
            "threadId": session,
            "input": [{"type": "text", "text": prompt}]
        });
        if let Some(model) = setting(invocation.sent_settings(), MODEL_SETTING_KEY) {
            turn_params["model"] = json!(model);
        }
        if let Some(effort) = setting(invocation.sent_settings(), EFFORT_SETTING_KEY) {
            turn_params["effort"] = json!(effort);
        }
        let turn_response = process
            .request("turn/start", turn_params, deadline)
            .map_err(HandshakeFailure::after_start_request)?;
        let turn = turn_response
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| RpcFailure::protocol("Codex did not return a turn ID"))
            .map_err(HandshakeFailure::after_start_request)?
            .to_owned();
        if let Ok(mut state) = handle.state.lock() {
            state.session = Some(session.clone());
            state.turn = Some(turn.clone());
            state.reported_settings = Some(invocation.sent_settings().clone());
            state.turns_observed = 1;
            state
                .pending
                .push_back(ExecutionObservation::SettingsReported {
                    settings: invocation.sent_settings().clone(),
                });
            state
                .pending
                .push_back(ExecutionObservation::UsageObserved {
                    usage: ObservedUsage::unknown().with_turns(1),
                });
        }
        Ok((session, turn))
    }

    fn spawn_event_worker(
        &self,
        mut process: RpcProcess,
        handle: RunHandle,
        session: String,
        turn: String,
        resumed: bool,
    ) {
        let clock = Arc::clone(&self.clock);
        thread::spawn(move || {
            let mut usage = CodexUsage::new(resumed, None);
            let mut saw_delta = false;
            let mut final_response_observed = false;
            let mut terminal_seen = false;
            loop {
                match process.next() {
                    ReaderEvent::Message(message) => match process.respond_server(&message) {
                        Ok(true) => continue,
                        Ok(false) => {
                            if !terminal_seen
                                && apply_notification(
                                    &handle,
                                    &session,
                                    &turn,
                                    &mut usage,
                                    &mut saw_delta,
                                    &mut final_response_observed,
                                    &message,
                                )
                            {
                                terminal_seen = true;
                                process.close_input();
                            }
                        }
                        Err(_) => {
                            mark_protocol_failure(&handle, false, clock.as_ref());
                            process.close_input();
                            handle.kill("KILL");
                            return;
                        }
                    },
                    ReaderEvent::InvalidJson => {
                        mark_protocol_failure(&handle, true, clock.as_ref());
                        process.close_input();
                        handle.kill("KILL");
                        return;
                    }
                    ReaderEvent::LineTooLong | ReaderEvent::IoFailure => {
                        mark_protocol_failure(&handle, false, clock.as_ref());
                        process.close_input();
                        handle.kill("KILL");
                        return;
                    }
                    ReaderEvent::Eof => {
                        if let Ok(mut state) = handle.state.lock() {
                            state.stream_drained = true;
                            state.maybe_finalize(clock.as_ref());
                        }
                        return;
                    }
                }
            }
        });
    }

    fn cancel_invocation(&mut self, invocation: &InvocationId) -> Result<(), BackendCancelRefused> {
        let handle = self.runs.get(invocation).cloned().ok_or_else(|| {
            BackendCancelRefused::new("this Codex backend does not know the invocation")
        })?;
        {
            let mut state = handle
                .state
                .lock()
                .map_err(|_| BackendCancelRefused::new("the invocation record is poisoned"))?;
            if state.receipt.is_some() {
                return Ok(());
            }
            if state.turn_termination.is_none() {
                state.cancel_requested = true;
            }
            // When a provider termination already won the race, keep that
            // outcome but still clean up a server that has not closed its
            // protocol pipes.
        }
        handle.close_input();
        handle.kill("TERM");
        if !handle.wait_for_exit(Instant::now() + self.cancel_grace) {
            handle.kill("KILL");
        }
        Ok(())
    }
}

fn mark_protocol_failure(handle: &RunHandle, malformed: bool, clock: &dyn Clock) {
    if let Ok(mut state) = handle.state.lock() {
        state.stats.malformed_lines += u64::from(malformed);
        state.fail(CLASS_PROTOCOL_FAILURE, None);
        state.stream_drained = true;
        state.maybe_finalize(clock);
    }
}

fn apply_notification(
    handle: &RunHandle,
    session: &str,
    turn: &str,
    usage: &mut CodexUsage,
    saw_delta: &mut bool,
    final_response_observed: &mut bool,
    message: &Value,
) -> bool {
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").unwrap_or(&Value::Null);
    if params
        .get("threadId")
        .and_then(Value::as_str)
        .is_some_and(|reported| reported != session)
    {
        return false;
    }
    if params
        .get("turnId")
        .and_then(Value::as_str)
        .is_some_and(|reported| reported != turn)
    {
        if method == "thread/tokenUsage/updated" {
            usage.restored(params.get("tokenUsage").unwrap_or(&Value::Null));
        }
        return false;
    }

    match method {
        "item/agentMessage/delta" => {
            if let Some(delta) = params.get("delta").and_then(Value::as_str) {
                *saw_delta = true;
                let chars = delta.chars().count() as u64;
                if let Ok(mut state) = handle.state.lock() {
                    state.output_chars_observed = state.output_chars_observed.saturating_add(chars);
                    state
                        .pending
                        .push_back(ExecutionObservation::OutputObserved { chars });
                }
            }
        }
        "item/completed" if !*saw_delta => {
            if params.pointer("/item/type").and_then(Value::as_str) == Some("agentMessage")
                && let Some(text) = params.pointer("/item/text").and_then(Value::as_str)
            {
                *final_response_observed = !text.is_empty();
                let chars = text.chars().count() as u64;
                if let Ok(mut state) = handle.state.lock() {
                    state.output_chars_observed = state.output_chars_observed.saturating_add(chars);
                    state
                        .pending
                        .push_back(ExecutionObservation::OutputObserved { chars });
                }
            }
        }
        "item/completed" => {
            if params.pointer("/item/type").and_then(Value::as_str) == Some("agentMessage")
                && let Some(text) = params.pointer("/item/text").and_then(Value::as_str)
            {
                *final_response_observed = !text.is_empty();
            }
        }
        "thread/tokenUsage/updated" => {
            let raw = params.get("tokenUsage").unwrap_or(&Value::Null);
            if let Some(observed) = usage.update(raw)
                && let Ok(mut state) = handle.state.lock()
            {
                state.token_totals = Some(CodexTokenUsage::from(codex_counts(
                    raw.get("total").unwrap_or(&Value::Null),
                )));
                state.usage = observed;
                state
                    .pending
                    .push_back(ExecutionObservation::UsageObserved { usage: observed });
            }
        }
        "error" => {
            let error = params.get("error").unwrap_or(&Value::Null);
            let code = allowlisted_error_code(error).map(str::to_owned);
            if let Ok(mut state) = handle.state.lock() {
                count_dropped_failure_detail(&mut state, error);
                if params.get("willRetry").and_then(Value::as_bool) == Some(true) {
                    state
                        .pending
                        .push_back(ExecutionObservation::RetryObserved {
                            session: session.to_owned(),
                            turn: turn.to_owned(),
                            error_code: code,
                        });
                    return false;
                }
                state.fail(CLASS_ERROR_EVENT, code);
            }
            return true;
        }
        "turn/completed" => {
            if params.pointer("/turn/id").and_then(Value::as_str) != Some(turn) {
                return false;
            }
            if let Some(final_usage) = usage.finish()
                && let Ok(mut state) = handle.state.lock()
            {
                state.usage = final_usage;
                state
                    .pending
                    .push_back(ExecutionObservation::UsageObserved { usage: final_usage });
            }
            if let Ok(mut state) = handle.state.lock() {
                if params.pointer("/turn/status").and_then(Value::as_str) == Some("completed")
                    && *final_response_observed
                {
                    state.turn_termination = Some(Termination::Completed);
                } else if params.pointer("/turn/status").and_then(Value::as_str)
                    == Some("completed")
                {
                    state.fail(CLASS_EMPTY_RESPONSE, None);
                } else {
                    let error = params.pointer("/turn/error").unwrap_or(&Value::Null);
                    count_dropped_failure_detail(&mut state, error);
                    state.fail(
                        CLASS_TURN_FAILED,
                        allowlisted_error_code(error).map(str::to_owned),
                    );
                }
            }
            return true;
        }
        "item/started" | "item/updated" => {}
        _ => {
            if let Ok(mut state) = handle.state.lock() {
                state.stats.unmapped_lines += 1;
            }
        }
    }
    false
}

fn count_dropped_failure_detail(state: &mut CodexRun, error: &Value) {
    if error.get("message").is_some() {
        state.stats.dropped_failure_details += 1;
    }
}

fn allowlisted_error_code(error: &Value) -> Option<&'static str> {
    let info = error.get("codexErrorInfo")?;
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

impl ExecutionBackend for CodexBackend {
    fn start(&mut self, invocation: &BackendInvocation) -> Result<(), BackendStartFailure> {
        if let Some(outcome) = self.start_outcomes.get(invocation.invocation()) {
            return outcome.clone();
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
        let handle = self.runs.get(invocation)?;
        let mut state = handle.state.lock().ok()?;
        let observation = state.pending.get(state.scanned).cloned()?;
        state.scanned += 1;
        Some(observation)
    }

    fn unread_last(&mut self, invocation: &InvocationId) {
        if let Some(handle) = self.runs.get(invocation)
            && let Ok(mut state) = handle.state.lock()
            && state.scanned > state.consumed
        {
            state.scanned -= 1;
        }
    }

    fn reset_scan(&mut self, invocation: &InvocationId) {
        if let Some(handle) = self.runs.get(invocation)
            && let Ok(mut state) = handle.state.lock()
        {
            state.scanned = state.consumed;
        }
    }

    fn commit_scan(&mut self, invocation: &InvocationId) {
        if let Some(handle) = self.runs.get(invocation)
            && let Ok(mut state) = handle.state.lock()
        {
            let consumed = state.scanned;
            state.pending.drain(..consumed);
            state.scanned = 0;
            state.consumed = 0;
        }
    }

    fn receipt(&mut self, invocation: &InvocationId) -> Option<Receipt> {
        self.runs
            .get(invocation)
            .and_then(|run| run.state.lock().ok())
            .and_then(|state| state.receipt.clone())
    }
}

impl Drop for CodexBackend {
    fn drop(&mut self) {
        for handle in self.runs.values() {
            handle.close_input();
            handle.kill("KILL");
        }
    }
}
