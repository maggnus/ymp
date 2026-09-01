#![forbid(unsafe_code)]

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::path::{Component, Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;
use ymp_application::Application;
use ymp_application::WorkspaceSubmission;
use ymp_domain::commitment::{InvocationClosure, InvocationState, RootTerminal, Verdict};
use ymp_domain::{Command, EventKind, MAX_IDENTIFIER_CHARS, RunStatus, digest_bytes};
use ymp_runtime_api::{
    AdmittedProgram, CancellationToken, DiagnosticSummary, InvocationRequest, LaunchDescriptor,
    McpBinding, ProbeReport, ProbeTransportIdentity, ProgramIdentity, ProgramRequirement,
    ProgramRole, Readiness, RuntimeDriver, RuntimeError, RuntimeEvent, RuntimeEventKind,
    RuntimeFailureKind, RuntimeKind, TOOL_HOST_PROBE_ENVIRONMENT,
    TOOL_HOST_PROBE_INTERNAL_ARGUMENTS, TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND,
    TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION, TOOL_HOST_PROBE_SCHEMA_VERSION,
    TOOL_HOST_PROBE_SERVER_VERSION, TOOL_HOST_PROBE_WORKSPACE_SERVER, ToolHostProbeCost,
    ToolHostProbeCostAvailability, ToolHostProbeEffect, ToolHostProbeError,
    ToolHostProbeInvocation, ToolHostProbeRequest, ToolHostProbeResourceVector,
    ToolHostProbeRuntimeIdentity, ToolHostProbeTerminal, ToolHostProbeTool,
    ToolHostProbeToolEventDigest, ToolHostProbeTrace, ToolHostProbeTrust, Usage,
    admit_lifecycle_programs, evidence_digest, probe_transport_digest,
    tool_host_probe_tool_schema_digest, unestablished_terminations,
};
use ymp_runtime_codex::{PINNED_CODEX_PROMPT_POLICY, codex_compatibility_contract_digest};

mod kernel;

pub use kernel::{ManagedKernel, ManagedTermination, Submission};

const CONTRACT_SCHEMA_VERSION: u32 = 1;
const MAX_CONTRACT_BYTES: usize = 1024 * 1024;
const MAX_PROMPT_BYTES: usize = 64 * 1024;
const MAX_RUNTIME_EVIDENCE_BYTES: u64 = 4 * 1024 * 1024;
const RUNTIME_EVIDENCE_SCHEMA_VERSION: u32 = 3;
const CODEX_TOOL_HOST_ROUTE: &str = "openai_responses_chatgpt";
const CODEX_TOOL_HOST_DRIVER: &str = "ymp-runtime-codex";
const CODEX_TOOL_HOST_DRIVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long a controller waits for its worker to end before it stops waiting.
///
/// The wait exists because supervision owns the runtime session and ends its process tree on the
/// way out, and that teardown takes as long as the operating system needs: a signalled process tree
/// is given roughly a fifth of a second to leave and about a second more after the second signal.
/// The limit is set well above that, so an orderly ending is never cut short, and it is a limit all
/// the same, because a runtime that stopped answering after a resume would otherwise hold the
/// controller for as long as it stayed silent — which is unbounded.
pub const CONTROLLER_SHUTDOWN_LIMIT: Duration = Duration::from_secs(5);

/// How often the closing controller reads whether its worker has finished.
const CONTROLLER_SHUTDOWN_POLL: Duration = Duration::from_millis(2);

/// How long a yielded slice waits for a wake before it reads the cancellation token again.
const WAKE_WAIT_POLL: Duration = Duration::from_millis(10);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractFile {
    schema_version: u32,
    contract_id: String,
    source: PathBuf,
    prompt: String,
    #[serde(default)]
    capture_exclusions: Vec<String>,
    verifier: Option<VerifierFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifierFile {
    program: PathBuf,
    #[serde(default)]
    arguments: Vec<String>,
    negative_control: PathBuf,
    oracle_digest: String,
    #[serde(default = "default_verifier_wall_time_ms")]
    wall_time_ms: u64,
    #[serde(default = "default_verifier_output_limit_bytes")]
    output_limit_bytes: usize,
}

const fn default_verifier_wall_time_ms() -> u64 {
    60_000
}

const fn default_verifier_output_limit_bytes() -> usize {
    1024 * 1024
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedVerifier {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub negative_control: PathBuf,
    pub oracle_digest: String,
    pub wall_time_ms: u64,
    pub output_limit_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedContract {
    pub contract_id: String,
    pub contract_digest: String,
    pub source: PathBuf,
    pub prompt: String,
    pub capture_exclusions: Vec<String>,
    pub verifier: Option<ManagedVerifier>,
}

impl ManagedContract {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let bytes =
            fs::read(path).with_context(|| format!("read managed contract {}", path.display()))?;
        if bytes.len() > MAX_CONTRACT_BYTES {
            bail!(
                "managed contract exceeds its {MAX_CONTRACT_BYTES}-byte limit: {}",
                path.display()
            );
        }
        let contract: ContractFile = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse managed contract {}", path.display()))?;
        if contract.schema_version != CONTRACT_SCHEMA_VERSION {
            bail!(
                "unsupported managed contract schema version {}",
                contract.schema_version
            );
        }
        if contract.contract_id.is_empty()
            || contract.contract_id.chars().count() > MAX_IDENTIFIER_CHARS
        {
            bail!("contract_id must contain between 1 and {MAX_IDENTIFIER_CHARS} characters");
        }
        if contract.prompt.is_empty() || contract.prompt.len() > MAX_PROMPT_BYTES {
            bail!("prompt must contain between 1 and {MAX_PROMPT_BYTES} bytes");
        }
        for exclusion in &contract.capture_exclusions {
            validate_relative_path(exclusion)?;
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let source = if contract.source.is_absolute() {
            contract.source
        } else {
            parent.join(contract.source)
        }
        .canonicalize()
        .with_context(|| format!("canonicalize managed source for {}", path.display()))?;
        if !source.is_dir() {
            bail!(
                "managed contract source is not a directory: {}",
                source.display()
            );
        }
        let verifier = contract
            .verifier
            .map(|verifier| load_verifier(verifier, parent))
            .transpose()?;
        Ok(Self {
            contract_id: contract.contract_id,
            contract_digest: digest_bytes(&bytes),
            source,
            prompt: contract.prompt,
            capture_exclusions: contract.capture_exclusions,
            verifier,
        })
    }
}

fn load_verifier(verifier: VerifierFile, parent: &Path) -> anyhow::Result<ManagedVerifier> {
    if verifier.arguments.len() > 32
        || verifier
            .arguments
            .iter()
            .any(|argument| argument.len() > 4096)
    {
        bail!("verifier arguments exceed the declared count or size limit");
    }
    if !is_canonical_digest(&verifier.oracle_digest) {
        bail!("verifier oracle_digest is not a canonical lowercase SHA-256 digest");
    }
    if verifier.wall_time_ms == 0 || verifier.wall_time_ms > 10 * 60 * 1000 {
        bail!("verifier wall_time_ms must be between 1 and 600000");
    }
    if verifier.output_limit_bytes == 0 || verifier.output_limit_bytes > 16 * 1024 * 1024 {
        bail!("verifier output_limit_bytes must be between 1 and 16777216");
    }
    let program = resolve_contract_path(parent, verifier.program, "verifier program")?;
    if !program.is_file() {
        bail!("verifier program is not a file: {}", program.display());
    }
    if !is_executable_file(&program)? {
        bail!("verifier program is not executable: {}", program.display());
    }
    let negative_control = resolve_contract_path(
        parent,
        verifier.negative_control,
        "verifier negative control",
    )?;
    if !negative_control.is_dir() {
        bail!(
            "verifier negative control is not a directory: {}",
            negative_control.display()
        );
    }
    Ok(ManagedVerifier {
        program,
        arguments: verifier.arguments,
        negative_control,
        oracle_digest: verifier.oracle_digest,
        wall_time_ms: verifier.wall_time_ms,
        output_limit_bytes: verifier.output_limit_bytes,
    })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> anyhow::Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    Ok(fs::metadata(path)?.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> anyhow::Result<bool> {
    Ok(path.is_file())
}

fn resolve_contract_path(parent: &Path, path: PathBuf, kind: &str) -> anyhow::Result<PathBuf> {
    let path = if path.is_absolute() {
        path
    } else {
        parent.join(path)
    };
    path.canonicalize()
        .with_context(|| format!("canonicalize {kind} {}", path.display()))
}

fn is_canonical_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_relative_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("capture exclusion is not a normalized relative path: {path}");
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ManagedCandidateRequest {
    pub contract: ManagedContract,
    pub bridge_executable: PathBuf,
}

#[derive(Clone, Debug)]
pub enum ManagedRunEvent {
    Runtime(RuntimeEvent),
    CandidateAvailable {
        candidate_digest: String,
        change_count: Option<usize>,
    },
    Finished,
    Failed {
        detail: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RuntimeEvidenceKind {
    Launch {
        descriptor: LaunchDescriptor,
        descriptor_digest: String,
    },
    Started {
        opaque_session_digest: String,
    },
    Output {
        text_digest: String,
        text_bytes: usize,
    },
    McpToolCall {
        server: String,
        tool: String,
        status: String,
        arguments_digest: String,
        result_digest: Option<String>,
        error_digest: Option<String>,
    },
    Yielded {
        cursor_digest: String,
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

#[derive(Serialize)]
struct RuntimeProfileEvidence<'a> {
    schema_version: u32,
    runtime_kind: RuntimeKind,
    invocation_id: &'a str,
    probe: &'a ProbeReport,
    runtime_executable_digest: Option<String>,
    launch_descriptor: Option<&'a LaunchDescriptor>,
    launch_descriptor_digest: Option<&'a str>,
    /// The program that builds the managed workspace baseline. It is not part of the launch
    /// descriptor because it runs before the runtime is prepared.
    workspace_program: &'a AdmittedProgram,
    /// The utilities the run executes to observe and terminate what it started. They are named here
    /// so that the record enumerates every program the run executes on its own behalf, not only the
    /// ones the launch enters.
    lifecycle_programs: &'a [AdmittedProgram],
    coordination: CoordinationEvidence,
    environment_policy: &'static str,
}

#[derive(Serialize)]
struct CoordinationEvidence {
    transport: &'static str,
    bridge_executable_digest: String,
    endpoint_path_digest: String,
    allowed_tools: [&'static str; 4],
    invocation_scoped: bool,
    credential_values_recorded: bool,
}

#[derive(Serialize)]
struct RuntimeEvidenceDigestInput<'a> {
    schema_version: u32,
    run_id: &'a str,
    attempt_id: &'a str,
    invocation_id: &'a str,
    runtime_kind: RuntimeKind,
    contract_id: &'a str,
    contract_digest: &'a str,
    profile_digest: &'a str,
    sequence: u64,
    event_id: &'a str,
    predecessor_digest: &'a Option<String>,
    event: &'a RuntimeEvidenceKind,
}

#[derive(Serialize)]
struct RuntimeEvidenceEnvelope<'a> {
    schema_version: u32,
    run_id: &'a str,
    attempt_id: &'a str,
    invocation_id: &'a str,
    runtime_kind: RuntimeKind,
    contract_id: &'a str,
    contract_digest: &'a str,
    profile_digest: &'a str,
    sequence: u64,
    event_id: &'a str,
    predecessor_digest: &'a Option<String>,
    event: &'a RuntimeEvidenceKind,
    digest: &'a str,
}

pub struct ManagedRunHandle {
    attempt_id: String,
    invocation_id: String,
    runtime_kind: RuntimeKind,
    cancellation: CancellationToken,
    application: Arc<Mutex<Application>>,
    /// Read under a lock, so that one handle may be held by more than one thread. The endings this
    /// handle offers are serialized against each other because two of them may be issued at once,
    /// and a handle no two threads could hold would put that serialization out of reach of anything
    /// but the worker.
    receiver: Mutex<Receiver<ManagedRunEvent>>,
    /// The sending end of the channel a yielded slice waits on. It is released before the worker is
    /// waited for, so it is held in an option rather than outright: a handle that is winding the run
    /// down is the only thing that could still send a wake, and holding the channel open while
    /// waiting would be waiting for a wake nobody can send.
    control_sender: Option<Sender<ManagedControl>>,
    /// The committed record of this run's process slice. Whether the slice may resume is decided
    /// here and nowhere else, so the live run and a modelled one answer that question the same way.
    kernel: Arc<ManagedKernel>,
    /// Held by everything that may end this run: the worker on its way out, an operator's
    /// cancellation, and the verdict of a protected query. The terminal of a run is decided once,
    /// and it is decided against the committed facts, so the reading of those facts and the
    /// transition taken from it cannot be split by another ending arriving in between.
    terminal: Arc<Mutex<()>>,
    finished: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

enum ManagedControl {
    Wake { input: String },
}

/// On whose behalf a cancellation is issued, which is what decides whether a run that has already
/// finished may still be stopped.
#[derive(Clone, Copy, Eq, PartialEq)]
enum CancelScope {
    /// An operator's command. It stops the run wherever the run stands, including a completed slice
    /// whose candidate nothing has judged: an operator who stops a run means the candidate is not
    /// to be accepted afterwards.
    Commanded,
    /// A controller closing. It stops a run that is still working and leaves a run that has already
    /// recorded the terminal of its slice with the terminal it recorded.
    WhileTheSliceIsOpen,
}

/// Take the guard that serializes every ending of one managed run.
///
/// A poisoned guard is taken all the same. What it holds is an order of operations and not a datum
/// whose invariant a panic could leave half-written — the committed facts are the state of the run,
/// and each of them is committed whole — so refusing here would leave a run nobody could stop and
/// nothing safer than what stopping it does.
fn take_terminal(terminal: &Mutex<()>) -> std::sync::MutexGuard<'_, ()> {
    terminal
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ManagedRunHandle {
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    pub fn invocation_id(&self) -> &str {
        &self.invocation_id
    }

    pub const fn runtime_kind(&self) -> RuntimeKind {
        self.runtime_kind
    }

    pub fn try_next(&self) -> Option<ManagedRunEvent> {
        let receiver = self
            .receiver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        receiver.try_recv().ok()
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }

    /// Stop this run, in both records that state how it ended.
    ///
    /// A cancellation used to move the journal and set the token the runtime watches, and that was
    /// the whole of it. Where the runtime had already finished, nothing was watching the token any
    /// more: the journal called the run cancelled while the kernel still held it open on an
    /// unjudged candidate, and the verdict of a query issued afterwards drove that same run to
    /// acceptance. The kernel transition is therefore taken here whenever the slice is already
    /// closed, because a slice that closed has had its terminal recorded and will record no second
    /// one. While the slice is still open the worker records it, and the token this sets is what
    /// the worker reads to record a cancellation rather than whatever the runtime did last.
    ///
    /// A run the kernel has already ended is not cancelled at all. Renaming a committed terminal is
    /// what this path exists to prevent, so the cancellation is refused and neither record moves.
    ///
    /// Nothing about the state of the journal may leave this command unable to stop the run. The
    /// journal lock is taken poisoned, for the reason `take_terminal` takes the terminal guard
    /// poisoned, and a journal that refuses the command is reported only after the token has been
    /// set and the kernel transition taken. Refusing at the lock left the worst state this command
    /// has: the token the runtime watches was never set, so the run went on working, both records
    /// went on calling it running, and the controller that then waited for the worker waited for a
    /// run nothing had asked to stop.
    pub fn cancel(&self, reason: impl Into<String>) -> anyhow::Result<()> {
        self.cancel_run(reason.into(), CancelScope::Commanded)
    }

    fn cancel_run(&self, reason: String, scope: CancelScope) -> anyhow::Result<()> {
        let _terminal = take_terminal(&self.terminal);
        // A controller closing is not an operator stopping the work. The worker records the
        // terminal of its slice, submits its candidate and only then winds the runtime down, and a
        // close landing in that window used to stop a run that had already finished: both records
        // were moved to cancelled over a completed slice, and the committed candidate lost the
        // verdict it was waiting for. Whether the run has already answered is read out of the
        // committed state of its slice — under this guard, so the worker either has not recorded
        // its terminal yet or has recorded it together with the submission — and never out of the
        // flag the worker sets on its way out, which comes after both.
        if scope == CancelScope::WhileTheSliceIsOpen
            && matches!(self.kernel.invocation_state(), Ok(InvocationState::Closed))
        {
            return Ok(());
        }
        let ended = self.kernel.root_terminal()?;
        let refused = {
            let mut application = self
                .application
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if application.state().status == RunStatus::Running {
                if let Some(terminal) =
                    ended.filter(|terminal| *terminal != RootTerminal::Cancelled)
                {
                    bail!("the kernel has already ended this run as {terminal:?}");
                }
                application
                    .execute(
                        format!("{}.cancel", self.attempt_id),
                        Command::Cancel { reason },
                    )
                    .err()
            } else {
                None
            }
        };
        self.cancellation.cancel();
        // Which of the two records the kernel transition is decided by the committed state of the
        // slice, not by whether the worker thread has been observed to finish: the worker records
        // its terminal well before it reports itself finished, and a cancellation arriving in
        // between would otherwise reach a kernel nobody was going to close.
        if ended.is_none() && self.kernel.invocation_state()? == InvocationState::Closed {
            self.kernel.terminated(ManagedTermination::Cancelled)?;
        }
        // The run is stopped either way; what the journal would not take is reported rather than
        // passed over, because the record an operator reads then still calls the run running.
        if let Some(error) = refused {
            return Err(
                anyhow::Error::new(error).context("the journal did not record the cancellation")
            );
        }
        Ok(())
    }

    /// The committed record of this run's process slice.
    pub fn kernel(&self) -> &ManagedKernel {
        &self.kernel
    }

    /// The same record, as a handle that outlives this one.
    ///
    /// A controller that stopped waiting for a worker it could not end records the terminal of the
    /// run itself and is then closed, and closing it consumes this handle. Whoever closed it is
    /// exactly who needs to read that terminal afterwards, so the record is obtainable on its own
    /// rather than only through a handle that is about to be given up.
    pub fn kernel_record(&self) -> Arc<ManagedKernel> {
        Arc::clone(&self.kernel)
    }

    /// Ask the kernel to admit one resumption of the yielded slice, and resume the runtime with
    /// this instruction only if it does.
    ///
    /// Nothing about the decision is taken here. The kernel is asked whether this slice is at the
    /// head of the queue it must be admitted in, and then asked to commit the resumption; a slice
    /// that is running, closed, out of wakes, past its wake deadline or holding no fact worth
    /// resuming for is refused, and the instruction never reaches the runtime. A repeated delivery
    /// of the same wake returns the recorded result and resumes nothing a second time.
    pub fn wake(
        &self,
        command_id: impl Into<String>,
        input: impl Into<String>,
    ) -> anyhow::Result<()> {
        let command_id = command_id.into();
        let input = input.into();
        let command_chars = command_id.chars().count();
        if !(1..=MAX_IDENTIFIER_CHARS).contains(&command_chars) {
            bail!(
                "wake command identifier must contain between 1 and {MAX_IDENTIFIER_CHARS} characters"
            );
        }
        if input.is_empty() || input.len() > MAX_PROMPT_BYTES {
            bail!("wake input must contain between 1 and {MAX_PROMPT_BYTES} bytes");
        }
        if !self.kernel.admit_wake(&command_id, &input)? {
            return Ok(());
        }
        let Some(control) = self.control_sender.as_ref() else {
            bail!("managed runtime control channel is closed");
        };
        if control.send(ManagedControl::Wake { input }).is_err() {
            bail!("managed runtime control channel is closed");
        }
        Ok(())
    }

    /// Record in the kernel what a protected query decided about the candidate this run committed.
    ///
    /// The controller does not perform the query and takes no part in the decision: what reaches
    /// the kernel here is a verdict somebody else produced against an exact bundle. Recording it is
    /// what closes the work obligation and lets the run reach a terminal state, so a candidate
    /// nobody verified leaves the run open rather than passing for accepted.
    ///
    /// It is one of the endings this run may reach, so it is taken under the same guard as a
    /// cancellation: a verdict recorded first leaves the cancellation nothing to rename, and a
    /// cancellation recorded first stops the run, whereupon the kernel refuses the verdict.
    pub fn verified(&self, candidate_digest: &str, verdict: Verdict) -> anyhow::Result<()> {
        let _terminal = take_terminal(&self.terminal);
        self.kernel.verified(candidate_digest, verdict)
    }

    /// Release the control channel and wait for the worker, in that order, for as long as
    /// [`CONTROLLER_SHUTDOWN_LIMIT`] allows.
    ///
    /// A yielded slice waits for one of two things: the cancellation token, or a wake arriving over
    /// the control channel. Waiting for the worker while the sending end of that channel is still
    /// held waits for a wake that can no longer be sent — this handle is the only sender, and it is
    /// on its way out — so a cancellation that did not reach the token left the wait unbounded.
    /// Released first, the closed channel is what the worker reads as the end of its wakes: it
    /// interrupts the runtime, records the terminal of its slice and leaves, whatever became of the
    /// cancellation.
    ///
    /// Releasing the channel bounds the wait only where the worker is watching it. A runtime that
    /// stopped answering holds supervision inside the call that reads its next event, where neither
    /// the closed channel nor the cancellation token is read, and a controller that waited there
    /// waited for as long as that runtime stayed silent. The wait is therefore given a limit: the
    /// worker that has not finished by then is left running, and a run that still owes a terminal
    /// is stopped in both records here instead, so a controller that stopped waiting does not leave
    /// a run whose records still call it running. What the run owes is read out of the committed
    /// state of its slice rather than out of the flag this wait watches — the flag is set after the
    /// terminal is recorded and after the runtime has been wound down, so a worker held in that
    /// window has finished its accounting and is owed nothing.
    fn release_control_and_join(&mut self) -> WorkerShutdown {
        self.control_sender = None;
        let Some(worker) = self.worker.take() else {
            return WorkerShutdown::AlreadyReleased;
        };
        let deadline = Instant::now() + CONTROLLER_SHUTDOWN_LIMIT;
        while !self.is_finished() {
            if Instant::now() >= deadline {
                // The worker is not waited for, because what it is waiting for may have no bound. It
                // keeps the runtime session it owns and ends that session's process tree if it ever
                // returns; what this controller can still do is read what the run owes and say what
                // it found.
                drop(worker);
                return WorkerShutdown::Unbounded(self.record_unbounded_shutdown());
            }
            thread::sleep(CONTROLLER_SHUTDOWN_POLL);
        }
        WorkerShutdown::Ended(worker.join())
    }

    /// Stop a run whose supervision did not end within the limit, where a terminal is still owed.
    ///
    /// Whether one is owed is decided by the committed state of the slice, exactly as a
    /// cancellation decides it, and never by the flag the worker sets on its way out. The worker
    /// records the terminal of its slice, submits its candidate and only then winds the runtime
    /// down, and ending a process tree takes as long as it takes: a controller that read the flag
    /// would find a run whose terminal is committed and whose candidate is submitted, and would
    /// record an infrastructure fault over both — naming a fault where supervision had in fact
    /// finished its accounting, and taking from the candidate the verdict it was waiting for. A
    /// slice the record already calls closed is therefore left alone, and what the controller has
    /// to say about the worker it stopped waiting for is said in the report and nowhere else.
    ///
    /// Which terminal is honest for a slice that is still open is decided by what actually stopped
    /// the run. An operator's cancellation is what a token already set says happened, and the
    /// journal holds that terminal from the moment the cancellation was issued, so the kernel is
    /// given the same one; naming an infrastructure fault there would leave the two records
    /// disagreeing about one run. Where nothing cancelled the run, supervision that could not be
    /// ended is itself the fault, and both records are given it, together with the detail that the
    /// runtime's processes were never established to have ended.
    ///
    /// A record that would not take what it was given is answered with, because this controller is
    /// the last place that can say so: the channel a run reports its failures over belongs to the
    /// worker that is being left behind, and nothing else is going to reach the operator.
    fn record_unbounded_shutdown(&self) -> ShutdownRecord {
        let cancelled = self.cancellation.is_cancelled();
        let _terminal = take_terminal(&self.terminal);
        match self.kernel.invocation_state() {
            Ok(InvocationState::Closed) => return ShutdownRecord::AlreadyCommitted,
            Ok(_) => {}
            // A state that cannot be read is not the state "still open". Recording a terminal on it
            // is what could rename one the worker has already committed, so the kernel is left
            // alone and the reading that failed is carried out instead.
            //
            // The journal is moved all the same. The committed facts of a run are read through one
            // lock, and a lock a panic poisoned stays poisoned: a run whose state cannot be read
            // now will not be judged, closed or advanced later either. Leaving the journal on
            // running would then show an operator a run in progress that nothing can move, so the
            // record is failed here and names the reading that failed as the reason.
            Err(error) => {
                let error = error.to_string();
                let detail = format!(
                    "managed_runtime_slice_state_unread: supervision was still running {} ms after \
                     the controller released its control channel, and the committed state of its \
                     slice could not be read, so no terminal was recorded there: {error}",
                    CONTROLLER_SHUTDOWN_LIMIT.as_millis()
                );
                let unrecorded =
                    record_infrastructure_failure(&self.application, &self.attempt_id, &detail);
                return ShutdownRecord::Unread { error, unrecorded };
            }
        }
        let mut unrecorded = Vec::new();
        if !cancelled {
            let detail = format!(
                "managed_runtime_supervision_did_not_end: supervision was still running {} ms \
                 after the controller released its control channel, so nothing was established \
                 about the processes of this run",
                CONTROLLER_SHUTDOWN_LIMIT.as_millis()
            );
            if let Some(refused) =
                record_infrastructure_failure(&self.application, &self.attempt_id, &detail)
            {
                unrecorded.push(refused);
            }
        }
        if let Err(error) = self.kernel.terminated(if cancelled {
            ManagedTermination::Cancelled
        } else {
            ManagedTermination::Failed(InvocationClosure::InfrastructureError)
        }) {
            unrecorded.push(format!("managed_runtime_terminal_unrecorded: {error}"));
        }
        ShutdownRecord::Recorded((!unrecorded.is_empty()).then(|| unrecorded.join("; ")))
    }

    /// Close the controller and state what became of the runtime it supervised.
    ///
    /// The wait is bounded, so a close can end while the worker still holds the runtime session and
    /// the process tree of that session. A caller that reports the close to an operator has to be
    /// able to tell those two endings apart, which a failure alone does not say: it is read here,
    /// and [`Self::join`] is the same close for callers that only need whether it succeeded.
    pub fn close(mut self) -> ManagedShutdown {
        let limit = CONTROLLER_SHUTDOWN_LIMIT.as_millis();
        match self.release_control_and_join() {
            WorkerShutdown::Ended(Err(_)) => ManagedShutdown::WorkerPanicked,
            WorkerShutdown::Unbounded(ShutdownRecord::Recorded(unrecorded)) => {
                let report = format!(
                    "managed runtime supervision did not end within {limit} ms; the run was \
                     stopped in both records and its worker was left running"
                );
                ManagedShutdown::WorkerLeftRunning(match unrecorded {
                    Some(unrecorded) => format!("{report}; {unrecorded}"),
                    None => report,
                })
            }
            WorkerShutdown::Unbounded(ShutdownRecord::AlreadyCommitted) => {
                ManagedShutdown::WorkerLeftRunning(format!(
                    "managed runtime supervision did not report itself finished within {limit} \
                     ms; it had already recorded the terminal of its slice, so nothing was \
                     recorded here and the worker was left winding the runtime down"
                ))
            }
            WorkerShutdown::Unbounded(ShutdownRecord::Unread { error, unrecorded }) => {
                let report = format!(
                    "managed runtime supervision did not end within {limit} ms; the committed \
                     state of its slice could not be read, so the run was failed in the journal \
                     alone: {error}"
                );
                ManagedShutdown::WorkerLeftRunning(match unrecorded {
                    Some(unrecorded) => format!("{report}; {unrecorded}"),
                    None => report,
                })
            }
            WorkerShutdown::Ended(Ok(())) | WorkerShutdown::AlreadyReleased => {
                ManagedShutdown::Ended
            }
        }
    }

    pub fn join(self) -> anyhow::Result<()> {
        match self.close() {
            ManagedShutdown::Ended => Ok(()),
            ManagedShutdown::WorkerPanicked => bail!("managed runtime worker panicked"),
            ManagedShutdown::WorkerLeftRunning(report) => bail!("{report}"),
        }
    }
}

/// What a closed controller left behind, in the terms its caller can report.
#[derive(Debug)]
pub enum ManagedShutdown {
    /// The worker ended, so the runtime session it owned was dropped and the process tree of that
    /// session with it.
    Ended,
    /// The worker died of a panic. Its session is dropped as the stack unwinds out of the closure
    /// that owns it, so the process tree is ended there as well.
    WorkerPanicked,
    /// The wait ran out. The worker was left holding its runtime session and the process tree of
    /// that session, and what the controller found the run owed is stated here.
    WorkerLeftRunning(String),
}

/// What became of supervision when the controller stopped waiting for it.
enum WorkerShutdown {
    /// An earlier close of this handle had already released the worker.
    AlreadyReleased,
    /// The worker ended, either normally or by dying of a panic.
    Ended(thread::Result<()>),
    /// The worker had not ended when [`CONTROLLER_SHUTDOWN_LIMIT`] ran out. It was left running,
    /// and what the controller found the run owed is stated here.
    Unbounded(ShutdownRecord),
}

/// What the controller found when its wait ran out, and what it therefore wrote.
enum ShutdownRecord {
    /// The slice was still open, so the terminal the worker owed was recorded here, carrying
    /// whatever the two records declined to take.
    Recorded(Option<String>),
    /// The worker had already committed the terminal of its slice — it was winding the runtime down
    /// rather than working — so neither record was written to.
    AlreadyCommitted,
    /// The committed state of the slice could not be read, so no terminal was recorded in the
    /// kernel. The journal was failed on the reading that failed, carrying whatever it declined to
    /// take.
    Unread {
        error: String,
        unrecorded: Option<String>,
    },
}

impl Drop for ManagedRunHandle {
    fn drop(&mut self) {
        // Whether this run is still working is read out of the committed state of its slice, the
        // way the wait that ran out reads it. A close that decided by the finished flag stopped a
        // run held between the terminal it had recorded and the flag it had not yet set — the
        // window the runtime is wound down in — and renamed a completed slice cancelled.
        let _ = self.cancel_run(
            "managed runtime controller closed".to_owned(),
            CancelScope::WhileTheSliceIsOpen,
        );
        // A wait that ran out records the terminal of the run itself, so a dropped controller
        // leaves no run whose records still call it running, and it leaves in bounded time.
        let _ = self.release_control_and_join();
    }
}

/// Whether the launch of the runtime being started has to be attested.
#[derive(Clone, Copy, Eq, PartialEq)]
enum LaunchAttestation {
    Required,
    Waived,
}

/// The one gate every start of a runtime passes, wherever that start is written.
///
/// Two things are established here and nowhere else. The runtime must attest the programs its
/// launch enters, because attestation is what binds the bytes that execute to the bytes that were
/// admitted; and the utilities the run observes and ends its own processes with must be admitted,
/// because a run that cannot see what it starts cannot say what it left running. The admitted
/// utilities are returned, so the caller records what it will execute on its own behalf.
///
/// A start written outside this crate reaches the same gate by calling this function. That every
/// such start does is not left to reading: `ymp-cli/tests/unattested_runtime_is_unreachable.rs`
/// rejects a shipped module that builds a runtime driver and starts it without this call, and
/// drives the built product to establish that no command offers a runtime this gate would refuse.
pub fn admit_runtime_start(kind: RuntimeKind) -> anyhow::Result<Vec<AdmittedProgram>> {
    admit_start(kind, LaunchAttestation::Required)
}

fn admit_start(
    kind: RuntimeKind,
    attestation: LaunchAttestation,
) -> anyhow::Result<Vec<AdmittedProgram>> {
    if attestation == LaunchAttestation::Required && !requires_launch_attestation(kind) {
        bail!(
            "{kind:?} does not attest the programs its launch enters, so it cannot start a managed \
             run"
        );
    }
    // The run observes and ends its own processes with these utilities. One that cannot be admitted
    // stops the run here, naming the program and the reason, rather than leaving a run that reports
    // a clean termination it was never able to establish.
    admit_lifecycle_programs()
        .context("the utilities this run observes and ends its own processes with")
}

/// Starts a managed run that produces a candidate. The runtime passes [`admit_runtime_start`]
/// before anything of it is executed.
pub fn start_managed_candidate(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    request: ManagedCandidateRequest,
) -> anyhow::Result<ManagedRunHandle> {
    start_candidate(application, driver, request, LaunchAttestation::Required)
}

/// Starts a managed run with a runtime whose launch is not attested. This exists for the checks
/// that must drive the controller without a runtime executable; nothing the product ships names it.
#[doc(hidden)]
pub fn start_unattested_managed_candidate(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    request: ManagedCandidateRequest,
) -> anyhow::Result<ManagedRunHandle> {
    start_candidate(application, driver, request, LaunchAttestation::Waived)
}

/// Executes one runtime-reported workspace nonce write/read probe without entering the
/// Application, kernel, board, task, recruitment, candidate, persistence or attestation paths.
///
/// Fake fixtures and the one behaviorally compatible Codex profile are admitted. The returned
/// trace remains untrusted; only the Application controller can perform read-back and attest it.
pub fn execute_tool_host_probe(
    driver: &dyn RuntimeDriver,
    workspace: &Path,
    request: ToolHostProbeRequest,
) -> Result<ToolHostProbeTrace, ToolHostProbeError> {
    validate_tool_host_probe_request(&request)?;
    let launch_attestation = match (driver.kind(), request.expected_runtime.runtime_kind) {
        (RuntimeKind::Fake, RuntimeKind::Fake) => LaunchAttestation::Waived,
        (RuntimeKind::Codex, RuntimeKind::Codex) => LaunchAttestation::Required,
        _ => {
            return Err(ToolHostProbeError::RuntimeIdentityMismatch {
                field: "runtime_kind",
            });
        }
    };
    if request.cancellation.is_cancelled() {
        return Err(ToolHostProbeError::Cancelled);
    }
    let workspace = validate_tool_host_probe_workspace(workspace, &request.workspace_path)?;
    admit_start(driver.kind(), launch_attestation).map_err(|error| {
        ToolHostProbeError::StartFailed {
            detail: error.to_string(),
        }
    })?;
    let probe = driver
        .probe()
        .map_err(|error| ToolHostProbeError::RuntimeProbeFailed {
            detail: error.to_string(),
        })?;
    if probe.readiness != Readiness::Ready {
        return Err(ToolHostProbeError::RuntimeNotReady {
            detail: probe.detail,
        });
    }
    validate_tool_host_probe_projection(driver, &probe, &request.expected_runtime)?;
    let runtime = match driver.kind() {
        RuntimeKind::Fake => driver.tool_host_probe_identity().map_err(|error| {
            ToolHostProbeError::RuntimeProbeFailed {
                detail: error.to_string(),
            }
        })?,
        RuntimeKind::Codex => {
            reconstruct_codex_tool_host_probe_identity(driver, &probe, &workspace)?
        }
        _ => {
            return Err(ToolHostProbeError::LiveRuntimeForbidden);
        }
    };
    validate_tool_host_probe_identity(&runtime)?;
    compare_probe_transport_identity(
        &request.expected_runtime.probe_transport,
        &runtime.probe_transport,
    )?;
    compare_tool_host_probe_identity(&request.expected_runtime, &runtime)?;

    if driver.kind() == RuntimeKind::Codex {
        let immediate_transport = measure_current_tool_host_probe_transport(&workspace)?;
        compare_probe_transport_identity(&runtime.probe_transport, &immediate_transport)?;
    }

    let started_at = Instant::now();
    let mut session = driver
        .start_tool_host_probe(ToolHostProbeInvocation {
            request: request.clone(),
            workspace,
            allowed_tools: [
                ToolHostProbeTool::WorkspaceWrite,
                ToolHostProbeTool::WorkspaceRead,
            ],
        })
        .map_err(|error| ToolHostProbeError::StartFailed {
            detail: error.to_string(),
        })?;
    let mut progress = RuntimeProgress::new(&request.invocation_id);
    let mut events = Vec::new();
    let mut saw_started = false;
    let mut terminal_usage = None;
    let mut tool_digests = Vec::new();
    let mut runtime_reported_readback = None;

    loop {
        if request.cancellation.is_cancelled() {
            let _ = session.interrupt();
            return Err(ToolHostProbeError::Cancelled);
        }
        if elapsed_millis(started_at) > request.deadline_ms {
            request.cancellation.cancel();
            let _ = session.interrupt();
            return Err(ToolHostProbeError::TimedOut {
                limit_ms: request.deadline_ms,
            });
        }
        let event = match session.next_event() {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(RuntimeError::TimedOut { limit_ms }) => {
                if limit_ms != request.deadline_ms {
                    return Err(ToolHostProbeError::InvalidEventStream {
                        detail: "runtime timeout does not match the requested deadline".to_owned(),
                    });
                }
                return Err(ToolHostProbeError::TimedOut { limit_ms });
            }
            Err(error) => {
                return Err(ToolHostProbeError::RuntimeFailed {
                    detail: error.to_string(),
                });
            }
        };
        progress
            .validate(&event)
            .map_err(|error| ToolHostProbeError::InvalidEventStream {
                detail: error.to_string(),
            })?;
        if terminal_usage.is_some() {
            return Err(classify_extra_probe_event(&event));
        }
        match &event.event {
            RuntimeEventKind::Started { opaque_session_id }
                if !saw_started && opaque_session_id.is_empty() =>
            {
                return Err(ToolHostProbeError::InvalidEventStream {
                    detail: "runtime started with an empty session identifier".to_owned(),
                });
            }
            RuntimeEventKind::Started { .. } if !saw_started && tool_digests.is_empty() => {
                saw_started = true;
            }
            RuntimeEventKind::Started { .. } => {
                return Err(ToolHostProbeError::InvalidEventStream {
                    detail: "runtime repeated or reordered its started event".to_owned(),
                });
            }
            RuntimeEventKind::McpToolCall {
                server,
                tool,
                status,
                arguments,
                result,
                error,
            } => {
                if !saw_started {
                    return Err(ToolHostProbeError::InvalidEventStream {
                        detail: "runtime used a tool before its started event".to_owned(),
                    });
                }
                let expected = match tool_digests.len() {
                    0 => ToolHostProbeTool::WorkspaceWrite,
                    1 => ToolHostProbeTool::WorkspaceRead,
                    _ => {
                        return Err(unexpected_probe_tool(server, tool));
                    }
                };
                let readback = validate_tool_host_probe_event(
                    expected,
                    server,
                    tool,
                    status,
                    arguments,
                    result.as_ref(),
                    error.as_ref(),
                    &request,
                )?;
                let result = result
                    .as_ref()
                    .expect("validated tool-host probe result is present");
                tool_digests.push(ToolHostProbeToolEventDigest {
                    sequence: event.sequence,
                    tool: expected,
                    arguments_digest: digest_json(arguments).map_err(|error| {
                        ToolHostProbeError::InvalidEventStream {
                            detail: error.to_string(),
                        }
                    })?,
                    result_digest: digest_json(result).map_err(|error| {
                        ToolHostProbeError::InvalidEventStream {
                            detail: error.to_string(),
                        }
                    })?,
                });
                if let Some(readback) = readback {
                    runtime_reported_readback = Some(readback);
                }
            }
            RuntimeEventKind::Output { .. } => {
                return Err(ToolHostProbeError::UnexpectedOutput);
            }
            RuntimeEventKind::Completed { usage } => {
                terminal_usage = Some(usage.clone());
            }
            RuntimeEventKind::TimedOut { limit_ms, .. } => {
                if *limit_ms != request.deadline_ms {
                    return Err(ToolHostProbeError::InvalidEventStream {
                        detail: "runtime timeout does not match the requested deadline".to_owned(),
                    });
                }
                return Err(ToolHostProbeError::TimedOut {
                    limit_ms: *limit_ms,
                });
            }
            RuntimeEventKind::Cancelled { .. } => {
                return Err(ToolHostProbeError::Cancelled);
            }
            RuntimeEventKind::Failed { kind, .. } => {
                return Err(ToolHostProbeError::RuntimeFailed {
                    detail: format!("{kind:?}"),
                });
            }
            RuntimeEventKind::Launch { .. }
            | RuntimeEventKind::Yielded { .. }
            | RuntimeEventKind::Interrupted => {
                return Err(ToolHostProbeError::AmbiguousTerminal);
            }
        }
        events.push(event);
    }

    if request.cancellation.is_cancelled() {
        return Err(ToolHostProbeError::Cancelled);
    }
    if elapsed_millis(started_at) > request.deadline_ms {
        return Err(ToolHostProbeError::TimedOut {
            limit_ms: request.deadline_ms,
        });
    }
    if !saw_started {
        return Err(ToolHostProbeError::InvalidEventStream {
            detail: "runtime emitted no started event".to_owned(),
        });
    }
    if tool_digests.is_empty() {
        return Err(ToolHostProbeError::MissingToolEvent {
            tool: ToolHostProbeTool::WorkspaceWrite,
        });
    }
    if tool_digests.len() == 1 {
        return Err(ToolHostProbeError::MissingToolEvent {
            tool: ToolHostProbeTool::WorkspaceRead,
        });
    }
    let usage = terminal_usage.ok_or(ToolHostProbeError::AmbiguousTerminal)?;
    if session.usage() != usage {
        return Err(ToolHostProbeError::IncompleteUsage {
            field: "terminal_usage",
        });
    }
    validate_tool_host_probe_usage(&usage, &request.resource_reservation)?;
    let cost = match usage.cost_microusd {
        Some(cost) => ToolHostProbeCost {
            availability: ToolHostProbeCostAvailability::Reported,
            currency: Some("USD".to_owned()),
            amount_microusd: Some(cost),
        },
        None => ToolHostProbeCost {
            availability: ToolHostProbeCostAvailability::Unavailable,
            currency: None,
            amount_microusd: None,
        },
    };
    let runtime_reported_readback =
        runtime_reported_readback.ok_or(ToolHostProbeError::MissingToolEvent {
            tool: ToolHostProbeTool::WorkspaceRead,
        })?;
    let event_digest = evidence_digest(&serde_json::to_vec(&events).map_err(|error| {
        ToolHostProbeError::InvalidEventStream {
            detail: error.to_string(),
        }
    })?);
    let tool_event_digests: [ToolHostProbeToolEventDigest; 2] = tool_digests
        .try_into()
        .map_err(|_| ToolHostProbeError::AmbiguousTerminal)?;

    Ok(ToolHostProbeTrace {
        schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
        probe_id: request.probe_id,
        invocation_id: request.invocation_id,
        nonce_input: request.nonce.clone(),
        runtime_reported_readback: runtime_reported_readback.clone(),
        workspace_path: request.workspace_path,
        deadline_ms: request.deadline_ms,
        resource_reservation: request.resource_reservation.clone(),
        runtime,
        model_calls: request.resource_reservation.model_calls,
        usage,
        cost,
        input_digest: evidence_digest(request.nonce.as_bytes()),
        output_digest: evidence_digest(runtime_reported_readback.as_bytes()),
        tool_event_digests,
        event_digest,
        terminal: ToolHostProbeTerminal::Completed,
        trust: ToolHostProbeTrust::UntrustedRuntimeTrace,
    })
}

fn validate_tool_host_probe_request(
    request: &ToolHostProbeRequest,
) -> Result<(), ToolHostProbeError> {
    if request.schema_version != TOOL_HOST_PROBE_SCHEMA_VERSION {
        return Err(ToolHostProbeError::UnsupportedSchema {
            found: request.schema_version,
        });
    }
    for (field, value) in [
        ("probe_id", request.probe_id.as_str()),
        ("invocation_id", request.invocation_id.as_str()),
    ] {
        if value.is_empty() || value.chars().count() > MAX_IDENTIFIER_CHARS {
            return Err(ToolHostProbeError::InvalidIdentity { field });
        }
    }
    if request.nonce.is_empty() || request.nonce.len() > 4096 {
        return Err(ToolHostProbeError::InvalidNonce { max_bytes: 4096 });
    }
    if !is_normalized_relative_probe_path(&request.workspace_path) {
        return Err(ToolHostProbeError::InvalidWorkspacePath);
    }
    if request.deadline_ms == 0 || request.deadline_ms > 600_000 {
        return Err(ToolHostProbeError::InvalidDeadline);
    }
    validate_tool_host_probe_reservation(&request.resource_reservation, request.deadline_ms)?;
    validate_tool_host_probe_identity(&request.expected_runtime)
}

fn validate_tool_host_probe_identity(
    identity: &ToolHostProbeRuntimeIdentity,
) -> Result<(), ToolHostProbeError> {
    for (field, value) in [
        ("route", identity.route.as_str()),
        ("profile", identity.profile.as_str()),
        ("cli", identity.cli.as_str()),
        ("cli_version", identity.cli_version.as_str()),
        ("driver", identity.driver.as_str()),
        ("driver_version", identity.driver_version.as_str()),
    ] {
        if value.is_empty() || value.len() > 4096 {
            return Err(ToolHostProbeError::InvalidIdentity { field });
        }
    }
    for (field, digest) in [
        (
            "compatibility_contract_digest",
            identity.compatibility_contract_digest.as_str(),
        ),
        ("executable_digest", identity.executable_digest.as_str()),
    ] {
        if !is_sha256_digest(digest) {
            return Err(ToolHostProbeError::InvalidIdentity { field });
        }
    }
    if identity.runtime_kind == RuntimeKind::Codex
        && identity.compatibility_contract_digest != codex_compatibility_contract_digest()
    {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "compatibility_contract_digest",
        });
    }
    if identity.tool_schema_digest != tool_host_probe_tool_schema_digest() {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "tool_schema_digest",
        });
    }
    validate_probe_transport_shape(&identity.probe_transport)?;
    if identity.probe_transport_digest != probe_transport_digest(&identity.probe_transport) {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "probe_transport_digest",
        });
    }
    Ok(())
}

fn validate_tool_host_probe_reservation(
    vector: &ToolHostProbeResourceVector,
    deadline_ms: u64,
) -> Result<(), ToolHostProbeError> {
    for (field, exact, found) in [
        ("model_calls", 1, vector.model_calls),
        ("workspace_reads", 1, vector.workspace_reads),
        ("workspace_writes", 1, vector.workspace_writes),
        ("invocation_starts", 1, vector.invocation_starts),
    ] {
        if found != exact {
            return Err(ToolHostProbeError::InvalidReservation { field });
        }
    }
    if vector.max_input_tokens == 0 || vector.max_output_tokens == 0 {
        return Err(ToolHostProbeError::InvalidReservation {
            field: "token_limits",
        });
    }
    if vector.max_wall_time_ms != deadline_ms {
        return Err(ToolHostProbeError::InvalidReservation {
            field: "max_wall_time_ms",
        });
    }
    for (field, found) in [
        ("protected_queries", vector.protected_queries),
        ("external_actions", vector.external_actions),
        ("participant_starts", vector.participant_starts),
        ("attempt_starts", vector.attempt_starts),
        ("offer_creations", vector.offer_creations),
        ("obligation_creations", vector.obligation_creations),
        ("board_actions", vector.board_actions),
        ("task_actions", vector.task_actions),
        ("recruitment_actions", vector.recruitment_actions),
        ("candidate_actions", vector.candidate_actions),
        ("communication_actions", vector.communication_actions),
    ] {
        if found != 0 {
            return Err(ToolHostProbeError::InvalidReservation { field });
        }
    }
    Ok(())
}

fn is_normalized_relative_probe_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn validate_tool_host_probe_workspace(
    workspace: &Path,
    relative: &Path,
) -> Result<PathBuf, ToolHostProbeError> {
    if !workspace.is_absolute() || !workspace.is_dir() {
        return Err(ToolHostProbeError::WorkspaceUnavailable {
            detail: "workspace root must be an existing absolute directory".to_owned(),
        });
    }
    let canonical =
        workspace
            .canonicalize()
            .map_err(|error| ToolHostProbeError::WorkspaceUnavailable {
                detail: error.to_string(),
            })?;
    let mut candidate = canonical.clone();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(ToolHostProbeError::InvalidWorkspacePath);
        };
        candidate.push(component);
        if candidate.exists() {
            let resolved = candidate.canonicalize().map_err(|error| {
                ToolHostProbeError::WorkspaceUnavailable {
                    detail: error.to_string(),
                }
            })?;
            if !resolved.starts_with(&canonical) {
                return Err(ToolHostProbeError::InvalidWorkspacePath);
            }
        }
    }
    Ok(canonical)
}

fn reconstruct_codex_tool_host_probe_identity(
    driver: &dyn RuntimeDriver,
    probe: &ProbeReport,
    workspace: &Path,
) -> Result<ToolHostProbeRuntimeIdentity, ToolHostProbeError> {
    let cli_version = probe
        .version
        .clone()
        .ok_or(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "cli_version",
        })?;
    let executable_digest = regular_non_symlink_digest(driver.executable(), "executable_digest")?;
    let probe_transport = measure_current_tool_host_probe_transport(workspace)?;
    let probe_transport_digest = probe_transport_digest(&probe_transport);
    Ok(ToolHostProbeRuntimeIdentity {
        runtime_kind: RuntimeKind::Codex,
        route: CODEX_TOOL_HOST_ROUTE.to_owned(),
        profile: PINNED_CODEX_PROMPT_POLICY.to_owned(),
        cli: driver.executable().display().to_string(),
        cli_version,
        compatibility_contract_digest: codex_compatibility_contract_digest(),
        executable_digest,
        driver: CODEX_TOOL_HOST_DRIVER.to_owned(),
        driver_version: CODEX_TOOL_HOST_DRIVER_VERSION.to_owned(),
        tool_schema_digest: tool_host_probe_tool_schema_digest(),
        probe_transport,
        probe_transport_digest,
    })
}

