use std::error::Error;
use std::fmt;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use ymp_domain::{Check, CheckMethod, DomainError, Evidence, EvidenceFile, VerifierDigest, sha256};

pub const DEFAULT_CHECK_TIMEOUT: Duration = Duration::from_secs(120);
const FILE_CAPTURE_LIMIT: u64 = 4 * 1024 * 1024;
const WAIT_INTERVAL: Duration = Duration::from_millis(10);
static CHECK_MARKER_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Executes built-in checks without receiving journal access or authority to
/// record its own observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltinCheckExecutor {
    shell: PathBuf,
    protector: Option<PathBuf>,
    verifiers: Vec<VerifierDigest>,
    timeout: Duration,
}

impl BuiltinCheckExecutor {
    pub fn new() -> Result<Self, CheckExecutionError> {
        Self::with_shell("/bin/sh", DEFAULT_CHECK_TIMEOUT)
    }

    pub fn with_shell(
        shell: impl Into<PathBuf>,
        timeout: Duration,
    ) -> Result<Self, CheckExecutionError> {
        Self::with_verifier_files(shell, std::iter::empty::<PathBuf>(), timeout)
    }

    /// Captures every explicitly declared verifier before an invocation can
    /// change it. The shell is always included as the first verifier.
    pub fn with_verifier_files(
        shell: impl Into<PathBuf>,
        verifier_files: impl IntoIterator<Item = PathBuf>,
        timeout: Duration,
    ) -> Result<Self, CheckExecutionError> {
        let shell = shell.into();
        if !shell.is_absolute() || !shell.is_file() {
            return Err(CheckExecutionError::VerifierUnavailable(shell));
        }
        let mut paths = vec![shell.clone()];
        let protector = platform_protector();
        paths.extend(protector.iter().cloned());
        paths.extend(verifier_files);
        let mut verifiers = Vec::with_capacity(paths.len());
        for path in paths {
            if !path.is_absolute() || !path.is_file() {
                return Err(CheckExecutionError::VerifierUnavailable(path));
            }
            verifiers.push(
                VerifierDigest::new(path.clone(), read_digest(&path)?)
                    .map_err(CheckExecutionError::InvalidEvidence)?,
            );
        }
        Ok(Self {
            shell,
            protector,
            verifiers,
            timeout,
        })
    }

    pub fn execute(
        &self,
        check: &Check,
        workspace: &Path,
    ) -> Result<Evidence, CheckExecutionError> {
        let workspace = resolve_workspace(workspace)?;
        self.execute_in(check, &workspace, None)
    }

    /// Executes with an operating-system sandbox that denies writes to the
    /// protected directory while preserving the actual workspace as cwd.
    pub fn execute_protected(
        &self,
        check: &Check,
        workspace: &Path,
        protected: &Path,
    ) -> Result<Evidence, CheckExecutionError> {
        let workspace = resolve_workspace(workspace)?;
        let protected = protected.canonicalize().map_err(|error| {
            CheckExecutionError::ProtectionUnavailable {
                path: protected.to_owned(),
                message: error.to_string(),
            }
        })?;
        self.execute_in(check, &workspace, Some(&protected))
    }

