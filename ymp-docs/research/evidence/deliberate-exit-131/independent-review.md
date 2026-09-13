# YMP-131 independent UI review: double Ctrl+C, graceful exit and resume command

Reviewer: Claude Code (`claude-opus-5`), independent reviewer. Date: 2026-09-13.
Mode: read-only bounded patch review. No source edits, no commits, no native inference, no
provider probes, no writes to the user's `~/.ymp2`.

## Verdict

**ACCEPT, 9/10.** The accepted owner behaviour is implemented and confirmed by the evidence, with two
low-severity exceptions. Neither blocks acceptance:

- **F1 (Low, confirmed in a PTY).** The rule that omits `--home` ignores `YMP_HOME`. With
  `YMP_HOME` pointing elsewhere, `ymp --home ~/.ymp2` prints a command that opens the other
  metadata directory in that same shell.
- **F2 (Low, documentation precision).** "A repeated key is not a second press" cannot hold on
  Unix with the keyboard flags ymp requests. Holding Ctrl+C leaves ymp. The release notes
  mention the limitation, but they frame it too narrowly, and the interface guide states the
  guarantee without it.

## Reviewed source

| Item | Value |
|---|---|
| Implementation commit | `3bd5273e103dab0503de30d3ef86548988739ae7` in `/tmp/ymp131-double-ctrl-c` (clean tree), base `9eecdf0` |
| Patch size | 12 files, 748 insertions, 45 deletions: `Cargo.toml`, `Cargo.lock`, `README.md`, `ymp-docs/guides/interface.md`, `ymp-docs/guides/usage.md`, `ymp-docs/releases/0.4.2.md` (new), `ymp-tui/src/{exit.rs (new), lib.rs, state.rs, ui.rs, commands.rs, tests.rs}` |
| Integration | `2d49b9d` in `/Users/maggnus/Code/ymp2`: `git diff --quiet 3bd5273 2d49b9d -- <the 12 paths>` succeeds, so the bytes are identical. Unrelated backend 132 changes in main were ignored. |
| Parent binary provenance | `9181bfe..3bd5273` changes only `ymp-docs/releases/0.4.2.md`. The parent's binary built at `9181bfe` (`/tmp/ymp129-review-limit/target/debug/ymp`, sha256 `34f36c6ac9060688ffbc247f8254a846d378651d62f21f431a4daa27d4417dcc`) therefore carries the reviewed code. |

Out of scope, as instructed: headless progress and model naming.

## Evidence

