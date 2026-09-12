# YMP-118 R2 and YMP-127 native-name UI: independent review

13/09 04:35 HKT, 2026-09-13 — **ACCEPT, 9/10** on the final composition
`762b636a53dfb751891345ade50797d9ceb4fffb`, after an independent R2 pass over the author
candidate `08fcbf68127576b48289344f51c8ddf77323be6f`.

Code 9/10; evidence 9/10; consumer experience 9/10. All six findings of the core review are
closed. All three installed installations were read and displayed, including the Claude
installation the author could not reach. Two defects remain, both minor: two coordination
labels contradict the tools they name, and the primary clause of the new shared naming rule has
no negative control. Neither misstates a record and neither blocks the merge.

Reviewed as a fresh reviewer. No source was modified in either worktree. Mutations ran on a
separate `git archive` snapshot under `/tmp/fin-mutate`, each reverted and compared byte for
byte. Every reading used a temporary application home; `~/.ymp2/config.toml` was last written
at 04:02:44 HKT by the parent's supported refresh, before my first scan at 04:04, and nothing
here touched it.

## The two scopes, kept apart

**Original candidate `08fcbf6`.** The YMP-118 core corrections and the first native catalog
composition. Reviewed in full: required checks, F1 to F6 verification, three-installation read,
consumer walk.

**Final composition `762b636`.** Adds `1138725` (two coordination labels for the accepted
backend), `79702e9` (the shared naming rule, equivalent to author `8293080`) and `762b636`
itself (pre-scan progress and corrected migration wording, equivalent to author `ba08ea9`).
Reviewed for those three changes, re-verified for history, usage, selection and negative
controls.

## Composition, validated independently

| Claim | How it was checked | Result |
| --- | --- | --- |
| Head is the stated source | `git rev-parse HEAD` | `762b636a53df…`, working tree clean |
| Rust and bridge bytes equal author `ba08ea9` | `git rev-parse HEAD:ymp-rust` and `HEAD:ymp-bridges` against `ba08ea9` | both tree hashes identical: `c2d32de1…` and `43b1c4ff…` |
| The 123-file manifest describes this tree | recomputed `sha256` of every manifest entry | 123 files checked, 0 mismatches |
| The manifest itself is the one recorded | `shasum -a 256 source-files.json` | `b9b42142…`, matches the recorded value |
| The check logs are the ones recorded | `shasum -a 256` on six logs | every one matches `native-ui-composition-checks.json` |
| Composed backends are ancestors | `git merge-base --is-ancestor` | `08fcbf6`, `1138725`, `f0c214a`, `81b6208` all ancestors |
| The lock correction is not conflated | same | `5415392` is not an ancestor of this head |

The tree-hash comparison is stronger than a file listing: one hash per directory covers all 121
tracked files under `ymp-rust` and `ymp-bridges` at once.

## Checks

The parent's nine checks are hash-bound and were not duplicated. On `08fcbf6` I ran the required
set once myself.

| Check | `08fcbf6`, run by this review | `762b636`, parent log verified here |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no diagnostics | exit 0, no diagnostics |
| `cargo test --workspace` | exit 0, 352 passed over 26 binaries, 119 in `ymp-tui` | exit 0, 375 passed over 28 binaries, 121 in `ymp-tui` |
| ten explicit native fixtures | — | exit 0, 10 passed |
| Claude bridge check, test, build | built here, `tsc` exit 0 | exit 0 on all three |
| executable build | exit 0 | exit 0 |

The `08fcbf6` figures match the author's round-five table exactly.

## The Claude installation, read here

The author could not read one of the three installations because this worktree had no built
bridge. I built it: `node_modules` copied from the main checkout, whose `package.json` and
`package-lock.json` are byte-identical to this tree's, then `npm run build`, exit 0. Both
artefacts are ignored by `.gitignore`, so the tree stayed clean.

With the bridge present, all three installations answered, each by its own metadata interface
and with no prompt sent:

| Installation | Method | Offerings |
| --- | --- | --- |
| codex | `model/list` | 8 |
| claude | `Claude Query.supportedModels()` | 5 |
| glm | `session/new` | 6 |

19 profiles, 16 added, 3 legacy identifiers preserved. These are the parent's numbers, reached
independently. `refresh_catalog` builds its request with `prompt: String::new()`,
`read_only: true` and purpose `catalog_scan`, so the scan is metadata only by construction.

## What the interface shows

The Claude record at 80x24, which is the reading the brief required:

```
name           Default (recommended)
configured as  Claude
model          default
resolved to    claude-opus-5[1m]
name from      the installation, read by Claude Query.supportedModels() at 2026-09-12T20:04:19.238Z
effort         low, medium, high, xhigh, max · no default is named
```

