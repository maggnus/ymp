# YMP-143 evidence: the /git page (terminal interface)

The contract is [git-view.md](../../../architecture/git-view.md); the embedded git2 backend and its
own tests are recorded in [../backend](../backend/README.md). This directory covers the interface
only. Every interface test answers the page's requests in place of the backend; no test here opens
a repository, runs a Git program or invokes a provider.

## What was built

- `/git` opens a page listing the changed files of one comparison in one worktree: status, path
  (a rename also names its previous path), staged/unstaged/untracked, lines added and removed. The
  subtitle carries the branch (or `detached at …`, or `no commits yet`), the base for Committed,
  and the worktree path; branch names are kept out of the upper-cased table title so their case
  survives.
- `ymp-tui/src/git_view.rs` holds the page's state. `next_request` names at most one request at a
  time; `receive` applies its reply. Readings are made only while the page is shown, about once a
  second after the previous one answered, and at once after `r`. Every request carries a
  generation: leaving the page, changing the comparison or choosing another worktree makes earlier
  replies stale, and they are dropped. A request still admitted is never joined by a second one.
- Comparison: the first reading asks for none, so the backend chooses from the dirty state. `m`
  records a manual choice with the dirty state it was made in; when a reading reports the other
  state, the manual choice is dropped and the automatic one is requested immediately.
- `Enter` requests the actual per-file diff and shows it in the shared read-only preview popup,
  each line drawn by the existing diff line roles through `text::patch` (literal text, escaped
  control characters, the popup geometry of YMP-141). `d` inspects the row's record.
- Empty and failure states: `Reading Git`, `Not a Git working tree`, a failed reading with its
  reason, `No changes to display` with advice pointing to the other comparison, and a failed
  reading over a kept list marked as out of date. A failure is never shown as a clean tree.
- `w` lists worktrees and changes only the inspected root; the session's working directory, the
  files page and the configuration are untouched.
- `b` lists local branches. The current branch and branches checked out in another worktree cannot
  be chosen. Choosing one opens a confirmation (`checks out` badge). Confirming is refused while a
  run is active. Once confirmed:
  - `StartRun`, `FollowUp` and `Resume` are held in `App` (typed text returns to the composer) and
    again in the event loop, until the switch answers;
  - the switch is admitted even if the page has been left;
  - the event loop refuses it if a run is active, otherwise acquires `Store::project` and
    `Store::lock_project` for the worktree root and moves the lock into
    `git::switch_branch_guarded`, so the backend keeps it inside its worker until the checkout
    ends, even if the waiting task is dropped;
  - the result is reported, and nothing is stashed, committed, discarded or pushed.
- The interface guide gains a `Git` section and states the branch switch as the one exception to
  read-only pages.

## Tests (`ymp-tui/src/tests.rs`, section "Git page")

| Test | What it establishes |
| --- | --- |
| `git_page_follows_the_backend_until_a_manual_choice_and_again_once_the_tree_crosses` | Nothing is read before `/git`; the first reading leaves the comparison to the backend; one admitted request even five polls later; the next reading waits for the poll; `m` asks for Committed and keeps it while the tree stays dirty; a clean reading makes the next request automatic at once; empty-state wording and subtitle. |
| `a_late_git_reply_is_dropped_and_leaving_the_page_starts_nothing_beside_it` | Leaving and returning while a reading is admitted starts no second reading; its late reply is not shown; a fresh reading follows; nothing is read away from the page. |
| `git_selection_follows_the_exact_file_and_enter_shows_its_actual_patch` | Selection stays on the same native path when a later reading reorders files; `Enter` asks for that change's diff with the current snapshot; the reply opens the preview with the added line in the diff role colour and an escape character not reaching the terminal; `Esc` returns to the page. |
| `git_page_states_a_missing_repository_a_failed_reading_and_a_kept_list` | `NotRepository` is stated (not as "no changes"); `r` reads again; a later failure keeps the list and says the last reading failed. |
| `choosing_a_worktree_changes_only_what_the_git_page_inspects` | `w` asks for worktrees of the inspected root; the chooser shows a detached worktree; choosing it makes the next reading use that root; the working directory, configuration and project tree are unchanged. |
| `a_branch_switch_is_confirmed_holds_runs_and_reports_what_happened` | `b` lists branches; a branch checked out elsewhere is refused in the chooser; confirmation text and badge; confirming during an active run is refused; after confirming, a new run and `/resume` are held and the typed prompt is kept; the switch request carries root, branch and expected HEAD and is made off the page; a `Dirty` reply is reported and runs may start again. |

