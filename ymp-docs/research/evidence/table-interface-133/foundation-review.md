# YMP-133 table foundation and sidebar tables: independent review

**Verdict: two required fixes before the pages are built on this foundation.**

- **T1:** the sidebar TEAM table cuts model labels at common widths.
- **T2:** `table::widths` can return a layout wider than the room it was given.

The rest of the algorithm and of the sidebar wiring is correct.

**Scope and method.** `/tmp/ymp133-table-review` at `d0b674a`, compared with `49b3c79`: `table.rs`, the sidebar tables, the tests and the guide paragraph. The review was read-only. I ran no build or tests; the widths below are worked out from the source. The main-column pages and table controls are not in this commit and were not reviewed.

## Required

### T1 (Medium, regression): the TEAM table gives way on the model label instead of the activity

**Where:**

- `sidebar.rs:34-38` defines `TEAM_COLUMNS` as `•`, `AGENT*`, `ACTIVITY`.
- In `table.rs:93-101` a column title sets the column's minimum natural width.

**Why it happens:**

- The marker now takes its own 1-cell column plus a 2-cell gap. Before, it was `"● "`, 2 cells.
- `ACTIVITY` is always at least 8 cells wide because of its title, while the usual words are `idle` (4) and `working` (7).
- `AGENT*` is the flexible column, so it gives way first.
- The plan expected the opposite: "A narrow sidebar truncates TEAM activity first" (`/tmp/ymp133-table-plan.md:117`).

**Reproducer:** `sidebar::lines(app, W, _)` with the default layout.

| Terminal width | Sidebar inner width `W` | Member | `49b3c79` | `d0b674a` |
|---|---|---|---|---|
| 72–99 columns | 24 | idle, label `claude-opus-5` | `○ claude-opus-5  idle`, whole | `○  claude-opu…  idle` |
| 100–139 columns, for example 120x36 | 28 | working, label `claude-opus-5 max` | whole | `●  claude-opus-5 …  working`; the effort is lost |

- At `W = 24` a member that is `summarising` leaves `AGENT` only 8 cells: `claude-…`.
- The TOKENS table is not affected (15 cells at 24).
- The TASKS table loses 1 cell (12 instead of 13).

**Fix direction:**

- Keep the model label ahead of the activity word:
  - put the marker back inside the agent cell as a second span;
  - size the activity column by its words, with a shorter title or a title that may be cut.
- Add a test at `W = 24` and `W = 28` that these labels stay whole.

### T2 (Medium, algorithm contract): `table::widths` can overflow `width`, and it cuts the wrong column

**Where:**

- The last-resort step at `table.rs:128-137`.
- The doc comment at `table.rs:86-91` promises that the last visible column "is cut".

**What goes wrong:**

- When only rank-zero columns remain, the flexible column stops at its 8-cell floor.
- Only the last visible column is then cut, and never below 1 cell, so the widths plus gaps can still exceed `width`.
- The overflowing rows are then clipped by the paint rect without an ellipsis, and the rightmost columns disappear.
- The column that is cut is whichever is last, often a state word or a right-aligned figure, while the flexible text column still holds 8 cells.

**Reproducers:**

1. **Unit level.** Use `COLUMNS` from `table.rs` tests with the row `("a-very-long-model-identifier", "working", "5")`. `widths(&COLUMNS, rows, 9)` returns `[Some(8), Some(1), None]`: 8 + 2 + 1 = 11 cells for a width of 9.
2. **Planned Providers page** (`PROVIDER, COMMAND*, EXECUTABLE, ENABLED`, all rank 0), at a 60-column terminal, where the page content is 56 cells wide:
   - The row is `claude`, `claude-agent-acp`, a 47-cell executable path, `yes`.
   - Natural widths are 8, 16, 47 and 7.
   - The flexible column is squeezed to 8.
   - `ENABLED` is cut to 1.
   - The total is 8 + 8 + 47 + 1 + 6 = 70 cells for 56.
   - The path is clipped mid-word, and `ENABLED` is off screen.
   - The Agents (`AGENT*, READING, PROVIDER, ENABLED, TEAM`) and Reputation pages have the same rank-zero shape.

**Fix direction:**

- Guarantee that the widths plus gaps are at most `width`:
  1. after hiding by rank, squeeze the flexible column below `FLEX_MIN`, down to its title or a single cell;
  2. then cut non-flexible text columns, widest or rightmost first;
  3. cut a right-aligned figure only as a last resort, or hide it instead.
- Add unit tests:
  - the total never exceeds `width` for every `width` from 0 to the natural width;
  - a long non-flexible column fits;
  - a figure is not cut while a text column could still give way.
- The existing assertion `[Some(8), Some(4), None]` → `"a-very-…  wor…"` at width 14 fixes today's priority in place and will change.

## Optional

- **O1 (Low): a shortened table section can waste a row** (`sidebar.rs:112-124` with `fit` at `sidebar.rs:74-81`).
  - When `fit` gives a table section exactly `fixed + 1` rows (3), `shorten` draws only the title and the counter.
  - Example: TEAM with a title, column titles and 3 members, and 2 rows too many. It gets 3 rows and draws `TEAM` plus `▼ 3 more`, leaving a row empty; `49b3c79` showed one member there.
  - Fix direction: skip heights between `keep` and `fixed + 2` for table sections and give that row to the next section.
- **O2 (Low): an empty column still gets a gap** (`table.rs:104-107`, `:152-167`). A column with an empty title and no rows, such as the marker column of an empty table, still counts as visible. A page that draws an empty table starts its title line with a stray 2-cell gap.
- **O3 (tests):**
  - The narrow sidebar test checks line widths only from `TOKENS` down, and nothing checks that labels stay legible (T1).
  - `table.rs` has no test that the total stays within `width` (T2).
- **O4 (docs):** the new paragraph in `ymp-docs/guides/interface.md` merges into a line of about 150 characters, while the rest of the file wraps at about 100.

## Verified

- **Widths:**
  - Natural widths come from the titles and all rows, so widths stay the same when the sidebar shortens a section.
  - Columns hide by rank, highest first, and on a tie the rightmost goes first.
  - The flexible column's floor never cuts its title.
  - Column titles and rows share one set of widths.
  - `fit` pads every cell to exactly its column width and right-aligns figures.
  - A cut cell ends with `…`.
- **Sidebar data:**
  - TOKENS rows keep `app.agent_label` (the YMP-132 naming rule) and the usage figure style.
  - The unknown marker is right-aligned.
  - The session total stays on the `TOKENS` title, and `accepted N / M` moves to the `TASKS` title.
  - When every task is accepted, the TASKS section is its title alone.
  - Row order is unchanged.
- **Shortening:**
  - Column titles are never drawn without a row under them; with no room, the section is the title plus a counter.
  - Counters count rows only, not the column-title line.
  - `fixed.clamp(1, len)` is safe, because every section has a title.
- **Sidebar reach of T2:** the last-resort cut is not reached in the sidebar at an inner width of 24 or more. At 24, TEAM needs at most 24 cells (with `summarising`), TASKS 22 and TOKENS 17. T2 matters for the pages.

## Limits

- No compilation or test run; T1 and T2 are worked out from the source.
- No terminal rendering; ASCII markers were not checked separately.
- Pages, sorting, filtering and controls are outside this snapshot.
