# UI architecture review: state concentration (H7) and recovery communication (H6 / gap 4)

Read-only investigation of the source, the Git history and the committed verification reports. No
product change was made. This note answers two bounded questions from
`ymp-docs/requests/01-research-program-and-success-gaps.md`:

- H7, the UI half: is the concentration in `ymp-rust/crates/ymp-tui/src/state.rs` an active cost?
- H6 (UI portion) and success gap 4: what can the interface honestly say about acceptance commands
  and about recovering the working directory?

## Baseline

| Item | Value |
| --- | --- |
| Commit under review | `56e21dd` "Track live token usage per session and agent", 2026-09-12 12:18 +0800 |
| Source tree | matches `56e21dd`; research artefacts are being written in parallel, so `git status --porcelain` shows untracked `ymp-docs/requests/`, `ymp-docs/research/evidence/`, `ymp-docs/tasks/` |
| Commits in repository | 4 |
| Committed verification reports | `ymp-evals/reports/`: `initial-verification.md`, `working-directory-regression.md`, `ui-redesign-verification.md`, `token-usage-verification.md` |
| Not available in this repository | any time series of pull-request review latency, and any record of merge conflicts: the history is linear, single-author, with no pull requests and no merges |
| CI | `.github/workflows/check.yml`: fmt, clippy `-D warnings`, `cargo test --workspace`, bridge check and test |

**Model use.** This review made no product-provider calls and ran no experiment. It is not free:
the review itself is a delegated Claude Opus 5 run, as `AGENTS.md` requires for UI work, and it
consumes model quota like any other delegated session. Separately, the committed
`token-usage-verification.md` records opt-in real-provider probes (Codex, Claude, GLM ACP) that
also consumed quota; those were the parent's, not this review's.

All line references below are as of `56e21dd`.

### Reproducible read commands

```sh
git log --oneline
git log --format='%h %s' --numstat -- ymp-rust/crates/ymp-tui/
git show 56e21dd --stat -- ymp-rust/crates/ymp-tui/src/state.rs
git show 56e21dd -- ymp-rust/crates/ymp-tui/src/state.rs | grep '^@@'
git ls-tree -r --name-only 56e21dd ymp-rust/crates/ymp-tui/src/ \
  | while read f; do printf '%6s %s\n' "$(git show 56e21dd:$f | wc -l)" "$f"; done
git show 56e21dd:ymp-rust/crates/ymp-tui/src/state.rs | grep -n '^    // ---'
git show 56e21dd:ymp-rust/crates/ymp-tui/src/state.rs | grep -c 'Span::'
grep -rn 'page_selected\|page_top' ymp-rust/crates/ymp-tui/src/*.rs
grep -rn 'git' ymp-rust/crates/ymp-tui/src/
grep -c '^#\[test\]' ymp-rust/crates/ymp-tui/src/tests.rs
sed -n '1,200p' ymp-evals/reports/token-usage-verification.md
sed -n '1,200p' ymp-evals/reports/ui-redesign-verification.md
cargo test -p ymp-tui
```

## Part A — H7, concentration in `state.rs`

### A1. Observations

**Module sizes** (`git show 56e21dd:<path> | wc -l`): `state.rs` 1907, `tests.rs` 1902,
`views.rs` 1335, `ui.rs` 931, `text.rs` 454, `transcript.rs` 453, `usage.rs` 419,
`sidebar.rs` 418, `theme.rs` 375, `commands.rs` 301, `frame.rs` 270, `lib.rs` 255,
`terminal.rs` 100, `prefs.rs` 64.

**What `state.rs` contains.** Its own section banners divide it into:

| Section | Lines | Size |
| --- | --- | --- |
| types and preamble (`Focus`, `Overlay`, `Action`, `Route`, `Field`, `Viewport`, `App`) | 1..311 | 311 |
| Bookkeeping (events, page cache, session load) | 312..641 | 330 |
| Navigation | 642..706 | 65 |
| Scrolling | 707..803 | 97 |
| Themes and preferences | 804..827 | 24 |
| Keyboard | 828..1384 | 557 |
| Overlays | 1385..1628 | 244 |
| Submitting | 1629..1676 | 48 |
| Commands | 1677..1907 | 231 |