fn measure_current_tool_host_probe_transport(
    workspace: &Path,
) -> Result<ProbeTransportIdentity, ToolHostProbeError> {
    let executable = std::env::current_exe()
        .map_err(|error| ToolHostProbeError::RuntimeProbeFailed {
            detail: format!("resolve current ymp executable: {error}"),
        })?
        .canonicalize()
        .map_err(|error| ToolHostProbeError::RuntimeProbeFailed {
            detail: format!("canonicalize current ymp executable: {error}"),
        })?;
    let executable_digest = regular_non_symlink_digest(&executable, "probe_executable_digest")?;
    let metadata = fs::symlink_metadata(workspace).map_err(|error| {
        ToolHostProbeError::WorkspaceUnavailable {
            detail: error.to_string(),
        }
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ToolHostProbeError::WorkspaceUnavailable {
            detail: "workspace root must be a regular non-symlink directory".to_owned(),
        });
    }
    let canonical =
        workspace
            .canonicalize()
            .map_err(|error| ToolHostProbeError::WorkspaceUnavailable {
                detail: error.to_string(),
            })?;
    if canonical != workspace {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "canonical_workspace_root_digest",
        });
    }
    Ok(ProbeTransportIdentity {
        mcp_protocol_version: TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION.to_owned(),
        server_name: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
        server_version: TOOL_HOST_PROBE_SERVER_VERSION.to_owned(),
        tool_schema_digest: tool_host_probe_tool_schema_digest(),
        ordered_tools: [
            ToolHostProbeTool::WorkspaceWrite,
            ToolHostProbeTool::WorkspaceRead,
        ],
        server_executable_digest: executable_digest.clone(),
        launcher_executable_digest: executable_digest,
        internal_subcommand: TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND.to_owned(),
        arguments: TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        inherited_environment: TOOL_HOST_PROBE_ENVIRONMENT
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        canonical_workspace_root_digest: evidence_digest(canonical.as_os_str().as_encoded_bytes()),
    })
}

