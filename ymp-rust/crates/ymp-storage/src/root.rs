//! The single root every durable path the product writes lives under.
//!
//! A store is one run's durable state: its journal, its objects, its workspaces, its candidates
//! and its runtime evidence. Earlier builds made the store the whole of the product's footprint
//! and defaulted it to a directory beside the project, so a second run had nowhere to go until
//! the operator invented another directory name by hand.
//!
//! Here the footprint is a root, and a store is addressed inside it:
//!
//! ```text
//! .ymp/
//!   root.json                       the layout marker and its version
//!   projects/<project>/
//!     project.json                  the directory this project addresses
//!     runs/0001/                    one store: one run, its objects and its evidence
//!     runs/0002/
//! ```
//!
//! The project segment is derived from the directory the product was started in, and the run
//! segment is the next free ordinal. Neither is supplied by the operator, so a second project
//! and a second run are named by nothing anyone had to choose.
//!
//! What the layout separates, and what it leaves to the writer lock, is stated on [`StoreIntent`].
//!
//! The root carries a project level even though today's default root sits inside the project it
//! serves. That is deliberate: moving the default to a shared location later is then a change of
//! one path rather than a change of the layout, and two projects sharing a root already keep
//! disjoint state.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// The root the product keeps every durable path under when the operator names none.
pub const DEFAULT_ROOT: &str = ".ymp";

/// The store directory earlier builds defaulted to. This layout never writes into it and never
/// copies out of it; it is read where it stands or refused with the reason named.
pub const LEGACY_STORE: &str = ".ymp-data";

/// The layout version this build writes and reads. A root is not migrated.
pub const LAYOUT_VERSION: u64 = 1;

const ROOT_MARKER: &str = "root.json";
const PROJECT_MARKER: &str = "project.json";
const PROJECTS: &str = "projects";
const RUNS: &str = "runs";
const JOURNAL: &str = "events.jsonl";
const ROOT_KIND: &str = "ymp-root";

/// The project segment is a readable name and a digest of the exact directory it stands for. The
/// name is for the operator reading the tree; the digest is what keeps two projects apart.
const PROJECT_NAME_CHARS: usize = 32;
const PROJECT_DIGEST_CHARS: usize = 12;

/// Run directories are named by a four-digit ordinal, so a project holds this many runs before
/// the layout has to widen. Reaching it is stated rather than silently wrapped.
const MAX_RUNS: u32 = 9_999;

#[derive(Debug, Error)]
pub enum RootError {
    #[error(
        "{} is a store rather than a root: it holds a journal. Open it where it stands with \
         --data-root {}, or name a different root.",
        path.display(),
        path.display()
    )]
    StoreAsRoot { path: PathBuf },
    #[error("{} is not a readable root: {reason}", path.display())]
    UnreadableMarker { path: PathBuf, reason: String },
    #[error(
        "the root {} states layout version {found}; this build writes version {expected} and \
         migrates no root",
        path.display()
    )]
    UnsupportedLayout {
        path: PathBuf,
        found: u64,
        expected: u64,
    },
    #[error(
        "a store written by the earlier layout stands at {}, and nothing was copied out of it. \
         Read it where it stands with --data-root {}, or name the new root with --root {} to \
         leave it untouched.",
        legacy.display(),
        legacy.display(),
        root.display()
    )]
    LegacyStore { legacy: PathBuf, root: PathBuf },
    #[error(
        "{} already holds {limit} runs; no further run can be addressed under it",
        project.display()
    )]
    RunsExhausted { project: PathBuf, limit: u32 },
    #[error("root I/O error at {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

/// Which store of a project an invocation acts on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreIntent {
    /// The store the project last addressed — what a reader, a report or a cancellation acts on.
    /// A project that has addressed none names its first store without creating that store.
    Current,
    /// A store that holds no run, because this invocation intends to start one.
    ///
    /// The last store is reused when no run was ever committed into it, so a refused start leaves
    /// no empty directory behind. Creating a run directory is exclusive, so two invocations that
    /// both reach that step take different ordinals — but a directory holds no journal until its
    /// run is committed, and an invocation arriving inside that window is given the directory
    /// another one just took rather than a new one.
    ///
    /// The layout therefore does not promise that concurrent starts each receive a store. It
    /// promises that a store holds one run: the writer lock gives such a store to one invocation
    /// and refuses the others loudly, naming the store they lost, so no run of theirs is
    /// committed anywhere.
    New,
}

