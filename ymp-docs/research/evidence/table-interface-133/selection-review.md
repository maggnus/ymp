# YMP-133 selection corrections: independent review of the corrective delta

**Verdict: accepted.** The delta closes R1, R2 and P1–P6 from `/tmp/ymp133-controls-review-result.md`. I found no required fix. Three optional follow-ups are listed below; none of them blocks acceptance.

## Scope and method

- **Snapshot:** `/tmp/ymp133-selection-review` at `5e2506f1671dae137cc2b7858c9cb984773071f7`, "Snapshot record-selection corrections for independent review". Its parent is `a2f2707`, and the working tree is clean.
- **Delta reviewed:** `a2f2707..5e2506f`, touching:
  - `state.rs` (+/−98);
  - `table.rs` (+128, including 2 unit tests);
  - `views.rs` (+30);
  - `ui.rs` (+5);
  - `tests.rs` (+230), read only for what it asserts.
- **Earlier approvals:** the table and modal design approved earlier was not reopened.
- **Author's report:** `/tmp/ymp133-selection-fixes-report.md` was used only to learn the intended contract. Every conclusion below comes from the source.
- **What was run:** nothing. There was no build, no test run, no terminal run and no inference; conclusions are worked out from the source.

## Selection across rebuilds (R1, R2, P1): closed

- **Capture on every rebuild of the same view.**
  - `App::page` calls `remember_selection` whenever the cache is stale, for any cause: a new revision from `changed()`, a different width, or a dropped cache.
  - `remember_selection` reads `page_selected` against the page the reader last saw.
  - Every former `page_cache = None` site outside `set_view` now goes through `invalidate_page`, which captures first:
    - `refresh_records`;
    - `refresh_pool`, which covers `edit_config`, Space/`t` on Agents, `r` and `adopt_config`;
    - `apply_theme`;
    - both memory-retire paths;
    - memory search;
    - `rearrange`.
  - `usage()` no longer needs its own case: it bumps the revision, and the next build captures from the cached page.
- **The anchor does not leak across views.**
  - `table_anchor` is `(View, Anchor)`, and `App::page` restores it only when the stored view equals `self.view`.
  - `set_view` clears the anchor and drops the cache directly. The next build therefore finds no cache and no anchor and starts at the first row.
  - If an anchor from another view is still pending, it is taken and discarded on that build.
  - `Route::FollowUp`/`NewRun` set `View::Chat` directly. That leaves at most a pending anchor for the old view, which `set_view` clears on the next page change.
- **Live changes cannot redirect actions.** Every read of the selection builds first:
  - `page_action` via `selected_item`;
  - `inspect_selected_row` via `selected_item_at`;
  - `move_page_selection`;
  - `sort_by_key`, which now also builds before reading `page_selected`.

  An event that arrives after a frame and before a key is resolved against the page the reader saw, and the selection moves with its record.
- **Duplicate ids stay in their table.** `table::locate` looks for the anchor in this order:
  1. the key inside the table whose title matches the anchor;
  2. the key anywhere on the page;
  3. the first row at or after the old index;
  4. the last row.

  How this works on the pages:
  - On Team, a pool row whose id is also a member stays in "Available on this machine" as long as that table holds the id. That covers sorting and any filter that keeps it.
  - Titles are unique within each page, as verified in the previous review, so matching by title is unambiguous.
- **The fallbacks are sensible.**
  - A record that moves to another table is followed there by key: "Running now" to "Recorded earlier", "Members of the next run" to "Members of this session", or into "Recorded outside the captured team".
  - A removed record leaves the selection near its old position rather than at the top.
  - An anchor on a heading is `None`, so `select_a_row` applies as before.
  - A page with no rows keeps `page_selected`, and nothing is selectable.
- **Tests (read, not run).**
  - The four integration tests and the unit test `an_anchor_finds_its_record_in_its_own_table_then_elsewhere_then_nearby` assert:
    - the edited profile on sorted Agents;
    - the Team pool cycle and filter typing;
    - a task that moves before the next frame;
    - duplicate keys, a table that is gone, a record that is gone, and a heading.
  - The Agents path matches the parent's physical reproduction in `ymp-docs/research/evidence/table-interface-133/selection-negative`.

## Polish P2–P6: closed

- **P2:** Memory adds its heading only when there are entries (`views.rs`, `memory`).
- **P3:** `Item::is_record` (a row with more than one cell) drives both `table.unfiltered` and the `[N]` count. No record row has a single cell.
- **P4:**
  - `cycle_focus` ends filter typing and keeps the text.
  - Ctrl+J is not a composer newline while typing. In `main_key` the typing branch ignores it, because `plain` is false.
- **P5:** `value_cell` orders a value by its leading digits: `4`, `12`, `30`, `300 s`, `N of M`, `N + M`, `N member(s) · …`, `N+`. The captured values are formatted as plain integers, never with separators or `k`/`M` suffixes, so the leading figure is the real magnitude.
- **P6:** a squeezed sorted title cuts its name before its arrow.
  - The arrow is 1 cell with both marker sets (`↑`/`↓`, `^`/`v`).
  - At 1–2 cells only the arrow is drawn.
  - `fit` still bounds the cell to its column width.

## Optional follow-ups (not required)

- **F1 (Low): typing can outlive a focus change that does not go through `cycle_focus`.**
  - Two paths move focus to the composer without clearing `table.typing`:
    - a paste with no overlay open (`state.rs:408-409`);
    - the palette's Enter on a command that takes an argument (`state.rs:1883`).
  - If either happens while a filter is being typed, the filter line and its hints stay on the page, and Ctrl+J no longer inserts a newline in the composer until Tab or Esc.
  - Fix direction: gate Ctrl+J on `typing && focus == Main`, or clear `typing` wherever the focus leaves the page.
- **F2 (Low): Home can be overridden once by a pending anchor.**
  - Home sets `page_selected = 0` and then builds (`state.rs:1348-1351`).
  - If `refresh_records`/`refresh_pool` stored an anchor after the last frame and before Home, the build restores the old record, so that one Home press does nothing.
  - This needs an event and a keypress with no frame in between.
  - Fix direction: clear `table_anchor` in the Home branch.
- **F3 (Low): some captured Limits values are words that mean "not known", but they sort as text.**
  - Examples: `none` (token ceiling, turn allowance, review tokens) and `unknown` (tokens observed).
  - Sorting `VALUE` descending therefore puts them before the figures.
  - `Cell::empty`/`SortKey::Unknown` would keep them last in both directions. The table is heterogeneous and short.
- **F4 (Informational): the empty state for a filter now depends on the count of records.**
  - The empty state says "Nothing on this page matches /…" only when `unfiltered > 0` (records only).
  - On a page with only notes and no records, a filter that hides the notes shows the page's own empty text instead. Examples: Changes with nothing recorded, Memory with no entries.
  - That text is still true, and the header shows `</filter>`.

## Remaining factual limits

- I did not compile, run tests, run Clippy or run a terminal check. The author's workspace checks and the parent's extended terminal checker (Agents ENABLED edit, Team pool cycle) are separate.
- This verdict covers production source `5e2506f`. The author's final commit must be compared with it before this verdict is relied on.
- `tests.rs` was read only to see what the tests assert, not to judge whether they are complete.