| Evidence | Result | sha256 |
|---|---|---|
| Implementation report `/tmp/ymp131-implementation.md` | author's account of the change and its checks | `47610119baa390afaa7c63ac3eeea2ea247d507159db141e39e78c8614ae7794` |
| Author `cargo fmt` (`/tmp/ymp131-fmt.log`) | exit 0 | `19eaf43821a7660ec323a87c8457bf74823beb296c39f5e01aa8a683aa50f061` |
| Author strict Clippy (`/tmp/ymp131-clippy.log`) | exit 0, no warnings | `fc225bda65ef45eb9ac32f71be1fae78e22ba439b52ff895eed6383de55feae8` |
| Author workspace tests (`/tmp/ymp131-workspace-tests.log`) | 32 result sections: 441 passed, 0 failed, 2 ignored (recounted) | `3362a8a26a2d68c84971a70825f9c9ef1fe3d39479014185213b8d2d3f268c6c` |
| Parent PTY script `/tmp/ymp131-pty-check.py` | corrected version compares resolved paths | `5586cc95273f5c54d933904111731f8d2781781088c0e1530c49fcdd07893dcc` |
| Parent PTY result `/tmp/ymp131-parent-pty.log` | 5/5: double, input-reset, expiry, quit, sessionless | `1fa57d5ab1557d6d9da00e9b58484402f9f88d634fabdb4a32114a7ae007c708` |
| Parent active-exit script `ymp-evals/scripts/check-active-exit.py` (main working tree, untracked at review time) | real TUI, scripted ACP process that hangs in `session/prompt` | `ac7b19f3179bd44414b1f2098708f665c3443e37341c2fae9bb8cadb1a428953` |
| Parent active-exit result `/tmp/ymp131-parent-active-pty.log` | after the first press the invocation is still `running` and the prompt is shown; the second press exits with 0; session `paused`; 1 invocation `cancelled`; grants revoked; resume marker printed | `35f4aadb04e2f233b168a561c6c19566cec29962e6090f94794b6ba9119b46d8` |
| Negative control `/tmp/ymp131-pty-negative.log` | the older release binary leaves on the first idle Ctrl+C without the prompt | `6d6c0678538d70bdd086c876d004da0136630f939572819f3cd2371d23b7c304` |
| Independent build of a `git archive` snapshot (`/tmp/ymp131-review-snap` → `/tmp/ymp131-review-target`) | `BUILD_EXIT=0`, `ymp 0.4.2` | binary `6f2948161e45d05444ff956925a635e0dc7348aba7ae3b4af4b4b002b85cb215` |
| Independent `cargo test --locked -p ymp-tui --lib` (`/tmp/ymp131-review-tui-tests.log`) | 155 passed, 0 failed, 1 ignored | `65dd69af5091f392e3f795f7fa633772cdc1b8790d1e1853b54cd3afd16d2738` |
| Parent PTY script re-run on the independent binary (`/tmp/ymp131-review-parent-script.log`) | 5/5 | `5b8aa716b0f8fc04b65c17467076d63f0d8d3d5bc9ab94b9fe82fe1abb7b780b` |
| Focused PTY script for this review `/tmp/ymp131-review-focused.py` | cases A, B, C below | `b9e2afce2f9d809ff4aa4ad14372414372b2bf3de1345a8b4e8b7b76570dfb9d` |
| Focused PTY result `/tmp/ymp131-review-focused.log` | `FOCUSED_EXIT=0`; fixture root `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp131-focused-ix2azll3` (`results.json`, screen captures `A-*.txt`) | `f126662ec1cbaac5155b67f299cc945deb592ac21c2ce28ba9a81884523350dd` |

The tables below refer to the 16 new tests in `ymp-tui/src/tests.rs` by these numbers:

1. `a_first_ctrl_c_leaves_active_work_and_the_draft_untouched`
2. `a_first_ctrl_c_when_idle_does_not_leave`
3. `ctrl_c_twice_leaves_from_an_open_overlay_without_closing_it_first`
4. `a_first_ctrl_c_during_a_catalog_reading_stops_nothing`
5. `the_question_is_painted_and_withdrawn_when_its_window_passes`
6. `a_second_ctrl_c_after_the_window_only_asks_again`
7. `other_input_withdraws_the_question`
8. `a_repeated_or_released_key_is_not_a_second_press`
9. `ctrl_c_asks_the_same_question_from_every_region`
10. `quit_and_ctrl_d_still_leave_on_one_explicit_request`
11. `the_closing_status_names_the_work_it_waits_on`
12. `leaving_stops_the_work_and_names_a_session_opened_while_it_stopped`
13. `leaving_waits_only_so_long_for_work_that_does_not_stop`
14. `the_resume_command_names_the_saved_project_and_reads_back_through_a_shell`
15. `only_a_session_that_exists_gets_a_resume_command`
16. `a_word_with_quotes_or_control_characters_reads_back_exactly`

The focused checks used a temporary `HOME` and temporary metadata directories, mock
providers only, and a project path containing `' $(printf leaked) x`. Results:

- **A (80x24, standard home, `YMP_HOME` unset).**
  - The first Ctrl+C changed only row 23, the status row, which now reads
    `Press Ctrl-C again to exit`.
  - The draft `draft kept here` was still present after the first press and after the window
    expired. The prompt disappeared after 2.4 s.
  - An open command palette with the query `sto` stayed open under a first Ctrl+C. Again only
    row 23 changed.
  - The second press exited with 0, and the alternate screen was left before the hint.
  - The printed command omits `--home`. The shell read the `-C` path back exactly as the saved
    project path, without expansion. The session ID matched.
  - The printed options with `trace <id>` exit 0 in the same environment.
