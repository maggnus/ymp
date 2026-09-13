# YMP-145 evidence: full-row popup selection and scrolling geometry

Every scenario uses temporary ymp homes and projects with the mock provider or scripted Git replies;
no provider or model was invoked.

## Selection gap

### Cause

`frame::render_modal` painted body lines as given. Only some list builders produced lines that
fill the content width in the selection style:

- The Git worktree and branch choosers built their rows with `text::row`, styling the label and
  the detail but not the unstyled gap `text::row` inserts between them, nor anything after a row
  without a detail. That is the owner's screenshot: `› main` and `checked out here` highlighted
  with the surface background between them.
- The theme chooser styled only the marker and name; the kind label (muted) and the gap before
  the colour chips kept the surface background.
- The command palette and the inline completion list were correct only because `command_line`
  appended its own styled fill span.

### Correction (shared component)

- `frame::paint_list(frame, area, lines, chosen, theme)` paints the chosen row across the whole
  width of the content rect with `theme.selected()` and then draws the lines over it. Spans that
  set their own background (the theme chooser's colour chips) keep it. The padding cell on each
  side and the border are outside that rect and are unchanged.
- `ModalSpec::selected` names the chosen body line. `render_modal` maps it through the clamped
  scroll and paints with `paint_list`. The palette, theme chooser and Git choosers pass their
  chosen line; Inspect, Preview, prompts and confirmations pass `None`.
- The inline completion list uses `paint_list` too, and `command_line` no longer appends its own
  fill span. On the chosen theme row the kind label takes the selection style so it stays legible
  on the accent.
- Ordinary `text::row` callers (header, welcome facts, pages) are unchanged.

## Shrinking report

Not reproduced on current source or on the installed 0.4.6; no height change was made.

Audit of every `render_modal` caller in `ui.rs`:

| Surface | Body | Scroll | Finding |
| --- | --- | --- | --- |
| Inspect (chat entry, page row, file details) | complete, built once when opened | `popup_scroll`, clamped by `render_modal` | stable |
| Preview (file, Git diff) | complete, built once when opened | same | stable |
| Command palette | windowed list plus `more_line` whenever it overflows | 0 | constant length for a query |
| Theme chooser | windowed list, summary padded to the longest, `more_line` | 0 | constant length |
| Git worktree/branch chooser | windowed list plus `more_line` | 0 | constant length |
| Prompt | follows the typed text (content change, not selection) | 0 | not a scroll path |
| Confirmation | static question | 0 | stable |
| Inline completion list (own block) | at most 6 rows, fixed per match count | n/a | stable |

No overlay body is replaced while it is open except by its own key handler, which keeps the body
and changes only `scroll` or `selected`.

## Failing before, passing after

| File | What it shows |
| --- | --- |
| [before.txt](before.txt) | Production code unchanged from 0b95e4a, new and extended tests. Three tests fail, every failure is a selection background (`N of the M cells of "› main" keep another background`), e.g. 50 of 72 cells for the screenshot row at 120×32; Git choosers and the theme chooser fail in every theme, marker set and size; the palette and completion list pass. No geometry failure (`became`), no lost selection and no padding failure at any size, and the small-terminal Inspect test passes. |
| [after.txt](after.txt) | With the correction: the four tests plus the related palette, theme, Git and completion tests, 33 passed. Run before `cargo fmt --all`, which only reformatted. |

Tests (`ymp-tui/src/tests.rs`):

- `chosen_row_failures` reads actual Ratatui cells of the chosen row: every cell between the
  padding cells must carry the selection background (the cell a wide glyph covers counts as the
  glyph's), the padding cells must keep the surface background, and only blank trailing colour
  chips of a theme row may differ. `choice_list_failures` now runs it on every frame of its walk,
  in addition to the size and visible-selection checks.
- `a_chosen_popup_row_is_highlighted_from_its_marker_through_its_gaps_to_its_last_cell`: the
  screenshot (one branch `main`, checked out here), a mixed branch list (current, Cyrillic name
  without detail, branch checked out in `/elsewhere/表格`), a worktree list (branch, detached head,
  no commits), the palette and the inline completion list; Ember/Unicode, a light theme/ASCII and
  Slate/Unicode; 120×32 and 60×16; Down, Up, End, Home, j, k.
- `choosers_keep_their_size_in_small_terminals_with_unicode_and_boundary_lengths`: branch choosers
  of exactly the visible rows, one more, and 40 entries with Cyrillic names, and 30 Cyrillic
  worktrees, at 48×12 and 40×10, dark/Unicode and light/ASCII, first to last and back to the
  middle, Home and End.
- `a_read_only_surface_keeps_its_size_in_small_terminals_and_other_themes`: a 50-line Inspect body
  at 40×6, 48×8 and 60×12, light/ASCII and dark/Unicode, Down past the end, Up, PageUp, PageDown,
  Home, End; unchanged rect and body rows on every key, last line reachable.
- `a_choice_list_longer_than_its_surface_keeps_its_size_from_the_first_entry_to_the_last` also
  checks the chosen row's cells now.

## Real terminal (tmux)

Reports are in [terminal/](terminal/); paths are replaced by `<worktree>` and `<output>`.

| Run | Binary | Result |
| --- | --- | --- |
| `check-popup-scroll.py` | installed 0.4.6 | passed; all 9 cases (long/resized Inspect, long/short Preview, file Inspect, palette, theme chooser, narrow palette and theme chooser) keep one rect |
| `check-git-view.py --selection-baseline` (new flag) | installed 0.4.6 | reproduced: the chosen branch row `› feature … checked out here` has 47 of 72 content cells on the surface background |
| `check-git-view.py` | this branch | passed, including `chosen-branch-row-highlighted-across-content`: 72 of 72 cells on the accent, padding distinct |
| `check-git-view.py --ascii` | this branch | passed; `NO_COLOR` hides backgrounds, so the selection cells are not measured in this mode |
| `check-popup-scroll.py` and `--ascii` | this branch | passed; all 9 cases stable |

`check-git-view.py` gained `chosen_row`, which reads the ANSI capture of the branch chooser with the
existing `colored_characters` parser, and the `--selection-baseline` flag.

## Required workspace checks

Run once, sequentially, on the final code with `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_INCREMENTAL=0` and the target directory inside the worktree.

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | [fmt.txt](fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [clippy.txt](clippy.txt) |
| `cargo test --workspace` | exit 0; 579 passed, 0 failed, 2 ignored | [workspace-tests.txt](workspace-tests.txt) |

## Not covered

- The specific popup and build behind the renewed shrinking report remain unidentified: no path
  shrank in current source, in 0.4.6 in a real terminal, or in the new small-terminal, boundary,
  Unicode and theme cases. A screenshot or recording of the shrinking popup with `ymp --version`
  is needed to go further.
- Monochrome (`NO_COLOR`) selection relies on the marker and bold text; its cells are not measured.
- PageUp/PageDown are not keys of the palette, theme chooser or Git choosers, so they were covered
  only for Inspect and Preview.
- The theme chooser and palette were checked for selection in the unit tests only; the real-terminal
  selection check covers the branch chooser.
- No test of the palette at terminals under 10 rows, where its search line and rule take most of
  the body.
