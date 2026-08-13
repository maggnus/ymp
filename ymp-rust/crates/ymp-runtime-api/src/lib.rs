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

/// The part a program plays when a managed run starts. The role is recorded beside the digest so
/// that the evidence names what each admitted program does rather than only where it was read from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramRole {
    /// Opens the run's marker on an inherited descriptor and then replaces itself with the next
    /// program in the chain.
    LaunchShell,
    /// Removes the variables the shell introduced of its own accord, then replaces itself with the
    /// runtime executable.
    EnvironmentSanitiser,
    /// Builds the private baseline of the managed workspace before the runtime is started.
    Workspace,
}

impl std::fmt::Display for ProgramRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::LaunchShell => "launch shell",
            Self::EnvironmentSanitiser => "environment sanitiser",
            Self::Workspace => "workspace program",
        };
        formatter.write_str(name)
    }
}

/// A program whose bytes were read and digested before it was executed on a managed run's behalf.
///
/// The pinned runtime executable and the coordination bridge are admitted by copy, so the bytes
/// that were digested are the bytes the operating system loads. A system program cannot be admitted
/// that way — a copy of a platform binary is refused by the operating system — so it is admitted by
/// digest at its own path and re-verified immediately before and after the managed process is
/// created. The residual window between the last verification and the kernel's image load is stated
/// in the card that introduced this record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdmittedProgram {
    pub role: ProgramRole,
    pub path: PathBuf,
    pub digest: String,
}

impl AdmittedProgram {
    /// Reads the program's bytes and records their digest. This is the admission the later
    /// verifications compare against.
    pub fn admit(role: ProgramRole, path: impl Into<PathBuf>) -> Result<Self, RuntimeError> {
        let path = path.into();
        if !path.is_file() {
            return Err(RuntimeError::InvalidProfile(format!(
                "{role} is not a regular file: {}",
                path.display()
            )));
        }
        let digest = digest_bytes(&std::fs::read(&path)?);
        Ok(Self { role, path, digest })
    }

    /// Reads the program's bytes again and refuses when they differ from the admitted ones.
    pub fn verify(&self) -> Result<(), RuntimeError> {
        if !self.path.is_file() {
            return Err(RuntimeError::InvalidProfile(format!(
                "{} is no longer a regular file: {}",
                self.role,
                self.path.display()
            )));
        }
        if digest_bytes(&std::fs::read(&self.path)?) != self.digest {
            return Err(RuntimeError::InvalidProfile(format!(
                "{} changed after admission: {}",
                self.role,
                self.path.display()
            )));
        }
        Ok(())
    }
}

/// Refuses unless every admitted program still holds the bytes it was admitted with.
pub fn verify_admitted_programs(programs: &[AdmittedProgram]) -> Result<(), RuntimeError> {
    for program in programs {
        program.verify()?;
    }
    Ok(())
}

/// The programs the managed launch preamble enters before the runtime image is loaded. Product code
/// uses the system paths; the constructor exists because a test cannot substitute `/bin/sh` and the
/// operating system refuses to execute a copy of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchChain {
    shell: PathBuf,
    sanitiser: Option<PathBuf>,
}

impl LaunchChain {
    pub const SYSTEM_SHELL: &'static str = "/bin/sh";
    pub const SYSTEM_SANITISER: &'static str = "/usr/bin/env";

    pub fn new(shell: impl Into<PathBuf>, sanitiser: Option<PathBuf>) -> Self {
        Self {
            shell: shell.into(),
            sanitiser,
        }
    }

    /// Records the digest of every program in the chain. The set is fixed here rather than at spawn
    /// time, so a sanitiser that appears or disappears between admission and launch changes the
    /// chain and is refused instead of silently entering or leaving it.
    pub fn admit(&self) -> Result<Vec<AdmittedProgram>, RuntimeError> {
        if !cfg!(unix) {
            return Ok(Vec::new());
        }
        let mut chain = vec![AdmittedProgram::admit(
            ProgramRole::LaunchShell,
            self.shell.clone(),
        )?];
        if let Some(sanitiser) = &self.sanitiser {
            chain.push(AdmittedProgram::admit(
                ProgramRole::EnvironmentSanitiser,
                sanitiser.clone(),
            )?);
        }
        Ok(chain)
    }
}

