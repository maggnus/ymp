# Separate Git inspection

Owner direction recorded on 2026-09-13. This is a proposal and task definition, not implemented
functionality. The current research-only restriction remains in effect.

`/files` stays a filesystem navigator with the existing highlighted read-only file preview.
Editing and ratatui-code-editor integration are deferred. Git inspection belongs in a separate
section; it is not a status mode embedded into the file manager.

## Proposed scope

- A separate Git page (proposed command `/git`) selects a repository worktree and, for committed
  history, a branch or commit.
- Working changes distinguish staged, unstaged, untracked and conflicted paths. A file may have
  both staged and unstaged changes. These belong to the selected worktree, not an arbitrary branch.
- Committed content provides recent commits and the files/differences in the selected commit.
  A branch selects history for reading without checking it out or changing execution cwd.
- Refresh visible working changes asynchronously, retain selection, show stale/error state and
  provide manual refresh. Bound subprocess duration/output; do not run Git from rendering code.
- Existing source preview and explicit diff line styling can be reused. Opening committed content
  must read its actual Git object rather than the current filesystem file with the same path.

The first implementation would inspect only: branch checkout, stage, commit, reset and worktree
creation are separate operations outside this proposed scope. Selecting an existing worktree
changes the inspected directory without changing the session's execution directory.

## Existing foundation and feasibility

Current `repository.rs` detects Git markers; it does not collect status/history. The existing
`/diff` page reports session workspace changes and is not a complete Git status screen.
`/files` already opens bounded read-only previews with syntax highlighting on Enter.

Read-only command probes on local Git2.50.1 confirm porcelain-v2 status with branch headers and
NUL-delimited worktree listings are available. These are feasibility observations, not an
implemented asynchronous refresh or branch/history selector. Future acceptance needs temporary
Git fixtures for staged+unstaged changes, new/deleted/renamed files, conflicts, unborn/detached
HEAD, unusual paths and existing linked worktrees.

Primary references: [git-status](https://git-scm.com/docs/git-status),
[git-worktree](https://git-scm.com/docs/git-worktree), [git-log](https://git-scm.com/docs/git-log).