- **B (explicit `--home <standard>` while `YMP_HOME` points to another directory).**
  - Exit code 0. The printed command omits `--home`.
  - In the same environment, the printed options with `trace <id>` fail with
    `unable to open database file: …/other metadata/state.sqlite`.
  - Adding `--home <standard>` makes the same call exit 0. See F1.
- **C (custom home chosen only through `YMP_HOME`).**
  - The printed command names `--home '<custom home>'`.
  - Its options with `trace <id>` exit 0 in a shell without `YMP_HOME`.

## Findings

### F1 (Low, confirmed): the `--home` omission ignores `YMP_HOME`

- **Where.** `ymp-tui/src/lib.rs:319` takes the standard home to be `$HOME/.ymp2`.
  `ymp-tui/src/exit.rs:127-130` omits `--home` whenever the store home equals it. The CLI
  resolves a missing `--home` with `default_home()` (`ymp-cli/src/main.rs:151`), which returns
  `YMP_HOME` before `$HOME/.ymp2` (`ymp-core/src/config.rs:321-325`). `README.md:31` documents
  `YMP_HOME` as a supported override.
- **Failure.**
  - `YMP_HOME=/other ymp --home ~/.ymp2 -C <p> resume <id>` prints `ymp -C <p> resume <id>`.
    In the same shell, that command selects `/other`, where the session does not exist.
    Case B confirms this.
  - The same happens when a command printed by an ordinary `ymp` session is pasted into a shell
    that has `YMP_HOME` set.
  - The doc comment at `exit.rs:116-117` says the command "does not depend on the environment of
    the shell it is pasted into". That is true only when `--home` is printed.
- **Impact.** Resume fails with an error. Session IDs are UUIDs, so the command cannot open a
  different conversation, and nothing is lost. The saved session, the saved project directory
  and a custom home (cases A and C, parent PTY) are otherwise correct.
- **Suggested correction.**
  1. Compare the store home with the directory a bare `ymp` would open in the launching
     environment, that is `default_home()` from `ymp-core/src/config.rs`, rather than with
     `$HOME/.ymp2`. Alternatively, always print `--home`.
  2. Correct the comment and the docs sentence "`--home` when metadata is not in `~/.ymp2`"
     (`interface.md:91-92`, `0.4.2.md:13`) to match.
  3. Add a test. `resume_command` already takes `standard_home`, so the rule can be tested
     without changing the process environment.

### F2 (Low, documentation precision): a held Ctrl+C counts as a second press

- **Where.**
  - `ymp-tui/src/terminal.rs:53` pushes only `KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`
    and never `REPORT_EVENT_TYPES`.
  - In crossterm 0.28.1, legacy control bytes become `Press` (`src/event/sys/unix/parse.rs:106-109`),
    and so does a CSI-u key without an event-type field (`parse.rs:234-238` defaults a missing kind to 1, which `parse.rs:339-345` maps to `Press`). No terminal
    therefore delivers `Repeat` or `Release` to ymp on Unix.
  - The kind filter at `state.rs:942` is correct, but those kinds never arrive.
    `a_repeated_or_released_key_is_not_a_second_press` injects synthetic kinds.
- **Failure.** Holding Ctrl+C longer than the keyboard repeat delay produces a second `Press`
  within two seconds, and ymp leaves. A PTY byte stream cannot distinguish this from two taps,
  so it was not simulated.
- **Disclosure.** `0.4.2.md:18-19` limits the caveat to "terminals that do not report key event
  kinds". Because ymp never requests event kinds, the caveat covers every Unix terminal ymp
  drives. `interface.md:87-88` and `0.4.2.md:7` state the guarantee unconditionally.
- **Impact.** A single deliberate tap, shorter than the repeat delay, is still safe. Only a held
  key leaves. This does not block the accepted behaviour, provided the limitation is described
  precisely.
- **Suggested correction.** Either option works:
  - Request `REPORT_EVENT_TYPES` where the terminal reports keyboard-enhancement support. `Repeat`
    must then keep acting as a press for every key other than Ctrl+C; otherwise the filter at
    `state.rs:942` would stop auto-repeat of Backspace and the arrow keys in those terminals.
  - State in the guide that a held Ctrl+C counts as a second press in the terminals ymp
    currently drives.

