//! Workspace identity, canonical paths and immutable file content descriptions.
use crate::{Denial, Digest, Id, Ref, Result, journal::PolicySelection};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// A normalized, portable relative path. `.` denotes the workspace root only.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct WorkspacePath(String);
impl WorkspacePath {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value != "."
            && (value.is_empty()
                || value.len() > 4096
                || value.contains('\\')
                || value.contains(':')
                || value.chars().any(char::is_control)
                || value
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == ".." || part.len() > 255))
        {
            return Err(Denial::new(
                "workspace_path",
                "Expected a normalized relative path without aliases or traversal",
            ));
        }
        Ok(Self(value))
    }
    pub fn root() -> Self {
        Self(".".into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn contains(&self, other: &Self) -> bool {
        self.0 == "."
            || self == other
            || other
                .0
                .strip_prefix(&self.0)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        self.contains(other) || other.contains(self)
    }
    pub fn parent(&self) -> Option<Self> {
        if self.0 == "." {
            None
        } else {
            Some(
                self.0
                    .rsplit_once('/')
                    .map(|(p, _)| Self(p.into()))
                    .unwrap_or_else(Self::root),
            )
        }
    }
}
impl<'de> Deserialize<'de> for WorkspacePath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLocation {
    pub root: String,
    pub device: u64,
    pub inode: u64,
}
impl WorkspaceLocation {
    pub fn validate(&self) -> Result<()> {
        if !self.root.starts_with('/')
            || self.root.len() > 4096
            || self.root.chars().any(char::is_control)
            || (self.root != "/"
                && self.root[1..]
                    .split('/')
                    .any(|s| s.is_empty() || s == "." || s == ".."))
        {
            return Err(Denial::new(
                "workspace_root",
                "Workspace root must be an absolute canonical path",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceKind {
    Direct,
    IsolatedCopy(Id<Snapshot>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub id: Id<Workspace>,
    pub kind: WorkspaceKind,
    pub location: WorkspaceLocation,
    pub provider: PolicySelection,
}
impl Workspace {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockMode {
    Read,
    Write,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathLock {
    pub path: WorkspacePath,
    pub mode: LockMode,
    pub holder: Id,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: WorkspacePath,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFile {
    pub digest: Digest,
    pub bytes: u64,
    pub mode: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotTree {
    pub files: BTreeMap<WorkspacePath, SnapshotFile>,
    pub directories: BTreeMap<WorkspacePath, u32>,
}
impl SnapshotTree {
    pub fn validate(&self) -> Result<()> {
        if self.files.len() + self.directories.len() > 100_000 {
            return Err(Denial::new(
                "snapshot_limit",
                "Snapshot has too many entries",
            ));
        }
        let mut total = 0u64;
        for file in self.files.values() {
            if file.bytes > 32 * 1024 * 1024 {
                return Err(Denial::new(
                    "snapshot_limit",
                    "Snapshot file exceeds supported size",
                ));
            }
            total = total
                .checked_add(file.bytes)
                .ok_or_else(|| Denial::new("snapshot_limit", "Snapshot size overflow"))?;
        }
        if total > 128 * 1024 * 1024 {
            return Err(Denial::new(
                "snapshot_limit",
                "Snapshot exceeds supported total size",
            ));
        }
        if !self.directories.contains_key(&WorkspacePath::root()) {
            return Err(Denial::new(
                "snapshot_tree",
                "Snapshot must include its root directory",
            ));
        }
        for (path, mode) in self
            .directories
            .iter()
            .map(|(p, m)| (p, *m))
            .chain(self.files.iter().map(|(p, f)| (p, f.mode)))
        {
            if mode & !0o777 != 0 {
                return Err(Denial::new(
                    "snapshot_mode",
                    "Only ordinary permission bits are supported",
                ));
            }
            if let Some(parent) = path.parent()
                && !self.directories.contains_key(&parent)
            {
                return Err(Denial::new(
                    "snapshot_tree",
                    "Every entry requires a captured parent directory",
                ));
            }
        }
        if self.files.keys().any(|p| self.directories.contains_key(p)) {
            return Err(Denial::new(
                "snapshot_tree",
                "File and directory paths cannot coincide",
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<Digest> {
        self.validate()?;
        Digest::of_value(self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub id: Id<Snapshot>,
    pub workspace: Id<Workspace>,
    pub tree: SnapshotTree,
    pub taken: u64,
}
impl Snapshot {
    pub fn reference(&self) -> Result<Ref> {
        Ok(Ref {
            id: self.id.erased(),
            version: Digest::of_value(self)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureLimits {
    pub max_entries: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub max_depth: usize,
}
impl Default for CaptureLimits {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            max_file_bytes: 16 * 1024 * 1024,
            max_total_bytes: 64 * 1024 * 1024,
            max_depth: 128,
        }
    }
}
impl CaptureLimits {
    pub fn validate(&self) -> Result<()> {
        if self.max_entries == 0
            || self.max_entries > 100_000
            || self.max_file_bytes == 0
            || self.max_file_bytes > 32 * 1024 * 1024
            || self.max_total_bytes < self.max_file_bytes
            || self.max_total_bytes > 128 * 1024 * 1024
            || self.max_depth == 0
            || self.max_depth > 256
        {
            return Err(Denial::new(
                "capture_limits",
                "Capture limits exceed supported bounded allocation",
            ));
        }
        Ok(())
    }
}

/// Physical identities observed by trusted workspace I/O, not execution authority.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathComponent {
    pub name: String,
    pub identity: FileIdentity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathObservation {
    pub path: WorkspacePath,
    /// From filesystem root through the last existing component, inclusive.
    pub existing: Vec<PathComponent>,
    pub missing: Vec<String>,
}
impl PathObservation {
    pub fn validate(&self, location: &WorkspaceLocation) -> Result<()> {
        if self.existing.is_empty()
            || self.existing.len() + self.missing.len() > 512
            || self.existing[0].name != "/"
        {
            return Err(Denial::new(
                "path_observation",
                "A bounded physical ancestry is required",
            ));
        }
        let names: Vec<_> = self
            .existing
            .iter()
            .skip(1)
            .map(|c| c.name.as_str())
            .chain(self.missing.iter().map(String::as_str))
            .collect();
        for name in &names {
            if *name == "." || name.contains('/') {
                return Err(Denial::new(
                    "path_observation",
                    "Invalid physical path component",
                ));
            }
            WorkspacePath::new(*name)?;
        }
        let root_names: Vec<_> = location
            .root
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        let root_index = root_names.len();
        if self.existing.len() <= root_index
            || names.len() < root_index
            || names[..root_index] != root_names
            || self.existing[root_index].identity
                != (FileIdentity {
                    device: location.device,
                    inode: location.inode,
                })
        {
            return Err(Denial::new(
                "path_observation",
                "Observed ancestry does not bind the workspace root",
            ));
        }
        let relative = if names.len() == root_index {
            ".".to_owned()
        } else {
            names[root_index..].join("/")
        };
        if relative != self.path.as_str() {
            return Err(Denial::new(
                "path_observation",
                "Observation differs from the requested relative path",
            ));
        }
        Ok(())
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        // Start at the deepest shared physical object. This also catches aliases
        // of roots opened through different names and ancestor workspace roots.
        for (left, component) in self.existing.iter().enumerate().rev() {
            if let Some(right) = other
                .existing
                .iter()
                .position(|c| c.identity == component.identity)
            {
                let a: Vec<_> = self.existing[left + 1..]
                    .iter()
                    .map(|c| c.name.as_str())
                    .chain(self.missing.iter().map(String::as_str))
                    .collect();
                let b: Vec<_> = other.existing[right + 1..]
                    .iter()
                    .map(|c| c.name.as_str())
                    .chain(other.missing.iter().map(String::as_str))
                    .collect();
                // Conservatively conflate ASCII case and ambiguous Unicode names.
                // Distinct names are not evidence of disjoint resources on every FS.
                return a
                    .iter()
                    .zip(&b)
                    .all(|(x, y)| !x.is_ascii() || !y.is_ascii() || x.eq_ignore_ascii_case(y));
            }
        }
        false
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedPathLock {
    pub lock: PathLock,
    pub observation: PathObservation,
}

/// A durable journal instance and its physical file. Copies have a different
/// location even when their immutable store identity bytes were copied too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalIdentity {
    pub id: Digest,
    pub path: String,
    pub file: FileIdentity,
}
impl JournalIdentity {
    pub fn validate(&self) -> Result<()> {
        // Use the shared absolute-path validation; this observation names a file.
        WorkspaceLocation {
            root: self.path.clone(),
            device: self.file.device,
            inode: self.file.inode,
        }
        .validate()?;
        if self.file.inode == 0 {
            return Err(Denial::new(
                "journal_identity",
                "Journal file identity is unavailable",
            ));
        }
        Ok(())
    }
}

/// Immutable physical-root binding. The marker is metadata, not a grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceBinding {
    pub format: u32,
    pub root: WorkspaceLocation,
    pub journal: JournalIdentity,
}
impl WorkspaceBinding {
    pub fn validate(&self) -> Result<()> {
        if self.format != 1 {
            return Err(Denial::new(
                "binding_format",
                "Unsupported workspace binding format",
            ));
        }
        self.root.validate()?;
        self.journal.validate()
    }
}