The provider's own label is printed verbatim and the resolved identifier lives in its own field,
never as the name. Control labels come from the installation too. GLM's offering reads
`Thinking  Off (none), High (high), Max (max) · max by default`, using the returned
`display_name` and `value_names` rather than the internal id `thought_level`. An offering whose
catalog is silent reads `controls unknown · the catalog does not say which it offers`, and a
profile with no reading reads `Unresolved native model` with
`name from  nowhere yet`. Codex carries `chosen in the picker as gpt-6-astra · a selector, not a
model to send`.

**Selection.** Pressing `t` on GLM-5.2 added it to the team, and the row, the right panel and
the team page all moved together, each under the native name. The team went from two members to
three.

**Reading versus scanning.** `r` re-reads what is stored and advances the inspection time
without asking a provider anything. `R` spawns the scan as its own task: the status line reads
`Asking the installations what they offer` while the page keeps painting, and the completion
notice reads `Read 3 of 3 installation(s): 19 offering(s) stored, 16 actor(s) added, 3 existing
actor(s) now resolved to a model the installation named. No configured name was changed, and
nothing was asked of a model.` The scan is refused while a run is active and refused for a
disabled installation, each with its reason.

## History is never relabelled

The strongest check in this review. A scripted mock session ran to completion, then the two
profiles were renamed to `RELABELLED-One-Now` and `RELABELLED-Two-Now` and their models changed.
Reopening the session in the final composition:

| Surface | Before a session is loaded | With the recorded session loaded |
| --- | --- | --- |
| opening summary | `RELABELLED-One-Now, RELAB…` | — |
| right panel, team | `RELABELLED-One-Now` | `One As Captured` |
| right panel, tokens | — | `One As Captured` |
| transcript authors | — | `Two As Captured review · accepted` |
| `/team` members of this session | — | `One As Captured · mock-alpha  3 turn(s) here` |
| `/team` available on this machine | — | `RELABELLED-One-Now · mock  eligible` |
| member record | — | `name One As Captured`, `captured as mock-alpha · the identity recorded when the turn was admitted` |

Both lists coexist on one page with the right label for each. `/usage` names rows by the
captured identity and its record reads `ran as mock-alpha`, the model the turns actually ran
with, with `invocations 3 · reported 0 of 3` so unreported coverage stays unreported.

## Findings

### F1 — Minor. Two coordination labels contradict the tools they name.

`views.rs` labels `TeamOperation::TeamRead` as `read the board` and, from `1138725`,
`TeamOperation::BoardRead` as `read the plan`. The backend says the opposite: `team_read` is
"Read the shared team chat, including peer findings", and `board_read` is "Read the shared
board: exact plan/task versions, pending proposals, responsibility commitments and current
team". A reader of a grant record on `/assignments` or `/team` therefore sees the chat
permission called the board and the board permission called the plan.

The `TeamRead` label predates this work; adding `BoardRead` beside it is what makes the pair
mutually confusing. Nothing about authority is misstated, only the names.

Fix: label `TeamRead` as `read the team chat` and `BoardRead` as `read the shared board`.

### F2 — Minor, test coverage. The primary clause of the new naming rule has no negative control.

`actor_name` answers in four steps, and the first is the rule the commit exists for: an identity
a record captured names the actor as its turns ran. Deleting that clause leaves **all 121
`ymp-tui` tests passing**. The behaviour it guards is nonetheless load-bearing: with the clause
removed I rebuilt the executable and reopened the same recorded session, and the right panel
listed `RELABELLED-One-Now` and `RELABELLED-Two-Now` where the unmutated build lists `One As
Captured` and `Two As Captured`. That is exactly the disagreement the commit message says this
work exists to stop.

The reason the suite misses it is that its fixtures capture an identity whose `name` equals the
configured name, so the first and second clauses return the same string. The existing test
`turns_recorded_without_a_captured_identity_keep_the_name_they_ran_under` covers the second
clause only, and its own mutation does fail it.

Fix: add a test where the captured name differs from the current configured name, and assert the
whole window shows the captured one.

## Negative controls, run here

Four inversions on the separate snapshot, each restored and compared byte for byte.

| Inversion | Result |
| --- | --- |
| `actor_name` drops the clause for turns without a captured identity | `turns_recorded_without_a_captured_identity_keep_the_name_they_ran_under` failed, exit 101 |
| `actor_name` drops the captured-identity clause | **all 121 tests passed**, see F2 |
| `recorded_outcome` reads the grade field again for an allocation | `a_membership_decision_says_what_it_changed_and_what_it_reserved` and `a_record_that_carries_its_own_outcome_never_reads_as_ungraded` failed |
| `header_row` defers to the row painter again | `a_header_gives_up_its_summary_before_its_own_name`, `a_page_keeps_its_name_and_command_at_every_supported_size` and `the_status_line_keeps_what_the_window_is_doing_at_every_supported_size` failed |

## The six core findings, closed

