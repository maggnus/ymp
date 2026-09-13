//! Shared data contract for the embedded Git backend. Git operations never invoke a tool.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Uncommitted,
    Committed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub binary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub root: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub base: Option<String>,
    pub base_oid: Option<String>,
    pub dirty: bool,
    pub comparison: Comparison,
    pub files: Vec<Change>,
    pub notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub worktree: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    pub worktrees: Vec<Worktree>,
    pub branches: Vec<Branch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub text: String,
    pub notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitError {
    NotRepository,
    Unavailable(String),
    Failed(String),
    Timeout,
    TooLarge,
    Stale,
    Dirty,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRepository => write!(f, "Not a Git working tree"),
            Self::Unavailable(message) | Self::Failed(message) => f.write_str(message),
            Self::Timeout => write!(f, "Git inspection exceeded its work budget"),
            Self::TooLarge => write!(f, "Git data is too large to display"),
            Self::Stale => write!(f, "The repository changed; refresh before continuing"),
            Self::Dirty => write!(
                f,
                "The worktree has uncommitted changes; switching was refused"
            ),
        }
    }
}

impl std::error::Error for GitError {}

impl From<git2::Error> for GitError {
    fn from(error: git2::Error) -> Self {
        Self::Failed(error.message().to_owned())
    }
}

use git2::{BranchType, Delta, DiffOptions, Oid, Patch, Repository, Status, StatusOptions};
use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

pub const MAX_FILES: usize = 4096;
pub const MAX_DIFF_BYTES: usize = 256 * 1024;
const WORK_BUDGET: Duration = Duration::from_secs(8);
static WORKER: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(1)));

/// One admitted blocking call, even if its awaiting UI task is cancelled. Filesystem calls
/// cannot be preempted; the permit stays inside the worker until the operation actually ends.
async fn worker<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, GitError> + Send + 'static,
) -> Result<T, GitError> {
    let permit = WORKER
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| GitError::Unavailable("Git worker is unavailable".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        operation()
    })
    .await
    .map_err(|error| GitError::Failed(format!("Git worker failed: {error}")))?
}

fn budget(start: Instant) -> Result<(), GitError> {
    if start.elapsed() > WORK_BUDGET {
        Err(GitError::Timeout)
    } else {
        Ok(())
    }
}

fn open(cwd: &Path) -> Result<Repository, GitError> {
    let repo = Repository::discover(cwd).map_err(|error| {
        if error.code() == git2::ErrorCode::NotFound {
            GitError::NotRepository
        } else {
            error.into()
        }
    })?;
    if repo.workdir().is_none() {
        return Err(GitError::NotRepository);
    }
    Ok(repo)
}

fn root(repo: &Repository) -> Result<PathBuf, GitError> {
    repo.workdir()
        .ok_or(GitError::NotRepository)?
        .canonicalize()
        .map_err(|error| GitError::Failed(error.to_string()))
}

fn identity(repo: &Repository) -> Result<(Option<String>, Option<String>), GitError> {
    match repo.head() {
        Ok(head) => Ok((
            if head.is_branch() {
                Some(head.shorthand()?.to_owned())
            } else {
                None
            },
            head.target().map(|oid| oid.to_string()),
        )),
        Err(error)
            if matches!(
                error.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            let branch = repo
                .find_reference("HEAD")?
                .symbolic_target()?
                .and_then(|name| name.strip_prefix("refs/heads/"))
                .map(str::to_owned);
            Ok((branch, None))
        }
        Err(error) => Err(error.into()),
    }
}

fn path(bytes: &[u8]) -> Result<PathBuf, GitError> {
    #[cfg(unix)]
    let value = {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()))
    };
    #[cfg(not(unix))]
    let value =
        PathBuf::from(std::str::from_utf8(bytes).map_err(|_| {
            GitError::Failed("A Git path is not valid UTF-8 on this platform".into())
        })?);
    validate_path(&value)?;
    Ok(value)
}

fn validate_path(path: &Path) -> Result<(), GitError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(GitError::Failed(
            "Git returned an invalid relative file path".into(),
        ));
    }
    Ok(())
}

