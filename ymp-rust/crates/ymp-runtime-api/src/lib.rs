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
    #[serde(default)]
    pub coordination_executable: Option<PathBuf>,
    #[serde(default)]
    pub coordination_executable_digest: Option<String>,
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
        descriptor: Box<LaunchDescriptor>,
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
        // Signalling a process group cannot reach a descendant that leaves that group, and once
        // its own parent exits nothing in the live process table still links it to the run. The
        // observer therefore starts before the managed process does and keeps a record of every
        // descendant while its parent is still visible.
        descendants::start_observer();
    }
}

/// Records which managed process every descendant of this process belongs to, so that a descendant
/// which creates its own session and is later reparented to init can still be identified and
/// terminated with the run that started it.
#[cfg(unix)]
mod descendants {
    use std::collections::HashMap;
    use std::process::{Command, Stdio};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    /// How often the process table is read while a managed process is supervised.
    const OBSERVATION_INTERVAL: Duration = Duration::from_millis(100);
    /// Consecutive empty readings after which the observer stops. The next managed launch starts it
    /// again, so a foreground process that supervises nothing reads nothing.
    const IDLE_OBSERVATIONS_BEFORE_STOP: u32 = 50;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ProcessEntry {
        pid: u32,
        parent: u32,
        group: u32,
        /// The start time the operating system reports. It distinguishes the observed process from
        /// an unrelated one that later reuses the same process identifier.
        started: String,
    }

    #[derive(Clone, Debug)]
    struct Attribution {
        root: u32,
        started: String,
    }

    #[derive(Default)]
    struct Forest {
        members: HashMap<u32, Attribution>,
        observing: bool,
    }