fn regular_non_symlink_digest(
    path: &Path,
    field: &'static str,
) -> Result<String, ToolHostProbeError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| ToolHostProbeError::RuntimeProbeFailed {
            detail: error.to_string(),
        })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch { field });
    }
    fs::read(path)
        .map(|bytes| evidence_digest(&bytes))
        .map_err(|error| ToolHostProbeError::RuntimeProbeFailed {
            detail: error.to_string(),
        })
}

fn validate_probe_transport_shape(
    transport: &ProbeTransportIdentity,
) -> Result<(), ToolHostProbeError> {
    for (field, matches) in [
        (
            "mcp_protocol_version",
            transport.mcp_protocol_version == TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION,
        ),
        (
            "server_name",
            transport.server_name == TOOL_HOST_PROBE_WORKSPACE_SERVER,
        ),
        (
            "server_version",
            transport.server_version == TOOL_HOST_PROBE_SERVER_VERSION,
        ),
        (
            "transport_tool_schema_digest",
            transport.tool_schema_digest == tool_host_probe_tool_schema_digest(),
        ),
        (
            "ordered_tools",
            transport.ordered_tools
                == [
                    ToolHostProbeTool::WorkspaceWrite,
                    ToolHostProbeTool::WorkspaceRead,
                ],
        ),
        (
            "internal_subcommand",
            transport.internal_subcommand == TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND,
        ),
        (
            "arguments",
            transport.arguments
                == TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect::<Vec<_>>(),
        ),
        (
            "inherited_environment",
            transport.inherited_environment
                == TOOL_HOST_PROBE_ENVIRONMENT
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<_>>(),
        ),
    ] {
        if !matches {
            return Err(ToolHostProbeError::RuntimeIdentityMismatch { field });
        }
    }
    for (field, digest) in [
        (
            "server_executable_digest",
            transport.server_executable_digest.as_str(),
        ),
        (
            "launcher_executable_digest",
            transport.launcher_executable_digest.as_str(),
        ),
        (
            "canonical_workspace_root_digest",
            transport.canonical_workspace_root_digest.as_str(),
        ),
    ] {
        if !is_sha256_digest(digest) {
            return Err(ToolHostProbeError::InvalidIdentity { field });
        }
    }
    Ok(())
}