The file holds 67 functions and one struct with 40 fields. It contains almost no rendering: two
uses of `Span::` in 1907 lines, both inside `inspect_selected_entry`. Drawing lives in `ui.rs`,
`frame.rs`, `sidebar.rs`; page content in `views.rs`. The declared split (state and keyboard here,
pixels there) holds in practice.

**Churn.** `git log --numstat -- ymp-rust/crates/ymp-tui/` gives the whole history of the crate:

| Commit | Effect on the UI crate |
| --- | --- |
| `7d815be` | created the crate as a single `lib.rs` of 699 lines |
| `54f3da8` | rewrote it into 13 modules; `state.rs` created at 1773 lines, `lib.rs` 200/-645 |
| `c87aa73` | `theme.rs` only, 5 added and 5 removed |
| `56e21dd` | 9 files: `usage.rs` +419 (new), `tests.rs` +628, `views.rs` +189/-1, `state.rs` +135/-1, `ui.rs` +87/-14, `sidebar.rs` +69/-2, `commands.rs` +7, `theme.rs` +8, `lib.rs` +1 |

**Shape of the one feature-sized change to `state.rs`.** `56e21dd` touched it in 11 hunks
(`git show 56e21dd -- .../state.rs | grep '^@@'`): three lines in the `App` struct, one in the
constructor, one in the page context, one block of about 100 lines holding the new usage event
path and its helpers, six lines in `refresh_session_facts`, one in `load_session`, one in `/new`,
one in the command table, one in `select_a_row`.

**Cost of adding a whole page.** The new `/usage` destination needed one entry in `commands.rs`,
one arm in the view enum and the page builder in `views.rs`, and one line in the command
dispatcher in `state.rs`. It needed **zero** lines in the per-page key table `page_action`
(`state.rs`, Keyboard section), because the generic inspect path already covered it. The seam
usually predicted to grow with every page did not grow.

**Verification practice.** The crate has 65 tests in `tests.rs`, all headless: they build an `App`
against a temporary store, press keys, and render into a `TestBackend`. `cargo test -p ymp-tui`
completes in about 0.25 s. Two independent UI verification cycles are recorded:
`ymp-evals/reports/ui-redesign-verification.md` (version 0.2.0, 64 workspace tests including 50 UI
tests, plus a pseudo-terminal pass) and `ymp-evals/reports/token-usage-verification.md`
(version 0.3.0, 90 workspace tests including 65 UI tests, plus a release pseudo-terminal pass).
Neither report attributes any defect to module size.

### A2. The two regressions observed in this cycle, and their actual cause

Both were found while the token statistics feature was being built. Both are recorded in the
committed report `ymp-evals/reports/token-usage-verification.md` and are now covered by tests.
Neither is attributable to the length of `state.rs`.

**Regression 1 — the first frame painted a selection the model had already left.**
`ui.rs::page_view` read `app.page_selected` into a local *before* calling `app.page(width)`, and
`App::page` may move the selection when it rebuilds (`state.rs:390` builds, `state.rs:427`
`select_a_row` assigns `page_selected`). Result: opening `/usage` showed no breakdown until a key
was pressed, and the first `Down` skipped a row. The same pattern existed in `App::selected_item`
(`state.rs:793`). Cause: a read accessor with a side effect on shared selection state, plus two
callers that sampled that state around the call. Committed acceptance evidence: the report lists
"initial detail selection" among the UI checks and records that "Opening /usage immediately
displayed the session's input/output breakdown" in the release terminal run.

**Regression 2 — stale session status and turn count during a run.** The header and the sidebar
printed the stored session record while a run was working, so the token total grew while the
status and the turn counter stood still. Committed evidence, from the same report: "The terminal
check first rejected the earlier build that displayed growing tokens alongside a stale zero turn
count. The corrected build passed the same check", and, for the corrected behaviour, "Intermediate
frames showed growing token totals, a growing invocation count, and running status together. One
observed frame showed 100+ tokens and 1 / 80 turns." Cause: `App` mirrored persisted session facts
(`session_status`, `turns_used`, read in `refresh_session_facts`) with no place distinguishing
them from what the live run had already reported. Both surfaces read the field directly. Fixed by
deriving the presented values (`live_status`, `count_turns`), not by moving code.

