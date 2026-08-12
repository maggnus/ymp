#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use ymp_storage::{ObjectStore, ObjectStoreError};

const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const SUBMISSION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub object_digest: String,
    pub executable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotManifest {
    pub schema_version: u32,
    pub files: Vec<FileEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotRef {
    pub manifest_digest: String,
    pub file_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Change {
    Upsert {
        path: String,
        object_digest: String,
        executable: bool,
    },
    Delete {
        path: String,
    },
}

impl Change {
    fn path(&self) -> &str {
        match self {
            Self::Upsert { path, .. } | Self::Delete { path } => path,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SubmissionManifest {
    pub schema_version: u32,
    pub base_snapshot_digest: String,
    pub changes: Vec<Change>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SubmissionRef {
    pub manifest_digest: String,
    pub base_snapshot_digest: String,
    pub change_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateRef {
    pub base_snapshot_digest: String,
    pub submission_digest: String,
    pub snapshot_digest: String,
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("artifact I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    ObjectStore(#[from] ObjectStoreError),
    #[error("symlinks are not admitted into a POC source snapshot: {0}")]
    Symlink(PathBuf),
    #[error("artifact path is not a normalized relative UTF-8 path: {0}")]
    InvalidPath(String),
    #[error("materialization destination is not empty: {0}")]
    DestinationNotEmpty(PathBuf),
    #[error("submission base {actual} does not match expected base {expected}")]
    BaseMismatch { expected: String, actual: String },
    #[error("manifest contains duplicate path: {0}")]
    DuplicatePath(String),
    #[error("unsupported {kind} schema version {actual}")]
    UnsupportedSchema { kind: &'static str, actual: u32 },
    #[error("submission deletes a path that is absent from its base: {0}")]
    MissingDeleteTarget(String),
}

#[derive(Clone, Debug)]
pub struct ArtifactStore {
    objects: ObjectStore,
}

impl ArtifactStore {
    pub const fn new(objects: ObjectStore) -> Self {
        Self { objects }
    }

    pub fn capture_source(&self, root: impl AsRef<Path>) -> Result<SnapshotRef, ArtifactError> {
        self.capture_source_filtered(root.as_ref(), &BTreeSet::new())
    }

    pub fn capture_source_excluding(
        &self,
        root: impl AsRef<Path>,
        exclusions: &[&str],
    ) -> Result<SnapshotRef, ArtifactError> {
        let exclusions = normalized_exclusions(exclusions)?;
        self.capture_source_filtered(root.as_ref(), &exclusions)
    }

    fn capture_source_filtered(
        &self,
        root: &Path,
        exclusions: &BTreeSet<String>,
    ) -> Result<SnapshotRef, ArtifactError> {
        let mut files = Vec::new();
        self.collect_files(root, root, exclusions, &mut files)?;
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let manifest = SnapshotManifest {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            files,
        };
        self.store_snapshot(&manifest)
    }

    pub fn load_snapshot(&self, digest: &str) -> Result<SnapshotManifest, ArtifactError> {
        let manifest: SnapshotManifest = serde_json::from_slice(&self.objects.read(digest)?)?;
        if manifest.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(ArtifactError::UnsupportedSchema {
                kind: "snapshot",
                actual: manifest.schema_version,
            });
        }
        validate_entries(&manifest.files)?;
        for entry in &manifest.files {
            self.objects.verify(&entry.object_digest)?;
        }
        Ok(manifest)
    }

    pub fn materialize(
        &self,
        snapshot_digest: &str,
        destination: impl AsRef<Path>,
    ) -> Result<(), ArtifactError> {
        let destination = destination.as_ref();
        ensure_empty_directory(destination)?;
        let manifest = self.load_snapshot(snapshot_digest)?;
        for entry in manifest.files {
            let relative = checked_relative_path(&entry.path)?;
            let target = destination.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let bytes = self.objects.read(&entry.object_digest)?;
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)?;
            file.write_all(&bytes)?;
            file.flush()?;
            file.sync_all()?;
            set_executable(&target, entry.executable)?;
        }
        File::open(destination)?.sync_all()?;
        Ok(())
    }

    pub fn create_submission(
        &self,
        base_snapshot_digest: impl Into<String>,
        workspace: impl AsRef<Path>,
    ) -> Result<SubmissionRef, ArtifactError> {
        self.create_submission_excluding(base_snapshot_digest, workspace, &[])
    }

    pub fn create_submission_excluding(
        &self,
        base_snapshot_digest: impl Into<String>,
        workspace: impl AsRef<Path>,
        exclusions: &[&str],
    ) -> Result<SubmissionRef, ArtifactError> {
        let base_snapshot_digest = base_snapshot_digest.into();
        let base = self.load_snapshot(&base_snapshot_digest)?;
        let exclusions = normalized_exclusions(exclusions)?;
        let current_ref = self.capture_source_filtered(workspace.as_ref(), &exclusions)?;
        let current = self.load_snapshot(&current_ref.manifest_digest)?;
        let base_map = entry_map(
            base.files
                .into_iter()
                .filter(|entry| !is_excluded(&entry.path, &exclusions))
                .collect(),
        )?;
        let current_map = entry_map(current.files)?;
        let paths: BTreeSet<_> = base_map.keys().chain(current_map.keys()).cloned().collect();
        let mut changes = Vec::new();

        for path in paths {
            match (base_map.get(&path), current_map.get(&path)) {
                (Some(before), Some(after)) if before == after => {}
                (_, Some(after)) => changes.push(Change::Upsert {
                    path: path.clone(),
                    object_digest: after.object_digest.clone(),
                    executable: after.executable,
                }),
                (Some(_), None) => changes.push(Change::Delete { path: path.clone() }),
                (None, None) => unreachable!("path comes from one of the maps"),
            }
        }

        let manifest = SubmissionManifest {
            schema_version: SUBMISSION_SCHEMA_VERSION,
            base_snapshot_digest: base_snapshot_digest.clone(),
            changes,
        };
        validate_changes(&manifest.changes)?;
        let bytes = serde_json::to_vec(&manifest)?;
        let manifest_digest = self.objects.put(&bytes)?;
        Ok(SubmissionRef {
            manifest_digest,
            base_snapshot_digest,
            change_count: manifest.changes.len(),
        })
    }

    pub fn build_candidate(
        &self,
        expected_base_digest: &str,
        submission_digest: &str,
    ) -> Result<CandidateRef, ArtifactError> {
        let submission: SubmissionManifest =
            serde_json::from_slice(&self.objects.read(submission_digest)?)?;
        if submission.schema_version != SUBMISSION_SCHEMA_VERSION {
            return Err(ArtifactError::UnsupportedSchema {
                kind: "submission",
                actual: submission.schema_version,
            });
        }
        validate_changes(&submission.changes)?;
        if submission.base_snapshot_digest != expected_base_digest {
            return Err(ArtifactError::BaseMismatch {
                expected: expected_base_digest.to_owned(),
                actual: submission.base_snapshot_digest,
            });
        }
        let base = self.load_snapshot(expected_base_digest)?;
        let mut entries = entry_map(base.files)?;
        for change in submission.changes {
            match change {
                Change::Upsert {
                    path,
                    object_digest,
                    executable,
                } => {
                    self.objects.verify(&object_digest)?;
                    entries.insert(
                        path.clone(),
                        FileEntry {
                            path,
                            object_digest,
                            executable,
                        },
                    );
                }
                Change::Delete { path } => {
                    if entries.remove(&path).is_none() {
                        return Err(ArtifactError::MissingDeleteTarget(path));
                    }
                }
            }
        }
        let manifest = SnapshotManifest {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            files: entries.into_values().collect(),
        };
        let snapshot = self.store_snapshot(&manifest)?;
        Ok(CandidateRef {
            base_snapshot_digest: expected_base_digest.to_owned(),
            submission_digest: submission_digest.to_owned(),
            snapshot_digest: snapshot.manifest_digest,
        })
    }

    fn collect_files(
        &self,
        root: &Path,
        directory: &Path,
        exclusions: &BTreeSet<String>,
        output: &mut Vec<FileEntry>,
    ) -> Result<(), ArtifactError> {
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            if directory == root && matches!(entry.file_name().to_str(), Some(".git" | ".ymp-data"))
            {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(root).expect("walked path is below root");
            let normalized = normalized_string(relative)?;
            if is_excluded(&normalized, exclusions) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(ArtifactError::Symlink(path));
            }
            if metadata.is_dir() {
                self.collect_files(root, &path, exclusions, output)?;
            } else if metadata.is_file() {
                let bytes = fs::read(&path)?;
                output.push(FileEntry {
                    path: normalized,
                    object_digest: self.objects.put(&bytes)?,
                    executable: is_executable(&metadata),
                });
            }
        }
        Ok(())
    }

    fn store_snapshot(&self, manifest: &SnapshotManifest) -> Result<SnapshotRef, ArtifactError> {
        validate_entries(&manifest.files)?;
        let bytes = serde_json::to_vec(manifest)?;
        Ok(SnapshotRef {
            manifest_digest: self.objects.put(&bytes)?,
            file_count: manifest.files.len(),
        })
    }
}

fn normalized_exclusions(exclusions: &[&str]) -> Result<BTreeSet<String>, ArtifactError> {
    exclusions
        .iter()
        .map(|path| normalized_string(Path::new(path)))
        .collect()
}

fn is_excluded(path: &str, exclusions: &BTreeSet<String>) -> bool {
    exclusions
        .iter()
        .any(|excluded| path == excluded || path.starts_with(&format!("{excluded}/")))
}

fn entry_map(files: Vec<FileEntry>) -> Result<BTreeMap<String, FileEntry>, ArtifactError> {
    validate_entries(&files)?;
    Ok(files
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect())
}

fn validate_entries(files: &[FileEntry]) -> Result<(), ArtifactError> {
    let mut paths = BTreeSet::new();
    for entry in files {
        checked_relative_path(&entry.path)?;
        if !paths.insert(entry.path.as_str()) {
            return Err(ArtifactError::DuplicatePath(entry.path.clone()));
        }
    }
    Ok(())
}

fn validate_changes(changes: &[Change]) -> Result<(), ArtifactError> {
    let mut paths = BTreeSet::new();
    for change in changes {
        checked_relative_path(change.path())?;
        if !paths.insert(change.path()) {
            return Err(ArtifactError::DuplicatePath(change.path().to_owned()));
        }
    }
    Ok(())
}

fn ensure_empty_directory(path: &Path) -> Result<(), ArtifactError> {
    if path.exists() {
        if !path.is_dir() || fs::read_dir(path)?.next().is_some() {
            return Err(ArtifactError::DestinationNotEmpty(path.to_path_buf()));
        }
    } else {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn normalized_string(path: &Path) -> Result<String, ArtifactError> {
    let value = path
        .to_str()
        .ok_or_else(|| ArtifactError::InvalidPath(path.display().to_string()))?;
    let checked = checked_relative_path(value)?;
    checked
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .ok_or_else(|| ArtifactError::InvalidPath(path.display().to_string())),
            _ => Err(ArtifactError::InvalidPath(path.display().to_string())),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("/"))
}

fn checked_relative_path(path: &str) -> Result<PathBuf, ArtifactError> {
    let value = Path::new(path);
    if path.is_empty()
        || value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ArtifactError::InvalidPath(path.to_owned()));
    }
    Ok(value.to_path_buf())
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    let mode = permissions.mode();
    permissions.set_mode(if executable {
        mode | 0o100
    } else {
        mode & !0o111
    });
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) -> Result<(), std::io::Error> {
    Ok(())
}
