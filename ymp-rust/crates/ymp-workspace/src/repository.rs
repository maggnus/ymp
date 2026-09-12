//! What version control, if anything, covers the working directory.
//!
//! ymp keeps no copy of the user's files, so the only thing that can put an earlier version
//! back is a system the user already runs. This module answers one bounded question about
//! that: is there a Git marker at the working directory or above it? It reads directory
//! metadata and, for a `.git` file, its text. It runs no command, writes nothing, and
//! reports what it could not read rather than reading a missing entry as a negative answer.
//!
//! Absence of `.git` in the working directory establishes nothing on its own: the root of a
//! repository is often an ancestor, a linked worktree and a submodule carry a `.git` file
//! instead of a directory, and Git can be pointed elsewhere entirely by its environment.

use std::path::{Path, PathBuf};

/// Git variables that change which repository a command run here would use.
const ENVIRONMENT: &[&str] = &["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR"];

/// How many bytes of a `.git` file are read. The `gitdir:` line is the first one.
const LINK_BYTES: usize = 4096;

/// A `.git` entry, and what kind of entry it turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// The directory holding the entry: a repository root, or the root of a worktree.
    pub directory: PathBuf,
    pub kind: MarkerKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerKind {
    /// `.git` is a directory: an ordinary repository, or the main worktree of one.
    Directory,
    /// `.git` is a file naming another Git directory, the way a linked worktree or a
    /// submodule does. The named path is reported as written, unresolved.
    Link { gitdir: Option<String> },
    /// The entry exists and is neither a directory nor a readable file.
    Unreadable,
}

/// What discovery established, including where it had to stop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Repository {
    /// The nearest marker at or above the working directory.
    pub marker: Option<Marker>,
    /// The directory discovery started from, canonicalized when that was possible.
    pub start: PathBuf,
    /// Directories inspected, counting the working directory itself.
    pub inspected: usize,
    /// The last directory inspected. Nothing above it was looked at.
    pub stopped_at: PathBuf,
    /// Paths that could not be read, with the reason. A directory that cannot be read
    /// leaves what is above it unknown; it is not evidence of absence.
    pub unreadable: Vec<(PathBuf, String)>,
    /// Git environment variables set for this process.
    pub environment: Vec<(&'static str, String)>,
}

/// Walk the working directory and its ancestors, nearest first, and stop at the first
/// `.git` entry. Unreadable entries are recorded and the walk continues.
pub fn discover(directory: &Path) -> Repository {
    discover_with(directory, |name| std::env::var(name).ok())
}

/// Discovery with the Git environment supplied by the caller. The process environment is
/// read once, by [`discover`]; taking it as an argument keeps a test's subject exact.
pub fn discover_with(directory: &Path, variable: impl Fn(&str) -> Option<String>) -> Repository {
    let start = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_owned());
    let mut found = Repository {
        start: start.clone(),
        stopped_at: start.clone(),
        environment: ENVIRONMENT
            .iter()
            .filter_map(|name| variable(name).map(|value| (*name, value)))
            .filter(|(_, value)| !value.trim().is_empty())
            .collect(),
        ..Repository::default()
    };
    for directory in start.ancestors() {
        found.inspected += 1;
        found.stopped_at = directory.to_owned();
        let entry = directory.join(".git");
        // `metadata` follows a symlink, which is what Git does with a linked `.git`.
        let kind = match std::fs::metadata(&entry) {
            Ok(metadata) if metadata.is_dir() => MarkerKind::Directory,
            Ok(metadata) if metadata.is_file() => MarkerKind::Link {
                gitdir: match link_target(&entry) {
                    Ok(target) => target,
                    Err(reason) => {
                        found.unreadable.push((entry.clone(), reason));
                        None
                    }
                },
            },
            Ok(_) => MarkerKind::Unreadable,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                found.unreadable.push((entry, error.to_string()));
                continue;
            }
        };
        found.marker = Some(Marker {
            directory: directory.to_owned(),
            kind,
        });
        break;
    }
    found
}

/// The path a `.git` file names, as written. A file without a `gitdir:` line is readable
/// but says nothing, which is reported as unknown rather than as no repository.
fn link_target(path: &Path) -> Result<Option<String>, String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(LINK_BYTES as u64).read_to_string(&mut text))
        .map_err(|error| error.to_string())?;
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("gitdir:"))
        .map(|target| target.trim().to_owned())
        .find(|target| !target.is_empty()))
}

