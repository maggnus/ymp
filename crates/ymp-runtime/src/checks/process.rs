//! Retained-snapshot checks. macOS commands use a no-fork, read-only sandbox.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use ymp_domain::{
    Denial, Digest, Result,
    assignment::ErrorClass,
    journal::PolicySelection,
    verification::*,
    workspace::{SnapshotFile, WorkspacePath},
};
use ymp_kernel::{
    journal::ContentStore,
    ports::checks::{CheckExecution, CheckObservation, CheckRunner},
};

pub struct ProcessRunner {
    limits: CheckLimits,
}
impl ProcessRunner {
    pub fn new(limits: CheckLimits) -> Result<Self> {
        limits.validate()?;
        Ok(Self { limits })
    }
}
const SYSTEM_READS: &[&str] = &["/bin", "/usr/bin", "/usr/lib", "/usr/share", "/System"];
const ENVIRONMENT: &[(&str, &str)] = &[("PATH", "/usr/bin:/bin"), ("LANG", "C"), ("LC_ALL", "C")];
impl CheckRunner for ProcessRunner {
    fn environment(&self) -> Result<CheckEnvironment> {
        let mut identity = BTreeMap::from([
            ("isolation".into(), "macOS Seatbelt; deny default; no fork; no network; writes only to private scratch; metadata visible".into()),
            ("read_roots".into(), format!("/ (directory only), /dev/null, {}, retained target, declared verifier files, private scratch", SYSTEM_READS.join(", "))),
            ("variables".into(), "PATH=/usr/bin:/bin; LANG=C; LC_ALL=C; HOME=private scratch; TMPDIR=private scratch; YMP_CHECK_INPUTS=pinned verifier; stdin=null".into()),
        ]);
        for path in ["/usr/bin/sandbox-exec", "/bin/sh"] {
            identity.insert(
                path.into(),
                fs::read(path)
                    .map(|bytes| Digest::of(bytes).to_string())
                    .unwrap_or_else(|e| format!("unavailable: {e}")),
            );
        }
        // Read the OS release directly; no command and no inherited user environment.
        if let Ok(bytes) = fs::read("/System/Library/CoreServices/SystemVersion.plist") {
            identity.insert("system_version".into(), Digest::of(bytes).to_string());
        }
        if !cfg!(target_os = "macos") {
            identity.insert(
                "isolation".into(),
                "Command unavailable: no implemented confinement on this platform".into(),
            );
        }
        Ok(CheckEnvironment {
            runner: PolicySelection::new(
                "CheckRunner",
                "ProcessRunner",
                "1",
                serde_json::to_value(&self.limits)
                    .map_err(|e| Denial::new("check_environment", e.to_string()))?,
            )?,
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            identity,
            limits: self.limits.clone(),
        })
    }
    fn run(
        &self,
        execution: &CheckExecution<'_>,
        store: &dyn ContentStore,
    ) -> Result<CheckObservation> {
        if self.environment()? != *execution.environment {
            return Err(Denial::new(
                "check_environment",
                "Runner environment changed before execution",
            ));
        }
        let result = match &execution.check.spec {
            CheckSpec::ExactBytes { path, .. } => {
                let digest = execution
                    .target
                    .tree
                    .files
                    .get(path)
                    .map(|file| read_file(store, file).map(Digest::of))
                    .transpose()?;
                (CheckObservationKind::ExactBytes(digest), vec![], vec![])
            }
            CheckSpec::Command { .. } => command(execution, store),
        };
        let (kind, stdout, stderr) = result;
        Ok(CheckObservation {
            check: execution.check.reference(),
            target: execution.target.reference()?,
            environment: Digest::of_value(execution.environment)?,
            kind,
            stdout,
            stderr,
        })
    }
}
fn failure(
    class: ErrorClass,
    reason: impl Into<String>,
) -> (CheckObservationKind, Vec<u8>, Vec<u8>) {
    (
        CheckObservationKind::Error {
            class,
            reason: reason.into(),
        },
        vec![],
        vec![],
    )
}
fn read_file(store: &dyn ContentStore, file: &SnapshotFile) -> Result<Vec<u8>> {
    let bytes = store.get(&file.digest, file.bytes as usize)?;
    if bytes.len() as u64 != file.bytes || Digest::of(&bytes) != file.digest {
        return Err(Denial::new(
            "check_content",
            "Retained snapshot file bytes disagree",
        ));
    }
    Ok(bytes)
}
#[cfg(not(target_os = "macos"))]
fn command(
    _: &CheckExecution<'_>,
    _: &dyn ContentStore,
) -> (CheckObservationKind, Vec<u8>, Vec<u8>) {
    failure(
        ErrorClass::Environment,
        "Command confinement is not implemented on this platform",
    )
}
#[cfg(target_os = "macos")]
fn command(
    execution: &CheckExecution<'_>,
    store: &dyn ContentStore,
) -> (CheckObservationKind, Vec<u8>, Vec<u8>) {
    match launch(execution, store) {
        Ok(observation) => observation,
        Err(error) => failure(ErrorClass::Environment, error.to_string()),
    }
}
#[cfg(target_os = "macos")]
fn io(error: std::io::Error) -> Denial {
    Denial::new("check_environment", error.to_string())
}
#[cfg(target_os = "macos")]
struct Directory(PathBuf);
#[cfg(target_os = "macos")]
impl Directory {
    fn new() -> Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| Denial::new("check_directory", e.to_string()))?;
        let path = std::env::temp_dir().join(format!("ymp-check-{}", Digest::of(random)));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(io)?;
        Ok(Self(fs::canonicalize(path).map_err(io)?))
    }
}
#[cfg(target_os = "macos")]
impl Drop for Directory {
    fn drop(&mut self) {
        fn writable(path: &Path) {
            use std::os::unix::fs::PermissionsExt;
            // Never follow a scratch symlink into an external tree during cleanup.
            if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        writable(&entry.path());
                    }
                }
            }
        }
        writable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[cfg(target_os = "macos")]