fn status(repo: &Repository) -> Result<BTreeMap<PathBuf, (Status, Option<PathBuf>)>, GitError> {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .include_unreadable(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true)
        .update_index(false);
    let entries = repo.statuses(Some(&mut options))?;
    if entries.len() > MAX_FILES {
        return Err(GitError::TooLarge);
    }
    let mut found = BTreeMap::new();
    for entry in entries.iter() {
        let latest = entry.index_to_workdir().or_else(|| entry.head_to_index());
        let current = latest
            .as_ref()
            .and_then(|delta| delta.new_file().path_bytes())
            .unwrap_or_else(|| entry.path_bytes());
        let old = entry
            .head_to_index()
            .or_else(|| entry.index_to_workdir())
            .filter(|delta| matches!(delta.status(), Delta::Renamed | Delta::Copied))
            .and_then(|delta| delta.old_file().path_bytes())
            .map(path)
            .transpose()?;
        found.insert(path(current)?, (entry.status(), old));
    }
    Ok(found)
}

fn status_label(flags: Status) -> &'static str {
    if flags.is_conflicted() {
        "U"
    } else if flags.is_wt_new() {
        "?"
    } else if flags.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
        "R"
    } else if flags.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
        "D"
    } else if flags.is_index_new() {
        "A"
    } else if flags.contains(Status::WT_UNREADABLE) {
        "!"
    } else {
        "M"
    }
}

fn record(path: PathBuf, old_path: Option<PathBuf>, flags: Status, label: &str) -> Change {
    Change {
        path,
        old_path,
        status: label.into(),
        staged: flags.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE
                | Status::CONFLICTED,
        ),
        unstaged: flags.intersects(
            Status::WT_NEW
                | Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_RENAMED
                | Status::WT_TYPECHANGE
                | Status::WT_UNREADABLE
                | Status::CONFLICTED,
        ),
        untracked: flags.is_wt_new(),
        added: None,
        removed: None,
        binary: false,
    }
}

struct ResolvedBase {
    label: Option<String>,
    oid: Option<Oid>,
    notice: Option<String>,
}

fn resolve_base(
    repo: &Repository,
    requested: Option<&str>,
    branch: Option<&str>,
) -> Result<ResolvedBase, GitError> {
    let mut candidates = Vec::new();
    if let Some(requested) = requested {
        candidates.push(requested.to_owned());
    } else {
        if let Ok(reference) = repo.find_reference("refs/remotes/origin/HEAD") {
            if let Some(target) = reference.symbolic_target()? {
                candidates.push(target.to_owned());
            }
        }
        candidates.extend(["refs/heads/main".to_owned(), "refs/heads/master".to_owned()]);
        if let Some(branch) = branch {
            if let Ok(upstream) = repo
                .find_branch(branch, BranchType::Local)
                .and_then(|b| b.upstream())
            {
                candidates.push(upstream.get().name()?.to_owned());
            }
            candidates.push(format!("refs/heads/{branch}"));
        }
    }
    for name in candidates {
        if let Ok(commit) = repo.revparse_single(&name).and_then(|o| o.peel_to_commit()) {
            let label = name
                .strip_prefix("refs/heads/")
                .or_else(|| name.strip_prefix("refs/remotes/"))
                .unwrap_or(&name)
                .to_owned();
            let notice = (requested.is_none()
                && branch.is_some_and(|branch| name == format!("refs/heads/{branch}"))
                && !matches!(label.as_str(), "main" | "master"))
            .then(|| {
                "No independent base branch was identified; comparing this branch with itself."
                    .to_owned()
            });
            return Ok(ResolvedBase {
                label: Some(label),
                oid: Some(commit.id()),
                notice,
            });
        }
    }
    if requested.is_some() {
        return Err(GitError::Failed(
            "The comparison base cannot be resolved".into(),
        ));
    }
    Ok(ResolvedBase {
        label: None,
        oid: None,
        notice: Some("No comparison base is available".into()),
    })
}

fn diff_options() -> DiffOptions {
    let mut options = DiffOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true)
        .include_typechange(true)
        .include_unreadable(true)
        .max_size(MAX_DIFF_BYTES as i64);
    options
}

fn differences<'a>(
    repo: &'a Repository,
    comparison: Comparison,
    head: Option<Oid>,
    base: Option<Oid>,
    selected: Option<&Change>,
) -> Result<git2::Diff<'a>, GitError> {
    let head_tree = head
        .map(|oid| repo.find_commit(oid).and_then(|c| c.tree()))
        .transpose()?;
    let base_tree = base
        .map(|oid| repo.find_commit(oid).and_then(|c| c.tree()))
        .transpose()?;
    let mut options = diff_options();
    if let Some(selected) = selected {
        validate_path(&selected.path)?;
        options
            .disable_pathspec_match(true)
            .pathspec(&selected.path);
        if let Some(old) = &selected.old_path {
            validate_path(old)?;
            options.pathspec(old);
        }
    }
    let mut delta = match comparison {
        Comparison::Uncommitted => {
            if repo.index()?.has_conflicts() {
                // Merging index deltas into the worktree diff can reduce an unmerged file to
                // a mode-only change. Compare HEAD directly with its actual conflict text.
                repo.diff_tree_to_workdir(head_tree.as_ref(), Some(&mut options))?
            } else {
                repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut options))?
            }
        }
        Comparison::Committed => {
            options.include_untracked(false);
            repo.diff_tree_to_tree(base_tree.as_ref(), head_tree.as_ref(), Some(&mut options))?
        }
    };
    let mut find = git2::DiffFindOptions::new();
    find.renames(true).rename_limit(256);
    delta.find_similar(Some(&mut find))?;
    Ok(delta)
}