impl Repository {
    /// Factual sentences about the working directory, in the order a reader needs them.
    ///
    /// Each sentence states what was found or what could not be established. None of them
    /// concludes that the directory is outside version control, and none of them promises
    /// that anything can be put back.
    pub fn describe(&self) -> Vec<String> {
        let mut lines = Vec::new();
        match &self.marker {
            Some(Marker {
                directory,
                kind: MarkerKind::Directory,
            }) => lines.push(format!(
                "A Git repository directory is at {}. Restoring an earlier version of a file is that repository's own history, and only for content already committed to it.",
                directory.join(".git").display()
            )),
            Some(Marker {
                directory,
                kind: MarkerKind::Link { gitdir },
            }) => lines.push(match gitdir {
                Some(target) => format!(
                    "{} is a file naming the Git directory {}, as a linked worktree or a submodule does. That Git directory holds the history, and this inspection did not follow it.",
                    directory.join(".git").display(),
                    target
                ),
                None => format!(
                    "{} is a file, as a linked worktree or a submodule uses, and it names no Git directory that could be read. Which repository covers this directory is unknown.",
                    directory.join(".git").display()
                ),
            }),
            Some(Marker {
                directory,
                kind: MarkerKind::Unreadable,
            }) => lines.push(format!(
                "{} exists and is neither a directory nor a readable file, so which repository covers this directory is unknown.",
                directory.join(".git").display()
            )),
            None => lines.push(format!(
                "No .git entry was found in the working directory or in the {} directories inspected above it, up to {}. That is not a finding about version control: only .git entries were inspected, Git may stop earlier at a filesystem boundary, and a repository held below this directory or named by configuration outside it would not appear here.",
                self.inspected.saturating_sub(1),
                self.stopped_at.display()
            )),
        }
        for (path, reason) in &self.unreadable {
            lines.push(format!(
                "{} could not be read: {reason}. What that entry would have said is unknown.",
                path.display()
            ));
        }
        for (name, value) in &self.environment {
            lines.push(format!(
                "{name} is set to {value}, so Git run here may use a repository that none of the inspected directories names."
            ));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Discovery of the filesystem alone. An inherited variable cannot reach these tests,
    /// and nothing they do can reach another test's environment.
    fn discover_without_environment(directory: &Path) -> Repository {
        discover_with(directory, |_| None)
    }

    #[test]
    fn a_directory_nested_below_the_root_is_still_inside_the_repository() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        let nested = root.join("service/web/src");
        std::fs::create_dir_all(&nested).unwrap();

        let found = discover_without_environment(&nested);
        assert_eq!(
            found.marker,
            Some(Marker {
                directory: root.clone(),
                kind: MarkerKind::Directory
            })
        );
        assert_eq!(found.start, nested);
        assert_eq!(found.stopped_at, root, "the walk passed the root it found");
        assert_eq!(found.inspected, 4);
        assert!(found.unreadable.is_empty());
        let described = found.describe().join(" ");
        assert!(described.contains(&root.join(".git").display().to_string()));
        assert!(!described.to_lowercase().contains("no repository"));
    }

    #[test]
    fn a_git_file_is_reported_as_the_worktree_or_submodule_link_it_is() {
        let temp = tempfile::tempdir().unwrap();
        let work = temp.path().join("linked-worktree");
        std::fs::create_dir_all(&work).unwrap();
        let target = temp.path().join("main/.git/worktrees/linked");
        std::fs::write(work.join(".git"), format!("gitdir: {}\n", target.display())).unwrap();

        let found = discover_without_environment(&work);
        let Some(Marker {
            kind: MarkerKind::Link { gitdir },
            ..
        }) = &found.marker
        else {
            panic!("not reported as a link: {:?}", found.marker);
        };
        assert_eq!(
            gitdir.as_deref(),
            Some(target.display().to_string().as_str())
        );
        let described = found.describe().join(" ");
        assert!(described.contains("worktree"), "{described}");
        assert!(described.contains(&target.display().to_string()));

        // A file that names nothing readable is unknown, not an answer.
        std::fs::write(work.join(".git"), "unrelated text\n").unwrap();
        let found = discover_without_environment(&work);
        assert!(matches!(
            found.marker,
            Some(Marker {
                kind: MarkerKind::Link { gitdir: None },
                ..
            })
        ));
        assert!(found.describe().join(" ").contains("unknown"));
    }

    #[test]
    fn nothing_found_is_reported_as_the_bounds_of_the_search() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();

        let found = discover_without_environment(&nested);
        assert_eq!(found.marker, None);
        assert!(found.inspected >= 3);
        assert_eq!(found.stopped_at, Path::new("/"));
        let described = found.describe().join(" ");
        assert!(described.contains("No .git entry was found"), "{described}");
        assert!(described.contains("inspected"), "{described}");
        for claim in [
            "not under version control",
            "no repository",
            "not a repository",
        ] {
            assert!(
                !described.to_lowercase().contains(claim),
                "absence was concluded with {claim:?}: {described}"
            );
        }
    }

    #[test]
    fn git_environment_variables_are_reported_because_they_override_the_walk() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("project");
        std::fs::create_dir_all(&directory).unwrap();
        let found = discover_with(&directory, |name| match name {
            "GIT_DIR" => Some("/elsewhere/repository.git".to_owned()),
            "GIT_WORK_TREE" => Some("  ".to_owned()),
            _ => None,
        });
        assert_eq!(
            found.environment,
            vec![("GIT_DIR", "/elsewhere/repository.git".to_owned())],
            "a blank variable was reported as a setting"
        );
        assert!(found
            .describe()
            .join(" ")
            .contains("GIT_DIR is set to /elsewhere/repository.git"));
    }

    #[test]
    fn a_working_directory_that_cannot_be_canonicalized_is_still_inspected() {
        let found = discover_without_environment(Path::new("/ymp-test/missing/directory"));
        assert_eq!(found.start, Path::new("/ymp-test/missing/directory"));
        assert_eq!(found.marker, None);
        assert_eq!(found.inspected, 4);
        assert!(found.describe().join(" ").contains("No .git entry"));
    }
}
