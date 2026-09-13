//! The Git page: what it asks the embedded Git backend, and what it keeps of the answers.
//!
//! Nothing here reads a repository. [`GitView::next_request`] names the one request the event loop
//! hands to the backend next, and [`GitView::receive`] takes its reply back. A single request is
//! admitted at a time: the backend's library work runs on blocking threads that cannot be stopped
//! part way, so leaving the page or changing the comparison while it works never starts a second
//! piece of work beside the first. Each request carries the generation of the view it was made for,
//! and a reply for an earlier worktree, comparison or visit is dropped instead of shown.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use ymp_workspace::git::{self, Change, Choices, Comparison, Diff, GitError, Snapshot};

/// How long the page waits after one reading has finished before it asks for the next.
pub const POLL: Duration = Duration::from_secs(1);

/// What a chooser is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Worktree,
    Branch,
}

/// One piece of work for the backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Inspect {
        generation: u64,
        root: PathBuf,
        comparison: Option<Comparison>,
    },
    Choices {
        generation: u64,
        root: PathBuf,
        purpose: Purpose,
    },
    Diff {
        generation: u64,
        /// Boxed: a snapshot lists every changed file, and requests are moved often.
        snapshot: Box<Snapshot>,
        change: Change,
    },
    /// Check out `branch` in `root`, under the project lock the event loop hands to [`perform`].
    Switch {
        root: PathBuf,
        branch: String,
        expected_head: Option<String>,
    },
}

/// The backend's answer to a [`Request`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Inspect {
        generation: u64,
        result: Result<Snapshot, GitError>,
    },
    Choices {
        generation: u64,
        purpose: Purpose,
        result: Result<Choices, GitError>,
    },
    Diff {
        generation: u64,
        change: Change,
        result: Result<Diff, GitError>,
    },
    Switch {
        branch: String,
        result: Result<(), GitError>,
    },
}

/// Run `request` on the embedded backend. A branch switch keeps `guard`, the project lock, inside
/// the backend's worker until the checkout itself has ended, even if this future is dropped first;
/// a reading does not need one.
pub async fn perform<G: Send + 'static>(request: Request, guard: G) -> Reply {
    match request {
        Request::Inspect {
            generation,
            root,
            comparison,
        } => Reply::Inspect {
            generation,
            result: git::inspect(&root, comparison, None).await,
        },
        Request::Choices {
            generation,
            root,
            purpose,
        } => Reply::Choices {
            generation,
            purpose,
            result: git::choices(&root).await,
        },
        Request::Diff {
            generation,
            snapshot,
            change,
        } => {
            let result = git::diff(&snapshot, &change).await;
            Reply::Diff {
                generation,
                change,
                result,
            }
        }
        Request::Switch {
            root,
            branch,
            expected_head,
        } => {
            let result =
                git::switch_branch_guarded(&root, &branch, expected_head.as_deref(), guard).await;
            Reply::Switch { branch, result }
        }
    }
}

/// What a reply asks of the rest of the interface, beyond the page itself.
#[derive(Debug)]
pub enum Outcome {
    Nothing,
    Choices(Purpose, Result<Choices, GitError>),
    Diff(Change, Result<Diff, GitError>),
    Switched(String, Result<(), GitError>),
}

#[derive(Clone, Debug)]
struct Switch {
    root: PathBuf,
    branch: String,
    expected_head: Option<String>,
    started: bool,
}

#[derive(Clone, Debug)]
enum Wanted {
    Choices(Purpose),
    Diff(Change),
}

#[derive(Debug, Default)]
pub struct GitView {
    /// The worktree chosen to inspect. None inspects the directory ymp was started in.
    worktree: Option<PathBuf>,
    /// A comparison chosen by hand, and whether the tree had uncommitted changes when it was.
    manual: Option<(Comparison, bool)>,
    /// The last reading of the current worktree and comparison.
    snapshot: Option<Snapshot>,
    /// Why the last reading failed. A snapshot kept from before it is then out of date.
    error: Option<GitError>,
    generation: u64,
    /// A request was handed to the backend and has not answered yet.
    admitted: bool,
    /// When the last admitted request answered.
    finished: Option<Instant>,
    /// Read again as soon as nothing is admitted, without waiting for the poll.
    refresh: bool,
    wanted: Option<Wanted>,
    switch: Option<Switch>,
}

impl GitView {
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    pub fn error(&self) -> Option<&GitError> {
        self.error.as_ref()
    }

    /// The directory requests read: the chosen worktree, or the working directory.
    pub fn root<'a>(&'a self, cwd: &'a Path) -> &'a Path {
        self.worktree.as_deref().unwrap_or(cwd)
    }

    /// The branch a confirmed switch is moving to, until it has answered. No run starts meanwhile.
    pub fn switching(&self) -> Option<&str> {
        self.switch.as_ref().map(|switch| switch.branch.as_str())
    }

    /// The comparison to ask for: one chosen by hand while the tree is still on the side of clean
    /// or changed it was chosen on, and otherwise the backend's own choice.
    fn comparison(&self) -> Option<Comparison> {
        let (comparison, dirty) = self.manual?;
        match &self.snapshot {
            Some(snapshot) if snapshot.dirty != dirty => None,
            _ => Some(comparison),
        }
    }