fn parse_oid(value: Option<&str>) -> Result<Option<Oid>, GitError> {
    value.map(Oid::from_str).transpose().map_err(Into::into)
}

pub async fn inspect(
    cwd: &Path,
    comparison: Option<Comparison>,
    base: Option<&str>,
) -> Result<Snapshot, GitError> {
    let cwd = cwd.to_owned();
    let base = base.map(str::to_owned);
    worker(move || inspect_sync(&cwd, comparison, base.as_deref())).await
}

fn inspect_sync(
    cwd: &Path,
    comparison: Option<Comparison>,
    base: Option<&str>,
) -> Result<Snapshot, GitError> {
    let start = Instant::now();
    let repo = open(cwd)?;
    let root = root(&repo)?;
    let (branch, head) = identity(&repo)?;
    let statuses = status(&repo)?;
    budget(start)?;
    let dirty = !statuses.is_empty();
    let comparison = comparison.unwrap_or(if dirty {
        Comparison::Uncommitted
    } else {
        Comparison::Committed
    });
    let ResolvedBase {
        label: base,
        oid: base_id,
        mut notice,
    } = resolve_base(&repo, base, branch.as_deref())?;
    let head_id = parse_oid(head.as_deref())?;
    let base_oid = match (base_id, head_id) {
        (Some(base), Some(head)) => Some(repo.merge_base(base, head).map_err(|error| {
            GitError::Failed(format!(
                "Cannot find a comparison base: {}",
                error.message()
            ))
        })?),
        _ => None,
    };
    let mut files = BTreeMap::new();
    if comparison == Comparison::Uncommitted || (head_id.is_some() && base_oid.is_some()) {
        let delta = differences(&repo, comparison, head_id, base_oid, None)?;
        if delta.deltas().len() > MAX_FILES {
            return Err(GitError::TooLarge);
        }
        for (index, item) in delta.deltas().enumerate() {
            budget(start)?;
            let current = item
                .new_file()
                .path_bytes()
                .or_else(|| item.old_file().path_bytes())
                .ok_or_else(|| GitError::Failed("A diff path is missing".into()))?;
            let current = path(current)?;
            let old = matches!(item.status(), Delta::Renamed | Delta::Copied)
                .then(|| item.old_file().path_bytes().map(path).transpose())
                .transpose()?
                .flatten();
            let flags = if comparison == Comparison::Uncommitted {
                statuses
                    .get(&current)
                    .map_or(Status::CURRENT, |(flags, _)| *flags)
            } else {
                Status::CURRENT
            };
            let label = match item.status() {
                Delta::Added => "A",
                Delta::Deleted => "D",
                Delta::Renamed => "R",
                Delta::Copied => "C",
                Delta::Untracked => "?",
                Delta::Conflicted => "U",
                Delta::Unreadable => "!",
                Delta::Typechange => "T",
                _ => "M",
            };
            let mut change = record(
                current.clone(),
                old,
                flags,
                if flags.is_conflicted() { "U" } else { label },
            );
            change.untracked |= item.status() == Delta::Untracked;
            if let Some(patch) = Patch::from_diff(&delta, index)? {
                let (_, added, removed) = patch.line_stats()?;
                change.added = Some(added as u64);
                change.removed = Some(removed as u64);
                change.binary = patch.delta().flags().contains(git2::DiffFlags::BINARY);
            } else {
                change.binary = true;
            }
            files.insert(current, change);
        }
    }
    if comparison == Comparison::Uncommitted {
        for (path, (flags, old)) in statuses {
            files
                .entry(path.clone())
                .or_insert_with(|| record(path, old, flags, status_label(flags)));
        }
    } else if head_id.is_none() {
        notice = Some("No commits yet".into());
    }
    budget(start)?;
    if identity(&repo)?.1 != head {
        return Err(GitError::Stale);
    }
    Ok(Snapshot {
        root,
        branch,
        head,
        base,
        base_oid: base_oid.map(|id| id.to_string()),
        dirty,
        comparison,
        files: files.into_values().collect(),
        notice,
    })
}

