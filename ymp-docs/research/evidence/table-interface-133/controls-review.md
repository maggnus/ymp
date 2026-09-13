# YMP-133 table controls: independent review of the integrated source

**Verdict: T1 and T2 are closed. Two interaction defects remain, and both are required fixes.**

- **R1:** after a rebuild, the selection stays on a row position rather than on its record.
- **R2:** on the Team page, a sort or filter can move the selection into another table.

Nothing else found blocks acceptance. The optional items below are polish.

## Revisions and method

- **Reviewed source:** main in `/Users/maggnus/Code/ymp2` at `a2f2707125ad389fcfa2ef2a935f8fa749a489b5`, "Integrate sortable and filterable record tables across the interface".
- **Early snapshot read in full:** `/tmp/ymp133-controls-review` at `ea04c93728a0431f6d0be6bcd2abda50740a5c8d`.
- **Changes from the snapshot to main:**
  - `ea04c93..63e4780` (author):
    - `commands.rs`, `table.rs` and `ui.rs` changed by `rustfmt` formatting only;
    - the rest of the change is tests in `tests.rs` and `table.rs`.
  - `63e4780..a2f2707` (integration):
    - the modal R1/O1 changes in `frame.rs` and `ui.rs`, and their tests;
    - `e8fae95`, which touches only `ymp-evals/scripts/check-table-interface.py`.
  - `state.rs`, `views.rs`, `sidebar.rs` and `theme.rs` are identical in `ea04c93` and `a2f2707`.
- **Review scope:** the snapshot findings were re-read against main, and the line numbers below are from main. The modal fixes are not reviewed again.
- **What was run:** nothing. There was no build, no test run and no provider inference; the reproducers are worked out from the source.
- **Integrated test run:** `/tmp/ymp043-integrated-test.log` was still being written when this report was finished. I do not treat it as passed.

## Required

### R1 (Medium): a rebuild keeps the selection's position, not its record

**Where:**

- In `state.rs:434-488`, `App::page` re-sorts a page on every rebuild. A rebuild happens whenever the revision changes or the cache is dropped.
- The selection is carried across by key only when `table_anchor` is set (`state.rs:476-483`). Only `rearrange` sets it (`state.rs:491-496`), that is, only for a filter or sort keystroke.
- Every other rebuild keeps the numeric `page_selected`:
  - runtime events through `changed()` (`state.rs:628`);
  - `refresh_records` (`state.rs:764-767`) and `refresh_pool` (`state.rs:774-777`);
  - `edit_config` → `notice` + `refresh_pool` (`state.rs:1709-1728`);
  - prompt commits and commands (`state.rs:1968`, `state.rs:2130`).
- `usage()` (`state.rs:644-650`) keeps the key, but only for `View::Usage`.

**Why it matters:** before sorting, a rebuild kept the page's own order, so rows moved only when a record was added or removed. A sorted column now moves a row as soon as its value changes. The highlighted row, `d`/Enter, and the page actions then point at whatever record took that position.

**Reproducer, deterministic, no runtime:**

1. Open `/agents` with at least three profiles, some enabled and some disabled.
2. Press **Shift+N** to sort by `ENABLED`. With `R` reserved, the sort keys are A, E (READING), P, N (ENABLED) and T.
3. Select an enabled profile `X` that sits next to the disabled group.
4. Press **Space**. `X` is disabled and saved (`state.rs:1495-1513`). The page is rebuilt, and `X` moves into the other group.
5. `page_selected` is unchanged, so it now points at a neighbouring profile. A second **Space** enables or disables that profile, not `X`.

`t` (team membership) on the `TEAM` column (**Shift+T**) behaves the same way: `in team` is a known value, while an empty cell is unknown and sorts last.

**Live reproducer:**

1. Sort `/tasks` or `/assignments` by `STATE` while a run is active.
2. A `UiEvent::Task` or `UiEvent::AgentStatus` changes one row's state. The event calls `refresh_records` + `changed()`, so the page is rebuilt.
3. The selected position now holds another record.
4. A keypress that arrives between the event and the next frame is aimed at that other record, even though the old row is still on screen. This covers `d`, Enter, and `selected_item()` for page actions.