`help_lists_every_command_in_the_registry` (existing) now also covers `/git`.

## Required workspace checks

Run once on the final code with `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_INCREMENTAL=0` and the target directory inside the worktree, on top of the cherry-picked
backend commit (16425aa here, e789b81 on main).

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | [fmt.txt](fmt.txt) |
| Git page tests (`cargo test -p ymp-tui --lib -- git worktree branch_switch`) | exit 0; 9 passed | [git-page-tests.txt](git-page-tests.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [clippy.txt](clippy.txt) |
| `cargo test --workspace` | exit 0; 572 passed, 0 failed, 2 ignored | [workspace-tests.txt](workspace-tests.txt) |

An earlier run of the same chain failed strict Clippy on `large_enum_variant` (the snapshot inside
`Request::Diff`); the snapshot is now boxed, and the defensive run-hold message in the event loop
now sanitises the branch name. The table above is the run after those two changes and after the worktree chooser correction below;
the totals include its new test.

## Not covered here

- No real-terminal run and no run against a real repository through the interface. The backend's
  repository tests are in [../backend](../backend/README.md); terminal integration belongs to the
  parent.
- `lib.rs` scheduling is exercised by compilation and review only: `spawn_git`, its refusal while a
  run is active, lock acquisition, and handing the lock to `switch_branch_guarded`. No interface
  test drops a waiting switch; the lock's lifetime inside the worker is the backend's guarantee.
- Readings continue once a second while a popup is open over `/git`.
- The base branch cannot be chosen from the interface; `inspect` is always called with no override.
- There is no list of commits ahead of the base: the backend API does not provide one.
- A long chooser list is windowed like the command palette, but no test covers more options than
  the popup shows.
- Two worktrees whose paths differ only before a long identical ending would still look alike: the
  chooser keeps the end of each path, not the part where they differ.

## Worktree chooser correction

The parent's real-terminal run (main
`ymp-docs/research/evidence/git-paseo-143/terminal-after/failure-worktree-chooser.txt` and its
ASCII counterpart) showed two worktrees under a long shared prefix both drawn as
`/Users/…/research/evidence/git-paseo-…`: the row kept the start of the path and cut its end, and a
long branch name took the rest of the row, so neither `project` nor `linked-worktree` could be told
apart.

- A worktree path now keeps its end through `text::last_cells`, and a branch label in the branch
  chooser is cut at its end through `text::truncate`.
- The detail column is bounded to a third of the row (at least 8 cells) with `text::truncate`, and
  the label is given exactly the cells left, so `text::row` no longer cuts it.
- A branch checked out in another worktree is described by that worktree's own name
  (`files::display_name`) rather than its full path.

`the_worktree_chooser_keeps_the_end_of_paths_that_share_a_long_prefix` renders two worktrees under
a 90-character shared prefix with long branch names, at 120×36 and 60×24, with Unicode and ASCII
markers. It requires a row that contains `/project` and one that contains `/linked-worktree`, each
still showing its branch, and that choosing the second row makes the next reading inspect exactly
that worktree.

| File | What it shows |
| --- | --- |
| [chooser-before.txt](chooser-before.txt) | Chooser unchanged: the test fails with `/project cannot be told apart at 120x36`. |
| [chooser-after.txt](chooser-after.txt) | With the correction: the new test and every other Git page test pass. |