fn compare_probe_transport_identity(
    expected: &ProbeTransportIdentity,
    observed: &ProbeTransportIdentity,
) -> Result<(), ToolHostProbeError> {
    validate_probe_transport_shape(expected)?;
    validate_probe_transport_shape(observed)?;
    for (field, matches) in [
        (
            "mcp_protocol_version",
            expected.mcp_protocol_version == observed.mcp_protocol_version,
        ),
        ("server_name", expected.server_name == observed.server_name),
        (
            "server_version",
            expected.server_version == observed.server_version,
        ),
        (
            "transport_tool_schema_digest",
            expected.tool_schema_digest == observed.tool_schema_digest,
        ),
        (
            "ordered_tools",
            expected.ordered_tools == observed.ordered_tools,
        ),
        (
            "server_executable_digest",
            expected.server_executable_digest == observed.server_executable_digest,
        ),
        (
            "launcher_executable_digest",
            expected.launcher_executable_digest == observed.launcher_executable_digest,
        ),
        (
            "internal_subcommand",
            expected.internal_subcommand == observed.internal_subcommand,
        ),
        ("arguments", expected.arguments == observed.arguments),
        (
            "inherited_environment",
            expected.inherited_environment == observed.inherited_environment,
        ),
        (
            "canonical_workspace_root_digest",
            expected.canonical_workspace_root_digest == observed.canonical_workspace_root_digest,
        ),
    ] {
        if !matches {
            return Err(ToolHostProbeError::RuntimeIdentityMismatch { field });
        }
    }
    Ok(())
}

fn validate_tool_host_probe_projection(
    driver: &dyn RuntimeDriver,
    probe: &ProbeReport,
    expected: &ToolHostProbeRuntimeIdentity,
) -> Result<(), ToolHostProbeError> {
    if probe.kind != driver.kind() || expected.runtime_kind != driver.kind() {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "runtime_kind",
        });
    }
    if probe.executable != expected.cli || driver.executable().display().to_string() != expected.cli
    {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch { field: "cli" });
    }
    if !probe
        .version
        .as_deref()
        .is_some_and(|version| !version.is_empty() && version.len() <= 4096)
    {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "cli_version",
        });
    }
    let actual_executable_digest =
        regular_non_symlink_digest(driver.executable(), "executable_digest")?;
    if actual_executable_digest != expected.executable_digest {
        return Err(ToolHostProbeError::RuntimeIdentityMismatch {
            field: "executable_digest",
        });
    }
    Ok(())
}