impl Default for LaunchChain {
    fn default() -> Self {
        let sanitiser = Path::new(Self::SYSTEM_SANITISER);
        Self {
            shell: PathBuf::from(Self::SYSTEM_SHELL),
            sanitiser: sanitiser.is_file().then(|| sanitiser.to_owned()),
        }
    }
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
    /// The programs the launch preamble enters before the runtime image is loaded, in the order it
    /// enters them.
    #[serde(default)]
    pub launch_chain: Vec<AdmittedProgram>,
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

/// Creates the file whose open descriptor marks a managed run. It is created outside any directory
/// the managed process is given, under a name no other run uses, and is readable only by this user.
#[doc(hidden)]
pub fn create_launch_marker() -> std::io::Result<PathBuf> {
    use std::sync::atomic::AtomicU64;
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join("ymp-runtime");
    std::fs::create_dir_all(&directory)?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| std::io::Error::other("system clock is before the epoch"))?
        .as_nanos();
    let path = directory.join(format!(
        "launch-{}-{unique}-{}.marker",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&path)?;
    Ok(path)
}

/// Builds the command that starts a managed process so that the process, and every descendant it
/// ever creates, holds the run's marker open.
///
/// The marker is opened on a descriptor above the standard three and is not close-on-exec, so it is
/// inherited across every fork and preserved across every exec. Unlike a process group, a session,
/// a parent link or a working directory, it is unaffected by `setsid`, by any number of intermediate
/// processes exiting, by reparenting to init, by changing directory, and by redirecting or closing
/// the standard descriptors. Holding it therefore identifies a process the run started even when
/// nothing the operating system reports still connects that process to the run.
///
/// The preamble is entered through the shell only to open that descriptor; it then replaces itself
/// with the program the caller named, so the process that runs, its arguments and its identifier are
/// the ones the launch descriptor attests. The variables the shell introduces of its own accord are
/// removed again, so the managed process still sees exactly the environment the driver declared.
///
/// The shell and the sanitiser are taken from the admitted chain rather than chosen here, so the
/// programs that execute are exactly the programs whose digests were recorded. The caller verifies
/// that chain immediately before and after the managed process is created.
#[doc(hidden)]
pub fn managed_launch_command(
    program: &Path,
    arguments: &[String],
    marker: &Path,
    chain: &[AdmittedProgram],
) -> Result<Command, RuntimeError> {
    #[cfg(unix)]
    {
        const OPEN_MARKER: &str = "exec 9<\"$1\"; shift; exec ";
        let shell = match chain.first() {
            Some(shell) if shell.role == ProgramRole::LaunchShell => &shell.path,
            _ => {
                return Err(RuntimeError::InvalidProfile(
                    "managed launch chain does not begin with an admitted shell".to_owned(),
                ));
            }
        };
        let preamble = match chain.get(1) {
            Some(sanitiser) if sanitiser.role == ProgramRole::EnvironmentSanitiser => {
                let sanitiser = shell_quoted(&sanitiser.path)?;
                format!("{OPEN_MARKER}{sanitiser} -u PWD -u SHLVL -u OLDPWD -u _ \"$@\"")
            }
            None => format!("{OPEN_MARKER}\"$@\""),
            Some(_) => {
                return Err(RuntimeError::InvalidProfile(
                    "managed launch chain records an unexpected program".to_owned(),
                ));
            }
        };
        if chain.len() > 2 {
            return Err(RuntimeError::InvalidProfile(
                "managed launch chain records more programs than the preamble enters".to_owned(),
            ));
        }
        let mut command = Command::new(shell);
        command
            .arg("-c")
            .arg(preamble)
            .arg("ymp-managed-launch")
            .arg(marker)
            .arg(program)
            .args(arguments);
        Ok(command)
    }
    #[cfg(not(unix))]
    {
        let _ = marker;
        if !chain.is_empty() {
            return Err(RuntimeError::InvalidProfile(
                "this platform enters no launch chain".to_owned(),
            ));
        }
        let mut command = Command::new(program);
        command.args(arguments);
        Ok(command)
    }
}

/// Renders a path as a single shell word, so a chain program whose path contains a shell
/// metacharacter cannot extend the preamble.
#[cfg(unix)]
fn shell_quoted(path: &Path) -> Result<String, RuntimeError> {
    let path = path.to_str().ok_or_else(|| {
        RuntimeError::InvalidProfile(format!(
            "launch chain program path is not UTF-8: {}",
            path.display()
        ))
    })?;
    Ok(format!("'{}'", path.replace('\'', r"'\''")))
}

/// Binds a marker to the managed process that was started with it, so that terminating that process
/// can find every descendant still holding the marker open.
#[doc(hidden)]
#[cfg(unix)]
pub fn register_launch_marker(child: &Child, marker: PathBuf) {
    descendants::remember_marker(child.id(), marker);
}

#[doc(hidden)]
#[cfg(not(unix))]
pub fn register_launch_marker(_child: &Child, _marker: PathBuf) {}

/// Identifies the processes a managed run started. The authoritative property is the run's marker,
/// which every descendant carries whatever becomes of its ancestors; the process table is read as
/// well, so a descendant still attached by parent or process group is found even when it was started
/// by a caller that installed no marker.
#[cfg(unix)]
mod descendants {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
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
        markers: HashMap<u32, PathBuf>,
        observing: bool,
    }