    fn execute_in(
        &self,
        check: &Check,
        workspace: &Path,
        protected: Option<&Path>,
    ) -> Result<Evidence, CheckExecutionError> {
        let workspace_text =
            workspace
                .to_str()
                .ok_or_else(|| CheckExecutionError::WorkspaceUnavailable {
                    path: workspace.to_owned(),
                    message: "path is not valid UTF-8".to_owned(),
                })?;

        let started = Instant::now();
        let evidence = match check.method() {
            CheckMethod::Command { command } => {
                self.require_pinned_verifier()?;
                let marker = protected.map(|_| StartMarker::new()).transpose()?;
                let mut process = match protected {
                    Some(path) => protected_command(
                        self.protector.as_deref().ok_or_else(|| {
                            CheckExecutionError::ProtectionUnavailable {
                                path: path.to_owned(),
                                message: "no supported operating-system sandbox is available"
                                    .to_owned(),
                            }
                        })?,
                        &self.shell,
                        command,
                        workspace,
                        path,
                        marker
                            .as_ref()
                            .expect("protected command has a marker")
                            .path(),
                    )?,
                    None => {
                        let mut process = Command::new(&self.shell);
                        process.arg("-c").arg(command);
                        process
                    }
                };
                let mut child = process
                    .current_dir(workspace)
                    .env("PWD", workspace)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .process_group(0)
                    .spawn()
                    .map_err(|error| CheckExecutionError::StartFailed(error.to_string()))?;
                let status = loop {
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|error| CheckExecutionError::WaitFailed(error.to_string()))?
                    {
                        stop_process_group(child.id())?;
                        break status;
                    }
                    let elapsed = started.elapsed();
                    if elapsed >= self.timeout {
                        stop_process_group(child.id())?;
                        child
                            .wait()
                            .map_err(|error| CheckExecutionError::WaitFailed(error.to_string()))?;
                        return Err(CheckExecutionError::TimedOut(self.timeout));
                    }
                    thread::sleep(WAIT_INTERVAL.min(self.timeout.saturating_sub(elapsed)));
                };
                if let Some(marker) = marker.as_ref()
                    && !marker.observed()
                {
                    return Err(CheckExecutionError::ProtectionUnavailable {
                        path: protected.expect("protected command has a path").to_owned(),
                        message: "sandbox exited before the check shell started".to_owned(),
                    });
                }
                self.require_pinned_verifier()?;
                let exit_code = status.code().ok_or(CheckExecutionError::NoExitCode)?;
                Evidence::command(
                    check.clone(),
                    workspace_text,
                    exit_code,
                    elapsed_millis(started.elapsed()),
                    self.verifiers.clone(),
                )
                .map_err(CheckExecutionError::InvalidEvidence)?
            }
            CheckMethod::ExactBytes { path, .. } => {
                let actual = capture_file(workspace, path)?;
                Evidence::exact_bytes(
                    check.clone(),
                    workspace_text,
                    elapsed_millis(started.elapsed()),
                    actual,
                )
                .map_err(CheckExecutionError::InvalidEvidence)?
            }
        };
        Ok(evidence)
    }

    fn require_pinned_verifier(&self) -> Result<(), CheckExecutionError> {
        for verifier in &self.verifiers {
            let actual = read_digest(verifier.path())?;
            if actual != verifier.sha256() {
                return Err(CheckExecutionError::VerifierChanged {
                    path: verifier.path().to_owned(),
                    expected: verifier.sha256().to_owned(),
                    actual,
                });
            }
        }
        Ok(())
    }
}

fn resolve_workspace(workspace: &Path) -> Result<PathBuf, CheckExecutionError> {
    let workspace =
        workspace
            .canonicalize()
            .map_err(|error| CheckExecutionError::WorkspaceUnavailable {
                path: workspace.to_owned(),
                message: error.to_string(),
            })?;
    if workspace.is_dir() {
        Ok(workspace)
    } else {
        Err(CheckExecutionError::WorkspaceUnavailable {
            path: workspace,
            message: "path is not a directory".to_owned(),
        })
    }
}

struct StartMarker {
    directory: PathBuf,
    path: PathBuf,
}