fn compare_tool_host_probe_identity(
    expected: &ToolHostProbeRuntimeIdentity,
    observed: &ToolHostProbeRuntimeIdentity,
) -> Result<(), ToolHostProbeError> {
    for (field, matches) in [
        (
            "runtime_kind",
            expected.runtime_kind == observed.runtime_kind,
        ),
        ("route", expected.route == observed.route),
        ("profile", expected.profile == observed.profile),
        ("cli", expected.cli == observed.cli),
        (
            "compatibility_contract_digest",
            expected.compatibility_contract_digest == observed.compatibility_contract_digest,
        ),
        (
            "executable_digest",
            expected.executable_digest == observed.executable_digest,
        ),
        ("driver", expected.driver == observed.driver),
        (
            "driver_version",
            expected.driver_version == observed.driver_version,
        ),
        (
            "tool_schema_digest",
            expected.tool_schema_digest == observed.tool_schema_digest,
        ),
        (
            "probe_transport",
            expected.probe_transport == observed.probe_transport,
        ),
        (
            "probe_transport_digest",
            expected.probe_transport_digest == observed.probe_transport_digest,
        ),
    ] {
        if !matches {
            return Err(ToolHostProbeError::RuntimeIdentityMismatch { field });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_tool_host_probe_event(
    expected: ToolHostProbeTool,
    server: &str,
    tool: &str,
    status: &str,
    arguments: &Value,
    result: Option<&Value>,
    error: Option<&Value>,
    request: &ToolHostProbeRequest,
) -> Result<Option<String>, ToolHostProbeError> {
    if server != TOOL_HOST_PROBE_WORKSPACE_SERVER || tool != expected.to_string() {
        return Err(unexpected_probe_tool(server, tool));
    }
    if status != "completed" {
        return Err(ToolHostProbeError::InvalidToolEvent {
            tool: expected,
            field: "status",
        });
    }
    if error.is_some() {
        return Err(ToolHostProbeError::InvalidToolEvent {
            tool: expected,
            field: "error",
        });
    }
    let result = result.ok_or(ToolHostProbeError::InvalidToolEvent {
        tool: expected,
        field: "result",
    })?;
    match expected {
        ToolHostProbeTool::WorkspaceWrite => {
            if arguments
                != &serde_json::json!({
                    "path": request.workspace_path,
                    "content": request.nonce,
                })
            {
                return Err(ToolHostProbeError::InvalidToolEvent {
                    tool: expected,
                    field: "arguments",
                });
            }
            if result != &serde_json::json!({"bytes_written": request.nonce.len()}) {
                return Err(ToolHostProbeError::InvalidToolEvent {
                    tool: expected,
                    field: "result",
                });
            }
            Ok(None)
        }
        ToolHostProbeTool::WorkspaceRead => {
            if arguments != &serde_json::json!({"path": request.workspace_path}) {
                return Err(ToolHostProbeError::InvalidToolEvent {
                    tool: expected,
                    field: "arguments",
                });
            }
            let readback = result
                .as_object()
                .filter(|object| object.len() == 1)
                .and_then(|object| object.get("content"))
                .and_then(Value::as_str)
                .ok_or(ToolHostProbeError::InvalidToolEvent {
                    tool: expected,
                    field: "result",
                })?;
            if readback != request.nonce {
                return Err(ToolHostProbeError::InvalidToolEvent {
                    tool: expected,
                    field: "result",
                });
            }
            Ok(Some(readback.to_owned()))
        }
    }
}

fn forbidden_probe_effect(server: &str, tool: &str) -> Option<ToolHostProbeEffect> {
    let boundary = format!("{server}/{tool}").to_ascii_lowercase();
    if boundary.contains("board") {
        Some(ToolHostProbeEffect::Board)
    } else if boundary.contains("recruit") || boundary.contains("participant") {
        Some(ToolHostProbeEffect::Recruitment)
    } else if boundary.contains("candidate") || boundary.contains("submit") {
        Some(ToolHostProbeEffect::Candidate)
    } else if boundary.contains("task") || boundary.contains("offer") || boundary.contains("bid") {
        Some(ToolHostProbeEffect::Task)
    } else {
        None
    }
}

fn unexpected_probe_tool(server: &str, tool: &str) -> ToolHostProbeError {
    match forbidden_probe_effect(server, tool) {
        Some(effect) => ToolHostProbeError::ForbiddenEffect {
            effect,
            server: server.to_owned(),
            tool: tool.to_owned(),
        },
        None => ToolHostProbeError::UnexpectedToolUse {
            server: server.to_owned(),
            tool: tool.to_owned(),
        },
    }
}

fn classify_extra_probe_event(event: &RuntimeEvent) -> ToolHostProbeError {
    match &event.event {
        RuntimeEventKind::Output { .. } => ToolHostProbeError::UnexpectedOutput,
        RuntimeEventKind::McpToolCall { server, tool, .. } => unexpected_probe_tool(server, tool),
        _ => ToolHostProbeError::AmbiguousTerminal,
    }
}

fn validate_tool_host_probe_usage(
    usage: &Usage,
    reservation: &ToolHostProbeResourceVector,
) -> Result<(), ToolHostProbeError> {
    let total_tokens = usage
        .input_tokens
        .saturating_add(usage.cached_input_tokens)
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.reasoning_output_tokens);
    if total_tokens == 0 {
        return Err(ToolHostProbeError::IncompleteUsage { field: "tokens" });
    }
    if usage.wall_time_ms == 0 {
        return Err(ToolHostProbeError::IncompleteUsage {
            field: "wall_time_ms",
        });
    }
    if usage.cost_microusd.is_none() && !usage.cost_by_model.is_empty() {
        return Err(ToolHostProbeError::IncompleteUsage { field: "cost" });
    }
    if !usage.cost_is_attributed() {
        return Err(ToolHostProbeError::IncompleteUsage {
            field: "cost_by_model",
        });
    }
    if usage.protected_queries != 0 || usage.in_flight_excess != Default::default() {
        return Err(ToolHostProbeError::ReservationExceeded {
            field: "forbidden_usage",
        });
    }
    for (field, found, limit) in [
        (
            "input_tokens",
            usage.input_tokens,
            reservation.max_input_tokens,
        ),
        (
            "cached_input_tokens",
            usage.cached_input_tokens,
            reservation.max_cached_input_tokens,
        ),
        (
            "output_tokens",
            usage.output_tokens,
            reservation.max_output_tokens,
        ),
        (
            "reasoning_output_tokens",
            usage.reasoning_output_tokens,
            reservation.max_reasoning_output_tokens,
        ),
        (
            "wall_time_ms",
            usage.wall_time_ms,
            reservation.max_wall_time_ms,
        ),
    ] {
        if found > limit {
            return Err(ToolHostProbeError::ReservationExceeded { field });
        }
    }
    if let Some(found) = usage.cost_microusd {
        match reservation.max_cost_microusd {
            Some(limit) if found <= limit => {}
            None if found == 0 => {}
            _ => {
                return Err(ToolHostProbeError::ReservationExceeded {
                    field: "cost_microusd",
                });
            }
        }
    }
    Ok(())
}

fn elapsed_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_runtime_probe(kind: RuntimeKind, probe: &ProbeReport) -> anyhow::Result<()> {
    if probe.kind != kind {
        bail!("runtime_probe_kind_mismatch");
    }
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    if kind == RuntimeKind::Codex {
        let version = probe
            .version
            .as_deref()
            .context("codex_runtime_projection_missing")?;
        if version.is_empty() {
            bail!("codex_runtime_projection_missing");
        }
    }
    Ok(())
}

fn start_candidate(
    application: Arc<Mutex<Application>>,
    driver: Box<dyn RuntimeDriver>,
    request: ManagedCandidateRequest,
    attestation: LaunchAttestation,
) -> anyhow::Result<ManagedRunHandle> {
    let lifecycle_programs = admit_start(driver.kind(), attestation)?;
    let probe = driver.probe()?;
    validate_runtime_probe(driver.kind(), &probe)?;
    let bridge_executable = request.bridge_executable.canonicalize().with_context(|| {
        format!(
            "canonicalize current ymp executable {}",
            request.bridge_executable.display()
        )
    })?;
    let runtime_kind = driver.kind();
    let probed_runtime_executable_digest = digest_regular_file(driver.executable())?;
    let bridge_executable_digest = digest_regular_file(&bridge_executable)?.ok_or_else(|| {
        anyhow::anyhow!(
            "current ymp executable is not a regular file: {}",
            bridge_executable.display()
        )
    })?;
    let workspace_program = admit_workspace_program()?;
    let contract_id = request.contract.contract_id.clone();
    let contract_digest = request.contract.contract_digest.clone();
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let invocation_id = format!("invocation-{}", Uuid::new_v4());
    let evidence_invocation_id = invocation_id.clone();
    let (run_id, base_digest, workspace, evidence_directory, controller_cursor) = {
        let mut application = application
            .lock()
            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
        if application.state().status != RunStatus::Running {
            bail!("run is already terminal");
        }
        if !application.state().active_attempts.is_empty() {
            bail!("the POC profile admits only one active attempt");
        }
        let base = application
            .artifact_store()
            .capture_source(&request.contract.source)?;
        let workspace = application.private_workspace_path(&attempt_id)?;
        let evidence_directory = application.runtime_evidence_path(&attempt_id)?;
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)?;
        initialize_private_git(&workspace, &workspace_program)?;
        application.execute(
            format!("{attempt_id}.start"),
            Command::StartAttempt {
                attempt_id: attempt_id.clone(),
            },
        )?;
        (
            application.state().run_id.clone(),
            base.manifest_digest,
            workspace,
            evidence_directory,
            application.state().last_sequence,
        )
    };

    // The committed record this run's process slice lives in. It is prepared before anything is
    // executed, because the authority to run a slice is spent out of the work's own account and a
    // slice the kernel would not fund is one this controller must not start. The record it is
    // prepared in is the run's own journal, so what the kernel decides is readable after this
    // process is gone.
    let kernel = match ManagedKernel::prepare(
        Arc::clone(&application),
        &attempt_id,
        &invocation_id,
        &contract_id,
        &base_digest,
        &digest_bytes(request.contract.prompt.as_bytes()),
    ) {
        Ok(kernel) => Arc::new(kernel),
        Err(error) => {
            let detail = error.to_string();
            return Err(failed_start(&application, &attempt_id, &detail, error));
        }
    };

    let token = Uuid::new_v4().simple().to_string();
    let socket_path = std::env::temp_dir()
        .join("ymp-runtime")
        .join(format!("{}.sock", Uuid::new_v4().simple()));
    let endpoint_path_digest = digest_bytes(socket_path.to_string_lossy().as_bytes());
    let exclusions = request.contract.capture_exclusions.clone();
    let rpc_server = match ymp_agent_rpc::AgentRpcServer::start_with_submission_for_invocation(
        &socket_path,
        &token,
        &attempt_id,
        &invocation_id,
        Arc::clone(&application),
        WorkspaceSubmission::new(&base_digest, &workspace, exclusions.clone()),
    ) {
        Ok(server) => server,
        Err(error) => {
            let detail = error.to_string();
            return Err(failed_start(
                &application,
                &attempt_id,
                &detail,
                error.into(),
            ));
        }
    };
    let invocation_control = rpc_server
        .invocation_control()
        .context("managed invocation control was not created")?;
    let cancellation = CancellationToken::default();
    let invocation_request = InvocationRequest {
        invocation_id: invocation_id.clone(),
        attempt_id: attempt_id.clone(),
        workspace: workspace.clone(),
        mcp: Some(McpBinding {
            executable: bridge_executable,
            socket_path,
            token,
        }),
        prompt: request.contract.prompt,
        cancellation: cancellation.clone(),
    };
    let launch_descriptor = match driver.prepare_launch(&invocation_request) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            let detail = runtime_error_code(&error).to_owned();
            return Err(failed_start(
                &application,
                &attempt_id,
                &detail,
                error.into(),
            ));
        }
    };
    if requires_launch_attestation(runtime_kind) && launch_descriptor.is_none() {
        return Err(failed_start(
            &application,
            &attempt_id,
            "runtime_launch_descriptor_missing",
            anyhow::anyhow!("{runtime_kind:?} did not provide a launch descriptor"),
        ));
    }
    if let Some(descriptor) = &launch_descriptor {
        validate_launch_descriptor(descriptor, &attempt_id, &invocation_id, &workspace)?;
        // A driver that omitted the chain, or that bound one of its programs to a digest it chose
        // itself rather than to a location this account cannot write, would reach the same launch
        // through a program the account could have planted. Both are refused here, where the
        // runtime kind says the launch is one that must be attested.
        if cfg!(unix) && requires_launch_attestation(runtime_kind) {
            let admits_shell = descriptor
                .launch_chain
                .iter()
                .any(|program| program.role == ProgramRole::LaunchShell);
            let bound_by_location = descriptor
                .launch_chain
                .iter()
                .all(|program| program.identity == ProgramIdentity::SystemPath);
            if !admits_shell || !bound_by_location {
                return Err(failed_start(
                    &application,
                    &attempt_id,
                    "runtime_launch_chain_missing",
                    anyhow::anyhow!(
                        "{runtime_kind:?} did not admit the programs its launch chain enters at a \
                         location this account cannot write"
                    ),
                ));
            }
        }
    }
    let launch_descriptor_digest = launch_descriptor
        .as_ref()
        .map(digest_serialized)
        .transpose()?;
    let runtime_executable_digest = launch_descriptor
        .as_ref()
        .map(|descriptor| descriptor.executable_digest.clone())
        .map(Some)
        .unwrap_or(probed_runtime_executable_digest);
    let bridge_executable_digest = launch_descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.coordination_executable_digest.clone())
        .unwrap_or(bridge_executable_digest);
    if let Err(error) = fs::create_dir_all(&evidence_directory) {
        let detail = error.to_string();
        return Err(failed_start(
            &application,
            &attempt_id,
            &detail,
            error.into(),
        ));
    }
    let mut evidence_file = match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(evidence_directory.join("events.jsonl"))
    {
        Ok(file) => file,
        Err(error) => {
            let detail = error.to_string();
            return Err(failed_start(
                &application,
                &attempt_id,
                &detail,
                error.into(),
            ));
        }
    };
    let profile_digest = match write_runtime_profile_evidence(
        &evidence_directory,
        RuntimeProfileEvidence {
            schema_version: RUNTIME_EVIDENCE_SCHEMA_VERSION,
            runtime_kind,
            invocation_id: &evidence_invocation_id,
            probe: &probe,
            runtime_executable_digest,
            launch_descriptor: launch_descriptor.as_ref(),
            launch_descriptor_digest: launch_descriptor_digest.as_deref(),
            workspace_program: &workspace_program,
            lifecycle_programs: &lifecycle_programs,
            coordination: CoordinationEvidence {
                transport: "stdio_mcp_via_private_rpc",
                bridge_executable_digest,
                endpoint_path_digest,
                allowed_tools: ["read_control", "read_events", "yield", "submit"],
                invocation_scoped: true,
                credential_values_recorded: false,
            },
            environment_policy: match runtime_kind {
                RuntimeKind::Codex => "synthetic_allowlist_v1",
                // Claude Code resolves its subscription credential from the home directory, so a
                // generated home is authenticated only after the operator's credential is
                // delegated into it. `SECURITY.md` requires that weaker profile to be labelled
                // rather than presented as strict containment.
                RuntimeKind::ClaudeCode => "synthetic_allowlist_with_delegated_credential_v1",
                RuntimeKind::Fake => "deterministic_fixture",
            },
        },
    ) {
        Ok(digest) => digest,
        Err(error) => {
            let detail = error.to_string();
            return Err(failed_start(&application, &attempt_id, &detail, error));
        }
    };
    // The slice is admitted before the process exists, so a runtime is never running under a slice
    // the kernel never funded.
    if let Err(error) = kernel.start_invocation() {
        let detail = error.to_string();
        return Err(failed_start(&application, &attempt_id, &detail, error));
    }
    let mut session = match driver.start_prepared(invocation_request, launch_descriptor.as_ref()) {
        Ok(session) => session,
        Err(error) => {
            // A launch that failed may also have failed to end what it had already started. That is
            // kept where it was found and read here, so the run records it beside the failure.
            let mut detail = runtime_error_code(&error).to_owned();
            let unestablished = unestablished_terminations();
            if !unestablished.is_empty() {
                detail.push_str(&format!(
                    "; managed_runtime_termination_unestablished: {}",
                    unestablished.join("; ")
                ));
            }
            let failure = failed_start(&application, &attempt_id, &detail, error.into());
            let _ = kernel.terminated(ManagedTermination::Failed(
                InvocationClosure::InfrastructureError,
            ));
            return Err(failure);
        }
    };
    let (sender, receiver) = channel();
    let (control_sender, control_receiver) = channel();
    let worker_kernel = Arc::clone(&kernel);
    let terminal = Arc::new(Mutex::new(()));
    let worker_terminal = Arc::clone(&terminal);
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = Arc::clone(&finished);
    let worker_application = Arc::clone(&application);
    let worker_attempt = attempt_id.clone();
    let worker_invocation = invocation_id.clone();
    let worker_cancellation = cancellation.clone();
    let initial_launch_descriptor = launch_descriptor.clone();
    let worker = thread::Builder::new()
        .name(format!("ymp-runtime-{worker_attempt}"))
        .spawn(move || {
            let _rpc_server = rpc_server;
            // A worker that dies of a panic owes the kernel the terminal a worker that returns an
            // error owes it. Left to unwind, the thread takes the ending of the run with it: the
            // slice stays open, the run reaches no terminal at all, and a cancellation issued
            // afterwards moves the journal alone and reports success for stopping a run the kernel
            // still holds running. Supervision that died is an infrastructure failure, so it is
            // caught here and recorded as one on the way out.
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let result = (|| -> anyhow::Result<()> {
                    let mut completed = false;
                    let mut predecessor_digest = None;
                    let mut runtime_progress = RuntimeProgress::new(&worker_invocation);
                    let mut expected_session = None;
                    let mut saw_launch = !requires_launch_attestation(runtime_kind);
                    let mut pending_error = None;
                    let mut terminal_failure = None;
                    let mut application_cursor = controller_cursor;
                    let mut yield_cursor = 0;
                    // Whether the runtime has already been told that the run is over. The
                    // interruption is delivered once; every runtime treats a second one as the
                    // terminal it already reached, and delivering it again would be supervision
                    // acting on a decision it has already carried out.
                    let mut interrupt_delivered = false;
                    loop {
                        // A cancellation reaches the runtime here, at the first moment supervision
                        // holds the session again, and not only where a yielded slice waits for a
                        // wake. A runtime that is working is between two events rather than waiting
                        // for one, and the interruption used to be delivered only in that wait: a
                        // runtime that watched the token stopped itself, and one that did not went
                        // on working until it finished of its own accord, whatever an operator had
                        // asked for. Delivery no longer depends on the runtime reading the token —
                        // supervision reads it and interrupts the session, wherever the run stands.
                        //
                        // What this cannot reach is a runtime that has stopped answering inside the
                        // call that reads its next event: supervision is held in that call and
                        // holds the session while it is there. That wait is bounded by the
                        // controller instead, at `CONTROLLER_SHUTDOWN_LIMIT`.
                        if !interrupt_delivered && worker_cancellation.is_cancelled() {
                            interrupt_delivered = true;
                            if let Err(error) = session.interrupt() {
                                pending_error = Some(error);
                            }
                        }
                        let mut event = match pending_error.take().map_or_else(
                            || session.next_event(),
                            Err::<Option<RuntimeEvent>, RuntimeError>,
                        ) {
                            Ok(Some(event)) => event,
                            Ok(None) => {
                                if worker_cancellation.is_cancelled() {
                                    break;
                                }
                                bail!("runtime ended without a terminal event");
                            }
                            Err(error) => RuntimeEvent {
                                sequence: runtime_progress.next_sequence()?,
                                event_id: format!(
                                    "{}.event-{}",
                                    worker_invocation,
                                    runtime_progress.next_sequence()?
                                ),
                                invocation_id: worker_invocation.clone(),
                                event: RuntimeEventKind::Failed {
                                    kind: runtime_error_kind(&error),
                                    usage: session.usage(),
                                    diagnostic: runtime_error_diagnostic(&error),
                                },
                            },
                        };
                        if matches!(
                            &event.event,
                            RuntimeEventKind::Completed { .. } | RuntimeEventKind::Yielded { .. }
                        ) {
                            event.event = authoritative_lifecycle_event(
                                LifecycleAuthority {
                                    application: &worker_application,
                                    attempt_id: &worker_attempt,
                                    invocation_id: &worker_invocation,
                                    invocation_control: &invocation_control,
                                    application_cursor: &mut application_cursor,
                                    yield_cursor: &mut yield_cursor,
                                },
                                &event.event,
                                session.usage(),
                            )?;
                        }
                        if !saw_launch && !matches!(&event.event, RuntimeEventKind::Launch { .. }) {
                            bail!("{runtime_kind:?} emitted an event before launch attestation");
                        }
                        runtime_progress.validate(&event)?;
                        match &event.event {
                            RuntimeEventKind::Launch { descriptor } => {
                                validate_runtime_launch(
                                    runtime_kind,
                                    initial_launch_descriptor.as_ref(),
                                    descriptor,
                                    saw_launch,
                                    expected_session.as_deref(),
                                )?;
                                saw_launch = true;
                            }
                            RuntimeEventKind::Started { opaque_session_id } => {
                                if let Some(expected) = &expected_session {
                                    if expected != opaque_session_id {
                                        bail!("runtime session identifier changed across resume");
                                    }
                                } else {
                                    expected_session = Some(opaque_session_id.clone());
                                }
                            }
                            _ => {}
                        }
                        predecessor_digest = Some(append_runtime_evidence(
                            &mut evidence_file,
                            &run_id,
                            &worker_attempt,
                            runtime_kind,
                            &contract_id,
                            &contract_digest,
                            &profile_digest,
                            &predecessor_digest,
                            &event,
                        )?);
                        let yielded = matches!(&event.event, RuntimeEventKind::Yielded { .. });
                        let terminal = matches!(
                            &event.event,
                            RuntimeEventKind::Completed { .. }
                                | RuntimeEventKind::Failed { .. }
                                | RuntimeEventKind::TimedOut { .. }
                                | RuntimeEventKind::Cancelled { .. }
                                | RuntimeEventKind::Interrupted
                        );
                        completed = matches!(&event.event, RuntimeEventKind::Completed { .. });
                        terminal_failure = match &event.event {
                            RuntimeEventKind::Failed { .. } => Some("managed_runtime_failed"),
                            RuntimeEventKind::TimedOut { .. } => Some("managed_runtime_timed_out"),
                            _ => None,
                        };
                        // The yield is committed before it is announced, so a controller acting
                        // on the announcement cannot reach the kernel before the record it
                        // decides against.
                        if let RuntimeEventKind::Yielded { cursor } = &event.event {
                            worker_kernel.yielded(cursor)?;
                        }
                        let _ = sender.send(ManagedRunEvent::Runtime(event));
                        if terminal {
                            break;
                        }
                        if yielded {
                            loop {
                                if worker_cancellation.is_cancelled() {
                                    interrupt_delivered = true;
                                    if let Err(error) = session.interrupt() {
                                        pending_error = Some(error);
                                    }
                                    break;
                                }
                                match control_receiver.recv_timeout(WAKE_WAIT_POLL) {
                                    Ok(ManagedControl::Wake { input }) => {
                                        if let Err(error) = session.resume(input) {
                                            pending_error = Some(error);
                                        }
                                        break;
                                    }
                                    Err(RecvTimeoutError::Timeout) => {}
                                    Err(RecvTimeoutError::Disconnected) => {
                                        interrupt_delivered = true;
                                        if let Err(error) = session.interrupt() {
                                            pending_error = Some(error);
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // How this slice ended is recorded in the kernel before anything else is
                    // reported, in the terminal vocabulary the run is accountable in. A slice that
                    // completed leaves its work obligation open, because whether its candidate is
                    // accepted is decided by a protected query this controller does not perform.
                    //
                    // The guard covers the reading of the cancellation and the candidate that
                    // follows it, so an operator's cancellation either is already visible here and
                    // is the terminal recorded, or finds the slice closed and takes the kernel
                    // transition itself. It cannot land between the two and stop the run under the
                    // submission.
                    //
                    // A cancellation the guard makes visible here is what this slice ended of,
                    // whatever the runtime reported on its way out. Interrupting a runtime is how
                    // a cancellation reaches one that is working, and the tool call the
                    // interruption lands in the middle of fails: classified by that failure the
                    // slice would close as a runtime failure, which the kernel accounts as an
                    // infrastructure error, while the journal the same cancellation moved reports
                    // the run as cancelled. The two records would then name different terminals
                    // for one run, and the kernel's would name a fault where an operator had
                    // merely stopped the work.
                    //
                    // A limit the slice ran out of is the one thing a cancellation does not absorb.
                    // It is not a failure the interruption produced but a measure of what the run
                    // consumed, and a cancellation arriving afterwards neither caused it nor gives
                    // the run the budget back. The run is still stopped as cancelled, in both
                    // records, and the slice closes on the limit so that the overrun is accounted
                    // for rather than lost to whichever ending arrived last.
                    let terminal = take_terminal(&worker_terminal);
                    let cancelled = worker_cancellation.is_cancelled();
                    worker_kernel.terminated(if cancelled {
                        if terminal_failure == Some("managed_runtime_timed_out") {
                            ManagedTermination::CancelledPastLimit
                        } else {
                            ManagedTermination::Cancelled
                        }
                    } else {
                        match terminal_failure {
                            Some("managed_runtime_timed_out") => {
                                ManagedTermination::Failed(InvocationClosure::LimitExceeded)
                            }
                            Some(_) => ManagedTermination::Failed(InvocationClosure::RuntimeError),
                            None if completed => ManagedTermination::Completed,
                            None => ManagedTermination::Failed(InvocationClosure::RuntimeError),
                        }
                    })?;
                    // A cancelled run reports no failure of its own: what the runtime reported is
                    // how the cancellation reached it, and recording that as a supervision failure
                    // would move the journal off the cancellation it already holds.
                    if cancelled {
                        return Ok(());
                    }
                    if let Some(failure) = terminal_failure {
                        bail!(failure);
                    }
                    if !completed {
                        bail!("runtime ended without a completed event");
                    }
                    // What the work produced is read out of the run's own record: the base its
                    // attempt started from and the exact object standing at every path the result
                    // changed. The kernel is given that construction rather than a digest, so the
                    // ancestry of the result reaches the journal with it.
                    let construction = {
                        let application = worker_application
                            .lock()
                            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
                        if application.state().status != RunStatus::Running {
                            return Ok(());
                        }
                        application
                            .candidate_construction()?
                            .context("completed runtime has no controller-committed candidate")?
                    };
                    // The work the run is accountable for now carries the exact candidate a
                    // protected query would be spent on.
                    //
                    // Where the kernel cannot state what the work produced, the run says which
                    // bound it exceeded and ends. Sealing the digest alone would leave a record an
                    // operator cannot tell from a run whose construction was never journalled at
                    // all, and announcing the candidate would send a result for judgement while its
                    // own accounting states nothing about where it came from. Both records are
                    // moved to the same ending: the journal carries the reason, and the kernel is
                    // stopped as the infrastructure condition it is.
                    // A result whose construction the kernel cannot state is still a result: the
                    // work is sealed with it, the run goes on to the verdict, and what the record
                    // would otherwise be silent about — that its ancestry is missing, and which
                    // bound stopped it — is written into the run's own journal instead. Reading
                    // that record, an operator can tell this run from one whose construction was
                    // never journalled at all, which a seal on its own does not allow.
                    if let Submission::ProvenanceUnrecorded {
                        reason,
                        protocol_rule,
                    } = worker_kernel.submitted(&construction)?
                    {
                        worker_application
                            .lock()
                            .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?
                            .record_candidate_provenance_unrecorded(
                                format!("{worker_attempt}.provenance"),
                                &worker_attempt,
                                &construction.candidate_digest,
                                reason,
                                protocol_rule,
                            )?;
                    }
                    let candidate_digest = construction.candidate_digest;
                    drop(terminal);
                    let _ = sender.send(ManagedRunEvent::CandidateAvailable {
                        candidate_digest,
                        change_count: None,
                    });
                    Ok(())
                })();
                // The runtime session terminates its process tree when it is dropped, so it is
                // dropped here rather than at the end of the thread. Without this the controller
                // could observe a terminal outcome while the managed processes were still being
                // signalled, and a reading of the process table taken at that moment would be
                // racing the supervisor instead of measuring it. A panic drops it here too, while
                // the stack unwinds out of the closure that owns it, so the process tree is ended
                // either way.
                drop(session);
                result
            }));
            // Whichever way supervision ended, it is named in one vocabulary from here on. A
            // panic is not a diagnosis the run can report anything further about, so what is kept
            // of it is that supervision died rather than the payload it died with.
            let supervision = match outcome {
                Ok(Ok(())) => None,
                Ok(Err(error)) => {
                    let _ = error;
                    Some("managed_runtime_supervision_failed")
                }
                Err(panic) => {
                    let _ = panic;
                    Some("managed_runtime_worker_panicked")
                }
            };
            // The places that end the process tree while they are already reporting a cancellation,
            // a time limit or a runtime failure — and the session being dropped, which reports to
            // nobody — keep what they could not establish. It is read here, because a run may
            // report that it left nothing running only where that was measured.
            let unestablished = unestablished_terminations();
            let kept = (!unestablished.is_empty()).then(|| {
                format!(
                    "managed_runtime_termination_unestablished: {}",
                    unestablished.join("; ")
                )
            });
            let detail = match (supervision, kept) {
                (Some(supervision), kept) => Some(kept.map_or_else(
                    || supervision.to_owned(),
                    |kept| format!("{supervision}; {kept}"),
                )),
                (None, kept) => kept,
            };
            if let Some(detail) = detail {
                // A journal that would not take the failure is reported with it: the run is what
                // carries that news to whoever is watching, because the record they would otherwise
                // read still calls the run running.
                let unrecorded =
                    record_infrastructure_failure(&worker_application, &worker_attempt, &detail);
                let detail = match unrecorded {
                    Some(unrecorded) => format!("{detail}; {unrecorded}"),
                    None => detail,
                };
                // Supervision that failed on its way out still owes the kernel a terminal: a slice
                // left open would hold the run open on capacity nothing is running under. It is one
                // more ending, so it is recorded under the same guard as the others.
                //
                // What the kernel would not take is carried out with the failure, for the reason
                // the journal's refusal is carried out with it: both records are written to the
                // same journal, so a journal that stopped taking records leaves the run without
                // either ending, and the run itself is the only thing that can still say so.
                let unrecorded_terminal = {
                    let _terminal = take_terminal(&worker_terminal);
                    worker_kernel
                        .terminated(ManagedTermination::Failed(
                            InvocationClosure::InfrastructureError,
                        ))
                        .err()
                        .map(|error| format!("managed_runtime_terminal_unrecorded: {error}"))
                };
                let detail = match unrecorded_terminal {
                    Some(unrecorded) => format!("{detail}; {unrecorded}"),
                    None => detail,
                };
                let _ = sender.send(ManagedRunEvent::Failed { detail });
            }
            let _ = sender.send(ManagedRunEvent::Finished);
            worker_finished.store(true, Ordering::Release);
        })?;

    Ok(ManagedRunHandle {
        attempt_id,
        invocation_id,
        runtime_kind,
        cancellation,
        application,
        receiver: Mutex::new(receiver),
        control_sender: Some(control_sender),
        kernel,
        terminal,
        finished,
        worker: Some(worker),
    })
}

struct LifecycleAuthority<'a> {
    application: &'a Arc<Mutex<Application>>,
    attempt_id: &'a str,
    invocation_id: &'a str,
    invocation_control: &'a ymp_agent_rpc::InvocationControl,
    application_cursor: &'a mut u64,
    yield_cursor: &'a mut u64,
}

fn authoritative_lifecycle_event(
    authority: LifecycleAuthority<'_>,
    runtime_event: &RuntimeEventKind,
    fallback_usage: Usage,
) -> anyhow::Result<RuntimeEventKind> {
    let completed_usage = match runtime_event {
        RuntimeEventKind::Completed { usage } => Some(usage.clone()),
        RuntimeEventKind::Yielded { .. } => None,
        _ => bail!("controller classification requires a lifecycle runtime event"),
    };
    let application = authority
        .application
        .lock()
        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
    let events = application.events_after(*authority.application_cursor)?;
    if let Some(last) = events.last() {
        *authority.application_cursor = last.sequence;
    }
    let submissions: Vec<_> = events
        .iter()
        .filter(|event| match &event.event {
            EventKind::CandidateSubmitted {
                attempt_id: submitted_attempt,
                ..
            } => submitted_attempt == authority.attempt_id,
            _ => false,
        })
        .collect();
    drop(application);
    let yields = authority
        .invocation_control
        .confirmations_after(*authority.yield_cursor)?;
    if let Some(last) = yields.last() {
        *authority.yield_cursor = last.sequence;
    }
    let action_count = submissions.len().saturating_add(yields.len());
    let classified = match action_count {
        1 if submissions.len() == 1 && completed_usage.is_some() => RuntimeEventKind::Completed {
            usage: completed_usage.expect("checked completed usage"),
        },
        1 if yields.len() == 1 => {
            let confirmation = &yields[0];
            if confirmation.attempt_id != authority.attempt_id
                || confirmation.invocation_id != authority.invocation_id
            {
                bail!("yield confirmation identity does not match managed invocation");
            }
            RuntimeEventKind::Yielded {
                cursor: confirmation.command_id.clone(),
            }
        }
        _ => RuntimeEventKind::Failed {
            kind: RuntimeFailureKind::Protocol,
            usage: completed_usage.unwrap_or(fallback_usage),
            diagnostic: Some(DiagnosticSummary::from_bytes(
                b"controller_action_count_invalid",
                false,
            )),
        },
    };
    Ok(classified)
}

#[allow(clippy::too_many_arguments)]
fn append_runtime_evidence(
    file: &mut File,
    run_id: &str,
    attempt_id: &str,
    runtime_kind: RuntimeKind,
    contract_id: &str,
    contract_digest: &str,
    profile_digest: &str,
    predecessor_digest: &Option<String>,
    runtime_event: &RuntimeEvent,
) -> anyhow::Result<String> {
    let event = runtime_evidence_kind(&runtime_event.event)?;
    let input = RuntimeEvidenceDigestInput {
        schema_version: RUNTIME_EVIDENCE_SCHEMA_VERSION,
        run_id,
        attempt_id,
        invocation_id: &runtime_event.invocation_id,
        runtime_kind,
        contract_id,
        contract_digest,
        profile_digest,
        sequence: runtime_event.sequence,
        event_id: &runtime_event.event_id,
        predecessor_digest,
        event: &event,
    };
    let digest = digest_bytes(&serde_json::to_vec(&input)?);
    let envelope = RuntimeEvidenceEnvelope {
        schema_version: RUNTIME_EVIDENCE_SCHEMA_VERSION,
        run_id,
        attempt_id,
        invocation_id: &runtime_event.invocation_id,
        runtime_kind,
        contract_id,
        contract_digest,
        profile_digest,
        sequence: runtime_event.sequence,
        event_id: &runtime_event.event_id,
        predecessor_digest,
        event: &event,
        digest: &digest,
    };
    let bytes = serde_json::to_vec(&envelope)?;
    let existing = file.metadata()?.len();
    if existing.saturating_add(bytes.len() as u64 + 1) > MAX_RUNTIME_EVIDENCE_BYTES {
        bail!("runtime evidence exceeded its {MAX_RUNTIME_EVIDENCE_BYTES}-byte transcript limit");
    }
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_data()?;
    Ok(digest)
}

fn write_runtime_profile_evidence(
    directory: &Path,
    profile: RuntimeProfileEvidence<'_>,
) -> anyhow::Result<String> {
    let profile = serde_json::to_value(profile)?;
    let digest = digest_bytes(&serde_json::to_vec(&profile)?);
    let envelope = serde_json::json!({
        "profile": profile,
        "digest": digest,
    });
    let bytes = serde_json::to_vec(&envelope)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("profile.json"))?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_data()?;
    Ok(digest)
}

