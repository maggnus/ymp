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
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramRole {
    /// Opens the run's marker on an inherited descriptor and then replaces itself with the next
    /// program in the chain.
    LaunchShell,
    /// Removes the variables the shell introduced of its own accord, then replaces itself with the
    /// runtime executable.
    EnvironmentSanitiser,
    /// Reads the operator's credential material and hands it to the generated home the managed
    /// process is given. It touches the credential, so it belongs to the trusted chain.
    CredentialReader,
    /// Builds the private baseline of the managed workspace before the runtime is started.
    Workspace,
    /// Reports the process table, from which descendants of the managed process are attributed.
    ProcessTable,
    /// Reports which live processes hold the run's marker open, where the process file system that
    /// answers the same question is absent.
    DescriptorHolders,
    /// Signals the managed process and its descendants.
    Signal,
    /// Reports the access-control entries of a location, which its mode does not describe.
    AccessControl,
}

impl std::fmt::Display for ProgramRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::LaunchShell => "launch shell",
            Self::EnvironmentSanitiser => "environment sanitiser",
            Self::CredentialReader => "credential reader",
            Self::Workspace => "workspace program",
            Self::ProcessTable => "process table reader",
            Self::DescriptorHolders => "descriptor holder reader",
            Self::Signal => "signal program",
            Self::AccessControl => "access-control reader",
        };
        formatter.write_str(name)
    }
}

/// What binds an admitted program to the program the run means, as opposed to whatever bytes happen
/// to be reachable under that name when the run starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramRequirement {
    /// The program must stand at an absolute path whose file and every ancestor directory belong to
    /// the superuser and are writable by nobody else. The account a run executes under cannot
    /// create or replace such a file, so it cannot decide which bytes are admitted.
    SystemPath,
    /// The program must have exactly these bytes. This carries the binding where the location
    /// cannot, and obliges the caller to state in advance which program it means.
    StatedDigest(String),
}

/// What bound an admitted program to the program the run meant. Recorded in the evidence, because a
/// digest alone does not say whether the account under test could have chosen the bytes behind it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramIdentity {
    SystemPath,
    StatedDigest,
}

/// A program whose identity was established and whose bytes were read and digested before it was
/// executed on a managed run's behalf.
///
/// The pinned runtime executable and the coordination bridge are admitted by copy, so the bytes
/// that were digested are the bytes the operating system loads. A system program cannot be admitted
/// that way — a copy of a platform binary is refused by the operating system — so it is admitted at
/// its own path and re-verified immediately before and after the managed process is created. The
/// residual window between the last verification and the kernel's image load is stated in the card
/// that introduced this record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdmittedProgram {
    pub role: ProgramRole,
    pub path: PathBuf,
    pub identity: ProgramIdentity,
    pub digest: String,
}

impl AdmittedProgram {
    /// Establishes the program's identity, then reads its bytes and records their digest. Both are
    /// re-established by [`AdmittedProgram::verify`] before the program is executed.
    pub fn admit(
        role: ProgramRole,
        path: impl Into<PathBuf>,
        requirement: &ProgramRequirement,
    ) -> Result<Self, RuntimeError> {
        let path = path.into();
        if !path.is_file() {
            return Err(RuntimeError::InvalidProfile(format!(
                "{role} is not a regular file: {}",
                path.display()
            )));
        }
        let digest = digest_bytes(&std::fs::read(&path)?);
        let identity = match requirement {
            ProgramRequirement::SystemPath => {
                require_system_path(role, &path)?;
                ProgramIdentity::SystemPath
            }
            ProgramRequirement::StatedDigest(expected) => {
                if &digest != expected {
                    return Err(RuntimeError::InvalidProfile(format!(
                        "{role} at {} is not the program that was expected",
                        path.display()
                    )));
                }
                ProgramIdentity::StatedDigest
            }
        };
        Ok(Self {
            role,
            path,
            identity,
            digest,
        })
    }

    /// The requirement this program was admitted under, so a later holder of the record can
    /// re-establish the same binding instead of only re-reading the digest.
    pub fn requirement(&self) -> ProgramRequirement {
        match self.identity {
            ProgramIdentity::SystemPath => ProgramRequirement::SystemPath,
            ProgramIdentity::StatedDigest => ProgramRequirement::StatedDigest(self.digest.clone()),
        }
    }

    /// Re-establishes the program's identity and reads its bytes again, refusing when either the
    /// binding or the bytes differ from the admitted ones. A program admitted for its location is
    /// checked against that location again, so a holder of the record re-establishes the same
    /// binding rather than only re-reading a digest at a path someone else named.
    pub fn verify(&self) -> Result<(), RuntimeError> {
        if !self.path.is_file() {
            return Err(RuntimeError::InvalidProfile(format!(
                "{} is no longer a regular file: {}",
                self.role,
                self.path.display()
            )));
        }
        if self.identity == ProgramIdentity::SystemPath {
            require_system_path(self.role, &self.path)?;
        }
        // For a program admitted against a stated digest this comparison is the whole binding; for
        // one admitted against its location it is what catches a change to the bytes standing there.
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

/// Refuses unless the program stands where only the superuser could have put it. Every ancestor
/// directory is examined, because a writable directory anywhere on the path lets the account
/// exchange the file the name resolves to.
///
/// Ownership and mode answer only part of the question. A location can name this account, its
/// group or everyone in an access-control entry while its mode shows nothing, and an account that
/// holds such an entry can replace the file the name resolves to exactly as the mode would have
/// let it. Both grounds are therefore read, and the refusal states each one that failed.
#[cfg(unix)]
fn require_system_path(role: ProgramRole, path: &Path) -> Result<(), RuntimeError> {
    require_superuser_ancestry(role, path, AccessControlEntries::Read)
}

/// Whether the ancestry walk also reads the access-control entries of each location.
///
/// The entries are read by running the platform's own utility, and that utility stands under the
/// same system directories. Its own admission therefore takes [`AccessControlEntries::Unread`],
/// which is what stops the reading from requiring itself.
#[cfg(unix)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum AccessControlEntries {
    Read,
    Unread,
}

#[cfg(unix)]
fn require_superuser_ancestry(
    role: ProgramRole,
    path: &Path,
    entries: AccessControlEntries,
) -> Result<(), RuntimeError> {
    use std::os::unix::fs::MetadataExt;

    if !path.is_absolute() {
        return Err(RuntimeError::InvalidProfile(format!(
            "{role} was not resolved to an absolute path: {}",
            path.display()
        )));
    }
    let canonical = path.canonicalize()?;
    let mut current = Some(canonical.as_path());
    let mut refusals = Vec::new();
    while let Some(component) = current {
        let metadata = std::fs::metadata(component)?;
        let mut grounds = Vec::new();
        if metadata.uid() != 0 {
            grounds.push(format!(
                "it belongs to account {} rather than to the superuser",
                metadata.uid()
            ));
        }
        if metadata.mode() & 0o022 != 0 {
            grounds.push(format!(
                "its mode {:04o} grants write beyond its owner",
                metadata.mode() & 0o7777
            ));
        }
        if entries == AccessControlEntries::Read
            && let Some(entry) = access_control::write_grant(component)?
        {
            grounds.push(format!(
                "an access-control entry grants write access: {entry}"
            ));
        }
        if !grounds.is_empty() {
            refusals.push(format!("{} {}", component.display(), grounds.join(", ")));
        }
        // The walk continues past the first ground it finds, so that the refusal names every
        // location on the path this account can reach and the ground on which each one fails.
        current = component.parent();
    }
    if !refusals.is_empty() {
        return Err(RuntimeError::InvalidProfile(format!(
            "{role} at {} stands under a location which this account can write, so the bytes \
             admitted for it are not bound to the program the run means: {}",
            path.display(),
            refusals.join("; ")
        )));
    }
    Ok(())
}

/// Reads the access-control entries of a location, which its mode does not describe.
///
/// The entries are read by running the platform's own listing utility, because the calls that
/// answer the question belong to the C library and this workspace executes no unsafe code. That
/// utility is admitted before it runs, under the ownership and mode of its own location; the
/// entries of its ancestors cannot be read without running it, so that one leg of the rule cannot
/// apply to the reader itself. An account that already held a write-granting entry on a system
/// directory could therefore hide that entry from every later reading — but it could equally
/// replace the reader, the process table or the pinned runtime outright, so this is the same
/// boundary the rest of this record draws rather than a new one.
///
/// A verdict is remembered per location and per the moment that location's metadata last changed.
/// Adding, changing or removing an entry changes that moment, so a remembered verdict is never the
/// answer to a question about a location whose access has since been rearranged.
#[cfg(unix)]
mod access_control {
    use super::{AccessControlEntries, ProgramRole, RuntimeError};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::{Mutex, OnceLock};

