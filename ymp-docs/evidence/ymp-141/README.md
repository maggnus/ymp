# YMP-141 evidence: stable popup size while scrolling

Every scenario uses a temporary ymp home and project; no provider or model was invoked.

## Cause

`frame::render_modal` sized a floating surface from the rows left after `skip(scroll)`, and the
Inspect and Preview keys let `scroll` reach the last line (`total - 1`). Scrolling towards the end
therefore shrank the surface, and it moved because its vertical position is derived from its
height. `PageUp`/`PageDown` stepped by the height of the page underneath, not by the rows the
popup shows.

## Correction

- `render_modal` is as tall as `min(body lines, rows the area allows)` and clamps the scroll to
  `frame::modal_last_scroll`, the point where the last line reaches the bottom row. Callers that
  pass `scroll: 0` (theme chooser, palette, prompt, confirmation) get the same height as before,
  so their selection, cursor and footer placement are unchanged.
- The draw records `Viewport::modal_rows`. Inspect and Preview share `popup_scroll`: `Down`/`j`,
  `End` and `PageDown` stop at the last full page, `PageUp`/`PageDown` move by the popup's rows,
  and a scroll left beyond the end by a taller window is clamped before the key applies, so the
  first `Up` moves the view. Exit keys and `Ctrl+C` handling are unchanged.
- A shorter window keeps the top line (the rest stays reachable with `Down`); a taller one never
  shows blank rows under the last line. Text, colours and labels are unchanged.
- [interface.md](../../guides/interface.md) states the paging and end behaviour.

## Failing before, passing after

| File | What it shows |
| --- | --- |
| [before-popup-scroll.txt](before-popup-scroll.txt) | Production code unchanged from 17fb99b: both tests fail at their first size assertion. Preview `height 38 → 37` on the first `Down` near the end; Inspect `height 28 → 5` after `End`. |
| [after-popup-scroll.txt](after-popup-scroll.txt) | With the correction: both tests pass. |

The before run used the first draft of the tests. Before the after run three expectations in parts
the before run never reached were corrected: paging continuity covers two pages (the third stops
at the end), a shorter window keeps the top line rather than the last line, and the Unicode row
check reads a wide glyph cell by cell (`表 格`).

Tests (`ymp-tui/src/tests.rs`):

- `a_file_preview_keeps_its_size_while_it_scrolls_to_its_last_line`: 120-line file mixing ASCII
  and Cyrillic/CJK rows at 120×40; 140 `Down` presses with an unchanged rect on every frame; last
  row reached with no blank rows; first `Up` after overshooting moves; `PageDown` continuity and
  end stop; `PageUp` from the end; resize to 30, 60 and 40 rows; `Up` after growing; no actions,
  unchanged configuration and project tree; `Ctrl+C` twice still quits.
- `an_inspect_surface_keeps_its_size_for_long_short_and_empty_records`: ASCII markers, 100×30,
  bodies of 60, exactly the visible rows, one more, 3, 1 and 0 lines; `End`, `Up`, `Down`, `k`,
  `j`, `PageUp`, `PageDown`, `Home` each keep the rect and show the expected last line; `Esc`
  closes.

## Required workspace checks

Run with `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0` and the target
directory inside the worktree, once on the final code.

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | [fmt.txt](fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [clippy.txt](clippy.txt) |
| `cargo test --workspace` | exit 0; 556 passed, 0 failed, 2 ignored | [workspace-tests.txt](workspace-tests.txt) |

## Not covered here

- No real-terminal capture; the parent's terminal checker covers that separately.
- Palette, theme chooser, prompt and confirmation were audited in source (they always pass
  `scroll: 0`) and exercised by the existing workspace tests, not by new popup-specific tests.