fn digest_regular_file(path: &Path) -> anyhow::Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(digest_bytes(&fs::read(path)?)))
}

fn runtime_evidence_kind(event: &RuntimeEventKind) -> anyhow::Result<RuntimeEvidenceKind> {
    Ok(match event {
        RuntimeEventKind::Launch { descriptor } => RuntimeEvidenceKind::Launch {
            descriptor: descriptor.as_ref().clone(),
            descriptor_digest: digest_serialized(descriptor)?,
        },
        RuntimeEventKind::Started { opaque_session_id } => RuntimeEvidenceKind::Started {
            opaque_session_digest: digest_bytes(opaque_session_id.as_bytes()),
        },
        RuntimeEventKind::Output { text } => RuntimeEvidenceKind::Output {
            text_digest: digest_bytes(text.as_bytes()),
            text_bytes: text.len(),
        },
        RuntimeEventKind::McpToolCall {
            server,
            tool,
            status,
            arguments,
            result,
            error,
        } => RuntimeEvidenceKind::McpToolCall {
            server: server.clone(),
            tool: tool.clone(),
            status: status.clone(),
            arguments_digest: digest_json(arguments)?,
            result_digest: result.as_ref().map(digest_json).transpose()?,
            error_digest: error.as_ref().map(digest_json).transpose()?,
        },
        RuntimeEventKind::Yielded { cursor } => RuntimeEvidenceKind::Yielded {
            cursor_digest: digest_bytes(cursor.as_bytes()),
        },
        RuntimeEventKind::Completed { usage } => RuntimeEvidenceKind::Completed {
            usage: usage.clone(),
        },
        RuntimeEventKind::Failed {
            kind,
            usage,
            diagnostic,
        } => RuntimeEvidenceKind::Failed {
            kind: *kind,
            usage: usage.clone(),
            diagnostic: diagnostic.clone(),
        },
        RuntimeEventKind::TimedOut { limit_ms, usage } => RuntimeEvidenceKind::TimedOut {
            limit_ms: *limit_ms,
            usage: usage.clone(),
        },
        RuntimeEventKind::Cancelled { usage } => RuntimeEvidenceKind::Cancelled {
            usage: usage.clone(),
        },
        RuntimeEventKind::Interrupted => RuntimeEvidenceKind::Interrupted,
    })
}

fn digest_json(value: &Value) -> anyhow::Result<String> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
}

struct RuntimeProgress {
    invocation_id: String,
    last_sequence: u64,
    event_ids: HashSet<String>,
}

impl RuntimeProgress {
    fn new(invocation_id: &str) -> Self {
        Self {
            invocation_id: invocation_id.to_owned(),
            last_sequence: 0,
            event_ids: HashSet::new(),
        }
    }

    fn next_sequence(&self) -> anyhow::Result<u64> {
        self.last_sequence
            .checked_add(1)
            .context("runtime event sequence exhausted")
    }

    fn validate(&mut self, event: &RuntimeEvent) -> anyhow::Result<()> {
        if event.invocation_id != self.invocation_id {
            bail!("runtime event invocation_id does not match the launch descriptor");
        }
        let event_id_chars = event.event_id.chars().count();
        if !(1..=MAX_IDENTIFIER_CHARS).contains(&event_id_chars) {
            bail!("runtime event_id must contain between 1 and {MAX_IDENTIFIER_CHARS} characters");
        }
        if self.event_ids.contains(&event.event_id) {
            bail!("runtime repeated event_id {}", event.event_id);
        }
        let expected_sequence = self
            .last_sequence
            .checked_add(1)
            .context("runtime event sequence exhausted")?;
        if event.sequence != expected_sequence {
            bail!(
                "runtime event sequence {} does not match expected sequence {expected_sequence}",
                event.sequence
            );
        }
        self.event_ids.insert(event.event_id.clone());
        self.last_sequence = event.sequence;
        Ok(())
    }
}

fn digest_serialized(value: &impl Serialize) -> anyhow::Result<String> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
}

fn validate_launch_descriptor(
    descriptor: &LaunchDescriptor,
    attempt_id: &str,
    invocation_id: &str,
    workspace: &Path,
) -> anyhow::Result<()> {
    if descriptor.schema_version != 1 {
        bail!(
            "unsupported launch descriptor schema version {}",
            descriptor.schema_version
        );
    }
    if descriptor.attempt_id != attempt_id
        || descriptor.invocation_id != invocation_id
        || descriptor.working_directory != workspace
    {
        bail!("launch descriptor identity does not match the managed request");
    }
    let executable_digest = digest_regular_file(&descriptor.executable)?
        .context("launch descriptor executable is not a regular file")?;
    if executable_digest != descriptor.executable_digest {
        bail!("launch descriptor executable digest does not match its bytes");
    }
    // The controller re-establishes each program under the requirement it was admitted with, not
    // only its digest: a program recorded as standing where the account cannot write is checked
    // against that location again here. What the controller does not do is decide which programs
    // the chain should contain; that check is made where the runtime kind is known, because only
    // there is it known whether the launch is one that must be attested.
    for program in &descriptor.launch_chain {
        program
            .verify()
            .map_err(|error| anyhow::anyhow!("launch descriptor {error}"))?;
    }
    match (
        descriptor.coordination_executable.as_deref(),
        descriptor.coordination_executable_digest.as_deref(),
    ) {
        (Some(executable), Some(expected_digest)) => {
            let actual_digest = digest_regular_file(executable)?
                .context("launch descriptor MCP executable is not a regular file")?;
            if actual_digest != expected_digest {
                bail!("launch descriptor MCP executable digest does not match its bytes");
            }
        }
        (None, None) => {}
        _ => bail!("launch descriptor MCP executable and digest must be declared together"),
    }
    let mut names = HashSet::new();
    for variable in &descriptor.environment {
        if variable.name.is_empty() || !names.insert(variable.name.as_str()) {
            bail!("launch descriptor environment contains an empty or duplicate name");
        }
        if variable.confidential && variable.value.is_some() {
            bail!("launch descriptor records a confidential environment value");
        }
        if let Some(value) = &variable.value
            && digest_bytes(value.as_bytes()) != variable.value_digest
        {
            bail!("launch descriptor environment digest does not match its value");
        }
    }
    Ok(())
}

/// Runtimes whose managed process must be created from an immutable launch descriptor and must
/// attest that descriptor before any other event.
const fn requires_launch_attestation(kind: RuntimeKind) -> bool {
    matches!(kind, RuntimeKind::Codex | RuntimeKind::ClaudeCode)
}

/// The exact arguments a resumed managed process may use. Each runtime names its own resume
/// operand, so the rule is stated per runtime rather than inferred from the observed arguments.
fn expected_resume_arguments(
    kind: RuntimeKind,
    initial: &[String],
    session: &str,
) -> anyhow::Result<Vec<String>> {
    let mut expected = initial.to_vec();
    match kind {
        RuntimeKind::Codex => {
            let prompt_source = expected
                .pop()
                .context("initial runtime launch has no prompt-source argument")?;
            expected.extend(["resume".to_owned(), session.to_owned(), prompt_source]);
        }
        RuntimeKind::ClaudeCode => {
            expected.extend(["--resume".to_owned(), session.to_owned()]);
        }
        RuntimeKind::Fake => bail!("this runtime does not attest a resumed launch"),
    }
    Ok(expected)
}

fn validate_runtime_launch(
    kind: RuntimeKind,
    initial: Option<&LaunchDescriptor>,
    descriptor: &LaunchDescriptor,
    after_initial: bool,
    expected_session: Option<&str>,
) -> anyhow::Result<()> {
    let initial =
        initial.context("runtime emitted launch evidence without a prepared descriptor")?;
    validate_launch_descriptor(
        descriptor,
        &initial.attempt_id,
        &initial.invocation_id,
        &initial.working_directory,
    )?;
    if !after_initial {
        if descriptor != initial {
            bail!("runtime initial launch differs from the prepared descriptor");
        }
        return Ok(());
    }
    if descriptor.executable != initial.executable
        || descriptor.executable_digest != initial.executable_digest
        || descriptor.coordination_executable != initial.coordination_executable
        || descriptor.coordination_executable_digest != initial.coordination_executable_digest
        || descriptor.launch_chain != initial.launch_chain
        || descriptor.environment != initial.environment
        || descriptor.working_directory != initial.working_directory
    {
        bail!("runtime resume changed the executable, launch chain, environment, or workspace");
    }
    let expected_session = expected_session.context("runtime resume has no established session")?;
    let expected_arguments = expected_resume_arguments(kind, &initial.arguments, expected_session)?;
    if descriptor.arguments != expected_arguments {
        bail!("runtime resume changed its prepared arguments or session identifier");
    }
    Ok(())
}

fn runtime_error_code(error: &RuntimeError) -> &'static str {
    match error {
        RuntimeError::Process(_)
        | RuntimeError::UnsuccessfulExit { .. }
        | RuntimeError::SanitizedUnsuccessfulExit { .. } => "runtime_process_failed",
        RuntimeError::TimedOut { .. } => "runtime_timed_out",
        RuntimeError::OutputLimitExceeded { .. } => "runtime_output_limit_exceeded",
        RuntimeError::MalformedEvent(_) | RuntimeError::NonUtf8Output => "runtime_protocol_failed",
        RuntimeError::InvalidProfile(_) | RuntimeError::Unsupported(_) => "runtime_profile_invalid",
        RuntimeError::RuntimeReportedFailure(_) => "runtime_reported_failure",
        RuntimeError::NotYielded => "runtime_not_yielded",
    }
}

fn runtime_error_kind(error: &RuntimeError) -> RuntimeFailureKind {
    match error {
        RuntimeError::Process(_)
        | RuntimeError::UnsuccessfulExit { .. }
        | RuntimeError::SanitizedUnsuccessfulExit { .. } => RuntimeFailureKind::ProcessExit,
        RuntimeError::OutputLimitExceeded { .. } => RuntimeFailureKind::OutputLimit,
        RuntimeError::RuntimeReportedFailure(_) => RuntimeFailureKind::RuntimeReported,
        _ => RuntimeFailureKind::Protocol,
    }
}

fn runtime_error_diagnostic(error: &RuntimeError) -> Option<DiagnosticSummary> {
    match error {
        RuntimeError::SanitizedUnsuccessfulExit { diagnostic, .. } => Some(diagnostic.clone()),
        _ => Some(DiagnosticSummary::from_bytes(
            error.to_string().as_bytes(),
            false,
        )),
    }
}

/// Record in the journal that this run failed for reasons of its supervision.
///
/// The journal lock is taken poisoned, for the reason the terminal guard is: a panic under it
/// leaves no half-written datum behind, because every fact of the run is appended to the journal
/// whole before it is applied in memory, and a command whose application a panic interrupted is
/// refused on the sequence it would repeat rather than written twice. Abandoning the record here
/// left the worse state of the two — the kernel closed the slice on the infrastructure error while
/// the journal, which is the record an operator reads, went on reporting the run as running.
///
/// The command may still be refused where the journal itself cannot take it: a full or unwritable
/// journal refuses the append, and a journal whose sequence a panic left behind the memory that
/// keeps it refuses the record it would repeat. That refusal is answered here rather than dropped,
/// because a run whose failure the journal declined leaves that record naming a run still in
/// progress, and nothing else in this process would ever say so.
#[must_use = "a failure the journal refused is reported with the run"]
fn record_infrastructure_failure(
    application: &Arc<Mutex<Application>>,
    attempt_id: &str,
    detail: &str,
) -> Option<String> {
    let mut application = application
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if application.state().status != RunStatus::Running {
        return None;
    }
    let reason = bounded_reason(detail);
    application
        .execute(
            format!("{attempt_id}.runtime-failed"),
            Command::FailInfrastructure { reason },
        )
        .err()
        .map(|error| format!("managed_runtime_failure_unrecorded: {error}"))
}

/// The failure one start reports, carrying what the journal would not record beside it.
///
/// Every start that fails records the failure in the journal and then reports it to its caller.
/// Where the journal declined the record, the caller is the last place that can still say so, so
/// the refusal travels out with the failure it belongs to.
fn failed_start(
    application: &Arc<Mutex<Application>>,
    attempt_id: &str,
    detail: &str,
    failure: anyhow::Error,
) -> anyhow::Error {
    match record_infrastructure_failure(application, attempt_id, detail) {
        Some(unrecorded) => failure.context(unrecorded),
        None => failure,
    }
}

fn bounded_reason(detail: &str) -> String {
    let mut end = detail.len().min(900);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        "managed runtime failed without diagnostic text".to_owned()
    } else {
        detail[..end].to_owned()
    }
}

