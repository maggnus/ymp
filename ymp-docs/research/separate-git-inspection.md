# Separate /git changes view

Owner direction recorded on 2026-09-13. This is research and a future task definition, not
implemented functionality. The current research-only restriction remains in effect.

`/files` stays a filesystem navigator with the existing highlighted read-only preview.
Editing and ratatui-code-editor integration remain deferred. `/git` is a separate command and
page for following changes, using Paseo's Changes panel as the functional reference.

The owner's library-unification direction applies: reuse the existing syntax highlighter,
semantic styles, diff line roles and shared display primitives where appropriate. A separate
Git page does not imply a separate highlighting stack or duplicated file-content renderer.
Any additional dependency must address a demonstrated gap and identify its replacement boundary.

## What the Paseo reference actually does

Inspected clean Paseo 0.8.0 source at `fa93c4290eaa87ae58452ab6e2012f85ae6e0c6b`. The installed
CLI also reports 0.8.0; matching version labels do not establish exact installed source identity.
The parent inspected server behavior and Claude independently inspected the UI. No app buttons
were clicked and no Git mutation was executed during this research.

- **Uncommitted** compares current working content against HEAD and includes untracked files.
- **Committed** compares the merge base of the resolved base reference and HEAD to HEAD. It
  shows committed branch changes, not all tracked files or only the most recent commit.
- Dirty worktrees default to Uncommitted; clean worktrees default to Committed. A manual choice
  is retained for that checkout until its dirty/clean state changes.
- No diff produces `No changes to display`. Repository discovery, loading, no repository,
  errors and oversized diffs have distinct states. An empty view can link to the other mode
  when changes exist there. A clean feature branch can still show committed changes.
- The branch switcher performs actual checkout. Its server rejects a dirty tree; the UI offers
  Stash & Switch, and may later offer restoration of a Paseo stash for the target branch.
  The comparison's baseRef comes from checkout status; this switcher does not choose that base.
- A collapsible Commits section lists commits ahead of the base. Commit itself is a separate
  action: by default it stages all changes, and the server can generate a missing message.
  No dedicated uncommit/undo-commit operation was found in the inspected app/server source.
- Filesystem and Git metadata observation drive asynchronous updates with a 1-second debounce;
  a 5-second polling fallback is used when watching is unavailable. Manual Refresh also exists.

The owner's phrase `commit uncommit` is interpreted here as the observed Committed/Uncommitted
view selector. A clarification was offered; it must not be treated as authorization to reset
commits. Actual commit/undo operations require their own explicit scope if that was intended.

## Proposed ymp behavior

1. `/git` opens a dedicated Changes page. The header identifies the selected worktree and its
   current branch and exposes `Uncommitted | Committed`, with a visible comparison base.
2. The body contains changed files and their added/deleted line counts. Opening a row shows its
   actual diff, using the existing semantic styles. No differences in the selected comparison
   means an empty message; it must not show a directory inventory or a false error-free result
   when Git failed.
3. Follow the reference's mode defaults and preserve an explicit selection until the dirty/clean
   boundary changes. New working edits appear in Uncommitted; after committing, branch changes
   remain reviewable in Committed. A collapsible ahead-of-base commit list is secondary content.
4. Selecting an existing worktree changes the inspected context. A separate explicit branch
   switch changes the checkout itself; it must be coordinated with active agent writes and
   handle dirty trees deliberately. It is not merely changing a history filter.
5. Refresh Git snapshots away from the rendering loop, bound subprocess time/output, preserve
   file selection and expose stale/error state. Historical content comes from Git objects;
   ordinary read-only file opening retains the current bounded loader/highlighter.

The first proposal does not import every Paseo action: push, pull, PR, merge, archive, discard,
automatic commit-message generation and undo-commit are not implied by adopting its view.
No branch switch, stash, commit or other mutation is being implemented or executed now.

## Existing foundation and remaining checks

Current `repository.rs` detects Git markers but does not collect status/history. The existing
`/diff` page reports session workspace changes and is not the Git Changes page described here.
`/files` already opens bounded read-only previews with syntax highlighting on Enter.

Native read-only probes on local Git 2.50.1 established porcelain-v2 status and NUL-delimited
worktree listing support. No asynchronous Git view exists in ymp yet. Implementation acceptance
will require fixtures for staged/unstaged changes, untracked/deleted/renamed files, conflicts,
unborn/detached HEAD, unusual paths and linked worktrees, plus UI/refresh/selection checks.

Source hashes and the joint assessment are in
[source-review.json](evidence/git-paseo-143/source-review.json). Deployed documentation requests
were unavailable (HTTP 403 for the index in direct HTTP); the reference uses the pinned source
and its local public documentation. Runtime interactions in the installed Paseo app and the
future ymp implementation were not tested.

## Sources

- [Paseo comparison state](https://github.com/getpaseo/paseo/blob/fa93c4290eaa87ae58452ab6e2012f85ae6e0c6b/packages/app/src/git/working-diff-comparison/state.ts)
- [Paseo Changes panel](https://github.com/getpaseo/paseo/blob/fa93c4290eaa87ae58452ab6e2012f85ae6e0c6b/packages/app/src/git/diff-pane.tsx)
- [Paseo branch switching](https://github.com/getpaseo/paseo/blob/fa93c4290eaa87ae58452ab6e2012f85ae6e0c6b/packages/app/src/hooks/use-branch-switcher.ts)
- [Paseo Git operations](https://github.com/getpaseo/paseo/blob/fa93c4290eaa87ae58452ab6e2012f85ae6e0c6b/packages/server/src/utils/checkout-git.ts)
- [Git status](https://git-scm.com/docs/git-status), [worktrees](https://git-scm.com/docs/git-worktree)