## Owner criteria

| Accepted behaviour | Implementation and evidence | Result |
|---|---|---|
| First Ctrl+C only shows `Press Ctrl-C again to exit`; it cancels no run or scan and clears no draft | `state.rs:949` handles Ctrl+C before input withdrawal (`954-956`) and overlay routing (`957`); `interrupt` (`1027-1037`) returns no action for a first press; `ui.rs:236-237` paints the prompt instead of the status. Tests 1, 2, 4, 9; parent active-exit check (invocation still running); focused A (only the status row changes; draft kept) | Pass |
| Second intentional press within 2 s leaves; other input or expiry withdraws; Repeat and Release do not count | Confirmation `< CONFIRM_WINDOW` (`state.rs:1029`); expiry `>=` (`1040-1048`) runs on every loop iteration (`lib.rs:111`, tick 40 ms at `lib.rs:45`); keys withdraw (`state.rs:954-956`), paste withdraws (`state.rs:381`); non-`Press` kinds are filtered (`942`). Tests 5-8; parent PTY double, input-reset, expiry; focused A (withdrawn after 2.4 s) | Pass; see F2 for auto-repeat |
| `/quit` stays an explicit exit, `/stop` an explicit cancellation | Test 10; command texts at `commands.rs:236-245`; parent PTY `quit`. `Ctrl+D` in an empty composer also leaves at once (pre-existing, documented) | Pass |
| Paint the closing status, wait boundedly for cancellation, restore the terminal | `lib.rs:291-306` withdraws the question, paints `closing_status` once, then calls `stop_and_wait`. `exit.rs:65-77` cancels both tokens and waits under one 10 s deadline. Input, terminal and guard are dropped before printing (`lib.rs:316-321`). Tests 11, 13; parent PTY and focused A-C: `ESC[?1049l` precedes the hint, exit 0 | Pass |
| Shell-safe command for the actual saved session, its project directory and a custom home; no fabricated session | The command is built from `store.session` and `get_project` (`exit.rs:123-124`), so it uses the saved project path, not the launch directory. `shell_word` uses `'\''` for single quotes and `$'…'` for control characters (`exit.rs:147-172`). No session: no command (`98-100`). Unreadable session: an explanation, no command (`105-107`). Tests 14-16; parent PTY `' $() literal` path through `/bin/sh`; focused A `$(printf leaked)` read back exactly | Pass, except the `YMP_HOME` case in F1 |
| The current `resume` CLI opens the TUI with history; `/resume` continues | `ymp-cli` `Resume` without `--headless` calls `ymp_tui::run(..., Some(session))`, which loads the session for reading. The farewell text says the command starts no agents and names `/resume`. The PTY launches through `resume` stayed in the interface until Ctrl+C | Pass (code reading; `/resume` continuation not walked) |

## Session capture, bounded cleanup and keyboard boundaries

**Session capture.**
- The command names `app.session` as it was at quit time. After the wait, `stop_and_wait`
  drains queued `Message` and `Finished` events (`exit.rs:78-84`; test 12).
- For a new run, `run_internal` creates the session (`engine.rs:659`) and announces it through
  `post`, which sends `UiEvent::Message` (`engine.rs:713`, `378-381`). No `await` precedes these
  steps or separates them (`engine.rs:580-716`).
- The interface spawns `engine.run` and `follow_up` without racing them against the token
  (`lib.rs:240-282`), and cancellation is cooperative. A run that has started has therefore
  already queued its session ID, and the drain reads it even when the wait times out.
- A cancelled run also sends `Finished` with `paused` (`engine.rs:759-785`).
- A follow-up that becomes a separate task opens a continuation session and announces it the
  same way. This matches how `App::event` switches the window to that session.
- The only saved session that is never announced comes from an error between `engine.rs:659`
  and `713`. The window never learns that ID either, and no command is printed, which satisfies
  "no fabricated session".