/// Resolves the workspace program once and binds it to a location the account this run executes
/// under cannot write.
///
/// A name resolved through the search path is not an identity: the search path here is led by
/// several directories this account owns, so a program planted in one of them would answer to the
/// name and be admitted as if it were the program the run means. The first candidate the search
/// path yields is therefore refused unless it stands where only the superuser could have put it,
/// rather than skipped in favour of a later one, because a program answering to that name from a
/// writable directory is a condition the run must report and not step over.
pub fn admit_workspace_program() -> anyhow::Result<AdmittedProgram> {
    let path = std::env::var_os("PATH").context("PATH is not set, so git cannot be resolved")?;
    admit_workspace_program_from(&path)
}

/// The search path is a parameter so that the binding can be checked against a planted program
/// without changing the environment of the process running the check.
fn admit_workspace_program_from(search_path: &OsStr) -> anyhow::Result<AdmittedProgram> {
    const PROGRAM: &str = "git";
    let resolved = std::env::split_paths(search_path)
        .map(|directory| directory.join(PROGRAM))
        .find(|candidate| candidate.is_file() && is_executable_file(candidate).unwrap_or(false))
        .with_context(|| format!("{PROGRAM} was not found on PATH"))?;
    Ok(AdmittedProgram::admit(
        ProgramRole::Workspace,
        resolved,
        &ProgramRequirement::SystemPath,
    )?)
}

