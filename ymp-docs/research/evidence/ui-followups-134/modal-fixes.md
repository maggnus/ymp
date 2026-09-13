# YMP-134 review R1 and O1: floating-surface corrections

This report covers two fixes to floating surfaces in the terminal interface:

- R1: the cleared margin around a surface must not cut the vertical rule beside the sidebar.
- O1: the palette and theme chooser selection must stay visible in a short terminal.

## Where the work is

- **Checkout and branch:** `/private/tmp/ymp131-double-ctrl-c`, branch `fix/ymp134-modal-review`, created from `b01ffe7`.
- **Exit branch:** `fix/double-ctrl-c` is unchanged at `79584d4`.
- **Commits:**
  1. `9a88591f0b6954dc6fc9668b27dd54272d5afe10`, "Keep the sidebar rule whole beside a floating surface (YMP-134 review R1)". It changes `frame.rs`, `ui.rs` and `tests.rs` (+100/−26).
  2. `6d72b317eb739328bff6aa934190992ae481b96b`, "Keep the palette and theme selection in view on a short terminal (YMP-134 review O1)". It changes `frame.rs`, `ui.rs` and `tests.rs` (+119/−5).
- **Scope:** only `ymp-rust/crates/ymp-tui/src/{frame.rs,ui.rs,tests.rs}`. Not changed:
  - pages or tables (`views.rs`);
  - backend, stored data or version;
  - README, documentation or the task register.
- **O2** (detailed mode repeating short messages) is not touched.

## R1: the margin keeps the rules whole

- **The change:**
  - `frame::clear_around(frame, rect, regions: &[Rect], theme)` blanks the one-cell margin only where it falls inside `regions`.
  - `render_modal` now takes `regions` instead of `bounds`.
- **Callers:**
  - `ui::render` passes the main column and the sidebar panel, or only the main column when there is no sidebar.
  - The completion list passes its main column, as before.
- **Effect:**
  - The rule under the header, the rule above the composer and the vertical sidebar rule keep every cell the surface itself does not cover.
  - The margin inside the main column and the sidebar is still blanked.
  - Padding and column alignment are unchanged.

## O1: the selection stays visible

- **Shared rule.** `frame::modal_body_rows(height, keys)` says how many body rows a surface shows: `height − 4`, minus 2 more when it has keys, and at least 1. `render_modal` uses the same rule, so its result is unchanged.
- **Palette:**
  - The list is at most 10 commands and never longer than the rows left under the search line.
  - When commands remain below, one row is kept for the "N more" count.
  - The body is not scrolled, so the search line, where the cursor sits, stays the first row.

  | Size | Before the fix | After the fix |
  |---|---|---|
  | 40x12 | 4 command rows; the selection disappears on the 5th command | 3 command rows plus the count |
  | 80x16 | 8 command rows; the selection disappears on the 9th command | 7 command rows plus the count |
  | 80x24 and taller | 10 command rows plus the count | unchanged |

- **Theme chooser:**
  - The list scrolls by `selected + 1 − body rows` when that is positive.
  - At 40x12 and 80x16 this is always 0, so the view is unchanged.
  - At 80x10 the list scrolls one row so the last theme stays visible.

## Tests

The tests are in `tests.rs`, right after `a_floating_surface_leaves_the_rules_around_the_body_intact`. Each test collects the failures for every size before it asserts.

- **`a_floating_surface_keeps_the_sidebar_rule_whole_outside_itself`**
  - Cases:
    - theme chooser at 120x36 and palette at 134x36, both ending one column short of the rule;
    - palette at 120x36, which covers the rule.
  - Checks:
    - every body row outside the surface's own area has `│` in column `main_width(W, sidebar_width(W))`;
    - the rule under the header and the rule above the composer are whole.
- **`the_palette_keeps_its_selection_in_view_on_a_short_terminal`**
  - Sizes: 40x12, 80x16 and 80x10.
  - Moves: 12 `Down`, then 12 `Up`.
  - After every move, both the selected command row and the search line must be on screen.
- **`the_theme_chooser_keeps_its_selection_in_view_on_a_short_terminal`**
  - Sizes: the same three.
  - Moves: `Home`, then `Down` through all five themes and back.
  - After every move, the selected theme's row must be on screen.

## Verification

**Environment** for every run:

- `CARGO_TARGET_DIR=/tmp/ymp129-review-limit/target`. This is the cache this checkout used for YMP-131; no other process was using it.
- `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`.
- Free disk space stayed at 2.1 GiB.

**Runs:**

1. **Failing before the fixes** (`/tmp/ymp134-modal-before.log`; the first version of the tests is in `/tmp/ymp134-modal-before-first.log`). The new tests ran on the `b01ffe7` source and exited with 101: 3 failed, and the existing rule test passed.
2. **State of commit `9a88591`** (`/tmp/ymp134-modal-r1.log`):
   - `cargo fmt --all --check` exited with 0.
   - `cargo test --locked -p ymp-tui --lib`: 174 passed, 1 ignored.
3. **Final state `6d72b31`** (`/tmp/ymp134-modal-o1.log`):
   - `cargo fmt --all --check` exited with 0.
   - `cargo test --locked -p ymp-tui --lib`: 176 passed, 0 failed, 1 ignored. This includes the double Ctrl+C, Inspect and floating-surface tests.
4. **Clippy** (`/tmp/ymp134-modal-clippy.log`): `cargo clippy --locked -p ymp-tui --all-targets -- -D warnings` exited with 0.
5. **Negative control with the committed tests** (`/tmp/ymp134-modal-negative-control.log`). With `frame.rs` and `ui.rs` taken from `b01ffe7`, the run exited with 101:
   - the theme chooser at 120x36 cut the sidebar rule on rows 7–20;
   - the palette at 134x36 cut it on rows 5–23;
   - the palette at 120x36 cut it on rows 5 and 23;
   - in the palette, `/tasks` went out of view at 40x12 after 4 moves, `/diff` at 80x16 after 8 moves and `/details` at 80x10 after 2 moves;
   - in the theme chooser, `Terminal` went out of view at 80x10 after 4 moves.

   The files were then restored from `HEAD`, the working tree was clean, and the four tests passed (exit 0).

**Not run:**

- workspace-wide Clippy or tests, and the bridge tests;
- PTY or tmux terminal checks;
- a release build or installation;
- any native provider inference.

## Limits

- **Theme chooser at 40x12 and 80x16.** The selection was already visible there before the fix. Those cases are regression checks; only 80x10 failed before.
- **Height 80x10.** It is the lowest height that keeps the full layout (`CHROME_MIN_HEIGHT = 10`). The existing smallest-size page test uses 40x12.
- **Theme summary.** At 40x12 the last line of the chosen theme's summary can still be cut. The surface shows a "N more" note at the bottom, and the selected row itself stays visible.
- **Possible merge conflict.** The new tests sit near the top of `tests.rs`. A textual conflict with the table work is possible only if that work edits the same place.