**A third artefact of the same class, still present.** The `width` argument means different things
at different call sites: `lib.rs:242` passes the full terminal width into `App::on_key`, while
`ui.rs:460` builds the page at the main column width. Page content therefore wraps for one width
and may be displayed at another, most visibly inside the inspect overlay. The current code
compensates with fixed caps (`views.rs:631` `LEDGER = 44`, `views.rs:635` `PROSE = 78`) rather than
an enforced invariant. This is a latent inconsistency, not a demonstrated current defect: no
verification run has reported a visible failure from it since the caps were introduced.

Regression tests covering these paths: `tests.rs:1820`
`opening_the_statistics_page_shows_the_session_breakdown_at_once`, `tests.rs:1672`
`a_new_snapshot_keeps_the_row_and_the_reading_position`, `tests.rs:1842`
`a_running_session_shows_live_tokens_turns_and_status_together`, `tests.rs:1889`
`browsing_a_saved_session_reports_what_was_stored`. The first and third were confirmed to fail
with the corrections reverted.

### A3. What cannot be inferred from this history

- **Churn per concern.** Two of the four commits created or rewrote the entire crate; one touched
  a single file. There is exactly one feature-sized incremental commit. No churn rate, no
  hot-spot ranking, and no trend can be computed from that.
- **Merge conflicts.** A single author on a linear `main`, no pull requests, no merges. Whether
  two concerns inside `state.rs` would collide in concurrent editing is unobservable here.
- **Review latency.** Verification reports exist and are detailed, but they are per-release
  narratives, not a time series. There is no per-change review duration, no queue data, and
  therefore no way to relate review effort to module size.
- **Regression risk attributable to file size.** Both defects in A2 have identified and different
  causes, neither of which is length. A statement of the form "1907 lines caused this" would be an
  assertion, not a finding.
- **A second reader's experience.** Navigability was assessed by one agent working in the file for
  one feature. That is weak evidence about comprehension cost and no evidence about onboarding.

A cheap way to make H7 decidable later, without any refactor now: keep one feature per commit and
revisit after roughly ten incremental feature commits, using two metrics this history cannot yet
provide — files touched per feature, and hunks per file per feature. A split becomes defensible
when a single concern repeatedly accounts for most hunks in the file while the rest of the file
stays untouched. `56e21dd` shows the opposite pattern.

### A4. Verdict for UI state concentration

**Reject the H7 split of `ymp-tui/src/state.rs` now.** The available evidence does not show an
active cost, and two observations argue against splitting at this point:

1. Observed: a UI feature is inherently cross-surface. The one measured feature touched state, a
   page, the sidebar, the header, the command registry and the tests. Decomposing `state.rs` would
   add public seams between parts that are currently private to one module.
2. Observed: the defects that did occur came from temporal coupling around a mutating accessor and
   from a missing distinction between persisted and live facts. Neither cause is removed by a
   split.

Two related claims are **inferences, not observations**, and are marked as such: that a split would
raise the number of files a typical feature touches, and that separating `state.rs:427-437` from
its readers in `ui.rs:461` and `state.rs:793` would make the coupling harder to notice. Both follow
from the structure described above; neither has been measured here, and a well-chosen module
boundary could contradict them.

The module is cohesive by its own stated boundary, it is fully testable headless, and its largest
section (Keyboard, 557 lines) is the keyboard contract itself, which is the one thing that must be
read as a whole to answer "what does this key do here".

This verdict covers the UI half of H7 only. `ymp-runtime/src/engine.rs` (1391 lines) was not
assessed; its inline prompt templates are a different argument with a different owner.

## Part B — H6 (UI portion) and success gap 4

### B1. What the code guarantees today

