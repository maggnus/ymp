# YMP-133: k9s-style tables in the TUI (bounded plan)

Status: planned next UI increment. This plan does not change application code and is separate from YMP-132.
Base: the integrated `main` after YMP-134. The existing draft branch starts at `5899589`
and predates the final 0.4.2 attribution fixes; it must not replace the integrated sources.

## Progress

These checkpoints describe implementation work; the task closes only after integrated verification
and installation. The [task register](README.md#ymp-133) holds the current timestamp and status.

- `[x]` Correct historical record origins: `44b6a08` and `49b3c79` are integrated and covered by the passing pre-table workspace checks.
- `[~]` Shared table layout and sidebar collections: initial implementation `d0b674a`; independent review requires narrow-label priority and a strict width bound (T1/T2).
- `[~]` Main-column tables, sorting, filtering and keyboard interaction.
- `[ ]` Independent final review, combined workspace and terminal checks, release installation.

## Owner requirements and implementation choices

- The owner requested k9s-style tables for all list data outside popups. This covers record
  pages and the sidebar lists TOKENS, TEAM and TASKS; the conversation and SESSION remain.
- The owner subsequently requested removal of sidebar navigation, expanded agent messages in
  Detailed mode, and better slash-panel and popup formatting inspired by the earlier ymp.
  YMP-134 tracks these changes separately. Do not restore NAVIGATE during the table conversion.
- The Inspect-only detail layout, sorting, filtering and exact keys below are implementation
  choices, not separately confirmed owner requirements. Keep them within the bounded UI change.
- The [direct-message audit](../research/paseo-direct-message-audit-2026-09-13.md) records the
  source conversation and distinguishes installed behavior from branch work.

## Out of scope

- Transcript, SESSION block, popup contents, `label.rs` API, YMP-132 behaviour.
- Backend core/storage/runtime, version, release notes, README, task register, real `~/.ymp2`.
- Configurable columns, persisted sort or filter, regex filters, mouse input.

## Behaviour

Layout
- A heading opens a table: an optional upper-case title, then a column-title row that stays
  visible while the list scrolls.
- Widths come from content. The flexible column (`*`) shrinks to 8 cells; columns with a hide
  rank drop out, highest rank first, when the page is narrow. Numbers are right-aligned.
- Sentence rows (each page's "what this page is" row, recovery notes) span the table.

Keys (main column focus only; composer and overlays unchanged; `Ctrl+C` stays first, as on main)
- `/` starts filter input. Typing narrows, Backspace edits, Up/Down move, Enter keeps the filter.
- `Esc` clears an active filter first; the next `Esc` closes the page as today.
- Filter: case-insensitive substring over all cells; a leading `!` inverts it. The page title
  reads `Title</filter>[N]`; an empty result reads `Nothing on this page matches /filter`.
  The filter resets when the page changes.
- `Shift+letter` sorts the table holding the selection by the column whose title offers that
  letter: the first letter of the title not taken by an earlier column or reserved by the page
  (`R` stays refresh on Agents and Providers). Repeating cycles ascending, descending, page order.
  The title shows `↑`/`↓` (`^`/`v` in ASCII) and underlines its letter. Unknown values sort last.
  The selected row is kept by key.
- `d` opens Inspect for the selected row on every page. Enter keeps each page's action and
  otherwise opens Inspect, as today.
- Memory search moves from `/` to `s`, because `/` is the table filter everywhere; the existing `/memory QUERY` command remains available. This routine key-binding choice is within the authorized UI change and does not open an owner question.

## Column map (`*` flexible, `(n)` hide rank, `>` right-aligned, `•` unnamed marker column)

| Page | Table | Columns |
|---|---|---|
| Help | each command group | COMMAND, SUMMARY* |
| Help | Keyboard | KEY, ACTION* |
| Tasks | tasks | •, TITLE*, STATE, GRADE(2), RESPONSIBLE(1) |
| Tasks | the plan | PLAN*, STATE |
| Tasks | proposals to change this plan | •, CHANGE*, OUTCOME, ASKED BY(1) |
| Tokens | Session | SCOPE*, TOKENS> |
| Tokens | Agents; Recorded outside the captured team | AGENT*, PROVIDER(1), TOKENS> (sorted by known total) |
| Sessions | - | SESSION, TITLE*, STATUS, TURNS>(2), CREATED(1) |
| Files | - | NAME*, KIND(1), SIZE> (sorted by bytes) |
| Changes | Recorded changes | PATH*, STATUS |
| Changes | Where accepted work was recorded | •, RESULT*, STATE |
| Checks | Recorded runs | •, COMMAND*, OUTCOME |
| Checks | Declared checks | •, COMMAND*, STATUS |
| Assignments | Running now; Left open; Recorded earlier | •, AGENT*, PURPOSE, STATE, STARTED(1) |
| Decisions | - | •, KIND*, ACTOR, OUTCOME, RECORDED(1) |
| Team | members | •, AGENT*, MODEL/READING(2), PROVIDER(1), STATE |
| Team | aside; roster; pool | •, AGENT*, STATE; •, RULE*, VALUE; •, MODEL*, PROVIDER, STATE |
| Agents | - | AGENT*, READING, PROVIDER, ENABLED, TEAM |
| Providers | - | PROVIDER, COMMAND*, EXECUTABLE, ENABLED |
| Memory | - | TITLE*, STATE, SCOPE |
| Reputation | - | AGENT, COMPETENCE*, OUTCOME, EVIDENCE |
| Limits | This session, as captured; The next run | LIMIT*, VALUE |
| Sidebar | TOKENS | AGENT*, TOKENS> |
| Sidebar | TEAM | •, AGENT*, ACTIVITY |
| Sidebar | TASKS | heading `TASKS | accepted N / M`; open tasks as •, TASK*, STATE |

## Commits

1. Table primitives, no visible change: `table.rs` (columns, cells with sort keys, widths,
   header and row rendering, filter and sort arrangement, sort-letter choice) with unit tests;
   ascending and descending markers in `theme.rs`.
2. Pages as tables: `Item` carries a heading's title and columns and a row's cells
   (`Item::table`, `Item::row`, `Item::note`, `View::sort_reserved`); the ~53 row and heading
   sites in `views.rs` follow the column map; `state.rs` keeps `Controls` and arranges the page
   before caching, with `/`, filter input, the `Esc` order, `Shift+letter`, `d`, and Memory `s`;
   `ui.rs` draws the title with filter and count, the filter input line, the sticky header, and
   no detail pane; `commands::KEYS` and `ymp-docs/guides/interface.md` describe the keys;
   the `tests.rs` helpers `row_prose`, `left_of_key`, `right_of_key`, `row_right`, `headings_of`
   and `pool_choices` read cells, and new tests cover filter and `!`, the `Esc` order, the sort
   cycle and indicator, selection kept after sorting, `d` opening Inspect, Memory `s`, hidden
   columns at a narrow width, and the absence of the detail pane.
3. Sidebar tables: compact TOKENS, TEAM and TASKS tables whose heading and column row are never
   split when the sidebar shortens; row-count and shortening tests.

## Verification

- Use a dedicated Cargo target for the implementation checkout, with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. Do not share compiled workspace artifacts across different checkouts.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test -p ymp-tui`, then `cargo test --workspace`; logs in `/tmp/ymp133-tables-*.log`.
- Negative controls: break the sort cycle, the `!` inversion and the sticky header in turn and
  show the targeted tests fail.
- TestBackend renders at 120 and 60 columns for Help, Tasks, Tokens, Team and Files, with Unicode
  and ASCII markers. No native provider inference.

## Risks

- Test churn in `tests.rs`, which the exit work on `main` also changed; base the branch on the
  integrated head rather than rebasing the draft across it.
- Key collisions: `/` on Memory (moved to `s`), future `d` bindings, uppercase letters that a
  page later reserves.
- A narrow sidebar truncates TEAM activity first.
- Pages whose order carries meaning (the plan, assignments) keep page order until a sort is chosen.

## Existing draft

`/tmp/ymp133-table-wip.patch` (1852 lines, against `6e9b396`) holds the uncommitted draft from
branch `feat/tui-k9s-tables` in `/private/tmp/ymp132-agent-attribution`: `table.rs` with unit
tests, `sidebar.rs`, the `ui.rs` page rendering, theme markers and a partial `views.rs`
conversion. It does not compile yet (`state.rs` and the remaining `views.rs` sites are missing).
An earlier application check succeeded against the then-current `main`; it does not establish
compatibility with the final integrated base. Use the patch as reference for the commits above,
not as a branch to merge. Four separate owner-requested UI commits on `feat/ymp133-tables`
are tracked in YMP-134 and do not implement these tables.

## Needed before implementation

- Integrate and verify YMP-134, then select a checkout from that integrated base.
- No owner answer is pending. Memory search may move to `s` as the routine key-binding choice
  described above; preserve `/memory QUERY` and document the change.