pub async fn choices(cwd: &Path) -> Result<Choices, GitError> {
    let cwd = cwd.to_owned();
    worker(move || choices_sync(&cwd)).await
}

fn choices_sync(cwd: &Path) -> Result<Choices, GitError> {
    let start = Instant::now();
    let repo = open(cwd)?;
    let current_root = root(&repo)?;
    let common = Repository::open(repo.commondir())?;
    let mut worktrees = BTreeMap::new();
    for opened in [&repo, &common] {
        if let Some(directory) = opened.workdir() {
            let (branch, head) = identity(opened)?;
            let path = directory
                .canonicalize()
                .map_err(|e| GitError::Failed(e.to_string()))?;
            worktrees.insert(path.clone(), Worktree { path, branch, head });
        }
    }
    for name in common.worktrees()?.iter() {
        budget(start)?;
        let name = name?.ok_or_else(|| GitError::Failed("A worktree name is not UTF-8".into()))?;
        let worktree = common.find_worktree(name)?;
        let opened = Repository::open_from_worktree(&worktree)?;
        let (branch, head) = identity(&opened)?;
        let path = root(&opened)?;
        worktrees.insert(path.clone(), Worktree { path, branch, head });
        if worktrees.len() > MAX_FILES {
            return Err(GitError::TooLarge);
        }
    }
    let current = identity(&repo)?.0;
    let mut branches = Vec::new();
    for item in repo.branches(Some(BranchType::Local))? {
        budget(start)?;
        let (branch, _) = item?;
        let name = branch
            .name()?
            .ok_or_else(|| GitError::Failed("A branch name is not UTF-8".into()))?
            .to_owned();
        let worktree = worktrees
            .values()
            .find(|w| w.branch.as_deref() == Some(&name) && w.path != current_root)
            .map(|w| w.path.clone());
        branches.push(Branch {
            current: current.as_deref() == Some(&name),
            name,
            worktree,
        });
        if branches.len() > MAX_FILES {
            return Err(GitError::TooLarge);
        }
    }
    branches.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Choices {
        worktrees: worktrees.into_values().collect(),
        branches,
    })
}

pub async fn diff(snapshot: &Snapshot, change: &Change) -> Result<Diff, GitError> {
    let snapshot = snapshot.clone();
    let change = change.clone();
    worker(move || diff_sync(&snapshot, &change)).await
}

fn diff_sync(snapshot: &Snapshot, change: &Change) -> Result<Diff, GitError> {
    let start = Instant::now();
    validate_path(&change.path)?;
    let repo = open(&snapshot.root)?;
    if root(&repo)? != snapshot.root {
        return Err(GitError::Stale);
    }
    if identity(&repo)?.1 != snapshot.head {
        return Err(GitError::Stale);
    }
    if snapshot.comparison == Comparison::Uncommitted {
        let full_path = snapshot.root.join(&change.path);
        match std::fs::symlink_metadata(&full_path) {
            Ok(metadata) if !(metadata.is_file() || metadata.file_type().is_symlink()) => {
                return Ok(Diff {
                    text: String::new(),
                    notice: Some(
                        "This entry is not a regular file; its contents are not read".into(),
                    ),
                })
            }
            Ok(metadata) if metadata.len() > MAX_DIFF_BYTES as u64 => {
                return Ok(Diff {
                    text: String::new(),
                    notice: Some("This file is too large for a diff preview".into()),
                })
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(GitError::Failed(error.to_string()))
            }
            _ => {}
        }
    }
    let delta = differences(
        &repo,
        snapshot.comparison,
        parse_oid(snapshot.head.as_deref())?,
        parse_oid(snapshot.base_oid.as_deref())?,
        Some(change),
    )?;
    let mut text = Vec::new();
    for index in 0..delta.deltas().len() {
        budget(start)?;
        if let Some(mut patch) = Patch::from_diff(&delta, index)? {
            if patch.delta().flags().contains(git2::DiffFlags::BINARY) {
                return Ok(Diff {
                    text: String::new(),
                    notice: Some("Binary or oversized file; no text diff is shown".into()),
                });
            }
            let bytes = patch.to_buf()?;
            if text.len() + bytes.len() > MAX_DIFF_BYTES {
                return Err(GitError::TooLarge);
            }
            text.extend_from_slice(&bytes);
        } else {
            return Ok(Diff {
                text: String::new(),
                notice: Some("Binary or oversized file; no text diff is shown".into()),
            });
        }
    }
    budget(start)?;
    let text = match String::from_utf8(text) {
        Ok(text) => text,
        Err(_) => {
            return Ok(Diff {
                text: String::new(),
                notice: Some(
                    "This diff contains bytes outside UTF-8; no text is substituted".into(),
                ),
            })
        }
    };
    let notice = text.is_empty().then(|| "No working-content difference from HEAD. Index changes may differ, or the file changed since refresh.".into());
    Ok(Diff { text, notice })
}