    pub fn remember_marker(root: u32, marker: PathBuf) {
        if let Ok(mut forest) = shared_forest().lock() {
            forest.markers.insert(root, marker);
        }
    }

    /// Reads which live processes hold the run's marker open. This is the answer to the ownership
    /// question that does not depend on any ancestor still existing.
    fn holders_of(marker: &Path) -> Vec<u32> {
        // Linux reports open descriptors in its own process file system, so no external program is
        // needed there.
        if Path::new("/proc/self/fd").is_dir() {
            return proc_holders(marker);
        }
        for program in ["/usr/sbin/lsof", "/usr/bin/lsof"] {
            if !Path::new(program).is_file() {
                continue;
            }
            let Ok(output) = Command::new(program)
                .arg("-t")
                .arg(marker)
                .stderr(Stdio::null())
                .output()
            else {
                continue;
            };
            return String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.trim().parse().ok())
                .collect();
        }
        Vec::new()
    }

    fn proc_holders(marker: &Path) -> Vec<u32> {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        let mut holders = Vec::new();
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(descriptors) = std::fs::read_dir(entry.path().join("fd")) else {
                continue;
            };
            if descriptors.flatten().any(|descriptor| {
                std::fs::read_link(descriptor.path()).is_ok_and(|to| to == marker)
            }) {
                holders.push(pid);
            }
        }
        holders
    }

    /// Attributes every current holder of the run's marker to that run. Reading open descriptors is
    /// far more expensive than reading the process table, so it is done at the points where the
    /// answer is acted upon rather than on every wait.
    pub fn absorb_marker_holders(root: u32) {
        let Some(marker) = shared_forest()
            .lock()
            .ok()
            .and_then(|forest| forest.markers.get(&root).cloned())
        else {
            return;
        };
        let holders = holders_of(&marker);
        if holders.is_empty() {
            return;
        }
        let Some(snapshot) = read_process_table() else {
            return;
        };
        let Ok(mut forest) = shared_forest().lock() else {
            return;
        };
        for pid in holders {
            if pid <= 1 || pid == std::process::id() {
                continue;
            }
            if let Some(entry) = snapshot.iter().find(|entry| entry.pid == pid) {
                forest.members.insert(
                    pid,
                    Attribution {
                        root,
                        started: entry.started.clone(),
                    },
                );
            }
        }
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
        if let Some(marker) = forest.markers.remove(&root) {
            let _ = std::fs::remove_file(marker);
        }
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
    // Ask the operating system who holds the run's marker open before each signal is sent. Between
    // the two questions the cheaper reading of the process table is enough: a holder found here
    // stays attributed until it dies, and anything it starts afterwards is its child.
    descendants::absorb_marker_holders(root);
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
    descendants::absorb_marker_holders(root);
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
    use super::{
        LaunchChain, configure_process_group, create_launch_marker, managed_launch_command,
        register_launch_marker, terminate_process_tree,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
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

    /// The reviewer's case: ordinary daemonisation. The first fork's parent returns at once, the
    /// second fork's parent exits at once, and the surviving grandchild owns a session, has been
    /// reparented to init and has redirected its standard descriptors, all within microseconds.
    fn double_forking_command() -> (&'static str, &'static str, &'static str) {
        const PERL: &str = "use POSIX; exit 0 if fork(); POSIX::setsid() or die 'setsid'; exit 0 if fork(); open(my $handle, '>', $ARGV[0]) or die 'pid file'; print $handle \"$$\\n\"; close $handle; sleep 120;";
        const PYTHON: &str = "import os, sys, time\nif os.fork(): raise SystemExit(0)\nos.setsid()\nif os.fork(): raise SystemExit(0)\nopen(sys.argv[1], 'w').write(str(os.getpid()) + '\\n')\ntime.sleep(120)";
        let candidates: [(&'static str, &'static str, &'static str); 2] = [
            ("/usr/bin/perl", "-e", PERL),
            ("/usr/bin/python3", "-c", PYTHON),
        ];
        candidates
            .into_iter()
            .find(|(program, _, _)| Path::new(program).is_file())
            .expect("a stock interpreter that can call setsid")
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

    /// Starts a shell script the way the runtime drivers start a managed process: through the
    /// managed launch, so the run's marker is inherited, and in its own process group.
    fn spawn_managed(script: &str, environment: &[(&str, &str)]) -> Child {
        let marker = create_launch_marker().expect("create the run marker");
        let chain = LaunchChain::default()
            .admit()
            .expect("admit the launch chain");
        let mut command = managed_launch_command(
            Path::new("/bin/sh"),
            &["-c".to_owned(), script.to_owned()],
            &marker,
            &chain,
        )
        .expect("build the managed launch command");
        for (name, value) in environment {
            command.env(name, value);
        }
        command.stdout(Stdio::null()).stderr(Stdio::null());
        configure_process_group(&mut command);
        let child = command.spawn().expect("spawn the managed process");
        register_launch_marker(&child, marker);
        child
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

    /// Ordinary daemonisation. Both intermediate processes are gone before any reading of the
    /// process table could have linked the survivor to the run, so nothing the operating system
    /// still reports connects it to the managed process. Ownership therefore has to come from a
    /// property the survivor itself carries.
    #[test]
    fn termination_reaches_a_double_forked_descendant_orphaned_at_once() {
        let pid_file = unique_pid_file("double-fork");
        let (interpreter, flag, script) = double_forking_command();
        let mut child = spawn_managed(
            "\"$YMP_DETACH_INTERPRETER\" \"$YMP_DETACH_FLAG\" \"$YMP_DETACH_SCRIPT\" \
             \"$YMP_DESCENDANT_PID_FILE\" </dev/null >/dev/null 2>&1; \
             while [ ! -s \"$YMP_DESCENDANT_PID_FILE\" ]; do sleep 0.05; done; exit 29",
            &[
                ("YMP_DETACH_INTERPRETER", interpreter),
                ("YMP_DETACH_FLAG", flag),
                ("YMP_DETACH_SCRIPT", script),
                ("YMP_DESCENDANT_PID_FILE", &pid_file.display().to_string()),
            ],
        );
        let descendant = read_pid(&pid_file);
        let status = child.wait().expect("wait for managed parent");
        assert_eq!(status.code(), Some(29));
        assert!(
            is_alive(descendant),
            "the daemonised descendant exited before the escape could be measured"
        );
        assert_eq!(
            parent_of(descendant),
            Some(1),
            "the daemonised descendant was not reparented to init"
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
            "a descendant orphaned by a double fork survived the supervisor"
        );
        terminated.expect("terminate the managed process tree");
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
        let mut child = spawn_managed(
            "\"$YMP_DETACH_INTERPRETER\" \"$YMP_DETACH_FLAG\" \"$YMP_DETACH_SCRIPT\" \
             \"$YMP_DESCENDANT_PID_FILE\" & \
             while [ ! -s \"$YMP_DESCENDANT_PID_FILE\" ]; do sleep 0.05; done; \
             sleep 0.4; exit 23",
            &[
                ("YMP_DETACH_INTERPRETER", interpreter),
                ("YMP_DETACH_FLAG", flag),
                ("YMP_DETACH_SCRIPT", script),
                ("YMP_DESCENDANT_PID_FILE", &pid_file.display().to_string()),
            ],
        );
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
