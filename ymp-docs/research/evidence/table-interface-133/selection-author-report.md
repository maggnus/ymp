# YMP-133 selection fixes: corrective pass report

- **Checkout:** `/private/tmp/ymp132-agent-attribution`
- **Branch:** `fix/ymp133-selection`, started from `a2f2707125ad389fcfa2ef2a935f8fa749a489b5`. That base includes the tables and the modal R1/O1 fixes, and both are kept.
- **Preserved:** `feat/ymp133-tables` and `feat/ymp133-tables-integrated` are untouched.
- **Not edited:** approved intent, task register, version, release documents and README.
- **Review addressed:** `/tmp/ymp133-controls-review-result.md`, required R1 and R2 and polish P1 to P6.

## Commit

| Commit | Scope |
|---|---|
| `5dd98cc` | Keep the selected record on every rebuild of a page, in its own table. Includes P1 to P6. Accepted by independent review (production source identical to snapshot `5e2506f`) and merged by the parent. |
| `58fe689` | Input-boundary follow-ups F1 to F3 from `/tmp/ymp133-selection-independent-review.md`. |

## One anchor contract (R1, R2, P1)

- **`table::Anchor`** records three things for the selected row:
  - the title of the table that holds it (the nearest heading above);
  - the row key;
  - its index.
- **`table::anchor(items, selected)`** captures it. It returns nothing when the selection is not on a row.
- **`table::locate(items, anchor)`** finds the selection on a rebuilt page in this order:
  1. the key in the same table;
  2. the key anywhere on the page, when the table no longer holds it (for example, an assignment that moved from "Running now" to "Recorded earlier");
  3. when the record is gone, the nearest row at or after the old index, or else the last row (P1).
- **`App::remember_selection`** stores `(view, Anchor)` from the page last built, unless an anchor is already pending. Two paths call it:
  - `App::page` calls it whenever the cache is stale, whether because the revision changed (a live event through `changed()`), the width changed, or the cache was dropped;
  - `App::invalidate_page` calls it before dropping the cache.
- **Every former `page_cache = None` site now uses `invalidate_page`:**
  - `refresh_records`;
  - `refresh_pool`, which covers `edit_config` and Space/`t` on Agents;
  - `apply_theme`;
  - retiring a memory entry, from the confirmation and from `/memory forget`;
  - a memory search;
  - `rearrange`, which covers a sort or filter keystroke.
- **`set_view`** still drops the anchor: a new page starts at its first row.
- **Restore.** `App::page` restores the anchor only when its view is the open view.
- **Usage page.** The `View::Usage` special case in `usage()`, together with `page_anchor` and `restore_page_anchor`, is removed. The statistics page is kept by the same contract.
- **`sort_by_key`** now reads `page_selected` after it builds the page, so a sort key pressed after a live event uses the table of the record the selection moved with.

## Polish

- **P2:** Memory adds its table heading only when there are entries.
- **P3:** `Item::is_record` is a row with more than one cell. The `[N]` count and `table.unfiltered` count records only, not explanatory notes.
- **P4:** leaving the page with Tab or Shift+Tab ends filter typing and keeps the text. While a filter is typed, Ctrl+J is not a composer newline.
- **P5:** `views::value_cell` sorts a value that begins with a figure by that figure, so `300 s` sorts as 300 and `4` before `12`. It is used for the Limits `VALUE` cells (both the next-run and the captured ones), the roster member count and the roster rules size. Any other value sorts as text, after all figures.
- **P6:** a squeezed sorted title keeps its direction arrow and cuts the name first (`TOK… ↑`). At 2 cells or fewer only the arrow is shown.

## Regressions

### Integration tests (`tests.rs`)

- **`selection_stays_on_an_edited_profile_that_its_sort_moves`**
  1. On Agents, it sorts by `ENABLED` until a disabled profile leads (`glm` in the default configuration) and selects it.
  2. It filters to that profile, keeps the filter and clears it with Esc, as the terminal check does.
  3. Space enables the profile, which moves it. The test asserts that it moved.
  4. The selection must still be that profile.
  5. A second Space must restore every profile's enabled state exactly as before.
