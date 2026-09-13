# YMP-133 tables, integrated: implementation report

- **Checkout:** `/private/tmp/ymp132-agent-attribution`
- **Branch:** `feat/ymp133-tables-integrated`, started from `b01ffe7`.
- **Preserved:** the earlier `feat/ymp133-tables` branch and its saved patch were not touched.
- **Not edited:** the task register, version, release documents, README, approved intent and backend crates.

## Commits

| Commit | Scope |
|---|---|
| `44b6a08` | **F3/F4.** A historical session, decision or board row is named only by its own linked assignment and invocation, or by the evidence captured for that record. Otherwise it shows `unknown model`. No consensus from later calls and no timestamp matching. |
| `6d12bf4` | **YMP-135** (separate, presentation only). A missing effort or a binary switch (`on`/`off`/`enabled`/`disabled`/`true`/`false`) shows the model alone. A reported `none` stays `MODEL none`, and graded levels stay `MODEL EFFORT`. Details keep the exact reported value. The change goes through the shared label helper. |
| `49b3c79` | **Strict references.** An explicit invocation ID that is not among the assignment's calls gives `unknown model`, with no fallback. With an assignment-only reference, the invocation is used only when it is the only call; otherwise the captured assignment evidence is used and no effort is borrowed. |
| `d0b674a` | **Sidebar tables:** TOKENS, TEAM and TASKS, plus the first table primitives. NAVIGATE is not restored. |
| `63e4780` | **Page tables.** Every non-popup page list becomes a table, with filter, sort, `d` Inspect, sticky column titles and no detail pane. Memory search moves to `s`. Includes the required foundation-review fixes T1/T2 and optional O1/O2. |

## Foundation review fixes (`/tmp/ymp133-table-foundation-review-result.md`)

### T1: the TEAM table cut model labels

- **Fix:**
  - `TEAM_COLUMNS` is now `AGENT`, `STATE*`.
  - The marker is a span inside the agent cell.
  - The activity column is the flexible one, with the short title `STATE`, so it gives way before the model and effort.
  - TASKS also carries its marker inside the task cell.
- **Test:** `a_narrow_sidebar_cuts_what_a_member_is_doing_before_its_model_and_effort`.
  - At inner widths 24 and 28, with the member working and summarising, `claude-opus-5 max` and `claude-opus-5` stay whole.
  - Every TEAM line fits.
  - At 28, `claude-opus-5 max  working` is whole.

### T2: `table::widths` could exceed its width

- **Fix:** the widths plus gaps now never exceed `width`. The order in which space is given up:
  1. hide columns by rank while the flexible column is at its floor;
  2. the flexible column, down to 1 cell;
  3. the widest left-aligned text column, down to 1 cell;
  4. the widest figure, down to 1 cell;
  5. whole columns from the right.
- **Rendering:** page rows compute widths for the room after the 2-cell selection marker and are padded to exactly the page width, so no cell is clipped at the edge. Heading titles are cut to the body width.
- **Test:** `a_table_never_takes_more_than_its_width` covers:
  - every width from 0 to natural + 2, with a sorted column: the total is at most the width, and the painted line is exactly the total;
  - a Providers-shaped row with a 49-cell non-flexible path at 56 cells: it fits and the figure stays whole;
  - figures are cut only after every text column is at 1 cell.
- **Changed assertion:** the unit test that pinned the old priority now expects `a-ve…  working` at 14 cells. The state word is kept and the name gives way.

### Optional O1 and O2

- **O1:** a table section given fewer than `fixed + 2` rows drops its column-title line and shows a record plus the counter, instead of wasting a row.
- **O2:** a column with neither a title nor any cell content is hidden, so it takes no gap.

## Implementation choices that differ from the plan

1. **Primitives in the sidebar commit.** The table primitives were first committed with the sidebar commit, not as a standalone no-visible-change commit, so strict Clippy passes at every commit. The final table module was then rewritten in the page commit.
2. **T1/T2 not split out.** They could not be a separate commit without interactive hunk staging, because `table.rs` in the page commit also depends on the new `views::Item`. They are described in the `63e4780` message.
3. **`unfiltered` count.** It is stored on `table::Controls` and shown as `[N]` only when the page has rows.
4. **Sessions page team.** It lists profile IDs. Recorded models are listed only for the loaded session, which has its own trace; the page does not read every session's trace.
5. **YMP-135 switch list.** It also includes `true` and `false`.
6. **Column titles.**
   - Marker columns are untitled.
   - The Team and Agents reading column is titled `READING`.
   - The sidebar activity column is titled `STATE` (T1).
7. **Flexible column on Providers.** It is `EXECUTABLE`, not `COMMAND`, because the path is the long cell. The plan's choice would cut the command before a 47-cell path.
8. **Sort keys.**
   - A key is the first free letter of a column title.
   - `R` is reserved on Agents and Providers, where it asks the installations.
   - Shift+letter sorts the table holding the selection, or the first table.
   - A letter no column answers falls through to the page's own keys.