    fn shared_forest() -> &'static Mutex<Forest> {
        static FOREST: OnceLock<Mutex<Forest>> = OnceLock::new();
        FOREST.get_or_init(|| Mutex::new(Forest::default()))
    }

    fn read_process_table() -> Option<Vec<ProcessEntry>> {
        let output = Command::new("/bin/ps")
            .args(["-A", "-o", "pid=,ppid=,pgid=,lstart="])
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            let (Some(pid), Some(parent), Some(group)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let (Ok(pid), Ok(parent), Ok(group)) = (pid.parse(), parent.parse(), group.parse())
            else {
                continue;
            };
            let started = fields.collect::<Vec<_>>().join(" ");
            if started.is_empty() {
                continue;
            }
            entries.push(ProcessEntry {
                pid,
                parent,
                group,
                started,
            });
        }
        (!entries.is_empty()).then_some(entries)
    }

    /// Attributes every live descendant of this process to the managed process it came from. A
    /// process keeps the attribution it was first given for as long as it stays alive, which is what
    /// survives the exit of its own parent; dead processes are dropped, so the record stays the size
    /// of the live tree.
    fn absorb(members: &mut HashMap<u32, Attribution>, snapshot: &[ProcessEntry]) {
        let this = std::process::id();
        let by_pid: HashMap<u32, &ProcessEntry> =
            snapshot.iter().map(|entry| (entry.pid, entry)).collect();
        let mut attributed: HashMap<u32, u32> = HashMap::new();
        for entry in snapshot {
            if let Some(previous) = members.get(&entry.pid)
                && previous.started == entry.started
            {
                attributed.insert(entry.pid, previous.root);
            }
        }
        // A managed process is a direct child of this process that leads its own process group,
        // which is what launching it through `configure_process_group` makes it. Ordinary helper
        // processes — reading the process table, sending a signal, running Git — are also direct
        // children but inherit this process's group, so they start no tree and the record stays
        // empty while nothing is supervised.
        for entry in snapshot {
            if entry.parent == this && entry.pid > 1 && entry.group == entry.pid {
                attributed.insert(entry.pid, entry.pid);
            }
        }
        loop {
            // A process group extends the attribution only through its own leader. A managed
            // process is a group leader because it was launched into a new group, whereas an
            // unmanaged sibling merely inherits this process's group and must not drag that whole
            // group into the run.
            let mut led_groups: HashMap<u32, u32> = HashMap::new();
            for (pid, root) in &attributed {
                if by_pid.get(pid).is_some_and(|entry| entry.group == *pid) {
                    led_groups.entry(*pid).or_insert(*root);
                }
            }
            let mut added = false;
            for entry in snapshot {
                if entry.pid <= 1 || entry.pid == this || attributed.contains_key(&entry.pid) {
                    continue;
                }
                let Some(root) = attributed
                    .get(&entry.parent)
                    .or_else(|| led_groups.get(&entry.group))
                    .copied()
                else {
                    continue;
                };
                attributed.insert(entry.pid, root);
                added = true;
            }
            if !added {
                break;
            }
        }
        members.clear();
        for (pid, root) in attributed {
            let Some(entry) = by_pid.get(&pid) else {
                continue;
            };
            members.insert(
                pid,
                Attribution {
                    root,
                    started: entry.started.clone(),
                },
            );
        }
    }

    pub fn start_observer() {
        let Ok(mut forest) = shared_forest().lock() else {
            return;
        };
        if forest.observing {
            return;
        }
        forest.observing = true;
        drop(forest);
        let spawned = std::thread::Builder::new()
            .name("ymp-descendant-observer".to_owned())
            .spawn(|| {
                let mut idle = 0u32;
                loop {
                    if let Some(snapshot) = read_process_table() {
                        let Ok(mut forest) = shared_forest().lock() else {
                            return;
                        };
                        absorb(&mut forest.members, &snapshot);
                        if forest.members.is_empty() {
                            idle = idle.saturating_add(1);
                            if idle >= IDLE_OBSERVATIONS_BEFORE_STOP {
                                forest.observing = false;
                                return;
                            }
                        } else {
                            idle = 0;
                        }
                    }
                    std::thread::sleep(OBSERVATION_INTERVAL);
                }
            });
        if spawned.is_err()
            && let Ok(mut forest) = shared_forest().lock()
        {
            forest.observing = false;
        }
    }

    /// Reads the process table once and returns every process still alive that belongs to `root`,
    /// including `root` itself. Reading first means a descendant created since the last observation
    /// is attributed before it is signalled.
    pub fn survivors(root: u32) -> Vec<u32> {
        let Some(snapshot) = read_process_table() else {
            return Vec::new();
        };
        let Ok(mut forest) = shared_forest().lock() else {
            return Vec::new();
        };
        absorb(&mut forest.members, &snapshot);
        let mut live: Vec<u32> = forest
            .members
            .iter()
            .filter(|(pid, attribution)| attribution.root == root && **pid > 1)
            .map(|(pid, _)| *pid)
            .collect();
        live.sort_unstable();
        live
    }

    pub fn forget(root: u32) {
        let Ok(mut forest) = shared_forest().lock() else {
            return;
        };
        forest
            .members
            .retain(|_, attribution| attribution.root != root);
    }

    /// Sends `signal` to the managed process group and to every attributed process individually.
    /// The group signal reaches members created since the reading; the individual signals reach the
    /// members that left the group.
    pub fn signal(root: u32, group: &str, signal: &str, individual: &[u32]) -> std::io::Result<()> {
        let status = Command::new("/bin/kill")
            .args([signal, group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let mut targets: Vec<String> = individual
            .iter()
            .filter(|pid| **pid > 1 && **pid != root && **pid != std::process::id())
            .map(u32::to_string)
            .collect();
        targets.sort();
        targets.dedup();
        if !targets.is_empty() {
            let mut arguments: Vec<&str> = vec![signal];
            arguments.extend(targets.iter().map(String::as_str));
            let _ = Command::new("/bin/kill")
                .args(arguments)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        status.map(|_| ())
    }
}

#[doc(hidden)]
#[cfg(unix)]
pub fn terminate_process_tree(child: &mut Child) -> std::io::Result<()> {
    let root = child.id();
    let group = format!("-{root}");
    let mut parent_reaped = child.try_wait()?.is_some();
    let _ = descendants::signal(root, &group, "-TERM", &descendants::survivors(root));
    for _ in 0..20 {
        parent_reaped |= child.try_wait()?.is_some();
        let remaining = descendants::survivors(root);
        if remaining.is_empty() && !process_group_exists(&group) {
            if !parent_reaped {
                let _ = child.wait()?;
            }
            descendants::forget(root);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let outcome = descendants::signal(root, &group, "-KILL", &descendants::survivors(root));
    if !parent_reaped {
        let _ = child.wait()?;
    }
    for _ in 0..100 {
        if descendants::survivors(root).is_empty() && !process_group_exists(&group) {
            descendants::forget(root);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let remaining = descendants::survivors(root).len();
    descendants::forget(root);
    match outcome {
        Ok(()) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("{remaining} managed process(es) and process group {group} survived SIGKILL"),
        )),
        Err(error) => Err(std::io::Error::other(format!(
            "failed to send SIGKILL to the managed process tree of {root}: {error}"
        ))),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{configure_process_group, terminate_process_tree};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant, SystemTime};

    fn unique_pid_file(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ymp-runtime-api-{label}-{}-{unique}.pid",
            std::process::id()
        ))
    }

    /// A shell cannot create a session, so the detaching descendant is written in whichever stock
    /// interpreter exposes `setsid`. The absence of all of them fails the test rather than skipping
    /// it, because a silent skip would report the escape as closed without measuring it.
    fn session_detaching_command() -> (&'static str, &'static str, &'static str) {
        detaching_command(false)
    }

    fn detaching_command(ignore_term: bool) -> (&'static str, &'static str, &'static str) {
        const PERL: &str = "use POSIX; POSIX::setsid() or die 'setsid'; open(my $handle, '>', $ARGV[0]) or die 'pid file'; print $handle \"$$\\n\"; close $handle; sleep 120;";
        const PERL_IGNORING_TERM: &str = "use POSIX; POSIX::setsid() or die 'setsid'; $SIG{TERM} = 'IGNORE'; open(my $handle, '>', $ARGV[0]) or die 'pid file'; print $handle \"$$\\n\"; close $handle; sleep 120;";
        const PYTHON: &str = "import os, sys, time; os.setsid(); open(sys.argv[1], 'w').write(str(os.getpid()) + '\\n'); time.sleep(120)";
        const PYTHON_IGNORING_TERM: &str = "import os, signal, sys, time; os.setsid(); signal.signal(signal.SIGTERM, signal.SIG_IGN); open(sys.argv[1], 'w').write(str(os.getpid()) + '\\n'); time.sleep(120)";
        let candidates: [(&'static str, &'static str, &'static str); 2] = [
            (
                "/usr/bin/perl",
                "-e",
                if ignore_term {
                    PERL_IGNORING_TERM
                } else {
                    PERL
                },
            ),
            (
                "/usr/bin/python3",
                "-c",
                if ignore_term {
                    PYTHON_IGNORING_TERM
                } else {
                    PYTHON
                },
            ),
        ];
        candidates
            .into_iter()
            .find(|(program, _, _)| Path::new(program).is_file())
            .expect("a stock interpreter that can call setsid")
    }

    fn read_pid(path: &Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(text) = fs::read_to_string(path)
                && let Ok(pid) = text.trim().parse()
            {
                return pid;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "descendant never recorded its process identifier in {}",
            path.display()
        );
    }

    fn is_alive(pid: u32) -> bool {
        Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn parent_of(pid: u32) -> Option<u32> {
        let output = Command::new("/bin/ps")
            .args(["-o", "ppid=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    /// The measured escape: a managed descendant creates its own session, so it leaves the process
    /// group the supervisor signals, and it is reparented to init when its own parent exits.
    #[test]
    fn termination_reaches_a_descendant_that_created_its_own_session() {
        let pid_file = unique_pid_file("session-escape");
        let (interpreter, flag, script) = session_detaching_command();
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(
                "\"$YMP_DETACH_INTERPRETER\" \"$YMP_DETACH_FLAG\" \"$YMP_DETACH_SCRIPT\" \
                 \"$YMP_DESCENDANT_PID_FILE\" & sleep 0.4; exit 19",
            )
            .env("YMP_DETACH_INTERPRETER", interpreter)
            .env("YMP_DETACH_FLAG", flag)
            .env("YMP_DETACH_SCRIPT", script)
            .env("YMP_DESCENDANT_PID_FILE", &pid_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_process_group(&mut command);
        let mut child = command.spawn().expect("spawn managed parent");
        let descendant = read_pid(&pid_file);
        let status = child.wait().expect("wait for managed parent");
        assert_eq!(status.code(), Some(19));
        assert!(
            is_alive(descendant),
            "the detached descendant exited before the escape could be measured"
        );
        assert_eq!(
            parent_of(descendant),
            Some(1),
            "the detached descendant was not reparented to init"
        );

        let terminated = terminate_process_tree(&mut child);

        let mut alive = true;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            alive = is_alive(descendant);
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if alive {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &descendant.to_string()])
                .status();
        }
        let _ = fs::remove_file(&pid_file);
        assert!(
            !alive,
            "a descendant that created its own session survived the supervisor"
        );
        terminated.expect("terminate the managed process tree");
    }

    /// Termination escalates and stays bounded. A descendant that ignores SIGTERM and has already
    /// left both the process group and the parent chain is still gone, and the call returns well
    /// inside the supervisor's own escalation bound rather than waiting for the process to end.
    #[test]
    fn termination_of_a_signal_ignoring_detached_descendant_is_bounded() {
        let pid_file = unique_pid_file("bounded-escalation");
        let (interpreter, flag, script) = detaching_command(true);
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(
                "\"$YMP_DETACH_INTERPRETER\" \"$YMP_DETACH_FLAG\" \"$YMP_DETACH_SCRIPT\" \
                 \"$YMP_DESCENDANT_PID_FILE\" & \
                 while [ ! -s \"$YMP_DESCENDANT_PID_FILE\" ]; do sleep 0.05; done; \
                 sleep 0.4; exit 23",
            )
            .env("YMP_DETACH_INTERPRETER", interpreter)
            .env("YMP_DETACH_FLAG", flag)
            .env("YMP_DETACH_SCRIPT", script)
            .env("YMP_DESCENDANT_PID_FILE", &pid_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_process_group(&mut command);
        let mut child = command.spawn().expect("spawn managed parent");
        let descendant = read_pid(&pid_file);
        let status = child.wait().expect("wait for managed parent");
        assert_eq!(status.code(), Some(23));
        assert_eq!(parent_of(descendant), Some(1));

        let started = Instant::now();
        let terminated = terminate_process_tree(&mut child);
        let elapsed = started.elapsed();

        let mut alive = true;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            alive = is_alive(descendant);
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if alive {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &descendant.to_string()])
                .status();
        }
        let _ = fs::remove_file(&pid_file);
        assert!(!alive, "the detached descendant survived the supervisor");
        terminated.expect("terminate the managed process tree");
        assert!(
            elapsed < Duration::from_secs(5),
            "termination took {elapsed:?}, which is outside its escalation bound"
        );
    }

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