- **`selection_stays_in_its_own_table_when_a_key_repeats_on_the_page`**
  1. On Team, it selects the pool row of a member whose id is also in the Members table.
  2. `M` twice must keep the selection in "Available on this machine", on the same key, with the pool table descending.
  3. After a third `M`, it types a filter one key at a time, and the selection must stay in that table.
- **`selection_follows_a_live_update_that_moves_its_record_before_the_next_frame`**
  1. On Tasks, with a run active and two tasks sorted by `STATE`, it selects the running task.
  2. A `UiEvent::Task` marks the task accepted, which moves it (asserted).
  3. `selected_item` is read before any frame is drawn, and it must still be that task.
- **`selection_table_counts_records_sorts_values_as_figures_and_filter_typing_ends_with_focus`**
  - Limits with no session must show `Limits[4]` (P3).
  - The next-run table sorted by `VALUE` must read turns 4, parallel 12, attempts 30, timeout 300 s (P5).
  - Ctrl+J while typing a filter changes neither the filter nor the composer, and Tab ends typing (P4).

### Unit tests (`table.rs`)

- **`an_anchor_finds_its_record_in_its_own_table_then_elsewhere_then_nearby`** covers:
  - the same key in two tables;
  - a table that is gone;
  - a record that is gone, where the selection goes to the nearest row or the last row;
  - a heading, which is not a selection;
  - a page with no rows.
- **`a_squeezed_sorted_title_keeps_its_direction`** (P6).

### Shown failing before the fix

- **How the swap was done** (with bash, one step at a time):
  - `state.rs`, `table.rs`, `views.rs` and `ui.rs` were replaced by their `a2f2707` versions and touched, while the new `tests.rs` stayed in place;
  - `cargo test -p ymp-tui --lib -- selection_` was run;
  - the fixed files were copied back, all five files were touched, and `shasum -c` passed;
  - only then was anything rebuilt.
- **Log:** `/tmp/ymp133-selection-before-fix.log`.

| Test | Result before the fix |
|---|---|
| Edited sorted Agents | FAILED at the main assertion: the selection was `codex`, expected `glm` |
| Team pool | FAILED right after the first `M`: selection in `("Members of the next run", "codex")`, expected `("Available on this machine", "codex")` |
| Live update | FAILED: another task's key was selected |
| Count, figures and typing | FAILED: `Limits[6]` |

- **Changes after the demonstration:**
  - The only later change to tests was the filter-then-clear step added to the Agents test. The main assertion it reaches is unchanged.
  - One source change came after the first attempt: `sort_by_key` now reads the selection after building the page.

### Test corrections during the pass

- **A precondition that could never hold.** The first draft of the Agents test added a disabled profile and assumed it would lead the sort. The default configuration already has a disabled `glm` profile, which led instead. Its first "failure before the fix" was that precondition, not the defect. The test now selects whichever disabled profile leads, and the demonstration above was repeated.
- **A stale selection read.** The `selected_place` helper read `page_selected` before building the page, which reported a stale index. It now builds first.
- **A Clippy error on the first full run.** Strict Clippy rejected `rows().last()` in `locate` (`double_ended_iterator_last`). It became `rows().next_back()`, with no change in behaviour, and fmt, Clippy and the workspace tests were all run again on that final source.
- **An invalid first swap attempt.** It used an unquoted variable in zsh and did not swap any files. It left an empty stray file, which was removed. Its result is not used.

## Input-boundary follow-ups (`58fe689`)

- **F1: a paste or palette command while a filter is typed.**
  - With no popup open and a page filter being typed, `App::paste` now appends the text to the filter on one line (newlines become spaces) and rebuilds the page. Neither the composer nor the focus changes.
  - A paste that does go to the composer ends filter typing.
  - The palette's Enter on a command that needs an argument ends filter typing before it moves the keys to the composer.
  - Popup editors and the palette field receive pastes as before.