**Bounded cleanup.**
- One deadline covers both the run and the catalog reading. `timeout_at` does not abort the
  task: the handle is dropped, and the task keeps running detached until the runtime is dropped
  when `main` returns.
- No `spawn_blocking` or `block_in_place` occurs in the workspace sources. Runtime shutdown
  after the bound is therefore not held up by blocking-pool work.
- The provider, RPC and check processes found by this review set `kill_on_drop(true)`:
  `ymp-providers/src/supervisor.rs:14`, `ymp-providers/src/rpc.rs:87`,
  `ymp-runtime/src/checker.rs:74`, `ymp-runtime/src/engine.rs:2674`. A full audit of who owns
  child processes was outside this bounded review.
- When the bound is exceeded, the farewell says that work may not have recorded its latest state
  (`exit.rs:92-96`).

**Keyboard and focus.**
- Every key passes through `on_key`, so no global shortcut bypasses withdrawal.
- Ctrl+C is intercepted before any overlay (test 3; focused A). Any other key first withdraws the question and then
  performs its usual action. `KeyCode::Modifier` is excluded; such events arrive only with flags
  ymp does not request. Paste withdraws; resize does not.
- `Action::Quit` stops the rest of the action batch (`lib.rs:172-176`).
- The window is measured when the loop processes the key. A stalled loop can therefore only turn
  a late second press into a new question, which fails safe.
- At 80 columns the prompt stays visible, because `header_row` keeps the left text and truncates
  the key hints (`text.rs:395-430`).

## Other observations (no change required for acceptance)

1. During the wait of up to 10 s, a third Ctrl+C does not force an exit. The bound is shown on
   screen.
2. External `SIGINT`, `SIGTERM` and `SIGHUP` remain unhandled, as before the patch. In raw mode a
   keyboard Ctrl+C does not raise `SIGINT`.
3. The command starts with the literal `ymp`, so it runs whichever `ymp` comes first on `PATH`.
   For a development binary launched by path, that may be a different installed version.
4. `$'…'` is emitted only when a saved path contains a control character. fish, and shells
   without ANSI-C quoting, cannot read that form.
5. `--home` is printed as `std::path::absolute` output, which is not canonical
   (`/var/folders/…`), while `-C` uses the saved canonical project path (`/private/var/…`). Both
   resolve to the same directories. The parent's first strict string comparison failed on
   exactly this difference (`/tmp/ymp131-pty-final-default-tmp.log`); the corrected script
   compares resolved paths. A home spelled differently from `$HOME/.ymp2`, for example through
   a symlink, is named redundantly, which is harmless.
6. A home or project path that is not valid UTF-8 produces "Session … could not be read again",
   followed by the encoding cause. The wording blames reading rather than encoding. This case is
   rare.

## Limitations of this review

- Read-only review. It used mock providers, the parent's scripted ACP stub, a temporary `HOME`
  and temporary metadata directories. No native inference and no credential reads.
- The full workspace suite was not re-run. The author's hash-bound logs were used, and only the
  `ymp-tui` library tests were run independently.
- Independent mutation controls were started on a separate archive, but the host ran out of disk
  before their baseline compiled. At the parent's instruction they were not repeated. Their
  partial build directory (`/tmp/ymp131-mut-target`, 334 MB) and archive copy (`/tmp/ymp131-mut`)
  were removed. The sensitivity of the 16 new tests was judged by reading them.
- An earlier version of my PTY script (`/tmp/ymp131-review-pty.py`) stopped on my own SQL query
  (`sessions` has no `status` column) and produced no evidence. `/tmp/ymp131-review-focused.py`
  supersedes it. The active-run capture it attempted is covered by the parent's
  `check-active-exit.py` and by the engine reading above.
- The following were not exercised physically: auto-repeat (see F2), the path where work outlives
  the 10 s bound (unit-tested with a future that never completes), a third Ctrl+C during the
  wait, and `/resume` continuation after reopening. Windows was not examined.
- `/tmp/ymp131-review-target` (1.0 GB) remains so the independent binary can be reproduced. It
  can be removed once this report is accepted.