/// Caller holds Store's project lock and excludes active/new agent runs during this operation.
/// No forced checkout, stash, hook, credential helper or external filter is invoked.
pub async fn switch_branch(
    cwd: &Path,
    branch: &str,
    expected_head: Option<&str>,
) -> Result<(), GitError> {
    switch_branch_guarded(cwd, branch, expected_head, ()).await
}

/// Retain the caller's project lock in the actual library worker, including when the async
/// waiter is dropped. A cancelled waiter must not release ownership while checkout still runs.
pub async fn switch_branch_guarded<G: Send + 'static>(
    cwd: &Path,
    branch: &str,
    expected_head: Option<&str>,
    guard: G,
) -> Result<(), GitError> {
    let cwd = cwd.to_owned();
    let branch = branch.to_owned();
    let head = expected_head.map(str::to_owned);
    worker(move || {
        let _guard = guard;
        switch_sync(&cwd, &branch, head.as_deref())
    })
    .await
}

fn switch_sync(cwd: &Path, branch: &str, expected_head: Option<&str>) -> Result<(), GitError> {
    let repo = open(cwd)?;
    if !git2::Branch::name_is_valid(branch)? {
        return Err(GitError::Failed("Invalid branch name".into()));
    }
    if identity(&repo)?.1.as_deref() != expected_head {
        return Err(GitError::Stale);
    }
    if repo.state() != git2::RepositoryState::Clean {
        return Err(GitError::Failed(
            "Finish the repository operation before switching branches".into(),
        ));
    }
    if !status(&repo)?.is_empty() {
        return Err(GitError::Dirty);
    }
    let choices = choices_sync(cwd)?;
    let chosen = choices
        .branches
        .iter()
        .find(|b| b.name == branch)
        .ok_or_else(|| GitError::Failed("Choose an existing local branch".into()))?;
    if chosen.current {
        return Ok(());
    }
    if chosen.worktree.is_some() {
        return Err(GitError::Failed(
            "This branch is already open in another worktree; select that worktree to inspect it"
                .into(),
        ));
    }
    let target_ref = format!("refs/heads/{branch}");
    // Reserve reference updates before touching files. A pre-existing HEAD.lock must refuse
    // the operation before checkout changes either the worktree or index.
    let mut transaction = repo.transaction()?;
    transaction.lock_ref("HEAD")?;
    if let Ok(current) = repo.head() {
        if current.is_branch() {
            transaction.lock_ref(current.name()?)?;
        }
    }
    transaction.lock_ref(&target_ref)?;
    if identity(&repo)?.1.as_deref() != expected_head {
        return Err(GitError::Stale);
    }
    // Another ymp process can have completed a checkout in another worktree between the
    // initial enumeration and acquiring the target lock. Recheck under that shared lock.
    if choices_sync(cwd)?
        .branches
        .iter()
        .any(|candidate| candidate.name == branch && candidate.worktree.is_some())
    {
        return Err(GitError::Failed(
            "This branch is already open in another worktree".into(),
        ));
    }
    let (previous_branch, previous_head) = identity(&repo)?;
    let previous = previous_branch
        .or(previous_head)
        .unwrap_or_else(|| "HEAD".into());
    transaction.set_symbolic_target(
        "HEAD",
        &target_ref,
        None,
        &format!("checkout: moving from {previous} to {branch}"),
    )?;
    // Resolve the target after its lock is held; a Reference obtained before locking may
    // still carry an earlier object id if another Git operation moved the branch meanwhile.
    let tree = repo
        .find_branch(branch, BranchType::Local)?
        .get()
        .peel_to_tree()?;
    // Preflight all paths for collisions without changing the worktree or index.
    let mut check = git2::build::CheckoutBuilder::new();
    check.dry_run();
    repo.checkout_tree(tree.as_object(), Some(&mut check))?;
    // SAFE refuses changed paths; ignored files are not overwritten either.
    let mut options = git2::build::CheckoutBuilder::new();
    options.safe().overwrite_ignored(false);
    repo.checkout_tree(tree.as_object(), Some(&mut options))?;
    transaction.commit().map_err(|error| {
        GitError::Failed(format!(
            "Files were checked out but HEAD could not be updated; inspect the repository: {}",
            error.message()
        ))
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