**Fix direction:** anchor every rebuild of the same view, not only rearrangements.

- In `page()`, when a stale cache for the same `view` exists and `table_anchor` is `None`, take the key of the currently selected row (with its table, see R2) before rebuilding and restore it afterwards.
- If the record is gone, keep the nearest index rather than jumping to the first row.
- The `View::Usage` special case in `usage()` then becomes unnecessary.
- Tests:
  - the Agents page sorted by `ENABLED`: Space, then assert that the selection is still `X`;
  - a sorted page where a record's sort value changes between two frames.

### R2 (Medium): an anchor matches the first row with the key on the page, even in another table

**Where:**

- `state.rs:478-482` (and `restore_page_anchor`, `state.rs:694-707`) looks for the first row anywhere on the page whose key matches.
- The Team page lists the same profile id in more than one table:
  - `member_row` (`views.rs:2158`) under "Members of this session" / "Members of the next run" (`views.rs:1663`);
  - `aside_row` under "Captured here, no longer a member";
  - `pool_row` (`views.rs:2526`, key `agent.profile.id` at `views.rs:2553`) under "Available on this machine" (`views.rs:1696`).
- The pool is read from the configured profiles, so every enabled member normally appears in both the Members table and the pool table.
- `sort_by_key` chooses the table from the heading above the selection (`state.rs:501-528`).

**Reproducer:**

1. Open `/team` and move the selection down to a row in **AVAILABLE ON THIS MACHINE** whose profile is also a member, for example `claude`.
2. Press **Shift+M** to sort the pool table by `MODEL`. The pool table is sorted, and `rearrange` anchors on `"claude"`.
3. The rebuild finds the **Members** row for `claude` first, so the selection jumps up into the Members table, and the view scrolls back to it.
4. Press **Shift+M** again to reverse the direction. The heading above the selection is now Members, which has no `M` key, so `sort_by_key` returns `false` and nothing happens. The pool table cannot be cycled to descending from where the reader started.
5. `d` now opens the member record instead of the pool record the reader was on.

Typing a filter (`/cl…`) on the same row also moves the selection into the Members table after every keystroke.

**Fix direction:**

- Anchor on the table as well as the key: the heading title plus the row key, since titles are unique on each page.
- Look up the key first in the same table, then anywhere on the page.
- Test: on the Team page, sort the pool table twice from a pool row, and assert both the direction and that the selection stays in that table.

## T1 and T2 from the foundation review

- **T1: closed.**
  - `sidebar.rs:32` is `TEAM_COLUMNS = [AGENT, STATE*]`. The marker is now inside the agent cell, and the activity word is the flexible column.
  - Widths worked out for member `claude-opus-5 max`, with the marker cell counted in `AGENT`:
    - at `W = 28`, `working` is whole: 19 + 2 + 7 = 28;
    - at `W = 24`, the label stays whole and `STATE` gives way.
  - For `summarising` beside `claude-opus-5` at `W = 24`, the label stays whole and the state reads `summar…`.
  - `tests.rs:8283-8326` checks both labels at 24 and 28 for two states, the line widths, and a whole `claude-opus-5 max  working` at 28.
- **T2: closed.**
  - `table::widths` (`table.rs:353`) squeezes columns in this order: the flexible column, then left-aligned text, then any column, each down to 1 cell (`give_way`, `table.rs:424`). It then drops whole columns from the right while the span still exceeds `width` (`table.rs:407`). The result can no longer be wider than `width`.
  - `a_table_never_takes_more_than_its_width` sweeps every width from 0 to the natural width + 2. It checks both the span and the painted width, and that a figure is not cut while text can still give way.
  - Page rows get `width − MARKER` (`ui.rs`, `table_lines`), so the marker never pushes a row past the page.
- **Foundation optional items:**
  - O1 (a wasted row in a shortened section) is closed: `sidebar.rs:108-112` gives the column-title row to a record.
  - O2 (a gap for an empty column) is closed: a zero-width column is `None`.

## Optional polish

- **P1:** when the anchored row is gone, the selection goes to the first row on the page, not the nearest one (`state.rs:482`). A filter keystroke that removes the selected row jumps to the top.
- **P2:** with no memory entries, Memory draws the `TITLE  STATE  SCOPE` column titles over nothing (`views.rs:2983`), because its heading is pushed unconditionally.
  - Every other page adds a heading only when it has rows.
  - The page's `empty` text still cannot appear, because the about note is a row. That was true before this change.