/// One root, resolved for one project.
#[derive(Clone, Debug)]
pub struct DataRoot {
    path: PathBuf,
    project: PathBuf,
    segment: String,
}

impl DataRoot {
    /// The root path used when the operator names none.
    pub fn default_path() -> PathBuf {
        PathBuf::from(DEFAULT_ROOT)
    }

    /// Refuse to begin beside a store written by the earlier layout.
    ///
    /// The old store is neither moved nor copied nor read as if it were a root: the refusal names
    /// it, names the invocation that reads it where it stands, and names the invocation that
    /// declares the new root and leaves it alone. This guards the default root only — an operator
    /// who named a root has already answered the question this asks.
    pub fn refuse_legacy_neighbour(root: &Path) -> Result<(), RootError> {
        let legacy = match root.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join(LEGACY_STORE),
            _ => PathBuf::from(LEGACY_STORE),
        };
        if legacy.join(JOURNAL).is_file() && !root.join(ROOT_MARKER).is_file() {
            return Err(RootError::LegacyStore {
                legacy,
                root: root.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Open a root for the directory the product was started in.
    pub fn open(path: &Path) -> Result<Self, RootError> {
        let project = std::env::current_dir().map_err(|source| RootError::Io {
            path: PathBuf::from("."),
            source,
        })?;
        Self::open_for_project(path, &project)
    }

    /// Open a root for a stated project directory.
    pub fn open_for_project(path: &Path, project: &Path) -> Result<Self, RootError> {
        if path.join(JOURNAL).is_file() {
            return Err(RootError::StoreAsRoot {
                path: path.to_path_buf(),
            });
        }
        create_dir_all(path)?;
        let marker = path.join(ROOT_MARKER);
        match fs::read(&marker) {
            Ok(bytes) => check_root_marker(path, &bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                write_json(
                    &marker,
                    &serde_json::json!({
                        "schema_version": LAYOUT_VERSION,
                        "kind": ROOT_KIND,
                    }),
                )?;
            }
            Err(source) => {
                return Err(RootError::Io {
                    path: marker,
                    source,
                });
            }
        }

        let project = fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
        let segment = project_segment(&project);
        Ok(Self {
            path: path.to_path_buf(),
            project,
            segment,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The directory this root addresses a project for.
    pub fn project_directory(&self) -> &Path {
        &self.project
    }

    /// The name that directory carries under the root.
    pub fn project_segment(&self) -> &str {
        &self.segment
    }

    /// The directory holding every store of this project.
    pub fn runs_directory(&self) -> PathBuf {
        self.path.join(PROJECTS).join(&self.segment).join(RUNS)
    }

    /// The store this invocation acts on.
    ///
    /// Both intents materialize the project's own directories, a reader's included, so a project
    /// directory that exists under a root always carries the marker naming what it stands for.
    pub fn store(&self, intent: StoreIntent) -> Result<PathBuf, RootError> {
        let runs = self.runs_directory();
        create_dir_all(&runs)?;
        self.write_project_marker()?;
        match intent {
            StoreIntent::Current => Ok(highest_run(&runs)?
                .map(|(_, path)| path)
                .unwrap_or_else(|| runs.join(run_segment(1)))),
            StoreIntent::New => match highest_run(&runs)? {
                // A store addressed but never started into holds no journal. It is where the
                // next run belongs, not a reason to leave an empty directory behind. Two
                // invocations reaching it at once are given it together; the writer lock, not
                // this choice, is what refuses all but one of them.
                Some((_, path)) if !path.join(JOURNAL).is_file() => Ok(path),
                Some((ordinal, _)) => self.claim(&runs, ordinal + 1),
                None => self.claim(&runs, 1),
            },
        }
    }

    /// Claim the first free ordinal from `first`. The directory creation is the claim: it fails
    /// when the name is taken, so two invocations claiming at once take different ordinals.
    fn claim(&self, runs: &Path, first: u32) -> Result<PathBuf, RootError> {
        for ordinal in first..=MAX_RUNS {
            let path = runs.join(run_segment(ordinal));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(RootError::Io { path, source }),
            }
        }
        Err(RootError::RunsExhausted {
            project: runs.to_path_buf(),
            limit: MAX_RUNS,
        })
    }

    fn write_project_marker(&self) -> Result<(), RootError> {
        let marker = self
            .path
            .join(PROJECTS)
            .join(&self.segment)
            .join(PROJECT_MARKER);
        if marker.exists() {
            return Ok(());
        }
        write_json(
            &marker,
            &serde_json::json!({
                "schema_version": LAYOUT_VERSION,
                "project_path": self.project.to_string_lossy(),
            }),
        )
    }
}

fn check_root_marker(path: &Path, bytes: &[u8]) -> Result<(), RootError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| RootError::UnreadableMarker {
            path: path.to_path_buf(),
            reason: format!("{ROOT_MARKER} is not readable JSON: {error}"),
        })?;
    match value.get("kind").and_then(Value::as_str) {
        Some(ROOT_KIND) => {}
        other => {
            return Err(RootError::UnreadableMarker {
                path: path.to_path_buf(),
                reason: format!(
                    "{ROOT_MARKER} names kind {} rather than {ROOT_KIND}",
                    other.unwrap_or("nothing")
                ),
            });
        }
    }
    let found = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| RootError::UnreadableMarker {
            path: path.to_path_buf(),
            reason: format!("{ROOT_MARKER} states no layout version"),
        })?;
    if found != LAYOUT_VERSION {
        return Err(RootError::UnsupportedLayout {
            path: path.to_path_buf(),
            found,
            expected: LAYOUT_VERSION,
        });
    }
    Ok(())
}

/// The name one project directory carries under a root.
///
/// The readable half is the directory's own name, reduced to characters every filesystem this
/// product runs on accepts. The digest half is taken over the whole path, so two directories that
/// share a name — `~/work/ymp` and `~/archive/ymp` — never share a segment.
fn project_segment(project: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(project.to_string_lossy().as_bytes());
    let digest = hex::encode(digest.finalize());
    let name = project
        .file_name()
        .map(|name| readable_name(&name.to_string_lossy()))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "project".to_owned());
    format!("{name}-{}", &digest[..PROJECT_DIGEST_CHARS])
}

fn readable_name(name: &str) -> String {
    let mut reduced = String::with_capacity(name.len());
    for character in name.chars() {
        let keep = character.is_ascii_alphanumeric() || character == '-' || character == '_';
        let next = if keep {
            character.to_ascii_lowercase()
        } else {
            '-'
        };
        if next == '-' && reduced.ends_with('-') {
            continue;
        }
        reduced.push(next);
        if reduced.len() >= PROJECT_NAME_CHARS {
            break;
        }
    }
    reduced.trim_matches('-').to_owned()
}

fn run_segment(ordinal: u32) -> String {
    format!("{ordinal:04}")
}

/// The highest run ordinal a project already holds, with its directory.
fn highest_run(runs: &Path) -> Result<Option<(u32, PathBuf)>, RootError> {
    let entries = match fs::read_dir(runs) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(RootError::Io {
                path: runs.to_path_buf(),
                source,
            });
        }
    };
    let mut highest: Option<(u32, PathBuf)> = None;
    for entry in entries {
        let entry = entry.map_err(|source| RootError::Io {
            path: runs.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        // Only a name this layout writes counts. Anything else under `runs/` is left alone
        // rather than treated as a store whose ordinal could be continued.
        if name.len() != 4 || !name.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Ok(ordinal) = name.parse::<u32>() else {
            continue;
        };
        if highest.as_ref().is_none_or(|(seen, _)| ordinal > *seen) {
            highest = Some((ordinal, entry.path()));
        }
    }
    Ok(highest)
}

fn create_dir_all(path: &Path) -> Result<(), RootError> {
    fs::create_dir_all(path).map_err(|source| RootError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), RootError> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("a marker value serializes");
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|source| RootError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{PROJECT_DIGEST_CHARS, project_segment, readable_name, run_segment};
    use std::path::Path;

    #[test]
    fn a_project_segment_states_the_directory_name_and_separates_equal_names() {
        let first = project_segment(Path::new("/home/operator/work/Ymp Project"));
        let second = project_segment(Path::new("/home/operator/archive/Ymp Project"));
        assert!(
            first.starts_with("ymp-project-"),
            "unexpected segment {first}"
        );
        assert!(second.starts_with("ymp-project-"));
        assert_ne!(
            first, second,
            "two directories sharing a name share a segment"
        );
        assert_eq!(first.len(), "ymp-project-".len() + PROJECT_DIGEST_CHARS);
    }

    #[test]
    fn a_readable_name_keeps_only_characters_every_filesystem_accepts() {
        assert_eq!(readable_name("../weird name!"), "weird-name");
        assert_eq!(readable_name("////"), "");
    }

    #[test]
    fn run_segments_sort_in_the_order_they_are_claimed() {
        assert!(run_segment(2) < run_segment(10));
        assert_eq!(run_segment(1), "0001");
    }
}