    /// The request to hand to the backend now, if any. A confirmed branch switch goes first and
    /// does not need the page; readings are made only while the page is `visible`.
    pub fn next_request(&mut self, visible: bool, now: Instant, cwd: &Path) -> Option<Request> {
        if self.admitted {
            return None;
        }
        if let Some(switch) = self.switch.as_mut().filter(|switch| !switch.started) {
            switch.started = true;
            self.admitted = true;
            return Some(Request::Switch {
                root: switch.root.clone(),
                branch: switch.branch.clone(),
                expected_head: switch.expected_head.clone(),
            });
        }
        if !visible || self.switch.is_some() {
            return None;
        }
        let root = self.root(cwd).to_path_buf();
        let generation = self.generation;
        let request = match self.wanted.take() {
            Some(Wanted::Choices(purpose)) => Request::Choices {
                generation,
                root,
                purpose,
            },
            Some(Wanted::Diff(change)) => {
                let snapshot = Box::new(self.snapshot.clone()?);
                Request::Diff {
                    generation,
                    snapshot,
                    change,
                }
            }
            None => {
                let due = self.refresh
                    || self
                        .finished
                        .is_none_or(|finished| now.saturating_duration_since(finished) >= POLL);
                if !due {
                    return None;
                }
                self.refresh = false;
                Request::Inspect {
                    generation,
                    root,
                    comparison: self.comparison(),
                }
            }
        };
        self.admitted = true;
        Some(request)
    }

    /// Take the reply to the admitted request.
    pub fn receive(&mut self, reply: Reply, now: Instant) -> Outcome {
        self.admitted = false;
        self.finished = Some(now);
        match reply {
            Reply::Inspect { generation, result } => {
                if generation != self.generation {
                    return Outcome::Nothing;
                }
                match result {
                    Ok(snapshot) => {
                        if self
                            .manual
                            .is_some_and(|(_, dirty)| dirty != snapshot.dirty)
                        {
                            // The tree crossed between clean and changed, so the choice made on
                            // the other side no longer holds: read the backend's own choice now.
                            self.manual = None;
                            self.refresh = true;
                        }
                        self.snapshot = Some(snapshot);
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
                Outcome::Nothing
            }
            Reply::Choices {
                generation,
                purpose,
                result,
            } if generation == self.generation => Outcome::Choices(purpose, result),
            Reply::Diff {
                generation,
                change,
                result,
            } if generation == self.generation => Outcome::Diff(change, result),
            Reply::Choices { .. } | Reply::Diff { .. } => Outcome::Nothing,
            Reply::Switch { branch, result } => {
                self.switch = None;
                self.generation += 1;
                self.wanted = None;
                self.refresh = true;
                Outcome::Switched(branch, result)
            }
        }
    }

    /// The admitted request ended without a reply, because its task failed.
    pub fn abandon(&mut self, reason: String, now: Instant) -> Outcome {
        match self.switch.as_ref().filter(|switch| switch.started) {
            Some(switch) => {
                let reply = Reply::Switch {
                    branch: switch.branch.clone(),
                    result: Err(GitError::Failed(reason)),
                };
                self.receive(reply, now)
            }
            None => {
                self.admitted = false;
                self.finished = Some(now);
                self.error = Some(GitError::Failed(reason));
                Outcome::Nothing
            }
        }
    }

    /// Leave the page. A request still admitted is left to finish, and its reply is dropped.
    pub fn leave(&mut self) {
        self.generation += 1;
        self.wanted = None;
        self.refresh = true;
    }

    /// Show the other comparison, and keep it until the tree crosses between clean and changed.
    /// Returns the comparison now shown, or nothing when no reading has succeeded to change from.
    pub fn toggle_comparison(&mut self) -> Option<Comparison> {
        let snapshot = self.snapshot.as_ref()?;
        let next = match snapshot.comparison {
            Comparison::Uncommitted => Comparison::Committed,
            Comparison::Committed => Comparison::Uncommitted,
        };
        self.manual = Some((next, snapshot.dirty));
        self.restart();
        Some(next)
    }

    /// Inspect `path` instead. The working directory of runs and of the files page stays where it is.
    pub fn choose_worktree(&mut self, path: PathBuf, cwd: &Path) {
        if same_directory(self.root(cwd), &path) {
            return;
        }
        self.worktree = (!same_directory(&path, cwd)).then_some(path);
        self.manual = None;
        self.restart();
    }

    /// A new context: nothing read so far belongs to it.
    fn restart(&mut self) {
        self.generation += 1;
        self.snapshot = None;
        self.error = None;
        self.wanted = None;
        self.refresh = true;
    }

    /// Read again as soon as nothing is admitted, without waiting for the poll.
    pub fn refresh(&mut self) {
        self.refresh = true;
    }

    pub fn want_choices(&mut self, purpose: Purpose) {
        self.wanted = Some(Wanted::Choices(purpose));
    }

    pub fn want_diff(&mut self, change: Change) {
        self.wanted = Some(Wanted::Diff(change));
    }

    /// Queue a confirmed switch. It is handed to the backend as soon as nothing else is admitted.
    pub fn begin_switch(&mut self, root: PathBuf, branch: String, expected_head: Option<String>) {
        self.switch = Some(Switch {
            root,
            branch,
            expected_head,
            started: false,
        });
    }

    /// The change on the current reading whose row has `key`.
    pub fn change(&self, key: &str) -> Option<&Change> {
        self.snapshot
            .as_ref()?
            .files
            .iter()
            .find(|change| row_key(&change.path) == key)
    }
}

/// Whether two paths name one directory, however each was written.
pub fn same_directory(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// A commit id short enough to read beside a branch name.
pub fn short_head(head: &str) -> String {
    head.chars().take(12).collect()
}

/// The key a changed file's row is found by: the native bytes of its path, so two names that are
/// displayed alike are still two rows, and a selection follows the exact file across readings.
#[cfg(unix)]
pub fn row_key(path: &Path) -> String {
    use std::fmt::Write as _;
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str()
        .as_bytes()
        .iter()
        .fold(String::from("git:"), |mut key, byte| {
            let _ = write!(key, "{byte:02x}");
            key
        })
}

#[cfg(not(unix))]
pub fn row_key(path: &Path) -> String {
    format!("git:{path:?}")
}