9. **Notes.** Page-explaining rows (about, recovery, how checks run, nothing captured) are notes: one cell spanning the table. They stay above sorted rows, and the filter treats them as rows.
10. **Inspect title.** The popup is titled by the row's flexible cell, and by the row key for notes or an empty cell.
11. **Filter typing.**
    - Printable keys go into the filter, and Up/Down still move the selection.
    - `Enter` keeps the filter.
    - `Esc` clears it before the page closes.
    - Opening another page clears it; sorts are kept per page and table for the window's lifetime.
    - With the composer focused, `/` is composer text.
12. **Helper rename not done.** The test helpers `left_of_key`, `right_of_key` and `row_right` were not renamed. They now return the row's cells joined with ` · ` (`Item::text`, test-only). Assertions that pinned a single column use the new `cell_of_key(app, width, key, COLUMN)`.
13. **Right-column budget test.** `no_row_puts_more_in_its_right_column_than_the_narrowest_column_holds` now checks the last cell of record rows, not notes.

## Checks

All runs used `CARGO_TARGET_DIR=/tmp/ymp132-test-target`, debug=0, incremental=0 and jobs=2.

| Check | Result | Log |
|---|---|---|
| `cargo fmt --all --check` | passed | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | `/tmp/ymp133-tables-clippy.log` |
| `cargo test -p ymp-tui` | 194 passed, 0 failed, 1 ignored (pre-existing) | `/tmp/ymp133-tables-tui.log` |
| `cargo test --workspace --no-fail-fast` | exit 0; 34 test binaries, 486 passed, 0 failed, 2 ignored; includes the CLI `message_labels` test | `/tmp/ymp133-tables-workspace.log` |

### New or rewritten tests in `63e4780`

| Test | What it covers |
|---|---|
| `table_filter_keeps_matching_rows_inverts_on_bang_and_clears_before_the_page_closes` | Case-insensitive filter; title `Help</THEME>[N]`; `Enter` keeps the filter and `j` is not typed; `Esc` clears the filter and a second `Esc` closes the page; `!` inverts; the no-match message; the filter resets on a page change; `/` goes to the focused composer. |
| `table_sort_orders_figures_as_numbers_keeps_unknown_last_and_the_selection` | Tokens by number (89 before 12345); unknown last in both directions; the arrow on the sorted column; the other table is unaffected; the selection is kept; the third press restores page order. |
| `table_keys_inspect_without_acting_and_memory_search_moves_to_s` | `d` on Sessions opens Inspect titled by the session title, without loading it; Memory `s` opens the search prompt; `/` filters. |
| `table_sessions_hide_lesser_columns_on_a_narrow_terminal` | At 120 columns every column shows; at 40, TURNS and CREATED are hidden and STATUS stays. |
| `table_column_titles_stay_on_screen_while_the_rows_scroll` | Help at 80x24 after `End`. |
| `a_page_has_no_detail_pane_and_its_record_opens_whole_at_every_supported_size` | Replaces the detail-pane test. |
| Statistics, change view and record-page statement tests | Now read the record through `d` and Inspect. |

The earlier sidebar test (`the_sidebar_lists_tokens_team_and_open_tasks_as_aligned_tables`) and the F3/F4, strict-reference and YMP-135 tests are kept; the F3/F4 and strict-reference tests now read the `ACTOR` cell. The exit, Ctrl+C and provenance regression tests are unchanged and pass.

### Negative controls

Each control patched the source, ran the targeted tests, and restored the source; `shasum -c` then passed. Every control failed its tests, not the build. Log: `/tmp/ymp133-tables-controls.log`.

The restore gave the files an older modification time than the last patched build. The first workspace run after the controls therefore reused a stale `ymp-tui` binary that still dropped the selection anchor, and `table_sort_orders…` failed there. The sources matched their checksums and `HEAD`. The restored files were then touched, and the workspace suite was run again from a rebuild with `--no-fail-fast`; that second run is the result recorded in Checks.

| Control | Tests that failed |
|---|---|
| Figures sorted as text | `sorting_cycles…`, `table_sort_orders…` |
| `!` ignored | `a_filter_keeps_matching…`, `table_filter_keeps…` |
| Sticky header disabled | `table_column_titles_stay…` |
| Last-resort give-way steps removed | `a_table_never_takes_more_than_its_width` |
| Old TEAM shape (flexible AGENT, ACTIVITY title) | `a_narrow_sidebar_cuts_what…` |
| `Esc` closes the page before the filter | `table_filter_keeps…` |
| Selection anchor dropped on sort | `table_sort_orders…` |

Earlier commits had their own controls, recorded in their work: the F3/F4 sessions and proposal tests, the strict-reference test, YMP-135 (5 tests failed) and the sidebar alignment and fixed-header tests.

## Limits

- No physical PTY run and no packaged binary; the parent does both. The tables were checked only through ratatui `TestBackend` renders.
- No provider inference at any effort. Label tests use stored fixtures and the in-process mock.
- **R1/O1 of the YMP-134 review** (modal margin, hidden palette row) were left to reviewer abc24a9b and are not in this branch.
- ASCII markers (`^`/`v`) were not rendered in a test; only the Unicode arrows were.
- Page column widths and hide ranks are fixed per page. The Agents, Providers and Reputation tables have no hide ranks and rely on the T2 give-way order at 60 columns.