impl StartMarker {
    fn new() -> Result<Self, CheckExecutionError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..16 {
            let sequence = CHECK_MARKER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "ymp-check-marker-{}-{elapsed:x}-{sequence:x}",
                std::process::id()
            ));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    let path = directory.join("started");
                    return Ok(Self { directory, path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(CheckExecutionError::StartFailed(format!(
                        "cannot create check start marker: {error}"
                    )));
                }
            }
        }
        Err(CheckExecutionError::StartFailed(
            "cannot allocate a unique check start marker".to_owned(),
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn observed(&self) -> bool {
        self.path.is_file()
    }
}

impl Drop for StartMarker {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[cfg(target_os = "macos")]
fn platform_protector() -> Option<PathBuf> {
    Some(PathBuf::from("/usr/bin/sandbox-exec"))
}

#[cfg(target_os = "linux")]
fn platform_protector() -> Option<PathBuf> {
    std::env::current_exe().ok().filter(|path| path.is_file())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_protector() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn protected_command(
    protector: &Path,
    shell: &Path,
    script: &str,
    _workspace: &Path,
    protected: &Path,
    marker: &Path,
) -> Result<Command, CheckExecutionError> {
    let escaped = sandbox_path(protected)?;
    let mut profile = format!(
        "(version 1)\n(allow default)\n(deny file-write* (subpath \"{escaped}\"))\n\
         (deny file-write-unlink (literal \"{escaped}\"))"
    );
    for ancestor in protected.ancestors().skip(1) {
        let escaped = sandbox_path(ancestor)?;
        profile.push_str(&format!(
            "\n(deny file-write-unlink (literal \"{escaped}\"))"
        ));
    }
    let mut command = Command::new(protector);
    command
        .arg("-p")
        .arg(profile)
        .arg(shell)
        .arg("-c")
        .arg("printf started > \"$1\" || exit 125; shift; exec \"$@\"")
        .arg("ymp-check-bootstrap")
        .arg(marker)
        .arg(shell)
        .arg("-c")
        .arg(script);
    Ok(command)
}

#[cfg(target_os = "macos")]
fn sandbox_path(path: &Path) -> Result<String, CheckExecutionError> {
    let text = path
        .to_str()
        .ok_or_else(|| CheckExecutionError::ProtectionUnavailable {
            path: path.to_owned(),
            message: "path is not valid UTF-8".to_owned(),
        })?;
    if text.chars().any(char::is_control) {
        return Err(CheckExecutionError::ProtectionUnavailable {
            path: path.to_owned(),
            message: "path contains control characters".to_owned(),
        });
    }
    Ok(text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(target_os = "linux")]
fn protected_command(
    protector: &Path,
    shell: &Path,
    script: &str,
    _workspace: &Path,
    protected: &Path,
    marker: &Path,
) -> Result<Command, CheckExecutionError> {
    let mut command = Command::new(protector);
    command
        .arg("__ymp_internal_check_sandbox")
        .arg(protected)
        .arg(marker)
        .arg(shell)
        .arg(script);
    Ok(command)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn protected_command(
    _protector: &Path,
    _shell: &Path,
    _script: &str,
    _workspace: &Path,
    protected: &Path,
    _marker: &Path,
) -> Result<Command, CheckExecutionError> {
    Err(CheckExecutionError::ProtectionUnavailable {
        path: protected.to_owned(),
        message: "this operating system is not supported".to_owned(),
    })
}

/// Internal Linux entry point used by the `ymp` executable to create a
/// self-contained user and mount namespace before starting the check shell.
/// Returns a process exit code and writes the start marker only after the shell
/// has been spawned inside the protected namespace.
#[cfg(target_os = "linux")]
pub fn run_linux_check_sandbox(arguments: &[std::ffi::OsString]) -> u8 {
    match run_linux_check_sandbox_inner(arguments) {
        Ok(code) => u8::try_from(code).unwrap_or(125),
        Err(message) => {
            eprintln!("error: cannot start protected check: {message}");
            125
        }
    }
}

#[cfg(target_os = "linux")]
fn run_linux_check_sandbox_inner(arguments: &[std::ffi::OsString]) -> Result<i32, String> {
    use nix::mount::{MsFlags, mount};
    use nix::sched::{CloneFlags, unshare};
    use nix::unistd::{getgid, getuid};

    let [protected, marker, shell, script] = arguments else {
        return Err(
            "internal sandbox requires protected path, marker, shell and script".to_owned(),
        );
    };
    let protected = PathBuf::from(protected)
        .canonicalize()
        .map_err(|error| format!("cannot resolve protected path: {error}"))?;
    let marker = PathBuf::from(marker);
    let shell = PathBuf::from(shell);
    if !protected.is_dir() || !marker.is_absolute() || !shell.is_absolute() {
        return Err("internal sandbox paths are invalid".to_owned());
    }

    let outer_uid = getuid().as_raw();
    let outer_gid = getgid().as_raw();
    unshare(CloneFlags::CLONE_NEWUSER)
        .map_err(|error| format!("cannot create user namespace: {error}"))?;
    let setgroups = Path::new("/proc/self/setgroups");
    if setgroups.exists() {
        fs::write(setgroups, "deny")
            .map_err(|error| format!("cannot disable namespace setgroups: {error}"))?;
    }
    fs::write("/proc/self/uid_map", format!("1 {outer_uid} 1\n"))
        .map_err(|error| format!("cannot configure namespace UID map: {error}"))?;
    fs::write("/proc/self/gid_map", format!("1 {outer_gid} 1\n"))
        .map_err(|error| format!("cannot configure namespace GID map: {error}"))?;
    unshare(CloneFlags::CLONE_NEWNS)
        .map_err(|error| format!("cannot create mount namespace: {error}"))?;
    mount(
        None::<&Path>,
        Path::new("/"),
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )
    .map_err(|error| format!("cannot make mount namespace private: {error}"))?;

    let mut ancestors = protected.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors
        .into_iter()
        .filter(|ancestor| *ancestor != Path::new("/"))
    {
        mount(
            Some(ancestor),
            ancestor,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .map_err(|error| format!("cannot protect ancestor '{}': {error}", ancestor.display()))?;
    }
    mount(
        Some(protected.as_path()),
        protected.as_path(),
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|error| format!("cannot make journal path read-only: {error}"))?;
    unshare(CloneFlags::CLONE_NEWPID)
        .map_err(|error| format!("cannot create PID namespace: {error}"))?;

    let current_exe = std::env::current_exe()
        .map_err(|error| format!("cannot resolve sandbox executable: {error}"))?;
    Command::new(current_exe)
        .arg("__ymp_internal_check_sandbox_stage2")
        .arg(marker)
        .arg(shell)
        .arg(script)
        .spawn()
        .map_err(|error| format!("cannot start PID-namespace helper: {error}"))?
        .wait()
        .map_err(|error| format!("cannot wait for PID-namespace helper: {error}"))?
        .code()
        .ok_or_else(|| "PID-namespace helper ended without an exit code".to_owned())
}

/// Second Linux sandbox stage. This process is PID 1 in the new namespace; it
/// replaces `/proc`, drops namespace capabilities, and only then starts the
/// caller's shell command.
#[cfg(target_os = "linux")]
pub fn run_linux_check_sandbox_stage2(arguments: &[std::ffi::OsString]) -> u8 {
    match run_linux_check_sandbox_stage2_inner(arguments) {
        Ok(code) => u8::try_from(code).unwrap_or(125),
        Err(message) => {
            eprintln!("error: cannot start protected check shell: {message}");
            125
        }
    }
}

#[cfg(target_os = "linux")]
fn run_linux_check_sandbox_stage2_inner(arguments: &[std::ffi::OsString]) -> Result<i32, String> {
    use caps::CapSet;
    use nix::mount::{MsFlags, mount};
    use nix::sys::prctl;
    use nix::unistd::{Gid, Uid, setgid, setuid};

    let [marker, shell, script] = arguments else {
        return Err("internal sandbox stage 2 requires marker, shell and script".to_owned());
    };
    let marker = PathBuf::from(marker);
    let shell = PathBuf::from(shell);
    if !marker.is_absolute() || !shell.is_absolute() {
        return Err("internal sandbox stage 2 paths are invalid".to_owned());
    }

    mount(
        Some("proc"),
        Path::new("/proc"),
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        None::<&str>,
    )
    .map_err(|error| format!("cannot mount private procfs: {error}"))?;
    for capability_set in [CapSet::Bounding, CapSet::Ambient, CapSet::Inheritable] {
        caps::clear(None, capability_set)
            .map_err(|error| format!("cannot clear {capability_set:?} capabilities: {error}"))?;
    }
    setgid(Gid::from_raw(1)).map_err(|error| format!("cannot select mapped GID: {error}"))?;
    setuid(Uid::from_raw(1)).map_err(|error| format!("cannot select mapped UID: {error}"))?;
    for capability_set in [CapSet::Effective, CapSet::Permitted] {
        caps::clear(None, capability_set)
            .map_err(|error| format!("cannot clear {capability_set:?} capabilities: {error}"))?;
    }
    prctl::set_no_new_privs().map_err(|error| format!("cannot set no-new-privileges: {error}"))?;

    let mut child = Command::new(shell)
        .arg("-c")
        .arg(script)
        .spawn()
        .map_err(|error| format!("cannot start check shell: {error}"))?;
    fs::write(&marker, b"started")
        .map_err(|error| format!("cannot record check-shell start: {error}"))?;
    child
        .wait()
        .map_err(|error| format!("cannot wait for check shell: {error}"))?
        .code()
        .ok_or_else(|| "check shell ended without an exit code".to_owned())
}

fn stop_process_group(id: u32) -> Result<(), CheckExecutionError> {
    let process_group = i32::try_from(id).map_err(|_| {
        CheckExecutionError::StopFailed(
            "check process ID does not fit the platform process ID".to_owned(),
        )
    })?;
    match killpg(Pid::from_raw(process_group), Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(CheckExecutionError::StopFailed(error.to_string())),
    }
}

fn capture_file(root: &Path, path: &Path) -> Result<EvidenceFile, CheckExecutionError> {
    let full = root.join(path);
    let ancestor = full
        .ancestors()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| CheckExecutionError::FileCapture {
            path: path.to_owned(),
            message: "no existing ancestor".to_owned(),
        })?;
    let ancestor = ancestor
        .canonicalize()
        .map_err(|error| CheckExecutionError::FileCapture {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    if !ancestor.starts_with(root) {
        return Err(CheckExecutionError::FileCapture {
            path: path.to_owned(),
            message: "path escapes the workspace".to_owned(),
        });
    }

    let bytes = if full.exists() {
        if !full.is_file() {
            return Err(CheckExecutionError::FileCapture {
                path: path.to_owned(),
                message: "path is not a file".to_owned(),
            });
        }
        let length = full
            .metadata()
            .map_err(|error| CheckExecutionError::FileCapture {
                path: path.to_owned(),
                message: error.to_string(),
            })?
            .len();
        if length > FILE_CAPTURE_LIMIT {
            return Err(CheckExecutionError::FileCapture {
                path: path.to_owned(),
                message: format!("file exceeds the {FILE_CAPTURE_LIMIT}-byte capture limit"),
            });
        }
        Some(
            fs::read(&full).map_err(|error| CheckExecutionError::FileCapture {
                path: path.to_owned(),
                message: error.to_string(),
            })?,
        )
    } else {
        None
    };
    EvidenceFile::new(path, bytes).map_err(CheckExecutionError::InvalidEvidence)
}

fn read_digest(path: &Path) -> Result<String, CheckExecutionError> {
    fs::read(path)
        .map(|bytes| sha256(&bytes))
        .map_err(|error| CheckExecutionError::VerifierRead {
            path: path.to_owned(),
            message: error.to_string(),
        })
}

fn elapsed_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckExecutionError {
    VerifierUnavailable(PathBuf),
    VerifierRead {
        path: PathBuf,
        message: String,
    },
    VerifierChanged {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    WorkspaceUnavailable {
        path: PathBuf,
        message: String,
    },
    ProtectionUnavailable {
        path: PathBuf,
        message: String,
    },
    FileCapture {
        path: PathBuf,
        message: String,
    },
    StartFailed(String),
    WaitFailed(String),
    StopFailed(String),
    TimedOut(Duration),
    NoExitCode,
    InvalidEvidence(DomainError),
}

impl fmt::Display for CheckExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerifierUnavailable(path) => write!(
                formatter,
                "check verifier '{}' is not an available absolute file",
                path.display()
            ),
            Self::VerifierRead { path, message } => write!(
                formatter,
                "cannot read check verifier '{}': {message}",
                path.display()
            ),
            Self::VerifierChanged {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "check verifier '{}' changed: expected SHA-256 {expected}, actual {actual}",
                path.display()
            ),
            Self::WorkspaceUnavailable { path, message } => write!(
                formatter,
                "check workspace '{}' is unavailable: {message}",
                path.display()
            ),
            Self::ProtectionUnavailable { path, message } => write!(
                formatter,
                "cannot protect journal path '{}' from the check command: {message}",
                path.display()
            ),
            Self::FileCapture { path, message } => write!(
                formatter,
                "cannot capture evidence file '{}': {message}",
                path.display()
            ),
            Self::StartFailed(message) => {
                write!(formatter, "cannot start check command: {message}")
            }
            Self::WaitFailed(message) => {
                write!(formatter, "cannot wait for check command: {message}")
            }
            Self::StopFailed(message) => write!(formatter, "cannot stop check command: {message}"),
            Self::TimedOut(timeout) => write!(
                formatter,
                "check command did not finish within {} seconds",
                timeout.as_secs()
            ),
            Self::NoExitCode => formatter.write_str("check command ended without an exit code"),
            Self::InvalidEvidence(error) => write!(formatter, "invalid check evidence: {error}"),
        }
    }
}

impl Error for CheckExecutionError {}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use ymp_domain::CriterionId;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ymp-check-{name}-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("test directory is created");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn command_check(command: &str) -> Check {
        Check::command(
            vec![CriterionId::new("checked").expect("valid criterion ID")],
            command,
        )
        .expect("valid command check")
    }

    #[test]
    fn command_checks_inherit_environment_and_derive_exit_outcomes() {
        let workspace = TestDir::new("command");
        let executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");

        let satisfied = executor
            .execute(&command_check("test -n \"$PATH\" && echo ok"), &workspace.0)
            .expect("successful command is observed");
        let failed = executor
            .execute(&command_check("exit 1"), &workspace.0)
            .expect("nonzero command is observed");

        assert!(satisfied.satisfied());
        assert_eq!(satisfied.exit_code(), Some(0));
        assert!(!failed.satisfied());
        assert_eq!(failed.exit_code(), Some(1));
        assert!(
            satisfied
                .verifiers()
                .iter()
                .any(|verifier| { verifier.path() == Path::new("/bin/sh") })
        );
    }

    #[test]
    fn exact_bytes_capture_present_and_missing_files_with_sha256() {
        let workspace = TestDir::new("files");
        fs::write(workspace.0.join("result.txt"), b"expected").expect("test file is written");
        let executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");

        let present = executor
            .execute(
                &Check::exact_bytes(
                    vec![CriterionId::new("file").expect("valid criterion ID")],
                    "result.txt",
                    b"expected".to_vec(),
                )
                .expect("valid exact-byte check"),
                &workspace.0,
            )
            .expect("file check is observed");
        let missing = executor
            .execute(
                &Check::exact_bytes(
                    vec![CriterionId::new("missing").expect("valid criterion ID")],
                    "missing.txt",
                    Vec::new(),
                )
                .expect("valid exact-byte check"),
                &workspace.0,
            )
            .expect("missing file is observed");

        assert!(present.satisfied());
        assert!(present.files()[0].sha256().is_some());
        assert!(!missing.satisfied());
        assert_eq!(missing.files()[0].bytes(), None);
        assert_eq!(missing.files()[0].sha256(), None);
    }

    #[test]
    fn changed_verifier_is_rejected_before_execution() {
        let directory = TestDir::new("verifier");
        let shell = directory.0.join("sh");
        fs::copy("/bin/sh", &shell).expect("shell copy is created");
        let executor = BuiltinCheckExecutor::with_shell(&shell, Duration::from_secs(1))
            .expect("executor captures its verifier");
        OpenOptions::new()
            .append(true)
            .open(&shell)
            .expect("shell copy opens")
            .write_all(b"changed")
            .expect("shell copy changes");

        assert!(matches!(
            executor.execute(&command_check("exit 0"), &directory.0),
            Err(CheckExecutionError::VerifierChanged { .. })
        ));
    }

    #[test]
    fn explicitly_declared_verifier_file_is_pinned() {
        let directory = TestDir::new("declared-verifier");
        let verifier = directory.0.join("verify.sh");
        fs::write(&verifier, b"initial").expect("verifier file is written");
        let executor = BuiltinCheckExecutor::with_verifier_files(
            "/bin/sh",
            [verifier.clone()],
            Duration::from_secs(1),
        )
        .expect("executor captures declared verifiers");
        fs::write(&verifier, b"changed").expect("verifier file changes");

        assert!(matches!(
            executor.execute(&command_check("exit 0"), &directory.0),
            Err(CheckExecutionError::VerifierChanged { path, .. }) if path == verifier
        ));
    }

    #[test]
    fn completed_shell_does_not_leave_background_processes_running() {
        let workspace = TestDir::new("completed-group");
        let executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");

        let evidence = executor
            .execute(
                &command_check("(sleep 0.1; echo escaped > after-exit) &"),
                &workspace.0,
            )
            .expect("shell completion is observed");

        assert!(evidence.satisfied());
        assert_eq!(evidence.exit_code(), Some(0));
        thread::sleep(Duration::from_millis(200));
        assert!(!workspace.0.join("after-exit").exists());
    }

    #[test]
    fn protected_execution_denies_absolute_journal_mutation_in_the_real_workspace() {
        let workspace = TestDir::new("protected-journal");
        let journal = workspace.0.join(".ymp/sessions");
        fs::create_dir_all(&journal).expect("journal directory is created");
        fs::write(journal.join("journal.db"), b"authoritative")
            .expect("journal fixture is written");
        fs::write(workspace.0.join("visible.txt"), b"visible")
            .expect("workspace fixture is written");
        let executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");

        let command = format!(
            "test -f visible.txt && /bin/rm -f {:?}",
            journal.join("journal.db")
        );
        let absolute =
            match executor.execute_protected(&command_check(&command), &workspace.0, &journal) {
                Ok(evidence) => evidence,
                Err(CheckExecutionError::ProtectionUnavailable { .. })
                    if !cfg!(target_os = "macos") =>
                {
                    assert_eq!(
                        fs::read(journal.join("journal.db")).expect("journal fixture reads"),
                        b"authoritative"
                    );
                    return;
                }
                Err(error) => panic!("protected command failed unexpectedly: {error}"),
            };

        assert!(!absolute.satisfied());
        assert_eq!(absolute.exit_code(), Some(1));
        assert_eq!(
            fs::read(journal.join("journal.db")).expect("journal fixture reads"),
            b"authoritative"
        );

        std::os::unix::fs::symlink(&journal, workspace.0.join("journal-link"))
            .expect("journal symlink is created");
        let linked = executor
            .execute_protected(
                &command_check("/bin/rm -f journal-link/journal.db"),
                &workspace.0,
                &journal,
            )
            .expect("symlinked command is observed");
        assert!(!linked.satisfied());
        assert_eq!(linked.exit_code(), Some(1));
        assert_eq!(
            fs::read(journal.join("journal.db")).expect("journal fixture reads"),
            b"authoritative"
        );

        let moved = workspace.0.join("moved-journal-parent");
        let rename = format!("/bin/mv {:?} {:?}", workspace.0.join(".ymp"), moved);
        let renamed = executor
            .execute_protected(&command_check(&rename), &workspace.0, &journal)
            .expect("ancestor rename is observed");
        assert!(!renamed.satisfied());
        assert_eq!(renamed.exit_code(), Some(1));
        assert!(workspace.0.join(".ymp").is_dir());
        assert!(!moved.exists());
    }

    #[test]
    fn sandbox_start_failure_is_not_evidence_of_a_failed_check() {
        let workspace = TestDir::new("sandbox-start-failure");
        let journal = workspace.0.join("sessions");
        fs::create_dir(&journal).expect("journal directory is created");
        let mut executor = BuiltinCheckExecutor::new().expect("executor captures its verifier");
        executor.protector = Some(PathBuf::from("/usr/bin/false"));

        let result = executor.execute_protected(&command_check("exit 1"), &workspace.0, &journal);
        assert!(
            matches!(
                result,
                Err(CheckExecutionError::ProtectionUnavailable { .. })
            ),
            "unexpected sandbox-start result: {result:?}"
        );
    }

    #[test]
    fn timeout_stops_the_check_without_inventing_an_exit_code() {
        let workspace = TestDir::new("timeout");
        let executor = BuiltinCheckExecutor::with_shell("/bin/sh", Duration::from_millis(20))
            .expect("executor captures its verifier");

        assert_eq!(
            executor.execute(
                &command_check("(sleep 0.1; echo escaped > after-timeout) & while :; do :; done",),
                &workspace.0,
            ),
            Err(CheckExecutionError::TimedOut(Duration::from_millis(20)))
        );
        thread::sleep(Duration::from_millis(200));
        assert!(!workspace.0.join("after-timeout").exists());
    }
}