/// Builds the private baseline of a managed workspace with the admitted program, re-establishing
/// its identity and bytes before each execution. This is the only copy: a second one that named the
/// program instead would resolve it through the search path again on every run.
pub fn initialize_private_git(workspace: &Path, git: &AdmittedProgram) -> anyhow::Result<()> {
    for (action, arguments) in [
        ("initialize private Git repository", vec!["init", "--quiet"]),
        ("stage private workspace", vec!["add", "--all"]),
        (
            "commit private baseline",
            vec![
                "-c",
                "user.name=ymp",
                "-c",
                "user.email=ymp@invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "ymp private baseline",
            ],
        ),
    ] {
        git.verify()?;
        let status = ProcessCommand::new(&git.path)
            .args(arguments)
            .current_dir(workspace)
            .status()
            .with_context(|| format!("{action} in {}", workspace.display()))?;
        if !status.success() {
            bail!("{action} failed with {status}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CONTROLLER_SHUTDOWN_LIMIT, ManagedCandidateRequest, ManagedContract, ManagedKernel,
        ManagedRunEvent, ManagedRunHandle, admit_workspace_program, admit_workspace_program_from,
        initialize_private_git, start_managed_candidate, start_unattested_managed_candidate,
        validate_launch_descriptor, validate_runtime_launch, validate_runtime_probe,
    };
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use ymp_agent_api::{AgentToolCall, AgentToolHandler, SubmitArguments, YieldArguments};
    use ymp_agent_rpc::SocketToolHandler;
    use ymp_application::Application;
    use ymp_domain::{Budget, RunStatus, digest_bytes};
    use ymp_runtime_api::{
        AdmittedProgram, CancellationToken, InvocationRequest, ProbeReport, ProgramIdentity,
        ProgramRequirement, ProgramRole, Readiness, RuntimeDriver, RuntimeError, RuntimeEvent,
        RuntimeEventKind, RuntimeKind, RuntimeSession, Usage,
    };
    use ymp_runtime_fake::{FakeRuntime, ScriptStep};

    struct SubmittingRuntime {
        inner: FakeRuntime,
    }

    impl RuntimeDriver for SubmittingRuntime {
        fn kind(&self) -> RuntimeKind {
            self.inner.kind()
        }

        fn executable(&self) -> &Path {
            self.inner.executable()
        }

        fn probe(&self) -> Result<ProbeReport, RuntimeError> {
            self.inner.probe()
        }

        fn start(
            &self,
            request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            std::fs::write(request.workspace.join("input.txt"), b"agent candidate\n")?;
            let binding = request.mcp.as_ref().ok_or_else(|| {
                RuntimeError::InvalidProfile("test runtime requires MCP binding".to_owned())
            })?;
            let mut client = SocketToolHandler::for_invocation(
                &binding.socket_path,
                &binding.token,
                &request.attempt_id,
                &request.invocation_id,
            );
            client
                .call(AgentToolCall::Submit(SubmitArguments {
                    command_id: "agent.submit".to_owned(),
                }))
                .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
            self.inner.start(request)
        }
    }

    struct LifecycleRuntime {
        inner: FakeRuntime,
    }

    impl RuntimeDriver for LifecycleRuntime {
        fn kind(&self) -> RuntimeKind {
            self.inner.kind()
        }

        fn executable(&self) -> &Path {
            self.inner.executable()
        }

        fn probe(&self) -> Result<ProbeReport, RuntimeError> {
            self.inner.probe()
        }

        fn start(
            &self,
            request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            let binding = request.mcp.as_ref().ok_or_else(|| {
                RuntimeError::InvalidProfile("test runtime requires MCP binding".to_owned())
            })?;
            let mut controller = SocketToolHandler::for_invocation(
                &binding.socket_path,
                &binding.token,
                &request.attempt_id,
                &request.invocation_id,
            );
            controller
                .call(AgentToolCall::Yield(YieldArguments {
                    command_id: "agent.yield".to_owned(),
                }))
                .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
            Ok(Box::new(LifecycleSession {
                inner: self.inner.start(request)?,
                controller,
            }))
        }
    }

    struct LifecycleSession {
        inner: Box<dyn RuntimeSession>,
        controller: SocketToolHandler,
    }

    impl RuntimeSession for LifecycleSession {
        fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
            self.inner.next_event()
        }

        fn resume(&mut self, input: String) -> Result<(), RuntimeError> {
            self.inner.resume(input)?;
            self.controller
                .call(AgentToolCall::Submit(SubmitArguments {
                    command_id: "agent.submit.after-wake".to_owned(),
                }))
                .map_err(|error| RuntimeError::RuntimeReportedFailure(error.to_string()))?;
            Ok(())
        }

        fn interrupt(&mut self) -> Result<(), RuntimeError> {
            self.inner.interrupt()
        }

        fn usage(&self) -> Usage {
            self.inner.usage()
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum ProgressFault {
        Gap,
        Reordered,
        RepeatedEventId,
        EmptyEventId,
        WrongInvocation,
    }

    #[test]
    fn codex_projection_records_any_nonempty_behaviorally_accepted_version() {
        let report = |version: Option<&str>| ProbeReport {
            kind: RuntimeKind::Codex,
            executable: "codex".to_owned(),
            version: version.map(str::to_owned),
            readiness: Readiness::Ready,
            detail: "measured fixture".to_owned(),
        };
        for version in ["codex-cli 0.151.0", "codex-cli 9.7.3"] {
            validate_runtime_probe(RuntimeKind::Codex, &report(Some(version)))
                .expect("version difference alone must not reject compatible behavior");
        }

        for version in [None, Some("")] {
            let error = validate_runtime_probe(RuntimeKind::Codex, &report(version))
                .expect_err("missing observed version evidence must fail closed")
                .to_string();
            assert!(error.contains("projection_missing"), "{version:?}: {error}");
        }
    }

    struct FaultingRuntime {
        fault: ProgressFault,
    }

    impl RuntimeDriver for FaultingRuntime {
        fn kind(&self) -> RuntimeKind {
            RuntimeKind::Fake
        }

        fn executable(&self) -> &Path {
            Path::new("ymp-internal-faulting")
        }

        fn probe(&self) -> Result<ProbeReport, RuntimeError> {
            Ok(ProbeReport {
                kind: RuntimeKind::Fake,
                executable: self.executable().display().to_string(),
                version: Some("test".to_owned()),
                readiness: Readiness::Ready,
                detail: "faulting runtime for supervisor regression".to_owned(),
            })
        }

        fn start(
            &self,
            request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            let invocation_id = request.invocation_id;
            let event = |sequence, event_id: String, event| RuntimeEvent {
                sequence,
                event_id,
                invocation_id: invocation_id.clone(),
                event,
            };
            let first_id = format!("{invocation_id}.event-1");
            let mut events = vec![event(
                1,
                first_id.clone(),
                RuntimeEventKind::Started {
                    opaque_session_id: "opaque".to_owned(),
                },
            )];
            match self.fault {
                ProgressFault::Gap => events.push(event(
                    3,
                    format!("{invocation_id}.event-3"),
                    RuntimeEventKind::Output {
                        text: "gap".to_owned(),
                    },
                )),
                ProgressFault::Reordered => {
                    events.push(event(
                        2,
                        format!("{invocation_id}.event-2"),
                        RuntimeEventKind::Output {
                            text: "ordered".to_owned(),
                        },
                    ));
                    events.push(event(
                        1,
                        format!("{invocation_id}.event-reordered"),
                        RuntimeEventKind::Output {
                            text: "reordered".to_owned(),
                        },
                    ));
                }
                ProgressFault::RepeatedEventId => events.push(event(
                    2,
                    first_id,
                    RuntimeEventKind::Output {
                        text: "repeated identifier".to_owned(),
                    },
                )),
                ProgressFault::EmptyEventId => events.push(event(
                    2,
                    String::new(),
                    RuntimeEventKind::Output {
                        text: "empty identifier".to_owned(),
                    },
                )),
                ProgressFault::WrongInvocation => events.push(RuntimeEvent {
                    sequence: 2,
                    event_id: format!("{invocation_id}.event-2"),
                    invocation_id: "substituted-invocation".to_owned(),
                    event: RuntimeEventKind::Output {
                        text: "wrong invocation".to_owned(),
                    },
                }),
            }
            events.push(event(
                4,
                format!("{invocation_id}.event-completed"),
                RuntimeEventKind::Completed {
                    usage: Usage::default(),
                },
            ));
            Ok(Box::new(FaultingSession {
                events: events.into(),
            }))
        }
    }

    struct FaultingSession {
        events: VecDeque<RuntimeEvent>,
    }

    impl RuntimeSession for FaultingSession {
        fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
            Ok(self.events.pop_front())
        }

        fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
            Err(RuntimeError::Unsupported("faulting test runtime resume"))
        }

        fn interrupt(&mut self) -> Result<(), RuntimeError> {
            self.events.clear();
            Ok(())
        }
    }

    fn executable_fixture(path: &Path, contents: &[u8]) {
        std::fs::write(path, contents).expect("write program fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(path)
                .expect("program metadata")
                .permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(path, permissions).expect("program permissions");
        }
    }

    fn stated(role: ProgramRole, path: PathBuf) -> AdmittedProgram {
        let digest = ymp_domain::digest_bytes(&std::fs::read(&path).expect("program bytes"));
        AdmittedProgram::admit(role, path, &ProgramRequirement::StatedDigest(digest))
            .expect("admit a program bound by its stated digest")
    }

    fn chain_descriptor(directory: &Path) -> ymp_runtime_api::LaunchDescriptor {
        let executable = directory.join("runtime");
        executable_fixture(&executable, b"#!/bin/sh\nexit 0\n");
        let shell = directory.join("chain-shell");
        executable_fixture(&shell, b"#!/bin/sh\nexec /bin/sh \"$@\"\n");
        let sanitiser = directory.join("chain-sanitiser");
        executable_fixture(&sanitiser, b"#!/bin/sh\nexec /usr/bin/env \"$@\"\n");
        ymp_runtime_api::LaunchDescriptor {
            schema_version: 1,
            invocation_id: "invocation-chain".to_owned(),
            attempt_id: "attempt-chain".to_owned(),
            executable: executable.clone(),
            executable_digest: ymp_domain::digest_bytes(
                &std::fs::read(&executable).expect("runtime bytes"),
            ),
            coordination_executable: None,
            coordination_executable_digest: None,
            launch_chain: vec![
                stated(ProgramRole::LaunchShell, shell),
                stated(ProgramRole::EnvironmentSanitiser, sanitiser),
            ],
            arguments: vec!["exec".to_owned(), "-".to_owned()],
            environment: Vec::new(),
            working_directory: directory.to_owned(),
        }
    }

    /// The controller admits the chain independently of the driver that declared it, so a program
    /// replaced between preparation and launch is refused even if the driver accepted it.
    #[test]
    fn a_launch_descriptor_whose_chain_program_changed_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let descriptor = chain_descriptor(temporary.path());
        for index in 0..descriptor.launch_chain.len() {
            let descriptor = chain_descriptor(temporary.path());
            validate_launch_descriptor(
                &descriptor,
                &descriptor.attempt_id,
                &descriptor.invocation_id,
                temporary.path(),
            )
            .expect("an unchanged chain is admitted");
            let replaced = &descriptor.launch_chain[index].path;
            let bytes = std::fs::read(replaced).expect("chain program bytes");
            std::fs::write(replaced, [bytes.as_slice(), b"# substituted\n"].concat())
                .expect("substitute chain program");
            let error = validate_launch_descriptor(
                &descriptor,
                &descriptor.attempt_id,
                &descriptor.invocation_id,
                temporary.path(),
            )
            .expect_err("a replaced chain program is refused")
            .to_string();
            assert!(error.contains("changed after admission"), "{error}");
        }
    }

    /// A resumed launch may change its session operand and nothing else. Without this the chain the
    /// run was admitted with could be exchanged at the first resume.
    #[test]
    fn a_resume_that_changes_the_launch_chain_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let initial = chain_descriptor(temporary.path());
        let mut resumed = initial.clone();
        resumed.arguments = vec![
            "exec".to_owned(),
            "resume".to_owned(),
            "session-1".to_owned(),
            "-".to_owned(),
        ];
        validate_runtime_launch(
            RuntimeKind::Codex,
            Some(&initial),
            &resumed,
            true,
            Some("session-1"),
        )
        .expect("an unchanged chain resumes");
        let mut exchanged = resumed.clone();
        exchanged.launch_chain.pop();
        let error = validate_runtime_launch(
            RuntimeKind::Codex,
            Some(&initial),
            &exchanged,
            true,
            Some("session-1"),
        )
        .expect_err("a resumed launch may not change its chain")
        .to_string();
        assert!(error.contains("launch chain"), "{error}");
    }

    /// The search path is led by directories this account owns, so a name resolved through it is
    /// not an identity. A program planted there answers to the name and must be refused rather than
    /// admitted, and rather than stepped over in favour of a later candidate.
    #[test]
    fn a_program_planted_earlier_in_the_search_path_is_refused() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let planted_directory = temporary.path().join("planted");
        std::fs::create_dir(&planted_directory).expect("planted directory");
        let planted = planted_directory.join("git");
        executable_fixture(&planted, b"#!/bin/sh\nexec /usr/bin/git \"$@\"\n");

        let system = admit_workspace_program().expect("the system program is admitted");
        assert_eq!(system.identity, ProgramIdentity::SystemPath);
        assert!(system.path.is_absolute());

        let search = std::env::join_paths([planted_directory.as_path(), Path::new("/usr/bin")])
            .expect("search path");
        let error = admit_workspace_program_from(&search)
            .expect_err("a planted program is refused")
            .to_string();
        assert!(error.contains("this account can write"), "{error}");
        assert!(
            error.contains(planted.to_str().expect("planted path")),
            "{error}"
        );
    }

    /// Declares a launch descriptor without admitting the programs its launch chain enters. The
    /// product drivers admit them; this exists so that a driver which does not is refused rather
    /// than reaching the same launch through an unadmitted shell.
    struct ChainlessRuntime {
        executable: PathBuf,
        chain: Vec<AdmittedProgram>,
    }

    impl RuntimeDriver for ChainlessRuntime {
        fn kind(&self) -> RuntimeKind {
            RuntimeKind::Codex
        }

        fn executable(&self) -> &Path {
            &self.executable
        }

        fn probe(&self) -> Result<ProbeReport, RuntimeError> {
            Ok(ProbeReport {
                kind: RuntimeKind::Codex,
                executable: self.executable.display().to_string(),
                version: Some("codex-cli 0.151.0".to_owned()),
                readiness: Readiness::Ready,
                detail: "chainless runtime for supervisor regression".to_owned(),
            })
        }

        fn start(
            &self,
            _request: InvocationRequest,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            Err(RuntimeError::Unsupported("chainless runtime start"))
        }

        fn prepare_launch(
            &self,
            request: &InvocationRequest,
        ) -> Result<Option<ymp_runtime_api::LaunchDescriptor>, RuntimeError> {
            Ok(Some(ymp_runtime_api::LaunchDescriptor {
                schema_version: 1,
                invocation_id: request.invocation_id.clone(),
                attempt_id: request.attempt_id.clone(),
                executable: self.executable.clone(),
                executable_digest: ymp_domain::digest_bytes(&std::fs::read(&self.executable)?),
                coordination_executable: None,
                coordination_executable_digest: None,
                launch_chain: self.chain.clone(),
                arguments: vec!["-".to_owned()],
                environment: Vec::new(),
                working_directory: request.workspace.clone(),
            }))
        }

        fn start_prepared(
            &self,
            _request: InvocationRequest,
            _descriptor: Option<&ymp_runtime_api::LaunchDescriptor>,
        ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
            panic!("a launch without an admitted chain must never be started");
        }
    }

    /// Two ways a driver could reach the same launch through a program this account could have
    /// planted: admit no chain at all, or bind the chain to a digest the driver chose itself rather
    /// than to a location the account cannot write. Both are refused before the runtime starts.
    #[test]
    fn a_launch_chain_the_account_could_have_chosen_is_refused_before_the_runtime_starts() {
        for (label, chain) in [
            ("no admitted chain", Vec::new()),
            ("a chain the driver bound to its own digest", Vec::new()),
        ] {
            let temporary = tempfile::tempdir().expect("temporary directory");
            let source = temporary.path().join("source");
            std::fs::create_dir(&source).expect("source directory");
            std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
            let executable = temporary.path().join("chainless-runtime");
            executable_fixture(&executable, b"#!/bin/sh\nexit 0\n");
            let chain = if chain.is_empty() && label.starts_with("a chain") {
                let shell = temporary.path().join("chain-shell");
                executable_fixture(&shell, b"#!/bin/sh\nexec /bin/sh \"$@\"\n");
                vec![stated(ProgramRole::LaunchShell, shell)]
            } else {
                chain
            };
            let application = Arc::new(Mutex::new(
                Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                    .expect("create application"),
            ));
            let started = start_managed_candidate(
                Arc::clone(&application),
                Box::new(ChainlessRuntime { executable, chain }),
                ManagedCandidateRequest {
                    contract: ManagedContract {
                        contract_id: "contract-chainless".to_owned(),
                        contract_digest: "c".repeat(64),
                        source,
                        prompt: "finish".to_owned(),
                        capture_exclusions: Vec::new(),
                        verifier: None,
                    },
                    bridge_executable: std::env::current_exe().expect("current executable"),
                },
            );
            let Err(error) = started else {
                panic!("{label} must be refused");
            };
            let error = error.to_string();
            assert!(
                error.contains("did not admit the programs"),
                "{label}: {error}"
            );
            assert_eq!(
                application.lock().expect("application lock").state().status,
                RunStatus::InfrastructureError,
                "{label}"
            );
        }
    }

    /// The workspace program is resolved through `PATH` once and executed by the absolute path its
    /// digest was taken from, so a replacement between admission and use is refused.
    #[test]
    fn a_replaced_workspace_program_refuses_to_build_the_baseline() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let admitted = admit_workspace_program().expect("admit the workspace program");
        assert!(admitted.path.is_absolute());
        assert!(admitted.verify().is_ok());

        let workspace = temporary.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let stand_in = temporary.path().join("git-stand-in");
        executable_fixture(&stand_in, b"#!/bin/sh\nexit 0\n");
        let stand_in = stated(ProgramRole::Workspace, stand_in);
        initialize_private_git(&workspace, &stand_in).expect("an unchanged program is executed");
        executable_fixture(&stand_in.path, b"#!/bin/sh\nexit 1\n");
        let error = initialize_private_git(&workspace, &stand_in)
            .expect_err("a replaced workspace program is refused")
            .to_string();
        assert!(error.contains("changed after admission"), "{error}");
    }

    #[test]
    fn contract_loads_relative_source_and_rejects_parent_exclusion() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        let path = temporary.path().join("contract.json");
        std::fs::write(
            &path,
            br#"{"schema_version":1,"contract_id":"contract-1","source":"source","prompt":"make the requested change","capture_exclusions":["target"]}"#,
        )
        .expect("write contract");
        let contract = ManagedContract::load(&path).expect("load contract");
        assert_eq!(
            contract.source,
            source.canonicalize().expect("canonical source")
        );
        assert_eq!(contract.capture_exclusions, ["target"]);

        let program = temporary.path().join("verifier");
        std::fs::write(&program, b"fixture").expect("write verifier fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&program)
                .expect("verifier metadata")
                .permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&program, permissions).expect("verifier permissions");
        }
        let negative = temporary.path().join("negative");
        std::fs::create_dir(&negative).expect("negative control directory");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "contract_id": "contract-1",
                "source": "source",
                "prompt": "make the requested change",
                "capture_exclusions": ["target"],
                "verifier": {
                    "program": "verifier",
                    "arguments": ["check"],
                    "negative_control": "negative",
                    "oracle_digest": "a".repeat(64)
                }
            }))
            .expect("serialize verifier contract"),
        )
        .expect("write verifier contract");
        let contract = ManagedContract::load(&path).expect("load verifier contract");
        let verifier = contract.verifier.expect("verifier configuration");
        assert_eq!(verifier.program, program.canonicalize().expect("program"));
        assert_eq!(
            verifier.negative_control,
            negative.canonicalize().expect("negative")
        );
        assert_eq!(verifier.wall_time_ms, 60_000);

        std::fs::write(
            &path,
            br#"{"schema_version":1,"contract_id":"contract-1","source":"source","prompt":"make the requested change","capture_exclusions":["../escape"]}"#,
        )
        .expect("write invalid contract");
        assert!(ManagedContract::load(&path).is_err());
    }

    #[test]
    fn completed_runtime_without_controller_action_is_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let application = Arc::new(Mutex::new(
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let contract = ManagedContract {
            contract_id: "contract-1".to_owned(),
            contract_digest: "c".repeat(64),
            source,
            prompt: "finish".to_owned(),
            capture_exclusions: vec!["target".to_owned()],
            verifier: None,
        };
        let runtime = FakeRuntime::with_script(vec![ScriptStep::Complete(Usage::default())]);
        let handle = start_unattested_managed_candidate(
            Arc::clone(&application),
            Box::new(runtime),
            ManagedCandidateRequest {
                contract,
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_failed = false;
        let mut saw_candidate = false;
        while Instant::now() < deadline && !handle.is_finished() {
            while let Some(event) = handle.try_next() {
                match event {
                    ManagedRunEvent::Runtime(event)
                        if matches!(event.event, RuntimeEventKind::Failed { .. }) =>
                    {
                        saw_failed = true;
                    }
                    ManagedRunEvent::CandidateAvailable { .. } => saw_candidate = true,
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        while let Some(event) = handle.try_next() {
            if matches!(event, ManagedRunEvent::CandidateAvailable { .. }) {
                saw_candidate = true;
            }
        }
        assert!(handle.is_finished());
        assert!(saw_failed);
        assert!(!saw_candidate);
        assert_eq!(
            application.lock().expect("application lock").state().status,
            RunStatus::InfrastructureError
        );
        assert!(
            application
                .lock()
                .expect("application lock")
                .state()
                .candidate_digest
                .is_none()
        );
        let transcript = temporary
            .path()
            .join("data/runtime-evidence")
            .join(handle.attempt_id())
            .join("events.jsonl");
        let transcript = std::fs::read_to_string(&transcript).expect("runtime transcript");
        assert!(!transcript.contains(".opaque"));
        let records: Vec<serde_json::Value> = transcript
            .lines()
            .map(|line| serde_json::from_str(line).expect("runtime evidence record"))
            .collect();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["event"]["type"], "started");
        assert_eq!(records[1]["event"]["type"], "failed");
        assert_eq!(records[1]["predecessor_digest"], records[0]["digest"]);
        assert!(
            application
                .lock()
                .expect("application lock")
                .export_evidence(temporary.path().join("export"))
                .is_err()
        );
        handle.join().expect("join worker");
    }

    fn assert_invalid_progress_is_terminal(
        runtime: Box<dyn RuntimeDriver>,
        _expected_detail: &str,
        recorded_events: usize,
    ) {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let data_root = temporary.path().join("data");
        let application = Arc::new(Mutex::new(
            Application::create(&data_root, "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let contract = ManagedContract {
            contract_id: "contract-1".to_owned(),
            contract_digest: "c".repeat(64),
            source,
            prompt: "finish".to_owned(),
            capture_exclusions: vec!["target".to_owned()],
            verifier: None,
        };
        let handle = start_unattested_managed_candidate(
            Arc::clone(&application),
            runtime,
            ManagedCandidateRequest {
                contract,
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !handle.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut failure = None;
        let mut saw_candidate = false;
        while let Some(event) = handle.try_next() {
            match event {
                ManagedRunEvent::CandidateAvailable { .. } => saw_candidate = true,
                ManagedRunEvent::Failed { detail } => failure = Some(detail),
                _ => {}
            }
        }
        assert!(handle.is_finished());
        assert!(!saw_candidate);
        let failure = failure.expect("typed managed-run failure");
        assert_eq!(failure, "managed_runtime_supervision_failed");
        let state = application.lock().expect("application lock");
        assert_eq!(state.state().status, RunStatus::InfrastructureError);
        assert!(state.state().candidate_digest.is_none());
        assert!(state.state().active_attempts.is_empty());
        drop(state);
        let transcript = temporary
            .path()
            .join("data/runtime-evidence")
            .join(handle.attempt_id())
            .join("events.jsonl");
        assert_eq!(
            std::fs::read_to_string(transcript)
                .expect("runtime transcript")
                .lines()
                .count(),
            recorded_events
        );
        handle.join().expect("join worker");
        drop(application);
        let recovered = Application::open(data_root).expect("reopen terminal application");
        assert_eq!(recovered.state().status, RunStatus::InfrastructureError);
        assert!(recovered.state().candidate_digest.is_none());
        assert!(recovered.state().active_attempts.is_empty());
    }

    #[test]
    fn duplicate_runtime_progress_is_terminal_without_candidate() {
        assert_invalid_progress_is_terminal(
            Box::new(FakeRuntime::with_script(vec![
                ScriptStep::Output("duplicate me".to_owned()),
                ScriptStep::DuplicateLast,
                ScriptStep::Complete(Usage::default()),
            ])),
            "runtime repeated event_id",
            2,
        );
    }

    #[test]
    fn other_invalid_runtime_progress_is_terminal_without_candidate() {
        for (fault, expected_detail, recorded_events) in [
            (ProgressFault::Gap, "does not match expected sequence", 1),
            (
                ProgressFault::Reordered,
                "does not match expected sequence",
                2,
            ),
            (
                ProgressFault::RepeatedEventId,
                "runtime repeated event_id",
                1,
            ),
            (
                ProgressFault::EmptyEventId,
                "runtime event_id must contain",
                1,
            ),
            (
                ProgressFault::WrongInvocation,
                "runtime event invocation_id does not match",
                1,
            ),
        ] {
            assert_invalid_progress_is_terminal(
                Box::new(FaultingRuntime { fault }),
                expected_detail,
                recorded_events,
            );
        }
    }

    #[test]
    fn agent_submission_captures_only_the_bound_workspace() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let application = Arc::new(Mutex::new(
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let contract = ManagedContract {
            contract_id: "contract-1".to_owned(),
            contract_digest: "c".repeat(64),
            source,
            prompt: "finish and submit".to_owned(),
            capture_exclusions: vec!["target".to_owned()],
            verifier: None,
        };
        let runtime = SubmittingRuntime {
            inner: FakeRuntime::with_script(vec![ScriptStep::Complete(Usage::default())]),
        };
        let handle = start_unattested_managed_candidate(
            Arc::clone(&application),
            Box::new(runtime),
            ManagedCandidateRequest {
                contract,
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !handle.is_finished() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut agent_candidate = None;
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::CandidateAvailable {
                candidate_digest,
                change_count,
            } = event
            {
                assert_eq!(change_count, None);
                agent_candidate = Some(candidate_digest);
            }
        }
        let candidate_digest = agent_candidate.expect("agent-submitted candidate event");
        assert_eq!(
            application
                .lock()
                .expect("application lock")
                .state()
                .candidate_digest
                .as_deref(),
            Some(candidate_digest.as_str())
        );
        let materialized = temporary.path().join("materialized");
        application
            .lock()
            .expect("application lock")
            .artifact_store()
            .materialize(&candidate_digest, &materialized)
            .expect("materialize submitted candidate");
        assert_eq!(
            std::fs::read(materialized.join("input.txt")).expect("candidate file"),
            b"agent candidate\n"
        );
        handle.join().expect("join worker");
    }

    #[test]
    fn managed_handle_wakes_one_session_once_per_command_identity() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
        let application = Arc::new(Mutex::new(
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let handle = start_unattested_managed_candidate(
            Arc::clone(&application),
            Box::new(LifecycleRuntime {
                inner: FakeRuntime::default(),
            }),
            ManagedCandidateRequest {
                contract: ManagedContract {
                    contract_id: "contract-yield".to_owned(),
                    contract_digest: "d".repeat(64),
                    source,
                    prompt: "yield then finish".to_owned(),
                    capture_exclusions: Vec::new(),
                    verifier: None,
                },
                bridge_executable: std::env::current_exe().expect("current executable"),
            },
        )
        .expect("start managed candidate");
        let attempt_id = handle.attempt_id().to_owned();
        let invocation_id = handle.invocation_id().to_owned();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut session_id = None;
        let mut saw_yield = false;
        let mut resumed_outputs = 0;
        let mut completed = 0;
        while Instant::now() < deadline && !saw_yield {
            while let Some(event) = handle.try_next() {
                if let ManagedRunEvent::Runtime(event) = event {
                    assert_eq!(event.invocation_id, invocation_id);
                    match event.event {
                        RuntimeEventKind::Started { opaque_session_id } => {
                            session_id = Some(opaque_session_id)
                        }
                        RuntimeEventKind::Yielded { .. } => saw_yield = true,
                        _ => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(saw_yield, "managed runtime did not yield");
        handle
            .wake("wake-command-1", "resume once")
            .expect("first wake");
        handle
            .wake("wake-command-1", "resume once")
            .expect("idempotent repeated wake");
        assert!(handle.wake("wake-command-1", "different input").is_err());

        while Instant::now() < deadline && !handle.is_finished() {
            while let Some(event) = handle.try_next() {
                if let ManagedRunEvent::Runtime(event) = event {
                    assert_eq!(event.invocation_id, invocation_id);
                    match event.event {
                        RuntimeEventKind::Output { text } if text == "fake runtime resumed" => {
                            resumed_outputs += 1;
                        }
                        RuntimeEventKind::Completed { .. } => completed += 1,
                        _ => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        while let Some(event) = handle.try_next() {
            if let ManagedRunEvent::Runtime(event) = event {
                assert_eq!(event.invocation_id, invocation_id);
                match event.event {
                    RuntimeEventKind::Output { text } if text == "fake runtime resumed" => {
                        resumed_outputs += 1;
                    }
                    RuntimeEventKind::Completed { .. } => completed += 1,
                    _ => {}
                }
            }
        }
        assert!(handle.is_finished());
        assert_eq!(resumed_outputs, 1);
        assert_eq!(completed, 1);
        assert_eq!(session_id, Some(format!("{attempt_id}.session")));
        assert_eq!(attempt_id, handle.attempt_id());
        let evidence = std::fs::read_to_string(
            temporary
                .path()
                .join("data/runtime-evidence")
                .join(&attempt_id)
                .join("events.jsonl"),
        )
        .expect("runtime evidence");
        for record in evidence.lines() {
            let record: serde_json::Value =
                serde_json::from_str(record).expect("runtime evidence record");
            assert_eq!(record["invocation_id"], invocation_id);
        }
        handle.join().expect("join worker");
    }

    /// A controller whose wait ran out over a slice whose committed state cannot be read moves the
    /// journal off running and names the reading that failed as the reason.
    ///
    /// The committed facts of a run are read through one lock, and a lock a panic poisoned stays
    /// poisoned: a state that cannot be read now will not be judged, closed or advanced later
    /// either. Recording a kernel terminal on it would rename one the worker may already have
    /// committed, so nothing is written there — but a journal left on running shows an operator a
    /// run in progress that nothing can move, which is what this used to leave behind.
    ///
    /// The reading is made to fail here by giving the controller a kernel whose slice was never
    /// started, because the lock a poison would have to be taken on belongs to the service that
    /// owns the ledger and cannot be reached from outside it. What the shutdown decides on is a
    /// reading of the committed state that failed, which is what this produces, and not the cause
    /// behind the failure, which it does not reproduce.
    ///
    /// The check that must fail: leave the journal alone where the state could not be read. The
    /// journal then carries no record of the reading that failed at all.
    #[test]
    fn a_wait_that_ran_out_over_an_unreadable_slice_fails_the_journal_with_its_reason() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let application = Arc::new(Mutex::new(
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let kernel = ManagedKernel::prepare(
            Arc::clone(&application),
            "attempt-unread",
            "invocation-unread",
            "scope-unread",
            &digest_bytes(b"base"),
            &digest_bytes(b"intent"),
        )
        .expect("prepare the kernel record");
        assert!(
            kernel.invocation_state().is_err(),
            "the state of this slice could be read, so the check measures nothing"
        );

        // A worker that outlasts the wait, which is the only state this path is reached from. It
        // holds the receiving end of the control channel, so releasing that channel ends nothing.
        let (_sender, receiver) = channel();
        let (control_sender, control_receiver) = channel();
        let released = Arc::new(AtomicBool::new(false));
        let holding = Arc::clone(&released);
        let worker = std::thread::Builder::new()
            .spawn(move || {
                let _control_receiver = control_receiver;
                let deadline = Instant::now() + CONTROLLER_SHUTDOWN_LIMIT * 4;
                while Instant::now() < deadline && !holding.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(5));
                }
            })
            .expect("the worker that outlasts the wait");
        let handle = ManagedRunHandle {
            attempt_id: "attempt-unread".to_owned(),
            invocation_id: "invocation-unread".to_owned(),
            runtime_kind: RuntimeKind::Fake,
            cancellation: CancellationToken::default(),
            application: Arc::clone(&application),
            receiver: Mutex::new(receiver),
            control_sender: Some(control_sender),
            kernel: Arc::new(kernel),
            terminal: Arc::new(Mutex::new(())),
            finished: Arc::new(AtomicBool::new(false)),
            worker: Some(worker),
        };

        let report = handle
            .join()
            .expect_err("a controller that stopped waiting reported success")
            .to_string();
        released.store(true, Ordering::Release);

        assert!(
            report.contains("could not be read"),
            "the controller reported something other than what it found: {report}"
        );
        assert!(
            report.contains("no recorded process slice"),
            "the report carries no reason for the reading that failed: {report}"
        );
        assert_eq!(
            application.lock().expect("application").state().status,
            RunStatus::InfrastructureError,
            "the journal still calls a run running that nothing can move"
        );
    }
}
