# YMP-131 implementation report

- Worktree and branch: `/private/tmp/ymp131-double-ctrl-c`, `fix/double-ctrl-c`.
- Commit: `3bd5273e103dab0503de30d3ef86548988739ae7`
  "Confirm Ctrl+C before leaving and print the resume command (YMP-131)", on top of `9eecdf0`.
  The branch contains only this commit. The YMP-132 backend commits that were briefly
  cherry-picked were removed with `git reset --keep` after the scope change; the originals remain
  on `fix/message-attribution`.
- The checks below ran on `9181bfe`. The amend to `3bd5273` changed only
  `ymp-docs/releases/0.4.2.md`; code is identical.

## Behavior

- `App::on_key` delegates to `on_key_at(key, width, now)`. Ctrl+C is handled before overlay and
  focus routing. The first press sets `exit_requested` and returns no actions: no `Cancel`, no
  `Quit`, and the draft, overlay, focus, view and theme are unchanged. A second press within
  `CONFIRM_WINDOW` (2 s) returns `Action::Quit`; a later press asks again.
- Any other key press (except a bare `KeyCode::Modifier`) or a paste withdraws the request.
  `KeyEventKind::Repeat` and `Release` are ignored entirely. The run loop calls
  `expire_exit_request(now)` on every iteration, so the prompt disappears after 2 s.
- While a request is pending, the status row shows `Press Ctrl-C again to exit` in the warning
  style; `app.status` is not overwritten.
- The second Ctrl+C, `/quit` and `Ctrl+D` in an empty composer share one exit path:
  1. `exit::closing_status` is painted immediately and names the active run and/or catalog
     reading and the 10 s bound.
  2. `exit::stop_and_wait` cancels the run and scan tokens and waits for both handles against one
     10 s deadline.
  3. Queued runtime events (`Message` and `Finished`) are drained, so a session reported while
     stopping is captured.
  4. Actions that follow `Quit` in the same input batch are not executed.
- After the loop, the input reader, `Terminal` and terminal guard are dropped (the alternate screen
  is left and raw mode is disabled). Only then is `exit::farewell` written to stdout. If the bound
  expired, it first prints a "stopped waiting" note. For a real session it prints:

  ```text
  Resume this session with:
  ymp [--home HOME] -C PROJECT resume SESSION_ID
  This opens the saved conversation and starts no agents. Use /resume there to continue the run, or send a message to continue the conversation.
  ```

  - `PROJECT` is `store.get_project(session.project_id).path`, the saved canonical project path,
    not the launch `-C`.
  - `HOME` is `std::path::absolute(store.home)`, printed whenever it differs from `$HOME/.ymp2`.
  - Words are single-quoted for POSIX shells; words with control characters use `$'\xHH'`
    escapes.
  - Without a session nothing is printed. For an unreadable session, an explanation is printed
    instead of a command.
- Docs and help were updated: help/palette text (`KEYS`, `/quit`, `/stop`), README (status 0.4.2
  and keyboard paragraph), `ymp-docs/guides/interface.md`, `ymp-docs/guides/usage.md`, and the
  new `ymp-docs/releases/0.4.2.md`.
- The workspace version and all eight workspace entries in `Cargo.lock` are bumped to 0.4.2.
- The task register is untouched.

## Files

`Cargo.toml`, `Cargo.lock`, `README.md`, `ymp-docs/guides/interface.md`,
`ymp-docs/guides/usage.md`, `ymp-docs/releases/0.4.2.md`, and
`ymp-rust/crates/ymp-tui/src/{exit.rs (new), lib.rs, state.rs, ui.rs, commands.rs, tests.rs}`.
No backend, CLI, storage or provider source changed.

## New tests (ymp-tui, section "Leaving", 16)

1. `a_first_ctrl_c_leaves_active_work_and_the_draft_untouched` (regression control).
2. `a_first_ctrl_c_when_idle_does_not_leave` (regression control).
3. `ctrl_c_twice_leaves_from_an_open_overlay_without_closing_it_first` (regression control).
4. `a_first_ctrl_c_during_a_catalog_reading_stops_nothing`.
5. `the_question_is_painted_and_withdrawn_when_its_window_passes`.
6. `a_second_ctrl_c_after_the_window_only_asks_again`.
7. `other_input_withdraws_the_question`: a key and a paste withdraw it; a bare modifier does not.
8. `a_repeated_or_released_key_is_not_a_second_press`.
9. `ctrl_c_asks_the_same_question_from_every_region`: composer, transcript, page, sidebar and
   theme chooser.
