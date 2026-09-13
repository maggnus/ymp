# YMP-146 owner recovery UI

Branch `feat/ymp146-recovery-ui`, base `211f314`. The change is limited to
`ymp-rust/crates/ymp-tui`, the terminal scenario
`ymp-evals/scripts/check-session-recovery.py` and this directory. Runtime, core, storage,
providers, CLI and Cargo files are unchanged; the UI only calls the accepted owner API and reads
the team, owner exclusion and current-files authorization records.

## Route in the window

`/team` with a loaded session shows three sections above the existing membership tables:

- **Session control** — the session, its hold state, the owner command in progress, a failed
  command that can be sent again unchanged, and a continuation that was authorized but did not
  start.
- **Current team** — members, agents leaving after their current work, agents joining and agents
  the owner removed, with the work each one holds, from `Engine::team_control`.
- **Stopped work** — every recovery stage that is not finished, with its concrete reason.

Enter on a row opens a choice list; unavailable choices stay visible with their reason.

| Row | Choices | Owner API |
| --- | --- | --- |
| Session | Pause session, Continue session, Wait for a condition | `owner_team_command` |
| Member | Add to the session (removed agents), Remove from the session, Replace | `owner_team_command` |
| Stopped stage | Review saved plan again, Inspect effects, Continue with current files, Retry, Continue, Wait, Pause, Release hold | `review_saved_plan_fresh`, `inspect_recovery`, `current_files_context` + `continue_with_current_files`, `control_recovery` |

- `/team add ID` and `/team remove ID` with a loaded session, Space on a member row and the
  member choices change the session team through `owner_team_command` with the read revision.
  Choices from the page always ask for confirmation; a typed command asks when the session has a
  fixed roster, and the question says the roster is amended. Starting preferences in
  `Config.team` are not changed. Without a session the commands edit preferences as before.
- A busy member that is replaced or removed keeps its current work; the page shows it as leaving.
- Review saved plan again lists eligible candidates; the plan author, agents that failed on the
  stage, removed and leaving agents are shown as unavailable with the reason. The owner API
  decides independence.
- Continue with current files reads the context on a blocking thread, then asks one question
  that names the session, directory, stage, failed calls, listed files and that effects stay
  unverified. Yes records the authorization, then starts the ordinary run of the same session in
  its saved directory. If the run cannot start, the receipt is kept and offered again without a
  second authorization.
- Owner work runs on its own task and cancellation token; one command at a time. Replies carry
  the request they answer: a reply for a session that is no longer displayed does not change the
  page, a failed command keeps its exact command ID for Retry, `/stop` cancels a review or an
  inspection, and quitting waits for owner work within the existing exit bound. Runs and branch
  switches are refused while a review, inspection or authorization uses the directory.

## Checks

All commands used `CARGO_TARGET_DIR=/Users/maggnus/.paseo/worktrees/1ms2ynax/ymp146-recovery-backend/target`.

Scenario tests in `ymp-tui/src/tests/recovery.rs` call the state layer and perform its requests
with the same `control::perform` the event loop uses, against real stores, the offline native
protocol fixture and compiled scripted backends:

1. `a_saved_plan_is_reviewed_again_and_continued_with_current_files_from_the_team_page` — replace
   the failed member, a new read-only review, refused inspection with its reason, the
   confirmation at 100x32, 60x16 and 48x12 in Unicode and ASCII, changed files refusing the
   authorization, the same command returning the same receipt, a run that did not start offered
   again without a second authorization, and the ordinary run completing with one plan, one plan
   review, one authorization, unchanged failures and no call to the failed agent.
2. `stage_holds_outlast_team_changes_and_unavailable_actions_name_their_cause` — no eligible
   reviewer, Wait and Release hold, a hold kept across a team change, unverified earlier
   execution, exhausted budget, menus at four sizes in Unicode and ASCII.
