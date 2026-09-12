# YMP-107 independent review, round 2: recorded checks and factual recovery limits

**Verdict: ACCEPT. R2 = 9/10.** Written 2026-09-12 20:39 HKT (2026-09-12T12:39Z). Both
corrections required by round 1 are in place and were confirmed against the running
interface, the no-session wording is corrected, and the task-aware scope added during
integration is correct, disclosed and matched by real, legacy and adverse records.

Reviewed the integrated candidate `3540ecd1c6344c5c0c17eda2334612f8ba32f7b9` in the
`integrate107` worktree, which was clean before and after the review and is still clean. The
candidate is four commits: integration base `5748512` carrying the current main with the 101
and 111 work, the cherry-picked delivery `4bc500a`, the wording correction `a45fe9c`, and the
task-aware integration `3540ecd`. Source was read only. Every probe ran in throwaway
directories under `/tmp` outside both worktrees. The round 1 report in the `review107`
checkout was not modified and is intact at 13415 bytes.

Reviewer: delegated Claude session, model `claude-opus-5`, permission mode `bypassPermissions`,
thinking level requested as `max`. The thinking level is not observable from inside the
session, so it is recorded as requested rather than as measured. No provider inference request
was made and no credential was read: every walk used `ymp demo --tui` or a hand-written
configuration with `kind = "mock"`, which returns before a process is spawned. The task
register and `intent.md` were not touched.