**Change tracking is hashes, not content.** `Workspace::open`
(`ymp-rust/crates/ymp-workspace/src/lib.rs:26`) records a SHA-256 fingerprint of every file in the
working directory at session start and stores it in the metadata directory as `workspace.json`.
`changes()` (`:60`) compares a fresh fingerprint against it and classifies each path as created,
modified or deleted. `diff()` (`:84`) returns those labels as text, not a content diff. The walk
skips `.git`, `node_modules`, `target`, `__pycache__`, `.DS_Store`, `.ymp2` (`:108-120`).
Consequence: ymp can say *what* changed and cannot restore *anything*. This is deliberate and
recorded: `ymp-evals/reports/working-directory-regression.md` states that ymp stores "file hashes,
change records, history, and task state under `~/.ymp2`" and creates "no hidden source copies or
Git repositories".

**The change list is written once, at the end of a run.** `workspace.save_changes()` is called at
`ymp-rust/crates/ymp-runtime/src/engine.rs:313`, after the run completes, fails or is cancelled.
During a run there is nothing to read, and if the process dies no list is ever written, although
`workspace.json` still makes it recomputable afterwards. The UI reads exactly that file:
`views.rs:804` `changes()` loads `session_dir/workspace/changes.json` (`:813`), and its empty state
already says "Change metadata is written when a run finishes or stops" (`views.rs:845`).

**The UI knows nothing about version control.** The only occurrence of `git` in the crate is the
skip list of the Files page (`views.rs:722`). Nothing reads or displays repository state.
`Task.base_commit` exists in the model and is set to `None` at `engine.rs:639` and `engine.rs:908`
and never read: a vestigial field, not a recovery mechanism.

**Acceptance commands run unguarded, and are journalled.** `Engine::checks` (`engine.rs:1066`) runs
each plan-supplied command through `/bin/sh -c` in the working directory with a 300 s timeout
(`:1073-1079`), and records a `check` event with the command, success flag and truncated output
(`:1086`). Commands run per task after its execution turn (`engine.rs:989`) and again for the whole
plan before the final review (`engine.rs:672-678`). No UI surface reads `check` events: the Tasks
page shows the *declared* `task.checks` strings in its detail and nothing about what actually ran.

**Where unrestricted access actually occurs.** Planning and bidding are read-only turns:
`engine.rs:811` (`"plan"`) and `engine.rs:934` (bid) pass `read_only = true`, as does the final
review at `engine.rs:686`; that flag selects `"sandbox":"read-only"` for Codex
(`ymp-rust/crates/ymp-providers/src/lib.rs:93`) and `permissionMode: "default"` for Claude
(`ymp-bridges/claude/src/index.ts:38`). The execution turn is the one that runs unrestricted:
`engine.rs:971` passes `read_only = false`, which selects `"approvalPolicy":"never"` with
`"sandbox":"danger-full-access"` (`providers/src/lib.rs:93`), `bypassPermissions` in the Claude
bridge (`index.ts:38`) and `"modeId":"bypass_permissions"` on the ACP path
(`providers/src/lib.rs:240`). The execute prompt says so plainly: "You may change files and run
tools autonomously" (`engine.rs:969`). By the time acceptance commands run, the execution stage has
already had unrestricted access to the same directory.

**There is no channel for asking the reader a question mid-run.** `UiEvent`
(`ymp-rust/crates/ymp-core/src/model.rs`) is one-way, engine to interface. `Action`
(`state.rs:82-104`) carries only Quit, StartRun, FollowUp, QueueMessage, Resume, Cancel. Nothing in
`ymp-runtime` waits for an answer (`grep -rn 'confirm\|approval\|prompt_user'
ymp-rust/crates/ymp-runtime/src/` returns nothing). A blocking confirmation would require a new
request and reply path across the engine boundary, plus a policy for what happens while it waits.

### B2. Interpretation

A prefix allowlist over acceptance commands cannot be presented in the interface as protection.
The commands are proposed by a read-only planning turn, but they run after an execution turn that
already had unrestricted access to the same directory, so gating them is not a boundary around the
run. The allowlist would also not be sound on its own terms: `cargo test` runs build scripts,
`npm run` runs whatever the repository declares, `pytest` imports `conftest.py` from the tree. A UI
element implying containment here would be a false statement about the system, which is worse than
no element at all.

