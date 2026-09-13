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

## Choice lists longer than their surface

A later source audit found the same shrinking in the lists that window their entries. The command
palette and the Git worktree and branch choosers kept a row for `N more` only while entries
remained below, so selecting the last entry drew the surface one row shorter. The theme chooser
had the same condition, and its height also followed the number of lines the chosen theme's
summary wraps to.

- `more_line` in `ui.rs` gives every list that does not fit the same row under it: the count of
  entries below, or a blank row once the last is shown. The palette, the Git chooser and the theme
  chooser push it whenever the list overflows, not only while entries remain below.
- The theme chooser gives the chosen summary the rows of the longest summary in the catalogue at
  the current width, padding the difference after the list.
- How many entries are listed, where the selection sits, the wording, the colours and the keys are
  unchanged.

`a_choice_list_longer_than_its_surface_keeps_its_size_from_the_first_entry_to_the_last` opens
the palette (every command), the theme chooser (every theme) and the branch chooser (40 branches)
at 120×36 and 60×16. It moves each from the first entry down to the last and back up to the middle,
then presses `Home` and `End` where the list takes them. After every key it requires the surface
rect to equal the first drawing and the selected entry to be painted inside it.

| File | What it shows |
| --- | --- |
| [choice-lists-before.txt](choice-lists-before.txt) | Lists unchanged: the palette at both sizes and the branch chooser at both sizes lose a row on the last entry, and the theme chooser at 120×36 grows a row on themes whose summary wraps to two lines. |
| [choice-lists-after.txt](choice-lists-after.txt) | With the correction: the new test and the existing palette, theme and Git chooser tests pass. |

Checks after the correction, run once with `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_INCREMENTAL=0` and a target directory inside the worktree:

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | [choice-lists-fmt.txt](choice-lists-fmt.txt) |
| Related tests (the new test, palette, theme chooser, Git choosers) | exit 0; 11 passed | [choice-lists-after.txt](choice-lists-after.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [choice-lists-clippy.txt](choice-lists-clippy.txt) |
| `cargo test --workspace` | exit 0; 573 passed, 0 failed, 2 ignored | [choice-lists-workspace-tests.txt](choice-lists-workspace-tests.txt) |

Not covered: no real-terminal run; the worktree chooser shares the branch chooser's code path and
is not walked separately; a theme chooser in a window too short for its whole body is clipped at
the tallest surface, as before.