## Commands and results

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all --check` | 0 | no diagnostics |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no diagnostics |
| `cargo test --workspace` | 0 | 161 passed, 0 failed, across 11 test binaries |
| `cargo build --bin ymp` | 0 | toolchain 1.89.0 as pinned |
| `ymp --home … -C … demo` (mock, project two levels below a `.git`) | 0 | one task-scoped and one unscoped check recorded |

## Round 1 findings, rechecked

**Required 1, the false reachability claim, is corrected.** The architecture document now
states that the remainder is not reachable from the interface, that `Enter` opens the lines the
row was already built with, and that the rest stays in the session log. Confirmed against the
product, not only the text: with a record holding the full 20000 characters the row's note
reads "208 further lines were recorded and no page shows them; they stay in the session log",
and the overlay opened by `Enter` scrolls to that same note with "0 more" remaining.

**Required 2, the overstated hint, is corrected.** The hint now reads
"Enter — show the recorded run" on every frame of the page, and the delivered test asserts both
the exact text and that `Enter` opens the same line count the row holds.

**Non-blocking 3, the no-session empty state, is corrected.** With no session open the page
reads "Nothing was read" over "No session is loaded, so no session log was read. Open one from
/sessions to see the commands ymp ran for it.", while the subtitle still says no session is
loaded. `Store::checks` is not called in that state.

**Non-blocking 4, a declared command hidden by another task's run, is resolved rather than
disclosed away.** Commit `a45fe9c` had disclosed it as text-alone matching; `3540ecd` found
that statement false on the integration base, where `Engine::checks` already takes a
`TaskAttemptRef` and writes it with the event, and replaced the disclosure with task-aware
matching and a description of what an unscoped record covers. The page text was updated in the
same commit, so no stale claim survives.

## The added task-aware scope

`Engine::checks` at `ymp-runtime/src/engine.rs:1807` writes `"task": task`, with
`Some(TaskAttemptRef::from(&*task))` from the per-task verification at `:1627` and `None` from
the final pass over the deduplicated union of declared commands at `:1188`. `CheckRun` gained
an `Option<TaskAttemptRef>` and `Store::checks` reads it through `recorded_task`, which accepts
only a reference that deserializes and carries a non-blank identifier; `TaskAttemptRef` has no
serde defaults, so a reference without an attempt fails to deserialize and is read as unscoped.
Coverage in `views.rs:968` requires the command text to match and the record either to name
this task or to name none.

Six control cases were driven through the interface on a session that had first been produced
by a real deterministic run:

| Case | Observed |
| --- | --- |
| Real per-task run | row reads `task  Create a greeting · attempt 1` |
| Real final pass | row reads `task  no task recorded` |
| Two tasks declare one command, one scoped run | the run names "Check the layout"; the declared list still holds that command for "Polish the page" alone |
| Legacy record with no `task` field, two tasks declaring it | row reads `no task recorded` and neither task appears as declared without a run |
| Record naming a task the session no longer holds | row falls back to the shortened identifier, `deadbeef · attempt 3` |
| Record scoped to a task now on a later attempt | treated as covered, row shows the recorded attempt |

The page subtitle read `6 recorded · 2 declared without a run` for that state, and the declared
rows were exactly the two commands with no qualifying run. The declared row's prose was updated
to match the logic: it now says the log holds no run "for those tasks and none recorded without
a task". A scan of the page and the document for the wording this contract forbids, including
the removed "whole record", returns nothing.

## Conflict resolution and scope

The cherry-pick was compared against the original commit by reducing both patches to their
added and removed lines: `9562f8b^..9562f8b` and `4bc500a^..4bc500a` are identical, 1478 diff
lines each, so nothing of the delivery was lost or altered while rebasing onto the newer base.
`write_task` and `write_plan` in `ymp-storage/src/lib.rs` are byte-identical to their versions
at base `5748512`, confirmed by hashing the function bodies at both revisions, and
`recorded_text` is byte-identical to its round 1 version. The storage diff against the base is
purely additive: `Store::checks`, `recorded_task`, `recorded_text` and their tests.

The candidate changes 14 files: four documents, `Cargo.lock`, one manifest, and eight Rust
files. It contains no engine change, no event-schema change, no migration, and no edit to
`ymp-docs/tasks/` or `intent.md`. The core change is two new types and one new field; the only
behavioural code outside the interface is one read on the store.

## Findings

None blocks acceptance.

1. **Coverage is by task identity, not by attempt.** A run recorded for attempt 1 covers its
   command for the same task now on attempt 2, so a command that was not re-run after a
   revision is not listed as declared without a recorded run. Demonstrated: a task at attempt 2
   whose only record was written for attempt 1 produced no declared row, and its recorded row
   displayed `attempt 1`. The page's wording is task-level throughout and never promises
   per-attempt scoping, so this is not an overclaim, but the row shows an attempt that the
   matching ignores and a reader may take the two as connected. Either matching on the pair, or
   saying on the page that a task's earlier attempt counts, would close the gap.

2. **`ymp-docs/architecture/recorded-checks-and-recovery.md` has no trailing newline**, while
   the other documents in the repository end with one. Cosmetic, one byte.

3. **Shared-frame clipping is still present and is explicitly deferred.** Detail text loses two
   columns in the inline pane and more in the `Enter` overlay, because pages wrap for a width
   the panes do not have. It was reproduced again here on the candidate and on an untouched
   page, it affects every page rather than these, and the assignment defers it to YMP-118, so
   it is recorded and not counted against this delivery.

## Limits of this review

The walks used the in-process mock provider, so no statement here rests on a real provider
turn. Terminal captures came from a minimal screen emulator written for round 1, which
reproduces cursor positioning, erasure and text placement but is not a full terminal; its
readings agree with the assertions of the delivered interface tests. Sizes exercised were 80x24
and 100x30 and 100x34 on the Ember palette with Unicode markers; the other four palettes were
read in `theme.rs` and not rendered. The legacy record was synthesised by writing an event
without a `task` field rather than by replaying a store written by an older build, which is the
same shape `Store::checks` sees but not the same provenance. `cargo` ran without `--locked` in
the candidate's own ignored `target/` directory; `Cargo.lock` was not modified and the worktree
stayed clean. The thinking level of the delivering session cannot be verified from the
repository, and the model attribution rests on the commit trailers.