A first-run confirmation is honest but is not a UI-only change: it needs the request and reply
channel described above, and its cost lands on autonomy, at unpredictable moments, in a tool whose
point is running unattended. Recommendation: park it as a runtime-level design question, not a UI
task.

What the interface can do honestly is disclose: name what ymp records, name what it cannot restore,
and show what was actually executed. All three are reads of data that already exists.

### B3. Verdict for the UI portion of H6

**Reject** the prefix allowlist as a UI-visible safety feature. **Park** the first-run
confirmation: it is a runtime concern needing a bidirectional channel that does not exist, and its
value is disclosure rather than containment. **Accept** disclosure, scoped in proposal 2.

## Follow-up proposals

Two, both small, both independently landable. Neither is a refactor for appearance.

### Proposal 1 — optional: make the page selection an output of building the page

**Status.** This is a cleanup of an API shape, not a fix for a demonstrated current defect. The two
defects it generalises are already corrected and covered by tests; the third artefact (the
ambiguous `width` argument) is latent, with no observed user-visible failure since the compensating
caps were added. Priority accordingly: below anything with an observed failure.

**Scope.** `state.rs` (`page`, `select_a_row`, `selected_item`, `page_anchor`,
`restore_page_anchor`) and `ui.rs::page_view`. No new module, no new file, no behaviour change. One
accessor returns the built page together with the selection index resolved for that width, so no
caller can sample `page_selected` that the current page has not validated; and the `width` argument
means one thing everywhere, namely the column the page is drawn in.

**Why it might still be worth doing.** The ordering invariant is currently held by two comments
(`ui.rs:459-461`, `state.rs:794-796`) **and** by regression tests (`tests.rs:1820`, `tests.rs:1672`)
that would fail if a caller reintroduced the old ordering on an existing page. What neither
protects is a *new* page added later with a new call site. The compensating constants `LEDGER` and
`PROSE` (`views.rs:631`, `:635`) exist because the width question is unanswered.

**Acceptance criteria.**

1. `grep -rn 'page_selected' ymp-rust/crates/ymp-tui/src/` shows no read outside `state.rs`
   (today: `ui.rs:461`).
2. Key handling and rendering agree on the page width for the same terminal size, asserted by a
   test (today `lib.rs:242` passes the terminal width and `ui.rs:460` the column width).
3. `tests.rs:1820` and `tests.rs:1672` still pass unchanged, and both still fail if the new
   accessor is bypassed.
4. `LEDGER` and `PROSE` are either removed as unnecessary or documented as a deliberate
   typographic choice rather than a width workaround.
5. No change to any page's visible output at 80x24 and 120x36.

**Evidence.** `state.rs:390`, `:427-437`, `:793-799`; `ui.rs:459-461`; `lib.rs:242-243`;
`views.rs:631-635`; A2 regressions 1 and 3; `ymp-evals/reports/token-usage-verification.md`.

**Reject it if.** The UI stops gaining pages. With no new destinations, the trap is inert and the
comments plus the existing tests are sufficient.

### Proposal 2 — recovery disclosure in the UI, with no safety promise

**Scope.** `views.rs` (Changed files page, Tasks page detail) and the welcome text in `ui.rs`.
Reads only. Two statements the interface does not currently make:

1. What ymp records and what it cannot restore: hashes in `workspace.json`, no copy of previous
   content, so restoring is the user's own version control or nothing.
2. What was actually executed: the `check` events already carry command, success and output
   (`engine.rs:1086`) and no surface reads them.

**Repository detection is deliberately not part of the minimum.** A `cwd/.git` existence test
cannot establish that a directory is outside version control: the working directory may be nested
inside a repository whose root is an ancestor, `.git` may be a file rather than a directory in a
worktree or submodule, and the directory may be under a different system entirely. Absence of
`cwd/.git` supports no conclusion. If repository awareness is wanted, it must come from validated
discovery in the workspace layer (ancestor walk, `.git` file indirection, explicit "unknown" when
detection fails), not from a filesystem guess in the render path, and its tests must include a
working directory nested below the repository root.