3. `owner_work_stays_with_its_session_and_a_native_review_stops_with_the_window` — a stale read,
   review, current-files read and authorization for a session that is no longer displayed;
   a session with no initialized team; a gated native review refused beside runs, cancelled by
   `/stop` and exit within the bound, and not issued twice by Retry.
4. `a_busy_member_is_replaced_during_a_run_without_touching_starting_preferences` — Replace
   during a run keeps the current invocation, the newcomer takes the next work, preferences stay
   unchanged, and without a session `/team add` edits preferences.
5. `recorded_effects_are_inspected_and_the_stage_continued_as_separate_owner_actions` — a usable
   inspection, then Continue as a separate decision, then `/resume` completes without planning.

The earlier sidebar test was changed: it edited preferences with `/team remove` on a loaded
session, which is now a session team command. It edits them on the agents page and asserts that
`/team remove` leaves `Config.team` alone.

### Mutation controls

[`mutations/mutate.py`](mutations/mutate.py) replaces one exact source fragment, runs the named
tests, restores the file and compares its SHA-256. Each run was alone, after the other builds.

| Mutation | Round 1 | Round 2 |
| --- | --- | --- |
| `/team add` or `/team remove` with a session edits preferences | 2 / 2 tests failed | — |
| stale read accepted regardless of generation and session | **survived** | failed at the new stale-read assertion |
| Authorize reply starts a run for a session not displayed | **survived** | failed: `a run started for a session that is not displayed` |
| stage hold ignored in the stage choices | failed for an unrelated reason¹ | failed at the hold reason of Review saved plan again |

¹ Round 1 replaced the hold pattern with `LegacyUnbound`, which is the fixture stage's own reason,
so the test failed before the hold. Round 2 uses a pattern guard that never matches. The two
survivors were closed by the assertions added to test 3; all sources were restored with matching
hashes in both rounds. Excerpts and reports are in [`mutations`](mutations).

### Final sequence

Run once on the final sources, after every fix and mutation restore
([summary](final-checks.txt)):

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all --check` | 0 | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 632 passed, 0 failed, 2 ignored (existing) in 36 result blocks |

The regressions found by the earlier `ymp-tui` run were fixed before it: the sidebar test above
and a Clippy `large_enum_variant` on `control::Request`, whose review and authorization variants
are now boxed. The terminal scenario used the debug binary built after that fix; later changes
were test assertions only.

## Terminal scenario

`check-session-recovery.py` drives the debug `ymp` binary in tmux against offline native
protocol fixtures (`ymp-runtime/tests/fixtures/fresh_plan_review.py`). A first `ymp run` stops
on a failed ACP plan review. In the resumed window: `/team`, `/team add fresh` (confirmed),
`/team remove failed` (confirmed), Review saved plan again with `fresh`, Continue with current
files (confirmed), and the ordinary run completes.

| Variant | Result | Checks |
| --- | --- | --- |
| [`terminal/unicode-100x32`](terminal/unicode-100x32/report.json) | passed | 11 / 11 |
| [`terminal/ascii-60x18`](terminal/ascii-60x18/report.json) | passed | 11 / 11 |

Both traces record one `plan_proposed`, one `plan_review`, one
`owner_current_files_authorized`, one accepted task, a completed session and the historical
failure of `failed`; after the owner actions there is one new `review_plan` request by `fresh`,
one `execute` request, no planning request and no request to `failed`; `config.toml` is
unchanged. Screens are saved per step. The SQLite state and full trace were replaced by
`trace-summary.json` to keep the evidence small.

The first two attempts failed in the script, not in the window: typing on a page goes to its
filter, the stage row lies below a 60x18 page, and the replacement agent had no model, which the
owner API correctly refused (“candidate needs verified native metadata”).

## Not verified

- Linux and Windows terminals; only macOS tmux was used.
- Real providers, credentials and real sessions; all native agents are protocol fixtures.
- Provider process trees on cancellation or exit beyond the fixture and the exit bound.
- A baseline on the installed 0.4.6, which was not touched.
