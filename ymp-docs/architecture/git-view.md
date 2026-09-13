# Git view integration contract

Implementation authorized2026-09-13 as YMP-143, with YMP-141 popup correction. The task register
records actual delivery. An embedded git2/libgit2 backend, Tokio and existing display/highlight components are used.
The owner forbids depending on a locally installed Git executable. One new embedded Git library
addresses that requirement; no parallel Git implementation is added. UI implementation belongs to the current Claude author.

## Backend API (`ymp_workspace::git`)

All records derive Clone/Debug/PartialEq/Eq. Paths preserve native identity. The UI must escape
paths and Git text through existing display helpers. Functions return `Result<T, GitError>`.

```rust
pub enum Comparison { Uncommitted, Committed }
pub struct Change {
    pub path: PathBuf, // relative to Snapshot.root
    pub old_path: Option<PathBuf>,
    pub status: String, // M/A/D/R/C/U/? etc.; literal display, not an operation
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub binary: bool,
}
pub struct Snapshot {
    pub root: PathBuf,
    pub branch: Option<String>, // None for detached HEAD
    pub head: Option<String>, // None before the first commit
    pub base: Option<String>, // selected/resolved base label
    pub base_oid: Option<String>, // captured merge base for Committed
    pub dirty: bool,
    pub comparison: Comparison,
    pub files: Vec<Change>,
    pub notice: Option<String>, // e.g. no base identified, never a false clean state
}
pub struct Worktree { pub path: PathBuf, pub branch: Option<String>, pub head: Option<String> }
pub struct Branch { pub name: String, pub current: bool, pub worktree: Option<PathBuf> }
pub struct Choices { pub worktrees: Vec<Worktree>, pub branches: Vec<Branch> }
pub struct Diff { pub text: String, pub notice: Option<String> }
pub enum GitError { NotRepository, Unavailable(String), Failed(String), Timeout, TooLarge, Stale, Dirty }
pub async fn inspect(cwd: &Path, comparison: Option<Comparison>, base: Option<&str>) -> Result<Snapshot, GitError>;
pub async fn choices(cwd: &Path) -> Result<Choices, GitError>;
pub async fn diff(snapshot: &Snapshot, change: &Change) -> Result<Diff, GitError>;
pub async fn switch_branch(cwd: &Path, branch: &str, expected_head: Option<&str>) -> Result<(), GitError>;
```

The exact implemented module is authoritative once delivered. `None` comparison resolves from
current dirty state. The UI preserves manual mode until dirty state changes, then requests the
automatic mode. Base selection can be overridden through `inspect`; default resolution tries
origin/HEAD, local main/master, upstream, then current branch (with an explanatory notice when
no independent base exists). Committed uses captured merge-base and HEAD; Uncommitted uses
HEAD versus working content including untracked files. Unborn HEAD is supported.

`diff` is per selected Change, bounded, and rejects stale HEAD; do not construct a patch from
lossily decoded paths. Use returned actual diff with the existing diff line-role renderer.
Binary/special/oversized/unreadable content yields a notice or an error. No file is read in paint.

## UI ownership and scheduling

The author owns `ymp-tui` including event-loop job scheduling. Fetch snapshots only while /git
is visible, about once per second after the previous request finishes, plus manual refresh.
Use one in-flight request per kind, preserve selection by native path, retain prior snapshot on
error with an explicit stale/error indication. Generation tokens prevent late responses from
another worktree/mode overwriting the current view. Choices and diffs load on demand. Discard
late read results when no longer relevant or exiting. Library operations run on a bounded
blocking-worker path; cooperative time checks do not preempt an operating-system filesystem
call. Keep at most one admitted job at a time so cancelled views cannot multiply background
work.

Branch switching is explicit and separate from comparison choice. Before starting it, reject
active/in-flight team runs. Hold the existing Store project lock for the selected canonical
worktree via `store.project` and `store.lock_project` throughout the operation; prevent new
Start/FollowUp/Resume actions while switching. The backend additionally validates branch, checks
expected HEAD and rejects any dirty state. No automatic stash, commit, undo, discard, remote
operation or new worktree creation. The embedded library does not run Git hooks or external diff/filter/credential tools.
Existing locks cover matching project directories in the same ymp home, not arbitrary external
programs or all possible overlapping directories. Keep this MVP boundary explicit.

Worktree choice changes only the inspected context. It never changes the session execution cwd,
provider configuration or /files directory. /files keeps its accepted read-only preview.
All source/diff roles share the current theme; no extra highlighter or editor component.

## Embedded Git choice

`git2 0.21.0` with `default-features=false` and `vendored-libgit2` provides repository status,
index/tree diffs, merge-base, branches, linked worktrees and checked checkout operations in one
API. libgit2 is compiled into the artifact; HTTPS/SSH transport features are disabled. No Git
executable is spawned and no system libgit2 installation is required. Building the source still
uses the project's normal compiler toolchain, including compilation of bundled C code.

gix0.87.1 was reviewed as a pure-Rust alternative. Its low-level worktree mutation APIs would
require more host assembly for this scoped branch-switch operation. It was not added alongside
git2. Both are viable libraries; choosing git2 here is a scope/API decision, not a claim that gix
cannot implement the behavior. Exact dependency metadata is retained in evidence.