**Acceptance criteria.**

1. The Changed files page states, in both its empty and populated states, that ymp records file
   hashes rather than copies and therefore cannot restore any earlier content. The statement is
   unconditional and makes no claim about whether the directory is under version control.
2. The executed acceptance commands and their exit status are readable for the loaded session, and
   the wording does not describe them as approved, gated, sandboxed or restricted.
3. No text anywhere claims containment. Reviewed against `providers/src/lib.rs:93` and `:240`,
   `ymp-bridges/claude/src/index.ts:38`, and the read-only/execute split at `engine.rs:811`,
   `:934`, `:971`.
4. Two tests: one asserting the recovery statement is present on the Changed files page, one
   asserting a session's acceptance commands and outcomes are visible at 80x24.
5. If, and only if, repository awareness is added: discovery lives outside the render path, is
   validated, distinguishes "no repository", "repository root is an ancestor" and "unknown", and
   has a test with the working directory nested inside a repository.
6. Not delivered by this proposal, and stated as such: a change list for a session whose process
   died. `save_changes` runs once (`engine.rs:313`); recomputation from `workspace.json` belongs to
   the runtime or workspace layer, not to the render path, and the UI cannot supply it alone.

**Evidence.** `ymp-workspace/src/lib.rs:26,60,84,108-120`; `engine.rs:313,672-678,811,934,971,
1066-1086`; `views.rs:722,804,813,845`; absence of git awareness in the UI; `Task.base_commit`
unused at `engine.rs:639,908`; `ymp-evals/reports/working-directory-regression.md`; gap 4 of the
request.

**Reject it if.** The owner intends to add real recovery (snapshots or a git integration) in the
near term. In that case the wording written now would have to be unwritten, and waiting is cheaper.
Disclosure is worth it precisely because no recovery mechanism is planned.

## Addendum — H9 (UI portion): what the interface labels today

Added after the appendices H8 and H9 were appended to the request. Scope: what the existing
surfaces actually label, and whether an optional weighted estimate should coexist with raw counts.
The formula, its coefficients, the price basis and its expiry, which journal is canonical, and the
H8 backend validity question are the parent's and are not judged here. No provider call was made
for this addendum.

### What is labelled now

- **Header.** One figure plus the word `tokens`, shortened to `tok` below 100 columns
  (`ui.rs:148-153`). The figure is `UsageTotals::known_total()`, which is input plus output
  (`ymp-core/src/usage.rs:79-85`), and input includes cache reads and writes by contract. So the
  header shows raw volume, and cache-read tokens are counted in it at full weight.
- **Sidebar.** The `TOKENS` section title carries the same session figure (`sidebar.rs:250`) and one
  row per agent carries the same kind of figure; the navigation badge repeats it
  (`sidebar.rs:159-163`).
- **`/usage`.** The subtitle is the same figure plus coverage in words; the list has one
  `session total` row (`views.rs:505`) and one row per agent. The selected row's detail is the only
  place where the composition appears: `input`, then `cache read` and `cache write` indented under
  it, then `output` with `reasoning` indented, then `total`, then the invocation counters
  (`usage.rs:311-330`). A note states the inclusion rule explicitly: input already includes cache
  reads and writes, output already includes reasoning, neither is added again (`usage.rs:416`).
- **No money anywhere.** The crate contains no price, currency or cost string; the only match for
  "spend" is prose about the turn budget (`views.rs:1290`). Nothing in the interface converts counts
  into money, and nothing weights them.
- **`/limits`** holds `parallel`, `turns`, `timeout`, `attempts` only (`views.rs:1283-1298`). There
  is no token budget there, so H9's suggestion to show the pair "in `/limits`" has no existing
  figure to attach to; `/usage` is the token surface.

### Assessment of the premise