| Finding | How it is closed at `08fcbf6`, verified here |
| --- | --- |
| F1, an opened record contradicted its row | `recorded_outcome` names the outcome the record carries; guarded by two tests |
| F2, the Limits page lost its title at 80 columns | `text::header_row` keeps the name and truncates the summary; the summary is also short enough to fit; guarded by three tests |
| F3, the requested value disappeared once something was sent | a setting whose requested and sent values differ now names both, with the three states still distinct |
| F4, `/reputation` asserted a credit it had not read | the field reports rather than asserts |
| F5, the module comment overstated its scope | scoped to the snapshots and naming the six pages that read during their own build |
| F6, the workspace count | annotated in place; the review's 290 at `7038a4c` is recorded as correct |

The two findings I raised against `08fcbf6` in this round are closed by `762b636`. The silent
cold start now announces what it is about to do, how many installations it will ask and the
per-installation deadline. The migration notice no longer claims a rename and states that no
configured name was changed.

## Known limits of this review

- The startup scan still blocks the window for about 11.5 seconds on a fresh configuration. The
  correction makes it say so rather than making it asynchronous, and the commit message is
  explicit about that. The interface already has an asynchronous path; the command-line startup
  does not use it. Recorded as an observation, not a finding.
- The ten explicit native fixtures, the bridge tests and the 375-test workspace run were read
  from the parent's hash-bound logs rather than re-run, as instructed.
- Team selection and execution assertions used scripted mock fixtures. No model was asked
  anything at any effort level, in this review or in the scans.
- Two of my own harness errors were found and discarded rather than reported: single letters on a
  page are page actions, so typing `/chat` on the profiles page toggled team membership and
  produced an apparent row-versus-record disagreement, and copying a project directory to a new
  path broke its session binding. Neither is an application defect.
- The walk's screen model is a `pyte` emulator. Every reading above was taken with an incremental
  UTF-8 decoder and contains no replacement characters.
- YMP-112 board proposals and YMP-114 knowledge inspection views are not part of this
  composition and were not reviewed.

## R3, bounded follow-up: both findings closed

13/09 04:52 HKT, 2026-09-13 — **ACCEPT, both R2 findings closed** on
`f4ad62c900263a26cad3365e3d1f3291bfb90bf8`, whose parent is the author source
`ba08ea9032a20f1293855fd01b720d1950ba9cd5`. Two files, 94 insertions and 5 deletions.

The author tree was never touched. The commit was read from the shared object store through my
own review worktree, and every control ran on a `git archive` snapshot at `/tmp/f4-snap`, each
mutation reverted and confirmed by `sha256` (`4b6da516c541ab0d…` before and after).

### F1, the permission labels

Closed, and wider than the finding. I had reported two labels; the author found a third with the
same defect. All seven coordination labels now match the tool each one names.

| Operation | Label at `f4ad62c` | Tool description in the same tree |
| --- | --- | --- |
| `TeamPost` | post to the team chat | "Post a concise finding or question to the shared team chat" |
| `TeamRead` | read the team chat | "Read the shared team chat, including peer findings" |
| `TasksList` | list tasks | "Inspect tasks, assignments, dependencies and outcomes" |
| `TaskPropose` | propose a task | "Propose a durable change after board_read" |
| `BoardRead` | read the shared board | "Read the shared board: exact plan/task versions, pending proposals…" |
| `MemorySearch` | search memory | "Find supported project knowledge and shared check experience" |
| `MemoryPropose` | propose memory | "Retain an unconfirmed lesson candidate with bound origin" |

`TeamPost` was mine to have caught and I did not; the correction is right.

### F2, the missing negative control

Closed. `a_finished_session_is_named_by_the_identity_its_turns_captured` records a turn whose
captured identity is `GPT-5.6-Terra` while the configuration still reads the old name, then
asserts the right panel, the assignment row and the team page all name the session by what its
turn captured. Two guard assertions keep the fixture honest: it fails if the captured name ever
equals the configured one, and it fails if the configuration no longer carries the old name. That
is what stops this test from decaying into the tautology the previous fixtures had become.

### Controls run here

| Inversion | Result |
| --- | --- |
| `actor_name` drops the captured-identity clause, the mutation that survived all 121 tests at `762b636` | `a_finished_session_is_named_by_the_identity_its_turns_captured` **FAILED**, 121 passed |
| the three labels revert to `post to the board`, `read the board`, `read the plan` | `an_assignment_shows_what_was_sent_and_never_claims_an_unreported_setting_applied` **FAILED**, 121 passed |

| Check on the snapshot | Result |
| --- | --- |
| `cargo test -p ymp-tui` | exit 0, **122 passed**, 0 failed |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy -p ymp-tui --all-targets -- -D warnings` | exit 0, no diagnostics |

The count moved from 121 to 122, which is the one test this commit adds. No provider was scanned
and no model was asked anything for this round; the accepted native-name behaviour from R2 stands
unchanged and was not re-derived. No real application home was read or written.

### Remaining

Nothing from R2 is open. The startup scan still blocks for about 11.5 seconds on a fresh
configuration while saying what it is doing, which R2 recorded as an observation rather than a
finding and which this commit does not touch.