fn write_file(
    root: &Path,
    path: &WorkspacePath,
    file: &SnapshotFile,
    store: &dyn ContentStore,
) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let path = root.join(path.as_str());
    fs::create_dir_all(path.parent().expect("relative file parent")).map_err(io)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(io)?;
    let bytes = read_file(store, file)?;
    output.write_all(&bytes).map_err(io)?;
    output
        .set_permissions(fs::Permissions::from_mode(file.mode))
        .map_err(io)?;
    if Digest::of(fs::read(path).map_err(io)?) != file.digest {
        return Err(Denial::new(
            "check_content",
            "Materialized verifier or snapshot changed",
        ));
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn quote(path: &Path) -> Result<String> {
    let value = path
        .to_str()
        .ok_or_else(|| Denial::new("check_path", "Sandbox paths must be UTF-8"))?;
    serde_json::to_string(value).map_err(|e| Denial::new("check_path", e.to_string()))
}
#[cfg(target_os = "macos")]
fn profile(subject: &Path, verifier: &Path, scratch: &Path) -> Result<String> {
    let mut roots = SYSTEM_READS
        .iter()
        .map(|p| format!("(subpath \"{p}\")"))
        .collect::<Vec<_>>();
    for path in [subject, verifier, scratch] {
        roots.push(format!("(subpath {})", quote(path)?));
    }
    // Root directory data is required by dyld on macOS 27. It does not allow descendant file reads.
    Ok(format!(
        "(version 1)(deny default)(allow file-read-metadata process-exec sysctl-read)(allow file-read-data file-map-executable (literal \"/\") (literal \"/dev/null\") {})(allow file-write* (subpath {}))(allow file-write-data (literal \"/dev/null\"))",
        roots.join(" "),
        quote(scratch)?
    ))
}
#[cfg(target_os = "macos")]
fn launcher_started(path: &Path) -> bool {
    use rustix::fs::OFlags;
    use std::{io::Read, os::unix::fs::OpenOptionsExt};

    let Ok(mut file) = fs::OpenOptions::new()
        .read(true)
        .custom_flags((OFlags::NOFOLLOW | OFlags::NONBLOCK).bits() as i32)
        .open(path)
    else {
        return false;
    };
    if !file
        .metadata()
        .is_ok_and(|meta| meta.is_file() && meta.len() == 7)
    {
        return false;
    }
    let mut bytes = [0u8; 8];
    matches!(file.read(&mut bytes), Ok(7)) && &bytes[..7] == b"started"
}

#[cfg(target_os = "macos")]
fn launch(
    execution: &CheckExecution<'_>,
    store: &dyn ContentStore,
) -> Result<(CheckObservationKind, Vec<u8>, Vec<u8>)> {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
    use std::{
        io::Read,
        os::unix::process::CommandExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let CheckSpec::Command {
        program,
        args,
        inputs,
    } = &execution.check.spec
    else {
        unreachable!()
    };
    let verifier = execution.verifier.ok_or_else(|| {
        Denial::new(
            "check_verifier",
            "Command has no retained verifier snapshot",
        )
    })?;
    let executable = verifier.tree.files.get(program).ok_or_else(|| {
        Denial::new(
            "check_program",
            "Executable is missing from the pinned verifier snapshot",
        )
    })?;
    if executable.mode & 0o111 == 0 {
        return Err(Denial::new(
            "check_program",
            "Pinned program is not executable",
        ));
    }
    let root = Directory::new()?;
    let subject = root.0.join("subject");
    let checks = root.0.join("verifier");
    let scratch = root.0.join("scratch");
    for path in [&subject, &checks, &scratch] {
        fs::create_dir(path).map_err(io)?;
    }
    for path in execution.target.tree.directories.keys() {
        fs::create_dir_all(subject.join(path.as_str())).map_err(io)?;
    }
    for (path, file) in &execution.target.tree.files {
        write_file(&subject, path, file, store)?;
    }
    let mut files = inputs.clone();
    files.insert(program.clone());
    for path in &files {
        let file = verifier.tree.files.get(path).ok_or_else(|| {
            Denial::new(
                "check_input",
                "Declared input is missing from the pinned verifier snapshot",
            )
        })?;
        write_file(&checks, path, file, store)?;
    }
    // Preserve captured subject metadata; the sandbox, not mode rewriting, enforces read-only access.
    use std::os::unix::fs::PermissionsExt;
    for (path, mode) in execution.target.tree.directories.iter().rev() {
        fs::set_permissions(
            subject.join(path.as_str()),
            fs::Permissions::from_mode(*mode),
        )
        .map_err(io)?;
    }
    let rules = profile(&subject, &checks, &scratch)?;
    let marker = scratch.join("started");
    let mut launch = Command::new("/usr/bin/sandbox-exec");
    launch
        .args([
            "-p",
            &rules,
            "/bin/sh",
            "-c",
            "printf 'started' > \"$1\"; shift; exec \"$@\"",
            "ymp-check",
        ])
        .arg(&marker)
        .arg(checks.join(program.as_str()))
        .args(args)
        .current_dir(&subject)
        .env_clear()
        .envs(ENVIRONMENT.iter().copied())
        .env("HOME", &scratch)
        .env("TMPDIR", &scratch)
        .env("YMP_CHECK_INPUTS", &checks)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = launch.spawn().map_err(io)?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    // Nonblocking pipes avoid waiting on inherited descriptors after process exit.
    let setup = fcntl_getfl(&stdout)
        .and_then(|flags| fcntl_setfl(&stdout, flags | OFlags::NONBLOCK))
        .and_then(|_| fcntl_getfl(&stderr))
        .and_then(|flags| fcntl_setfl(&stderr, flags | OFlags::NONBLOCK));
    if let Err(error) = setup {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Denial::new("check_pipe", error.to_string()));
    }
    let mut out = vec![];
    let mut err = vec![];
    let started = Instant::now();
    let limit = execution.environment.limits.output_bytes;
    let read = |pipe: &mut dyn Read, bytes: &mut Vec<u8>| -> Result<bool> {
        let mut buffer = [0u8; 8192];
        // One bounded chunk per iteration prevents a noisy process from hiding the deadline.
        match pipe.read(&mut buffer) {
            Ok(size) => {
                let keep = size.min(limit.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&buffer[..keep]);
                Ok(keep < size)
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                Ok(false)
            }
            Err(e) => Err(io(e)),
        }
    };
    let mut outcome = loop {
        match (read(&mut stdout, &mut out), read(&mut stderr, &mut err)) {
            (Ok(false), Ok(false)) => {}
            (Ok(_), Ok(_)) => {
                break CheckObservationKind::Error {
                    class: ErrorClass::Infrastructure,
                    reason: "Check exceeded its output limit".into(),
                };
            }
            (Err(error), _) | (_, Err(error)) => {
                break CheckObservationKind::Error {
                    class: ErrorClass::Infrastructure,
                    reason: error.to_string(),
                };
            }
        }
        if started.elapsed() >= Duration::from_millis(execution.environment.limits.timeout_ms) {
            break CheckObservationKind::Error {
                class: ErrorClass::Infrastructure,
                reason: "Check exceeded its wall-time limit".into(),
            };
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                // No forks are permitted. Drain at most the finite kernel pipe capacity, then classify.
                let mut overflow = false;
                for _ in 0..1024 {
                    let before = (out.len(), err.len());
                    overflow |= read(&mut stdout, &mut out).unwrap_or(true)
                        | read(&mut stderr, &mut err).unwrap_or(true);
                    if before == (out.len(), err.len()) || overflow {
                        break;
                    }
                }
                if overflow {
                    break CheckObservationKind::Error {
                        class: ErrorClass::Infrastructure,
                        reason: "Check exceeded its output limit".into(),
                    };
                }
                if !launcher_started(&marker) {
                    break CheckObservationKind::Error {
                        class: ErrorClass::Environment,
                        reason: "Sandbox launch marker is missing or invalid".into(),
                    };
                }
                break match status.code() {
                    // Shell exec failures cannot be distinguished from these explicit check exits; conservatively undecided.
                    Some(126 | 127) => CheckObservationKind::Error { class: ErrorClass::Environment, reason: "Executable could not be started, or returned reserved launcher status 126/127".into() },
                    Some(code) => CheckObservationKind::Exited(code),
                    None => CheckObservationKind::Error { class: ErrorClass::Infrastructure, reason: "Check ended by signal".into() },
                };
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(error) => {
                break CheckObservationKind::Error {
                    class: ErrorClass::Infrastructure,
                    reason: error.to_string(),
                };
            }
        }
    };
    // The sandbox denies process-fork, so a successful parent exit cannot leave detached descendants.
    let _ = child.kill();
    if let Err(error) = child.wait() {
        outcome = CheckObservationKind::Error {
            class: ErrorClass::Infrastructure,
            reason: error.to_string(),
        };
    }
    // Re-read pinned files after execution, even though the OS denies their modification.
    for path in &files {
        if !fs::read(checks.join(path.as_str()))
            .is_ok_and(|bytes| Digest::of(bytes) == verifier.tree.files[path].digest)
        {
            outcome = CheckObservationKind::Error {
                class: ErrorClass::Infrastructure,
                reason: "Pinned verifier changed during execution".into(),
            };
        }
    }
    Ok((outcome, out, err))
}