- **P3:** the `[N]` count (`ui.rs:543`) counts note rows, such as the about rows on Team, Changes, Checks, Limits, Memory and Reputation. On those pages it overstates the records, and a filter that matches only the about note shows `[1]`.
- **P4:** while a filter is being typed, Tab/BackTab still move focus and Ctrl+J still inserts a newline (`state.rs:1091-1110`, before `main_key`). After Tab, `typing` stays true: the filter line and its hints remain while keys go to the composer.
- **P5:** Limits `VALUE` (`views.rs:3528-3566`, captured rows) and the roster-rules size sort as text, so `12` comes before `4` and `300 s` is compared as a string. The tables are four to twelve rows long.
- **P6:** when a sorted column is squeezed, `fit` cuts the direction arrow from its title first, so the header no longer shows that the table is sorted.

## Verified

- **Unknown values last in both directions.**
  - `compare` (`table.rs:261`) returns unknown after known before the direction is applied.
  - A number and a text value keep a consistent order.
  - Values stored as unknown:
    - `token_cell` without a known total;
    - a directory's `SIZE`;
    - an empty task `GRADE`;
    - an empty Agents `TEAM`.
  - `tests.rs:8954` covers ascending, descending, reset, and the selection kept through the sort.
- **Numeric sort.** Token totals, session `TURNS` and file sizes use `Cell::number` with the real value. Created, started and recorded columns sort by the full RFC 3339 text.
- **Sorts per table.**
  - A sort is keyed by `(view, table title)`.
  - No page has two tables with the same title. I checked Tokens, Team, Limits, Changes, Checks, Assignments, Tasks with its board tables, and every page with one untitled table.
  - The Tokens test checks that sorting `Agents` leaves the `Session` table untouched.
- **Notes.**
  - Notes stay above a sorted table's rows, and rows before any heading are filtered but not sorted.
  - A note is painted as one line across the table.
  - No record row has a single cell under several columns.
- **Keys.**
  - Ctrl+C is handled before the overlay and every page key (`state.rs:1054`).
  - Esc order: overlay, then completion, then clearing the filter, then closing the page (`state.rs:1163-1180`).
  - While typing, letters, Space and digits go into the filter, so neither sort keys nor page actions fire.
  - `d` inspects.
  - Only upper-case letters sort, and only when the table under the selection has that key. Otherwise the key falls through to the page action.
  - `R` is reserved on Agents and Providers, and no other page has an upper-case action.
  - Memory search is now `s`.
- **Inspect.**
  - It reads the arranged page, and the popup is titled by the flexible cell.
  - No cell text is truncated to `ctx.width`, so a rebuild at the Inspect width keeps the same order. R1 still applies when data changes in between.
- **Sticky column titles.** For a body of two or more lines, the titles are painted over the top line only when that line is a row of the same table, and the selected row is moved below them (`ui.rs:615-630`).
- **Empty and filtered states.**
  - A filter that removes everything shows "Nothing on this page matches /…".
  - Opening another page clears the filter and the anchor.
- **Native labels.**
  - Pool rows use `label::offering`, falling back to `label::UNKNOWN_MODEL`.
  - Member and agent rows keep their label and reading words.
  - Tokens uses `presented_name`, and the sidebar uses `agent_label`.
  - Cells use `one_line` rather than width-based truncation. Cutting happens only in `fit`, and it shows `…`.
- **Every record page is a table:** Help, Tasks with its plan and proposals, Tokens, Sessions, Files, Changes, Checks, Team, Agents, Providers, Memory, Reputation, Limits, Assignments and Decisions. The only rows that remain notes are explanatory: the about rows, recovery, how checks run, and `captured:none`.

## Limits

- No compilation, no tests and no terminal rendering were run for this review.
- R1 and R2 were worked out from the source and not executed.
- The integrated workspace run `/tmp/ymp043-integrated-test.log` had not finished when this was written.
- A body one line high was not examined for the sticky header. It is not reachable at supported terminal sizes.