10. `quit_and_ctrl_d_still_leave_on_one_explicit_request`; `/stop` still returns `Cancel`.
11. `the_closing_status_names_the_work_it_waits_on`.
12. `leaving_stops_the_work_and_names_a_session_opened_while_it_stopped`: the run and scan tasks
    finish only on cancellation, and a `Message` queued during shutdown is captured.
13. `leaving_waits_only_so_long_for_work_that_does_not_stop`: a 50 ms bound yields `unfinished`.
14. `the_resume_command_names_the_saved_project_and_reads_back_through_a_shell`: the session is
    saved under `project ' $(printf leaked) "literal"`, the window starts elsewhere, and `/bin/sh`
    reads back `--home`, `-C` with the saved path, and `resume` with the ID.
15. `only_a_session_that_exists_gets_a_resume_command`: the standard home is omitted, no session
    prints nothing, and an unreadable session prints no command.
16. `a_word_with_quotes_or_control_characters_reads_back_exactly`: `bash --noprofile --norc` and
    `zsh -f`.

Time-dependent tests use injected `Instant` values; only the 50 ms bound test waits in real time.

## Evidence

- Regression control before the fix: `/tmp/ymp131-regression-control.log`. Tests 1-3 compiled
  against the unfixed source and failed (exit 101): `[Cancel]` instead of `[]`, `[Quit]` instead
  of `[]`, and `[]` instead of `[Quit]`.
- `cargo fmt --all --check`: `/tmp/ymp131-fmt.log`, exit 0.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: `/tmp/ymp131-clippy.log`,
  exit 0. An earlier run flagged `type_complexity` in a new test; that was fixed before the commit.
- `cargo test --locked --workspace`: `/tmp/ymp131-workspace-tests.log`, exit 0. 441 passed,
  0 failed and 2 ignored (existing fixture tests); ymp-tui alone has 155 passed and 1 ignored.
- Debug build `ymp 0.4.2` at `/tmp/ymp129-review-limit/target/debug/ymp`, SHA-256
  `34f36c6ac9060688ffbc247f8254a846d378651d62f21f431a4daa27d4417dcc`
  (`/tmp/ymp131-final-head.txt`). No release build or installation was made.
- PTY checks used the deterministic mock demo with temporary homes and no native inference:
  - `/tmp/ymp131-pty-final-canonical-tmp.log`: the parent script `/tmp/ymp131-pty-check.py`, run
    with `TMPDIR` set to its realpath, passed all five cases (`double`, `input-reset`, `expiry`,
    `quit`, `sessionless`).
  - `/tmp/ymp131-pty-final-exit-probe.log`, from `/tmp/ymp131-pty-exit-probe.py` (a diagnostic
    copy of the `double` case): in 3 of 3 runs the hint appeared after the first Ctrl+C, the
    process exited with status 0 about 21 ms after the second, and the alternate screen was left
    before the hint.
  - `/tmp/ymp131-pty-final-default-tmp.log`: the parent script with the default
    `TMPDIR=/var/folders/...` fails at `'wrong project'`; see below.

## Note on the parent PTY script

`ymp -C /var/folders/.../project...` stores the canonical project path
`/private/var/folders/.../project...`, because `Store::project` canonicalizes it. The hint prints
that saved path, as required. The script compares the printed path with the non-canonical
`str(project)`. Suggested fix: compare
`os.path.realpath(words[words.index('-C') + 1]) == os.path.realpath(project)`, or run the script
with a canonical `TMPDIR`. `--home` is printed as given (made absolute, not canonicalized), so
that comparison passes.

One earlier run, the first launch of a freshly built binary, failed at `'did not exit normally'`.
It did not reproduce in 22 later launches. The script calls `proc.poll()` immediately after PTY
EOF, so this may be a reaping race; `proc.wait(timeout=...)` would avoid it.

## Limitations

- Terminals that do not report key event kinds deliver auto-repeat as ordinary presses. Holding
  Ctrl+C there can count as the second press; no debounce was added.
- If the run or scan does not stop within 10 s, ymp leaves and says so. As before, those tasks are
  dropped with the runtime, and the latest progress may be unrecorded.
- `--home` is also printed when `YMP_HOME` names the metadata directory. This is deliberate: the
  command must not depend on the shell environment.
- A path that is not valid UTF-8 yields an explanation instead of a command. Control-character
  escapes use `$'...'`, which bash, zsh and ksh support but older dash does not.
- Not verified here: a release or installed binary, a real terminal emulator outside the Python
  PTY, and stopping a long-running real-provider run (mock only). The task register and the
  parent's PTY script were not modified.