- **F2: Home after an event and before the next frame.** Home now builds the page first, which restores any remembered record, and only then selects the first row. A pending anchor can no longer take the selection back. End and the other selection keys are unchanged.
- **F3: captured limit values recorded as unknown.**
  - `value_cell` gives `unknown` and `unknown …` (for example `unknown over 3 turn(s)`) an unknown sort key, so they stay last in both directions.
  - `none` stays a text value, because it may mean that no bound was set.
  - Displayed text is unchanged.
- **Regressions:**
  - `a_paste_while_a_filter_is_typed_stays_in_the_filter_and_the_palette_ends_typing`:
    1. On Help, a paste during filter typing extends the filter to `theme line`, leaves the composer empty, and keeps page focus and typing.
    2. After Enter keeps the filter, a paste goes to the composer.
    3. In a new window, a filter is typed, then Ctrl+P, `agent`, Down and Enter must leave `/agent ` in the composer, move focus to it and end typing.
    4. A following paste completes `/agent codex`.
  - `home_wins_over_a_record_remembered_from_a_live_update`:
    1. On Tasks, sorted by `STATE` descending, a ready task that is not first is selected.
    2. A `UiEvent::Task` blocks it with no frame in between, and then Home is pressed.
    3. The selection must be the first current row (asserted not to be that task), and End must select the last row.
  - `views::tests::a_value_recorded_as_unknown_sorts_as_unknown_and_none_stays_a_value` (unit): `300 s` sorts as a figure, `unknown` and `unknown over 3 turn(s)` as unknown, `none` as text, and the displayed text is kept.
- **Shown failing before the fix.**
  - **How the swap was done** (with bash):
    - `state.rs` and `views.rs` were replaced by their `5dd98cc` versions and touched, with the new `tests.rs` in place;
    - the two integration tests were run;
    - the fixed files were copied back, all three were touched, and `shasum -c` passed before any further build.
  - **Log:** `/tmp/ymp133-boundary-before-fix.log`.
  - **Results:**
    - The paste test FAILED at its first assertion: the filter stayed `the`, because the paste went to the composer. Its palette part was therefore not reached before the fix.
    - The Home test FAILED: the remembered task was selected instead of the first row.

## Checks on the final source

All runs used `CARGO_TARGET_DIR=/tmp/ymp132-test-target`, debug=0, incremental=0 and jobs=2.

| Check | Result | Log |
|---|---|---|
| `cargo fmt --all --check` | passed | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | `/tmp/ymp133-boundary-clippy.log` |
| `cargo test --workspace --no-fail-fast` | exit 0; 34 test binaries, 498 passed, 0 failed, 2 ignored; includes the 3 tests added in `58fe689`, the 6 added in `5dd98cc` and the CLI `message_labels` test | `/tmp/ymp133-boundary-workspace.log` |

These are the checks on `58fe689`, run after the source was restored and stable. The earlier run on `5dd98cc` (Clippy exit 0; 495 passed, 0 failed, 2 ignored; logs `/tmp/ymp133-selection-clippy.log` and `/tmp/ymp133-selection-workspace.log`) is superseded.

## Limits

- No terminal run. The parent's extended checker (Agents: sort `ENABLED`, filter, clear, Space twice; Team pool cycling) was not run here.
- No provider inference, and no real user metadata was read or changed. The Agents test saves its configuration only into the fixture's temporary home.
- **Key anywhere on the page.** When the anchored table no longer holds the key, `locate` takes the key anywhere on the page, as the review suggested. If a filter removes a pool row while the member row with the same id stays, the selection moves to the member row rather than to a nearby pool row.
- **P2 has no dedicated test.** A Memory page with no entries was already cleared. The heading change only prevents column titles over nothing if that clearing ever changes.
- **P5 uses a leading-figure rule.** A value such as `none` or `stop admitting` sorts as text after all figures, and a value recorded as `unknown` sorts last in both directions (F3).
- **F4 was not changed**, because it was informational in the review: on a page with notes only, a filter that hides the notes shows the page's own empty text.
- **No terminal run for `58fe689`.** The paste and palette boundaries were checked only with fixture key and paste events.
