# YMP-133 branch: owner-requested interface changes before the tables

Branch `feat/ymp133-tables` in `/private/tmp/ymp132-agent-attribution`, based on `main` at
`5899589`. Nothing here belongs to YMP-132. The table work itself has not started; its plan is
`/tmp/ymp133-table-plan.md`.

| Commit | Owner request | Change |
|---|---|---|
| `56c9d05` | The slash panel and popups are badly formatted | Floating surfaces get a cell of padding inside the border, a blank row before their keys and a one-cell cleared margin. The slash panel and the palette put summaries in one column and truncate them only to the remaining room; theme names and record labels align. |
| `6fd4401` | Do not show navigation in the sidebar | The NAVIGATE list, sidebar focus and navigation selection are removed. The sidebar is SESSION, TOKENS, TEAM and TASKS and never takes the keyboard. `Tab` moves between the composer and the transcript or page; a page opens with its command or from the palette. |
| `5c3dda8` | Detailed mode must stop collapsing agent reasoning | In detailed mode every plan, bid, review, learning note and execution report is drawn in full under the sentence the default mode shows. Before, the first raw line of the payload replaced the sentence and the body and long reports stayed collapsed. `Space` and its hint are inactive there; stream previews stay bounded. |
| `5e14f6e` | Follow-up to `56c9d05`, found in the 80x24 check | The margin cleared around a floating surface stays inside the body, so the palette at 80x24 no longer cuts a gap into the rule under the header. |

Scope: `ymp-tui` sources and tests, `ymp-docs/guides/interface.md` and `ymp-docs/guides/usage.md`.
No backend core, storage or runtime source, version, release notes, README, task register, real
`~/.ymp2` or native provider inference.

## Verification

Environment: `CARGO_TARGET_DIR=/tmp/ymp132-test-target CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`.

- `56c9d05` was checked visually at 120x36 earlier in the session (slash panel, palette, theme
  chooser, Inspect). Its tests are part of every run below.
- `6fd4401`: `cargo fmt --all --check` and `cargo clippy -p ymp-tui --all-targets -- -D warnings`
  exit 0; `cargo test -p ymp-tui` 168 passed, 1 ignored. Negative control with the previous
  `sidebar.rs`, `state.rs`, `ui.rs`, `frame.rs`, `views.rs` and `lib.rs`:
  `tab_cycles_one_focus_owner_and_esc_walks_back_to_the_composer` and
  `the_sidebar_lists_no_destinations` fail; the restored files match their SHA-1. Logs
  `/tmp/ymp133-nav-*.log`.
- `5c3dda8`: fmt and clippy exit 0; 169 passed, 1 ignored. Negative control with the previous
  `transcript.rs`, `ui.rs` and `state.rs`: `detailed_mode_collapses_no_agent_message` fails. The
  detailed transcript showed the raw JSON line instead of the plan sentence, "17 more lines ·
  Enter to read the full report" and the `Space expand` hint. Restored files match. Logs
  `/tmp/ymp133-details-*.log`.
- Workspace after `5c3dda8`: `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test --workspace`
  34 runs, 461 passed, 0 failed, 2 ignored. Logs `/tmp/ymp133-ws-*.log`.
- `5e14f6e`: fmt and clippy exit 0; 170 passed, 1 ignored. Negative control with the previous
  `frame.rs` and `ui.rs`: `a_floating_surface_leaves_the_rules_around_the_body_intact` fails with
  "the palette cut into the rule on row 1" and the row `───      ───`. Restored files match. Logs
  `/tmp/ymp133-margin-*.log`.
- Workspace after `5e14f6e`: `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test --workspace`
  34 runs, 462 passed, 0 failed, 2 ignored. Logs `/tmp/ymp133-ws2-*.log`.

Visual check in tmux (`tmux -L ymp133`, `HOME=/tmp/ymp133-look/fakehome`,
`YMP_HOME=/tmp/ymp133-look/home`). The completed scripted session `4a2770b1` was loaded for
reading with Enter on `/sessions`; nothing was resumed and no agent started.

- Build `5c3dda8` at 120x36 and 80x24: the sidebar shows SESSION, TOKENS, TEAM and TASKS and no
  NAVIGATE. The `main` build at 80x24 shows NAVIGATE with "▼ 15 more" above the same sections.
  The welcome names the commands and the palette as the way to open a page.
- `/details` at 120x36: the header shows `details`; the plan, the plan review, the reviews and
  the learning note keep their sentences with the full JSON below them, and the execution report
  is whole. After `Tab` the status row reads `Enter inspect  Esc composer`. Turning detailed mode
  off restored the one-line entries; in the default mode the status row offers `Space expand`.
- Slash panel and palette at 80x24, Inspect at 120x36 and 80x24: padding, aligned columns, a
  blank row before the keys and a cleared margin. The palette at 80x24 cut the header rule, which
  `5e14f6e` fixes.
- Build `5e14f6e`, palette at 80x24: the rule under the header is whole, and the palette keeps
  its padding, columns and keys. The tmux sessions were closed after the check; the builds stay
  in `/tmp/ymp133-look`.

## Limitations and open points

1. Detailed mode expands agent messages. Runtime notices (`ymp` notices, `plan_accepted`,
   `memory`) stay one row and are truncated at the width, as before; `Enter` shows them in full.
2. `Space` does nothing in detailed mode, rather than marking an entry that would stay expanded
   after detailed mode is turned off.
3. Not re-checked visually: the Prompt surface (agent model and instructions editors), the
   Confirm surface, and the theme chooser at 80x24, which only the rule test covers.
4. Seen and not touched by these commits: the sidebar's "none · the next message opens one" is
   clipped without an ellipsis at 80 and 120 columns; at 80 columns the welcome values end
   against the sidebar rule without a gap; the Inspect title of a task row is the task UUID,
   which the table plan addresses.
5. The YMP-133 tables are not started. The parent asked to align the integrated base first, and
   moving Memory search from `/` to `s` needs the owner's confirmation.