    /// The platform's listing utility, which reports a location's entries beside its mode.
    pub const READER: &str = "/bin/ls";

    /// A location as it stood when its entries were read.
    #[derive(Eq, Hash, PartialEq)]
    struct Reading {
        path: PathBuf,
        device: u64,
        inode: u64,
        changed_seconds: i64,
        changed_nanoseconds: i64,
    }

    fn readings() -> &'static Mutex<HashMap<Reading, Option<String>>> {
        static READINGS: OnceLock<Mutex<HashMap<Reading, Option<String>>>> = OnceLock::new();
        READINGS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn reading(path: &Path) -> Result<Reading, RuntimeError> {
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::metadata(path)?;
        Ok(Reading {
            path: path.to_owned(),
            device: metadata.dev(),
            inode: metadata.ino(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }

    /// The entry that grants write access to this location, if it carries one. An error means the
    /// entries could not be read at all, which is refused rather than read as their absence.
    pub fn write_grant(path: &Path) -> Result<Option<String>, RuntimeError> {
        let reading = reading(path)?;
        if let Ok(readings) = readings().lock()
            && let Some(remembered) = readings.get(&reading)
        {
            return Ok(remembered.clone());
        }
        let reader = admitted_reader()?;
        let grant = write_grant_from(&reader, path)?;
        if let Ok(mut readings) = readings().lock() {
            readings.insert(reading, grant.clone());
        }
        Ok(grant)
    }

    /// Establishes that the reader stands where only the superuser could have put it, under the
    /// ownership and mode of its location alone.
    fn admitted_reader() -> Result<PathBuf, RuntimeError> {
        let path = Path::new(READER);
        if !path.is_file() {
            return Err(RuntimeError::InvalidProfile(format!(
                "{} is not a regular file: {READER}",
                ProgramRole::AccessControl
            )));
        }
        super::require_superuser_ancestry(
            ProgramRole::AccessControl,
            path,
            AccessControlEntries::Unread,
        )?;
        Ok(path.to_owned())
    }

    /// The permissions an entry may grant without letting its holder change what stands at the
    /// location. Anything else in an allowing entry is treated as write access, so a permission
    /// this list does not know is refused rather than passed over.
    #[cfg(target_os = "macos")]
    const READ_ONLY_PERMISSIONS: [&str; 12] = [
        "read",
        "execute",
        "list",
        "search",
        "readattr",
        "readextattr",
        "readsecurity",
        "file_inherit",
        "directory_inherit",
        "limit_inherit",
        "only_inherit",
        "inherited",
    ];

    /// Reads the entries themselves, which this platform's listing utility prints under the
    /// location when it is asked for them. A denying entry can only take access away, so only the
    /// allowing ones are judged; an entry whose text this rule cannot read is reported as a grant,
    /// because an unread entry is not an absent one.
    #[cfg(target_os = "macos")]
    fn write_grant_from(reader: &Path, path: &Path) -> Result<Option<String>, RuntimeError> {
        let output = Command::new(reader)
            .arg("-lde")
            .arg(path)
            .stderr(Stdio::null())
            .output()?;
        if !output.status.success() {
            return Err(RuntimeError::InvalidProfile(format!(
                "the access-control entries of {} could not be read",
                path.display()
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let Some((index, entry)) = line.split_once(':') else {
                continue;
            };
            let index = index.trim();
            if index.is_empty() || !index.chars().all(|digit| digit.is_ascii_digit()) {
                continue;
            }
            let words: Vec<&str> = entry.split_whitespace().collect();
            let Some(decision) = words
                .iter()
                .position(|word| *word == "allow" || *word == "deny")
            else {
                return Ok(Some(line.trim().to_owned()));
            };
            if words[decision] == "deny" {
                continue;
            }
            let Some(permissions) = words.get(decision + 1) else {
                return Ok(Some(line.trim().to_owned()));
            };
            if permissions
                .split(',')
                .any(|permission| !READ_ONLY_PERMISSIONS.contains(&permission))
            {
                return Ok(Some(line.trim().to_owned()));
            }
        }
        Ok(None)
    }

    /// Where the listing utility does not print the entries, it still marks a location whose
    /// access is decided by more than its mode. The utility that prints the entries themselves is
    /// a separate package that is not installed everywhere, so the mark is taken as the answer: a
    /// location this rule cannot describe is not admitted.
    #[cfg(not(target_os = "macos"))]
    fn write_grant_from(reader: &Path, path: &Path) -> Result<Option<String>, RuntimeError> {
        let output = Command::new(reader)
            .arg("-ld")
            .arg(path)
            .stderr(Stdio::null())
            .output()?;
        if !output.status.success() {
            return Err(RuntimeError::InvalidProfile(format!(
                "the access-control entries of {} could not be read",
                path.display()
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let Some(mode) = text.split_whitespace().next() else {
            return Err(RuntimeError::InvalidProfile(format!(
                "the access-control entries of {} could not be read",
                path.display()
            )));
        };
        Ok(mode
            .contains('+')
            .then(|| format!("its access is decided by more than the mode {mode}")))
    }
}

#[cfg(not(unix))]
fn require_system_path(role: ProgramRole, path: &Path) -> Result<(), RuntimeError> {
    let _ = (role, path);
    Err(RuntimeError::Unsupported(
        "a system-path program identity on this platform",
    ))
}

/// Refuses unless every admitted program still holds the bytes it was admitted with.
pub fn verify_admitted_programs(programs: &[AdmittedProgram]) -> Result<(), RuntimeError> {
    for program in programs {
        program.verify()?;
    }
    Ok(())
}

/// The programs the managed launch preamble enters before the runtime image is loaded. Product code
/// uses the system paths, whose identity is their location; the other constructor exists because a
/// check cannot substitute `/bin/sh` and the operating system refuses to execute a copy of it, so
/// there the caller states which bytes it means instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchChain {
    shell: (PathBuf, ProgramRequirement),
    sanitiser: Option<(PathBuf, ProgramRequirement)>,
}

impl LaunchChain {
    pub const SYSTEM_SHELL: &'static str = "/bin/sh";
    pub const SYSTEM_SANITISER: &'static str = "/usr/bin/env";

    /// Builds a chain whose programs are bound by the digests the caller states rather than by
    /// their location.
    #[doc(hidden)]
    pub fn stated(shell: (PathBuf, String), sanitiser: Option<(PathBuf, String)>) -> Self {
        Self {
            shell: (shell.0, ProgramRequirement::StatedDigest(shell.1)),
            sanitiser: sanitiser
                .map(|(path, digest)| (path, ProgramRequirement::StatedDigest(digest))),
        }
    }

    /// Records the identity and digest of every program in the chain. The set is fixed here rather
    /// than at spawn time, so a sanitiser that appears or disappears between admission and launch
    /// changes the chain and is refused instead of silently entering or leaving it.
    pub fn admit(&self) -> Result<Vec<AdmittedProgram>, RuntimeError> {
        if !cfg!(unix) {
            return Ok(Vec::new());
        }
        let mut chain = vec![AdmittedProgram::admit(
            ProgramRole::LaunchShell,
            self.shell.0.clone(),
            &self.shell.1,
        )?];
        if let Some((path, requirement)) = &self.sanitiser {
            chain.push(AdmittedProgram::admit(
                ProgramRole::EnvironmentSanitiser,
                path.clone(),
                requirement,
            )?);
        }
        Ok(chain)
    }
}

impl Default for LaunchChain {
    fn default() -> Self {
        let sanitiser = Path::new(Self::SYSTEM_SANITISER);
        Self {
            shell: (
                PathBuf::from(Self::SYSTEM_SHELL),
                ProgramRequirement::SystemPath,
            ),
            sanitiser: sanitiser
                .is_file()
                .then(|| (sanitiser.to_owned(), ProgramRequirement::SystemPath)),
        }
    }
}

/// The lifecycle utilities a managed run executes to observe and terminate what it started. Each is
/// named by an absolute system path rather than resolved through the environment search path, and
/// each is admitted under that location before it is executed.
#[cfg(unix)]
mod lifecycle {
    use super::{AdmittedProgram, ProgramRequirement, ProgramRole, RuntimeError};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    pub const PROCESS_TABLE: &str = "/bin/ps";
    pub const SIGNAL: &str = "/bin/kill";
    pub const DESCRIPTOR_HOLDERS: [&str; 2] = ["/usr/sbin/lsof", "/usr/bin/lsof"];

    fn ledger() -> &'static Mutex<BTreeMap<PathBuf, AdmittedProgram>> {
        static LEDGER: OnceLock<Mutex<BTreeMap<PathBuf, AdmittedProgram>>> = OnceLock::new();
        LEDGER.get_or_init(|| Mutex::new(BTreeMap::new()))
    }

    fn placements() -> &'static Mutex<BTreeMap<ProgramRole, Vec<PathBuf>>> {
        static PLACEMENTS: OnceLock<Mutex<BTreeMap<ProgramRole, Vec<PathBuf>>>> = OnceLock::new();
        PLACEMENTS.get_or_init(|| Mutex::new(BTreeMap::new()))
    }

    /// Where a check placed a utility, so that a run which cannot admit one of them can be
    /// observed on a machine where all of them are in order. Nothing the product ships reaches
    /// this, which `ymp-cli/tests/unattested_runtime_is_unreachable.rs` establishes by reading the
    /// shipped source of every crate rather than by convention.
    pub fn place_for_fixture(role: ProgramRole, locations: Vec<PathBuf>) {
        if let Ok(mut placements) = placements().lock() {
            placements.insert(role, locations);
        }
    }

    /// The locations this platform offers for the role, in the order they are tried.
    fn locations(role: ProgramRole) -> Vec<PathBuf> {
        if let Ok(placements) = placements().lock()
            && let Some(placed) = placements.get(&role)
        {
            return placed.clone();
        }
        match role {
            ProgramRole::ProcessTable => vec![PathBuf::from(PROCESS_TABLE)],
            ProgramRole::Signal => vec![PathBuf::from(SIGNAL)],
            ProgramRole::DescriptorHolders => {
                DESCRIPTOR_HOLDERS.iter().map(PathBuf::from).collect()
            }
            ProgramRole::AccessControl => vec![PathBuf::from(super::access_control::READER)],
            _ => Vec::new(),
        }
    }

    /// Admits the utility on its first use and re-establishes its identity and bytes on every later
    /// use, so a utility that changes under a running supervisor stops being used.
    pub fn admitted(role: ProgramRole, path: &Path) -> Result<PathBuf, RuntimeError> {
        let program = AdmittedProgram::admit(role, path, &ProgramRequirement::SystemPath)?;
        let mut ledger = ledger().lock().map_err(|_| {
            RuntimeError::InvalidProfile("admitted program ledger failed".to_owned())
        })?;
        match ledger.get(path) {
            Some(recorded) if recorded == &program => {}
            Some(_) => {
                return Err(RuntimeError::InvalidProfile(format!(
                    "{role} changed after admission: {}",
                    path.display()
                )));
            }
            None => {
                ledger.insert(path.to_owned(), program);
            }
        }
        Ok(path.to_owned())
    }

    /// Resolves the utility that answers for the role and re-establishes it before it is executed.
    ///
    /// A location that stands empty is passed over, because a platform is free to keep the utility
    /// elsewhere. A location that holds a program which cannot be admitted is refused instead: a
    /// later candidate would answer a question this one has already answered wrongly, and the
    /// caller must learn that the run cannot see what it started.
    pub fn program(role: ProgramRole) -> Result<PathBuf, RuntimeError> {
        let locations = locations(role);
        for location in &locations {
            if !location.is_file() {
                continue;
            }
            return admitted(role, location);
        }
        Err(RuntimeError::InvalidProfile(format!(
            "{role} is present at none of {}",
            locations
                .iter()
                .map(|location| location.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }

    /// The record of the admitted utility, for the evidence the run writes before it starts.
    fn record(role: ProgramRole) -> Result<AdmittedProgram, RuntimeError> {
        let path = program(role)?;
        let ledger = ledger().lock().map_err(|_| {
            RuntimeError::InvalidProfile("admitted program ledger failed".to_owned())
        })?;
        ledger.get(&path).cloned().ok_or_else(|| {
            RuntimeError::InvalidProfile(format!(
                "{role} left no admission record: {}",
                path.display()
            ))
        })
    }

    /// Admits every lifecycle utility a managed run needs on this platform.
    ///
    /// Each one is required. A run that cannot admit the reader of the process table cannot see the
    /// processes it starts, and a run that cannot admit the signal program cannot end them; either
    /// way the absence of survivors would be the absence of an answer rather than a clean
    /// termination. The descriptor holders are asked for only where the process file system that
    /// answers the same question is absent.
    pub fn admit_all() -> Result<Vec<AdmittedProgram>, RuntimeError> {
        let mut programs = vec![
            record(ProgramRole::AccessControl)?,
            record(ProgramRole::ProcessTable)?,
            record(ProgramRole::Signal)?,
        ];
        if !Path::new("/proc/self/fd").is_dir() {
            programs.push(record(ProgramRole::DescriptorHolders)?);
        }
        Ok(programs)
    }
}

/// Admits every lifecycle utility a managed run needs and returns the record, so the run names each
/// program it executes on its own behalf before executing any of them — and refuses to start when
/// one of them cannot be admitted, rather than starting with a shortened record and reporting later
/// that it saw nothing.
#[cfg(unix)]
pub fn admit_lifecycle_programs() -> Result<Vec<AdmittedProgram>, RuntimeError> {
    lifecycle::admit_all()
}

#[cfg(not(unix))]
pub fn admit_lifecycle_programs() -> Result<Vec<AdmittedProgram>, RuntimeError> {
    Ok(Vec::new())
}

/// Places a lifecycle utility for a check that must observe a run which cannot admit one of them.
/// No shipped module reaches this; `ymp-cli/tests/unattested_runtime_is_unreachable.rs` reads the
/// source of every crate to establish that.
#[doc(hidden)]
#[cfg(unix)]
pub fn place_lifecycle_utility_for_fixture(role: ProgramRole, locations: Vec<PathBuf>) {
    lifecycle::place_for_fixture(role, locations);
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

/// One model's named share of a run's monetary consumption, as the runtime reported it.
#[derive(Clone, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(default)]
pub struct ModelSpend {
    pub model: String,
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
    /// The models that produced the recorded cost and what each of them spent, sorted by model
    /// name. An empty breakdown beside a recorded cost states that the cost reached the record
    /// unattributed; it is never filled in from the admitted profile, because a name the runtime
    /// did not report is not evidence of the route that spent the money.
    pub cost_by_model: Vec<ModelSpend>,
    pub wall_time_ms: u64,
    pub protected_queries: u64,
    pub in_flight_excess: InFlightExcess,
}

impl Usage {
    /// Folds one turn's per-model shares into the record, keeping a single entry per model and a
    /// stable order by model name.
    pub fn absorb_model_spend(&mut self, spends: &[ModelSpend]) {
        for spend in spends {
            match self
                .cost_by_model
                .iter_mut()
                .find(|recorded| recorded.model == spend.model)
            {
                Some(recorded) => {
                    recorded.cost_microusd =
                        recorded.cost_microusd.saturating_add(spend.cost_microusd);
                }
                None => self.cost_by_model.push(spend.clone()),
            }
        }
        self.cost_by_model.sort();
    }

    /// The sum of the named shares.
    pub fn attributed_cost_microusd(&self) -> u64 {
        self.cost_by_model
            .iter()
            .map(|spend| spend.cost_microusd)
            .fold(0_u64, u64::saturating_add)
    }

    /// Whether the recorded cost is backed by the models that produced it. A record with no cost
    /// has nothing to attribute and counts as attributed; a recorded cost whose named shares are
    /// missing or do not add up to it is unverified. Each share is rounded to whole microdollars
    /// before it is summed, so the sum may only trail the total — by less than one microdollar per
    /// named model — and never exceed it: an overshoot is not rounding and reads as unverified.
    /// The bound is per model, not per accounted turn, so a many-turn run can trail further and
    /// read conservatively as unverified; the record carries no turn count to widen it honestly.
    pub fn cost_is_attributed(&self) -> bool {
        let Some(total) = self.cost_microusd else {
            return true;
        };
        if total == 0 {
            return true;
        }
        if self.cost_by_model.is_empty() {
            return false;
        }
        let attributed = self.attributed_cost_microusd();
        attributed <= total && total - attributed <= self.cost_by_model.len() as u64
    }
}

pub const TOOL_HOST_PROBE_SCHEMA_VERSION: u32 = 3;
pub const TOOL_HOST_PROBE_WORKSPACE_SERVER: &str = "ymp.workspace";
pub const TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION: &str = "2025-11-25";
pub const TOOL_HOST_PROBE_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND: &str = "tool-host-probe-mcp";
pub const TOOL_HOST_PROBE_INTERNAL_ARGUMENTS: [&str; 2] =
    ["internal", TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND];
pub const TOOL_HOST_PROBE_ENVIRONMENT: [&str; 3] = [
    "YMP_TOOL_HOST_PROBE_WORKSPACE_ROOT",
    "YMP_TOOL_HOST_PROBE_PATH",
    "YMP_TOOL_HOST_PROBE_NONCE",
];
pub const TOOL_HOST_PROBE_TOOL_SCHEMA: &str = concat!(
    r#"{"schema_version":2,"tools":["#,
    r#"{"name":"workspace_write","description":"Write the controller nonce once at the controller path.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false}},"#,
    r#"{"name":"workspace_read","description":"Read the controller nonce once from the controller path.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}]}"#,
);

pub fn tool_host_probe_tool_schema_digest() -> String {
    digest_bytes(TOOL_HOST_PROBE_TOOL_SCHEMA.as_bytes())
}

/// The only tools a no-task-output probe invocation may receive. The fixed order is part of the
/// probe schema: the nonce is written once and then read once.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHostProbeTool {
    WorkspaceWrite,
    WorkspaceRead,
}

impl std::fmt::Display for ToolHostProbeTool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::WorkspaceWrite => "workspace_write",
            Self::WorkspaceRead => "workspace_read",
        })
    }
}

/// The exact stdio MCP transport measured by the trusted foreground controller before a probe
/// reservation is spent. Per-probe path and nonce bindings remain in the controller reservation;
/// this identity covers only the fixed transport contract and its canonical workspace root.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeTransportIdentity {
    pub mcp_protocol_version: String,
    pub server_name: String,
    pub server_version: String,
    pub tool_schema_digest: String,
    pub ordered_tools: [ToolHostProbeTool; 2],
    pub server_executable_digest: String,
    pub launcher_executable_digest: String,
    pub internal_subcommand: String,
    pub arguments: Vec<String>,
    pub inherited_environment: Vec<String>,
    pub canonical_workspace_root_digest: String,
}

pub fn probe_transport_digest(identity: &ProbeTransportIdentity) -> String {
    let bytes = serde_json::to_vec(identity).expect("probe transport identity is serializable");
    digest_bytes(&bytes)
}

/// The exact runtime tuple a caller expects the probe to exercise. The runtime reports the same
/// tuple independently through [`RuntimeDriver::tool_host_probe_identity`]; the supervisor returns
/// no trace when the two differ.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostProbeRuntimeIdentity {
    pub runtime_kind: RuntimeKind,
    pub route: String,
    pub profile: String,
    pub cli: String,
    pub cli_version: String,
    pub compatibility_contract_digest: String,
    pub executable_digest: String,
    pub driver: String,
    pub driver_version: String,
    pub tool_schema_digest: String,
    pub probe_transport: ProbeTransportIdentity,
    pub probe_transport_digest: String,
}

impl ToolHostProbeRuntimeIdentity {
    pub fn with_probe_transport(mut self, transport: ProbeTransportIdentity) -> Self {
        self.probe_transport_digest = probe_transport_digest(&transport);
        self.probe_transport = transport;
        self
    }
}

/// One reservation dedicated to a probe. Workspace effects are counted separately from every
/// run, task, candidate and communication dimension so none of those accounts can satisfy it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostProbeResourceVector {
    pub model_calls: u64,
    pub max_input_tokens: u64,
    pub max_cached_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_reasoning_output_tokens: u64,
    pub max_cost_microusd: Option<u64>,
    pub max_wall_time_ms: u64,
    pub workspace_reads: u64,
    pub workspace_writes: u64,
    pub invocation_starts: u64,
    pub protected_queries: u64,
    pub external_actions: u64,
    pub participant_starts: u64,
    pub attempt_starts: u64,
    pub offer_creations: u64,
    pub obligation_creations: u64,
    pub board_actions: u64,
    pub task_actions: u64,
    pub recruitment_actions: u64,
    pub candidate_actions: u64,
    pub communication_actions: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostProbeRequest {
    pub schema_version: u32,
    pub probe_id: String,
    pub invocation_id: String,
    pub nonce: String,
    pub workspace_path: PathBuf,
    pub deadline_ms: u64,
    pub resource_reservation: ToolHostProbeResourceVector,
    pub expected_runtime: ToolHostProbeRuntimeIdentity,
    #[serde(skip)]
    pub cancellation: CancellationToken,
}

/// The invocation delivered to a driver after the supervisor has checked the caller's request,
/// workspace boundary, separate reservation and expected runtime identity.
#[derive(Clone, Debug)]
pub struct ToolHostProbeInvocation {
    pub request: ToolHostProbeRequest,
    pub workspace: PathBuf,
    pub allowed_tools: [ToolHostProbeTool; 2],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHostProbeCostAvailability {
    Reported,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostProbeCost {
    pub availability: ToolHostProbeCostAvailability,
    pub currency: Option<String>,
    pub amount_microusd: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostProbeToolEventDigest {
    pub sequence: u64,
    pub tool: ToolHostProbeTool,
    pub arguments_digest: String,
    pub result_digest: String,
}

/// A successful runtime report says only that the untrusted runtime emitted the expected bounded
/// event sequence. It is not controller read-back, persistence, attestation or admission.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHostProbeTrust {
    UntrustedRuntimeTrace,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHostProbeTerminal {
    Completed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostProbeTrace {
    pub schema_version: u32,
    pub probe_id: String,
    pub invocation_id: String,
    pub nonce_input: String,
    pub runtime_reported_readback: String,
    pub workspace_path: PathBuf,
    pub deadline_ms: u64,
    pub resource_reservation: ToolHostProbeResourceVector,
    pub runtime: ToolHostProbeRuntimeIdentity,
    pub model_calls: u64,
    pub usage: Usage,
    pub cost: ToolHostProbeCost,
    pub input_digest: String,
    pub output_digest: String,
    pub tool_event_digests: [ToolHostProbeToolEventDigest; 2],
    pub event_digest: String,
    pub terminal: ToolHostProbeTerminal,
    pub trust: ToolHostProbeTrust,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolHostProbeEffect {
    Board,
    Task,
    Recruitment,
    Candidate,
}

impl std::fmt::Display for ToolHostProbeEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Board => "board",
            Self::Task => "task",
            Self::Recruitment => "recruitment",
            Self::Candidate => "candidate",
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ToolHostProbeError {
    #[error("unsupported tool-host probe schema version {found}")]
    UnsupportedSchema { found: u32 },
    #[error("invalid tool-host probe {field}")]
    InvalidIdentity { field: &'static str },
    #[error("tool-host probe nonce must contain between 1 and {max_bytes} bytes")]
    InvalidNonce { max_bytes: usize },
    #[error("tool-host probe path is not a normalized relative workspace path")]
    InvalidWorkspacePath,
    #[error("tool-host probe workspace is unavailable: {detail}")]
    WorkspaceUnavailable { detail: String },
    #[error("tool-host probe deadline is invalid")]
    InvalidDeadline,
    #[error("tool-host probe reservation is invalid in {field}")]
    InvalidReservation { field: &'static str },
    #[error("tool-host probe supports only the fake-runtime boundary in this build")]
    LiveRuntimeForbidden,
    #[error("tool-host runtime probe failed: {detail}")]
    RuntimeProbeFailed { detail: String },
    #[error("tool-host runtime is not ready: {detail}")]
    RuntimeNotReady { detail: String },
    #[error("tool-host runtime identity differs in {field}")]
    RuntimeIdentityMismatch { field: &'static str },
    #[error("tool-host probe invocation could not start: {detail}")]
    StartFailed { detail: String },
    #[error("tool-host probe event stream is invalid: {detail}")]
    InvalidEventStream { detail: String },
    #[error("tool-host probe is missing {tool}")]
    MissingToolEvent { tool: ToolHostProbeTool },
    #[error("tool-host probe used unexpected tool {server}/{tool}")]
    UnexpectedToolUse { server: String, tool: String },
    #[error("tool-host probe attempted forbidden {effect} effect through {server}/{tool}")]
    ForbiddenEffect {
        effect: ToolHostProbeEffect,
        server: String,
        tool: String,
    },
    #[error("tool-host probe {tool} event has invalid {field}")]
    InvalidToolEvent {
        tool: ToolHostProbeTool,
        field: &'static str,
    },
    #[error("tool-host probe emitted task output")]
    UnexpectedOutput,
    #[error("tool-host probe usage is incomplete in {field}")]
    IncompleteUsage { field: &'static str },
    #[error("tool-host probe exceeded its reservation in {field}")]
    ReservationExceeded { field: &'static str },
    #[error("tool-host probe reported an ambiguous terminal")]
    AmbiguousTerminal,
    #[error("tool-host probe exceeded its {limit_ms}-millisecond deadline")]
    TimedOut { limit_ms: u64 },
    #[error("tool-host probe was cancelled")]
    Cancelled,
    #[error("tool-host probe runtime failed: {detail}")]
    RuntimeFailed { detail: String },
    #[error("{0}")]
    RuntimeTerminalFailed(Box<ToolHostProbeTerminalFailure>),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFailureKind {
    ProcessExit,
    RuntimeReported,
    Protocol,
    OutputLimit,
}

/// A terminal runtime failure observed as a complete structured event. Unlike
/// [`ToolHostProbeError::RuntimeFailed`], this record preserves every bounded field the runtime
/// supplied and never stores raw diagnostic bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHostProbeTerminalFailure {
    pub kind: RuntimeFailureKind,
    pub usage: Usage,
    pub diagnostic: Option<DiagnosticSummary>,
    pub event_id: String,
    pub sequence: u64,
}

impl std::fmt::Display for ToolHostProbeTerminalFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "tool-host probe runtime terminal failure: kind={:?}, event_id={}, sequence={}, \
             usage[input_tokens={}, cached_input_tokens={}, output_tokens={}, \
             reasoning_output_tokens={}, cost_microusd=",
            self.kind,
            self.event_id,
            self.sequence,
            self.usage.input_tokens,
            self.usage.cached_input_tokens,
            self.usage.output_tokens,
            self.usage.reasoning_output_tokens,
        )?;
        match self.usage.cost_microusd {
            Some(cost) => write!(formatter, "{cost}")?,
            None => formatter.write_str("unavailable")?,
        }
        write!(
            formatter,
            ", cost_by_model_entries={}, attributed_cost_microusd={}, wall_time_ms={}, \
             protected_queries={}, in_flight_model_requests={}, in_flight_input_tokens={}, \
             in_flight_cached_input_tokens={}, in_flight_output_tokens={}, \
             in_flight_reasoning_output_tokens={}, in_flight_cost_microusd={}], diagnostic=",
            self.usage.cost_by_model.len(),
            self.usage.attributed_cost_microusd(),
            self.usage.wall_time_ms,
            self.usage.protected_queries,
            self.usage.in_flight_excess.model_requests,
            self.usage.in_flight_excess.input_tokens,
            self.usage.in_flight_excess.cached_input_tokens,
            self.usage.in_flight_excess.output_tokens,
            self.usage.in_flight_excess.reasoning_output_tokens,
            self.usage.in_flight_excess.cost_microusd,
        )?;
        match &self.diagnostic {
            Some(diagnostic) => diagnostic.fmt(formatter),
            None => formatter.write_str("none"),
        }
    }
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
    fn tool_host_probe_identity(&self) -> Result<ToolHostProbeRuntimeIdentity, RuntimeError> {
        Err(RuntimeError::Unsupported("tool-host probe identity"))
    }
    fn start_tool_host_probe(
        &self,
        _request: ToolHostProbeInvocation,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        Err(RuntimeError::Unsupported("tool-host probe invocation"))
    }
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

#[cfg(test)]
mod tool_host_probe_schema_tests {
    use super::{
        DiagnosticSummary, InFlightExcess, ModelSpend, ProbeTransportIdentity, RuntimeFailureKind,
        TOOL_HOST_PROBE_ENVIRONMENT, TOOL_HOST_PROBE_INTERNAL_ARGUMENTS,
        TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND, TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION,
        TOOL_HOST_PROBE_SCHEMA_VERSION, TOOL_HOST_PROBE_SERVER_VERSION,
        TOOL_HOST_PROBE_TOOL_SCHEMA, TOOL_HOST_PROBE_WORKSPACE_SERVER, ToolHostProbeCost,
        ToolHostProbeCostAvailability, ToolHostProbeError, ToolHostProbeRequest,
        ToolHostProbeResourceVector, ToolHostProbeRuntimeIdentity, ToolHostProbeTerminal,
        ToolHostProbeTerminalFailure, ToolHostProbeTool, ToolHostProbeToolEventDigest,
        ToolHostProbeTrace, ToolHostProbeTrust, Usage, evidence_digest, probe_transport_digest,
        tool_host_probe_tool_schema_digest,
    };
    use crate::{CancellationToken, RuntimeKind};
    use serde_json::Value;
    use std::path::PathBuf;

    fn reservation() -> ToolHostProbeResourceVector {
        ToolHostProbeResourceVector {
            model_calls: 1,
            max_input_tokens: 32,
            max_cached_input_tokens: 16,
            max_output_tokens: 16,
            max_reasoning_output_tokens: 8,
            max_cost_microusd: Some(100),
            max_wall_time_ms: 1_000,
            workspace_reads: 1,
            workspace_writes: 1,
            invocation_starts: 1,
            protected_queries: 0,
            external_actions: 0,
            participant_starts: 0,
            attempt_starts: 0,
            offer_creations: 0,
            obligation_creations: 0,
            board_actions: 0,
            task_actions: 0,
            recruitment_actions: 0,
            candidate_actions: 0,
            communication_actions: 0,
        }
    }

    fn identity() -> ToolHostProbeRuntimeIdentity {
        let probe_transport = transport_identity();
        ToolHostProbeRuntimeIdentity {
            runtime_kind: RuntimeKind::Fake,
            route: "fixture-route".to_owned(),
            profile: "fixture-profile".to_owned(),
            cli: "/fixture/runtime".to_owned(),
            cli_version: "fixture-cli 1".to_owned(),
            compatibility_contract_digest: "4".repeat(64),
            executable_digest: "5".repeat(64),
            driver: "fixture-driver".to_owned(),
            driver_version: "fixture-driver 1".to_owned(),
            tool_schema_digest: tool_host_probe_tool_schema_digest(),
            probe_transport_digest: probe_transport_digest(&probe_transport),
            probe_transport,
        }
    }

    fn transport_identity() -> ProbeTransportIdentity {
        ProbeTransportIdentity {
            mcp_protocol_version: TOOL_HOST_PROBE_MCP_PROTOCOL_VERSION.to_owned(),
            server_name: TOOL_HOST_PROBE_WORKSPACE_SERVER.to_owned(),
            server_version: TOOL_HOST_PROBE_SERVER_VERSION.to_owned(),
            tool_schema_digest: tool_host_probe_tool_schema_digest(),
            ordered_tools: [
                ToolHostProbeTool::WorkspaceWrite,
                ToolHostProbeTool::WorkspaceRead,
            ],
            server_executable_digest: "1".repeat(64),
            launcher_executable_digest: "2".repeat(64),
            internal_subcommand: TOOL_HOST_PROBE_INTERNAL_SUBCOMMAND.to_owned(),
            arguments: TOOL_HOST_PROBE_INTERNAL_ARGUMENTS
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            inherited_environment: TOOL_HOST_PROBE_ENVIRONMENT
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            canonical_workspace_root_digest: "3".repeat(64),
        }
    }

    fn request() -> ToolHostProbeRequest {
        ToolHostProbeRequest {
            schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
            probe_id: "probe-1".to_owned(),
            invocation_id: "invocation-1".to_owned(),
            nonce: "opaque-nonce".to_owned(),
            workspace_path: PathBuf::from("probe/nonce.txt"),
            deadline_ms: 1_000,
            resource_reservation: reservation(),
            expected_runtime: identity(),
            cancellation: CancellationToken::default(),
        }
    }

    fn trace() -> ToolHostProbeTrace {
        let request = request();
        ToolHostProbeTrace {
            schema_version: TOOL_HOST_PROBE_SCHEMA_VERSION,
            probe_id: request.probe_id,
            invocation_id: request.invocation_id,
            nonce_input: request.nonce.clone(),
            runtime_reported_readback: request.nonce.clone(),
            workspace_path: request.workspace_path,
            deadline_ms: request.deadline_ms,
            resource_reservation: request.resource_reservation,
            runtime: request.expected_runtime,
            model_calls: 1,
            usage: Usage {
                input_tokens: 3,
                output_tokens: 2,
                wall_time_ms: 7,
                ..Usage::default()
            },
            cost: ToolHostProbeCost {
                availability: ToolHostProbeCostAvailability::Unavailable,
                currency: None,
                amount_microusd: None,
            },
            input_digest: "a".repeat(64),
            output_digest: "a".repeat(64),
            tool_event_digests: [
                ToolHostProbeToolEventDigest {
                    sequence: 2,
                    tool: ToolHostProbeTool::WorkspaceWrite,
                    arguments_digest: "b".repeat(64),
                    result_digest: "c".repeat(64),
                },
                ToolHostProbeToolEventDigest {
                    sequence: 3,
                    tool: ToolHostProbeTool::WorkspaceRead,
                    arguments_digest: "d".repeat(64),
                    result_digest: "e".repeat(64),
                },
            ],
            event_digest: "f".repeat(64),
            terminal: ToolHostProbeTerminal::Completed,
            trust: ToolHostProbeTrust::UntrustedRuntimeTrace,
        }
    }

    #[test]
    fn request_and_trace_refuse_schema_extensions() {
        let mut request = serde_json::to_value(request()).expect("request value");
        request["unknown_authority"] = Value::Bool(true);
        assert!(serde_json::from_value::<ToolHostProbeRequest>(request).is_err());

        let mut extended_trace = serde_json::to_value(trace()).expect("trace value");
        extended_trace["unknown_authority"] = Value::Bool(true);
        assert!(serde_json::from_value::<ToolHostProbeTrace>(extended_trace).is_err());

        let mut missing_transport = serde_json::to_value(trace()).expect("trace value");
        missing_transport["runtime"]
            .as_object_mut()
            .expect("runtime object")
            .remove("probe_transport");
        assert!(serde_json::from_value::<ToolHostProbeTrace>(missing_transport).is_err());

        let mut missing_digest = serde_json::to_value(trace()).expect("trace value");
        missing_digest["runtime"]
            .as_object_mut()
            .expect("runtime object")
            .remove("probe_transport_digest");
        assert!(serde_json::from_value::<ToolHostProbeTrace>(missing_digest).is_err());

        for field in ["compatibility_contract_digest", "executable_digest"] {
            let mut missing = serde_json::to_value(trace()).expect("trace value");
            missing["runtime"]
                .as_object_mut()
                .expect("runtime object")
                .remove(field);
            assert!(
                serde_json::from_value::<ToolHostProbeTrace>(missing).is_err(),
                "runtime identity accepted missing {field}"
            );
        }
    }

    #[test]
    fn trace_vocabulary_cannot_claim_controller_authority() {
        let value = serde_json::to_value(trace()).expect("trace value");
        let object = value.as_object().expect("trace object");
        for forbidden in [
            "controller_read_back",
            "persisted",
            "attested",
            "model_ready",
            "admitted",
        ] {
            assert!(
                !object.contains_key(forbidden),
                "untrusted trace exposed authority field {forbidden}"
            );
        }
        assert_eq!(value["trust"], "untrusted_runtime_trace");

        let mut injected = value;
        injected["model_ready"] = Value::Bool(true);
        assert!(serde_json::from_value::<ToolHostProbeTrace>(injected).is_err());
    }

    #[test]
    fn fixed_tool_schema_contains_only_one_write_and_one_read() {
        let schema: Value =
            serde_json::from_str(TOOL_HOST_PROBE_TOOL_SCHEMA).expect("tool schema JSON");
        let tools = schema["tools"].as_array().expect("tool list");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "workspace_write");
        assert_eq!(tools[1]["name"], "workspace_read");
        assert_eq!(tool_host_probe_tool_schema_digest().len(), 64);
    }

    #[test]
    fn transport_digest_discriminates_every_bound_transport_dimension() {
        let identity = transport_identity();
        let digest = probe_transport_digest(&identity);
        let mut mutations = Vec::new();

        let mut mutated = identity.clone();
        mutated.mcp_protocol_version.push_str("-other");
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.server_name.push_str(".other");
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.server_version.push_str("-other");
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.tool_schema_digest = "4".repeat(64);
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.ordered_tools.swap(0, 1);
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.server_executable_digest = "5".repeat(64);
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.launcher_executable_digest = "6".repeat(64);
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.internal_subcommand.push_str("-other");
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated.arguments.push("--other".to_owned());
        mutations.push(mutated);
        let mut mutated = identity.clone();
        mutated
            .inherited_environment
            .push("YMP_TOOL_HOST_PROBE_OTHER".to_owned());
        mutations.push(mutated);
        let mut mutated = identity;
        mutated.canonical_workspace_root_digest = "7".repeat(64);
        mutations.push(mutated);

        for mutation in mutations {
            assert_ne!(probe_transport_digest(&mutation), digest);
        }
    }

    #[test]
    fn transport_identity_refuses_unknown_fields() {
        let mut value = serde_json::to_value(transport_identity()).expect("transport identity");
        value["child_claimed_expected_digest"] = Value::String("a".repeat(64));
        assert!(serde_json::from_value::<ProbeTransportIdentity>(value).is_err());
    }

    #[test]
    fn terminal_failure_serialization_equality_and_display_preserve_only_bounded_diagnostics() {
        let raw_diagnostic = b"raw child stderr must never survive";
        let diagnostic = DiagnosticSummary::from_bytes(raw_diagnostic, true);
        let usage = Usage {
            input_tokens: 101,
            cached_input_tokens: 41,
            output_tokens: 17,
            reasoning_output_tokens: 9,
            cost_microusd: Some(73),
            cost_by_model: vec![ModelSpend {
                model: "fixture-model".to_owned(),
                cost_microusd: 73,
            }],
            wall_time_ms: 4_321,
            protected_queries: 2,
            in_flight_excess: InFlightExcess {
                model_requests: 1,
                input_tokens: 3,
                cached_input_tokens: 4,
                output_tokens: 5,
                reasoning_output_tokens: 6,
                cost_microusd: 7,
            },
        };
        let failure = ToolHostProbeTerminalFailure {
            kind: RuntimeFailureKind::ProcessExit,
            usage,
            diagnostic: Some(diagnostic.clone()),
            event_id: "invocation-1.event-7".to_owned(),
            sequence: 7,
        };
        let error = ToolHostProbeError::RuntimeTerminalFailed(Box::new(failure.clone()));
        assert_eq!(error, error.clone());

        let serialized = serde_json::to_value(&failure).expect("serialize structured failure");
        let round_trip: ToolHostProbeTerminalFailure =
            serde_json::from_value(serialized).expect("deserialize structured terminal fields");
        assert_eq!(round_trip, failure);
        assert_eq!(round_trip.diagnostic, Some(diagnostic.clone()));
        assert_eq!(diagnostic.digest, evidence_digest(raw_diagnostic));

        let json = serde_json::to_string(&failure).expect("serialize terminal fields");
        let display = error.to_string();
        for expected in [
            "kind=ProcessExit",
            "event_id=invocation-1.event-7",
            "sequence=7",
            "input_tokens=101",
            "cached_input_tokens=41",
            "output_tokens=17",
            "reasoning_output_tokens=9",
            "cost_microusd=73",
            "wall_time_ms=4321",
            diagnostic.digest.as_str(),
            "bytes=35",
            "truncated=true",
        ] {
            assert!(
                display.contains(expected),
                "display omitted {expected}: {display}"
            );
        }
        let raw = String::from_utf8_lossy(raw_diagnostic);
        assert!(!json.contains(raw.as_ref()));
        assert!(!display.contains(raw.as_ref()));

        let without_diagnostic = ToolHostProbeTerminalFailure {
            diagnostic: None,
            ..failure
        };
        let serialized = serde_json::to_value(&without_diagnostic)
            .expect("serialize explicit missing diagnostic");
        assert!(serialized["diagnostic"].is_null());
        assert!(without_diagnostic.to_string().contains("diagnostic=none"));
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
        // The programs the preamble enters are selected by the role they were admitted under. The
        // record also names programs the run executed earlier on its own behalf, such as the reader
        // of the operator's credential; those are not entered here.
        let entered = |role| {
            let mut matching = chain.iter().filter(|program| program.role == role);
            let first = matching.next();
            (matching.next().is_none()).then_some(first).flatten()
        };
        let shell = entered(ProgramRole::LaunchShell)
            .ok_or_else(|| {
                RuntimeError::InvalidProfile(
                    "managed launch chain admits no single launch shell".to_owned(),
                )
            })?
            .path
            .clone();
        let preamble = match entered(ProgramRole::EnvironmentSanitiser) {
            Some(sanitiser) => {
                let sanitiser = shell_quoted(&sanitiser.path)?;
                format!("{OPEN_MARKER}{sanitiser} -u PWD -u SHLVL -u OLDPWD -u _ \"$@\"")
            }
            None if chain
                .iter()
                .all(|program| program.role != ProgramRole::EnvironmentSanitiser) =>
            {
                format!("{OPEN_MARKER}\"$@\"")
            }
            None => {
                return Err(RuntimeError::InvalidProfile(
                    "managed launch chain admits more than one environment sanitiser".to_owned(),
                ));
            }
        };
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

/// The processes that hold the given marker open, or the reason this could not be established.
///
/// Exposed so that a check can measure the difference between "nobody holds this marker" and "this
/// marker could not be read", which the underlying utility reports alike.
#[doc(hidden)]
#[cfg(unix)]
pub fn marker_holders(marker: &Path) -> Result<Vec<u32>, RuntimeError> {
    descendants::holders_of(marker)
}

/// Identifies the processes a managed run started. The authoritative property is the run's marker,
/// which every descendant carries whatever becomes of its ancestors; the process table is read as
/// well, so a descendant still attached by parent or process group is found even when it was started
/// by a caller that installed no marker.
#[cfg(unix)]
mod descendants {
    use super::RuntimeError;
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
    ///
    /// A failure to ask the question is not the answer "nobody". A utility that cannot be admitted,
    /// or that cannot be executed, is reported to the caller, because a run that reported no
    /// holders here would report a clean termination it never established.
    pub fn holders_of(marker: &Path) -> Result<Vec<u32>, RuntimeError> {
        // Linux reports open descriptors in its own process file system, so no external program is
        // needed there.
        let holders = if Path::new("/proc/self/fd").is_dir() {
            proc_holders(marker)?
        } else {
            let program = super::lifecycle::program(super::ProgramRole::DescriptorHolders)?;
            let output = Command::new(&program)
                .arg("-t")
                .arg(marker)
                .stderr(Stdio::null())
                .output()?;
            holders_reported(output.status.code(), &output.stdout).map_err(|reason| {
                RuntimeError::InvalidProfile(format!("{reason}: {}", program.display()))
            })?
        };
        if holders.is_empty() {
            readable_marker(marker)?;
        }
        Ok(holders)
    }

    /// Confirms the marker an empty answer was given about.
    ///
    /// "No process holds this file" and "this file could not be read" reach the caller in the same
    /// shape: the utility exits with code one and prints nothing in both cases, and the process file
    /// system silently skips a process whose descriptors this account may not list. An empty list is
    /// therefore an answer only where the marker still exists and this account can open it. Where it
    /// cannot, the question was not answered, and the caller is told so rather than being handed the
    /// answer "nobody", which would let a run report a termination it never established.
    fn readable_marker(marker: &Path) -> Result<(), RuntimeError> {
        std::fs::File::open(marker).map(drop).map_err(|error| {
            RuntimeError::InvalidProfile(format!(
                "no process was reported holding the run's marker open, and the marker itself \
                 could not be read, so nothing was established about the run: {} ({error})",
                marker.display()
            ))
        })
    }

    /// Reads the utility's answer, or refuses it.
    ///
    /// The utility distinguishes its two outcomes by exit code: it lists the holders and exits
    /// zero, or it names none and exits one. Every other exit is a failure to answer, and so is an
    /// exit of one that carries output, because that is the utility reporting something this rule
    /// cannot read. Only an answer is parsed, and only an answer is returned: "nobody holds the
    /// marker" ends a run, and "the question was not answered" must not be read as it.
    pub fn holders_reported(code: Option<i32>, stdout: &[u8]) -> Result<Vec<u32>, String> {
        let reported = String::from_utf8_lossy(stdout);
        let reported = reported.trim();
        match (code, reported.is_empty()) {
            (Some(1), true) => Ok(Vec::new()),
            (Some(0), false) => reported
                .lines()
                .map(|line| {
                    line.trim().parse().map_err(|_| {
                        format!(
                            "the descriptor holder reader reported a holder of the run's marker \
                             that cannot be read: {line}"
                        )
                    })
                })
                .collect(),
            _ => Err(format!(
                "the descriptor holder reader did not answer which processes hold the run's \
                 marker open: it exited with {} and reported {} byte(s)",
                code.map_or_else(|| "a signal".to_owned(), |code| code.to_string()),
                reported.len()
            )),
        }
    }

    fn proc_holders(marker: &Path) -> Result<Vec<u32>, RuntimeError> {
        let entries = std::fs::read_dir("/proc")?;
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
        Ok(holders)
    }

    /// Attributes every current holder of the run's marker to that run. Reading open descriptors is
    /// far more expensive than reading the process table, so it is done at the points where the
    /// answer is acted upon rather than on every wait.
    pub fn absorb_marker_holders(root: u32) -> Result<(), RuntimeError> {
        let Some(marker) = shared_forest()
            .lock()
            .ok()
            .and_then(|forest| forest.markers.get(&root).cloned())
        else {
            return Ok(());
        };
        let holders = holders_of(&marker)?;
        if holders.is_empty() {
            return Ok(());
        }
        let snapshot = read_process_table()?;
        let Ok(mut forest) = shared_forest().lock() else {
            return Err(RuntimeError::InvalidProfile(
                "the record of the run's processes failed".to_owned(),
            ));
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
        Ok(())
    }

    fn shared_forest() -> &'static Mutex<Forest> {
        static FOREST: OnceLock<Mutex<Forest>> = OnceLock::new();
        FOREST.get_or_init(|| Mutex::new(Forest::default()))
    }

    /// Reads the process table through the admitted utility. Every failure to read it is reported:
    /// a run that treated an unreadable table as an empty one would report that nothing it started
    /// is still running, on no evidence at all.
    fn read_process_table() -> Result<Vec<ProcessEntry>, RuntimeError> {
        let program = super::lifecycle::program(super::ProgramRole::ProcessTable)?;
        let output = Command::new(program)
            .args(["-A", "-o", "pid=,ppid=,pgid=,lstart="])
            .stderr(Stdio::null())
            .output()?;
        if !output.status.success() {
            return Err(RuntimeError::InvalidProfile(
                "the process table reader exited unsuccessfully".to_owned(),
            ));
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
        if entries.is_empty() {
            return Err(RuntimeError::InvalidProfile(
                "the process table reader reported no process at all".to_owned(),
            ));
        }
        Ok(entries)
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
                    // This background reading has no caller to report a failure to. The readings
                    // that termination acts upon are taken by `survivors`, which reads the table
                    // itself and reports what it could not establish, so this one stops rather
                    // than looping on a utility that can no longer be admitted.
                    let Ok(snapshot) = read_process_table() else {
                        if let Ok(mut forest) = shared_forest().lock() {
                            forest.observing = false;
                        }
                        return;
                    };
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
                    drop(forest);
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
    pub fn survivors(root: u32) -> Result<Vec<u32>, RuntimeError> {
        let snapshot = read_process_table()?;
        let Ok(mut forest) = shared_forest().lock() else {
            return Err(RuntimeError::InvalidProfile(
                "the record of the run's processes failed".to_owned(),
            ));
        };
        absorb(&mut forest.members, &snapshot);
        let mut live: Vec<u32> = forest
            .members
            .iter()
            .filter(|(pid, attribution)| attribution.root == root && **pid > 1)
            .map(|(pid, _)| *pid)
            .collect();
        live.sort_unstable();
        Ok(live)
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
        let program = super::lifecycle::program(super::ProgramRole::Signal)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let status = Command::new(&program)
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
            let _ = Command::new(&program)
                .args(arguments)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        status.map(|_| ())
    }
}

/// What a run could not establish about the processes it started.
///
/// Most of the places that end a process tree are reporting something else at that moment — a
/// cancellation, a time limit, a failure the runtime reported — or are a session being dropped,
/// which has no caller at all. None of them can return a termination failure, and none of them may
/// discard it either: the whole point of the check is that a run says what it left running. They
/// keep it here instead, and the controller reads it before it reports the run's outcome.
fn unestablished() -> &'static std::sync::Mutex<Vec<String>> {
    static UNESTABLISHED: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> =
        std::sync::OnceLock::new();
    UNESTABLISHED.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Ends the managed process tree where the caller cannot report a failure to do so, and keeps what
/// could not be established for the controller to report.
#[doc(hidden)]
pub fn end_process_tree_or_keep(child: &mut Child) {
    if let Err(error) = terminate_process_tree(child)
        && let Ok(mut kept) = unestablished().lock()
    {
        kept.push(error.to_string());
    }
}

/// Takes everything no caller could be told about since this was last read. An empty answer here is
/// the only ground on which a run may report that it left nothing running.
pub fn unestablished_terminations() -> Vec<String> {
    unestablished()
        .lock()
        .map(|mut kept| std::mem::take(&mut *kept))
        .unwrap_or_default()
}

#[doc(hidden)]
#[cfg(unix)]
pub fn terminate_process_tree(child: &mut Child) -> std::io::Result<()> {
    /// Every question this function asks about the operating system must be answered. An
    /// unanswerable question is returned to the caller, because termination is reported clean only
    /// where the absence of survivors was established rather than assumed.
    fn observed<T>(outcome: Result<T, RuntimeError>) -> std::io::Result<T> {
        outcome.map_err(|error| {
            std::io::Error::other(format!(
                "the processes of the managed run could not be observed: {error}"
            ))
        })
    }

    let root = child.id();
    let group = format!("-{root}");
    let mut parent_reaped = child.try_wait()?.is_some();
    // Ask the operating system who holds the run's marker open before each signal is sent. Between
    // the two questions the cheaper reading of the process table is enough: a holder found here
    // stays attributed until it dies, and anything it starts afterwards is its child.
    observed(descendants::absorb_marker_holders(root))?;
    let _ = descendants::signal(
        root,
        &group,
        "-TERM",
        &observed(descendants::survivors(root))?,
    );
    for _ in 0..20 {
        parent_reaped |= child.try_wait()?.is_some();
        let remaining = observed(descendants::survivors(root))?;
        if remaining.is_empty() && !process_group_exists(&group)? {
            if !parent_reaped {
                let _ = child.wait()?;
            }
            descendants::forget(root);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    observed(descendants::absorb_marker_holders(root))?;
    let outcome = descendants::signal(
        root,
        &group,
        "-KILL",
        &observed(descendants::survivors(root))?,
    );
    if !parent_reaped {
        let _ = child.wait()?;
    }
    for _ in 0..100 {
        if observed(descendants::survivors(root))?.is_empty() && !process_group_exists(&group)? {
            descendants::forget(root);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let remaining = observed(descendants::survivors(root))?.len();
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
    use super::descendants::holders_reported;
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

    /// The two answers the descriptor holder reader can give, and the several ways it can fail to
    /// give one. An exit of one carrying output is a failure: it is the utility reporting something
    /// this rule cannot read, and reading it as "nobody holds the marker" would end a run on an
    /// answer nobody gave.
    ///
    /// The check that must fail: parse the output whatever the exit was, and the ambiguous and
    /// failing cases below are read as an empty answer, with a non-zero exit.
    #[test]
    fn a_holder_reader_that_did_not_answer_is_not_read_as_an_empty_answer() {
        assert_eq!(
            holders_reported(Some(0), b"412\n413\n").expect("a listed answer is read"),
            vec![412, 413]
        );
        assert_eq!(
            holders_reported(Some(1), b"").expect("no holders is an answer the utility gives"),
            Vec::<u32>::new()
        );
        for (code, stdout, case) in [
            (
                Some(1),
                &b"412\n"[..],
                "an exit of one that reported something",
            ),
            (Some(2), &b""[..], "an exit this rule does not know"),
            (Some(0), &b""[..], "a successful exit that reported nothing"),
            (Some(0), &b"not-a-process\n"[..], "an unreadable holder"),
            (None, &b""[..], "an exit by signal"),
        ] {
            let refusal = holders_reported(code, stdout);
            assert!(
                refusal.is_err(),
                "{case} was read as an answer: {refusal:?}"
            );
        }
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

/// Whether the process group still exists. The signal program is the admitted one, and a failure to
/// run it is returned rather than read as the group's absence, which is what a run would otherwise
/// report as a clean termination.
#[cfg(unix)]
fn process_group_exists(group: &str) -> std::io::Result<bool> {
    let program = lifecycle::program(ProgramRole::Signal)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let status = Command::new(program)
        .args(["-0", group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
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