H9's statement that the user "currently sees only the raw sum" is accurate for the header, the
sidebar and the navigation badge. It is not accurate for `/usage`, where cache read and cache write
are labelled, placed under the input they belong to, and accompanied by the inclusion rule. The
distortion H9 describes therefore affects the one-line surfaces, not the statistics page. The label
itself is literally true: `tokens` names a volume, and no surface claims a cost. The risk is
interpretation, which is a real risk, and it is the strongest argument for offering a second number.

### The internal conflict in H9

H9's hypothesis says a single effective-tokens metric, "and only that metric displayed". Design
direction item 3 says "Display both numbers, label them honestly" and "Never show the raw sum
alone". These cannot both hold. From the UI side the second is the one to keep, and effective-only
should be rejected for two concrete reasons:

1. **It would lose information the providers did report.** `TokenCounts` fields are independently
   optional (`ymp-core/src/usage.rs:7-13`), and `known_total()` needs only input and output. A
   weighted figure needs `cache_read` and `cache_write`, which are not always reported: the
   committed `ymp-evals/reports/token-usage-verification.md` records that the installed GLM ACP
   agent reports only its last model request, and that migrated historical rows stay partial. With
   effective-only, a session whose input and output are known would display as unknown, or the
   interface would have to treat unreported cache as zero. The second option contradicts the rule
   the whole surface is built on, that a dash means unreported and is not a zero.
2. **It would overload the one marker that already has a meaning.** `+` currently means "lower
   bound, because an invocation is open, partial or unattributed". A figure derived from
   configurable coefficients is uncertain for an unrelated reason. Reusing `+` would merge a
   reporting fact with a modelling assumption; a derived figure needs its own mark and its own
   unknown semantics.

### Recommendation

Raw reported counts stay primary and canonical on the header, the sidebar and the navigation badge:
those are the numbers a user quotes, and they should remain the numbers a provider reported. An
optional weighted estimate may coexist, but only where there is room to qualify it, which today is
the detail of the `session total` row and of each agent row on `/usage`. If the owner proceeds, the
conditions that keep it honest are:

1. The estimate is one labelled line naming what it is and where its coefficients come from, not a
   second headline competing with the reported total.
2. It is suppressed when any field it needs is unreported, rather than computed with an assumed
   zero; the existing dash semantics apply unchanged.
3. It carries a mark distinct from `+`, and the legend distinguishes "lower bound because reporting
   is incomplete" from "estimate because pricing is assumed".
4. It is shown with the exact grouped figure, not the compact `12.3k` form, so a rounded number and
   a modelled number do not compound in one glance.
5. A money figure is ranked by what it actually is, not by where it came from. Documented billing
   data may be presented as billing. A cost field from a provider SDK may not: as of 2026-09-12 the
   Claude Agent SDK cost-tracking documentation describes `total_cost_usd` and `costUSD` as
   client-side estimates computed from bundled or configured price tables rather than authoritative
   billing (<https://code.claude.com/docs/en/agent-sdk/cost-tracking>, lines 83-88 at that date).
   Such a field is displayed as an estimated amount in US dollars, carries its provenance, which
   SDK and which price table produced it, and does not automatically outrank a local weighted
   estimate; two estimates computed from different price tables are two estimates. Billing,
   estimates and reported counts are never presented as the same kind of number.
6. Nothing is added to `/limits` unless a token budget actually exists there.

No screen was designed and no wording was drafted for this addendum; whether the estimate is worth
its coefficients at all depends on the parent's formula and price-basis analysis.

## Limits of this review

- Single-agent static reading of the source, the Git history and the committed reports in
  `ymp-evals/reports/`. No new runs, no product-provider calls, no instrumentation. The review run
  itself consumes model quota.
- The regressions in A2 are cited from `ymp-evals/reports/token-usage-verification.md`. A transient
  pseudo-terminal capture existed during the cycle under `/tmp`; it is not the record and is not
  relied on here. The corrected states are additionally asserted by `tests.rs:1842` and
  `tests.rs:1889`.
- Verification reports exist, but nothing in this repository measures review latency per change or
  records a merge conflict, so no claim about either is available at four commits. Section A3
  states what would make H7 measurable and when to look again.
- The engine half of H7 and every non-UI part of H6 were out of remit and are not judged here.
