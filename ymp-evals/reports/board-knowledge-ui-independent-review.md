# Independent review: the shared plan and retained knowledge on the existing pages (YMP-112 / YMP-114)

Reviewer: independent, not the author. Date: 2026-09-13, 06:01 HKT (2026-09-12T22:01Z).
Subject: composition `f6b58fa8037bb5bcac85d740c97df316883200ea` in `integrate_board_ui`.

**Verdict: ACCEPT.** Score **9 / 10**. Two findings, both in what a reader sees, neither a
blocker for the merge. Both are listed with the exact place to change and the exact test that
is missing.

## What was reviewed, and how its two sources were separated

The composition is claimed to be author `28c4299` for the interface and accepted main `2c6bbab`
for the backend. That claim was checked file by file rather than accepted. Of the 123 files
`f6b58fa` tracks under `ymp-rust` and `ymp-bridges`:

| Origin | Files |
| --- | --- |
| identical to both `28c4299` and `2c6bbab` | 113 |
| author `28c4299` only | 5 |
| accepted main `2c6bbab` only | 5 |
| neither | 0 |

The five author files are the whole of the interface change: `ymp-tui/src/provenance.rs`,
`state.rs`, `tests.rs`, `text.rs`, `views.rs`. The five main files are the backend the parent
composed in: `ymp-cli/tests/long_metadata_home.rs`, `ymp-runtime/Cargo.toml`,
`ymp-runtime/src/mcp.rs`, `ymp-runtime/src/mcp/socket.rs`, `ymp-runtime/tests/concurrency.rs`.
No file came from anywhere else. `28c4299`, `2c6bbab`, `f4ad62c`, `762b636` and `ba08ea9` are
all ancestors of the head.

Measured against the interface head this review already accepted (`f4ad62c`), the delta is
5 files, 1899 insertions, 109 deletions, all inside `ymp-tui`. There is no dashboard page and
no new backend semantics in the author's five files.

The three source hashes the author published for this head reproduce exactly from a clean
archive of `f6b58fa`:

```
1f73ef7962e5036ab39a509013561dbbbab835a95165c31371a22a6f65de8e2a  crates/ymp-tui/src/views.rs
cc25fe6e8befea56cf00a9d45a7cbd47d1f50d640c1d173f04ad05199d2286e9  crates/ymp-tui/src/provenance.rs
2186fb7a50a5dc4227aba2c0910eff50d93d2cf29f29d70e8446a632dc6c4d5a  crates/ymp-tui/src/tests.rs
```

## The parent's checks, independently re-bound

`/tmp/ymp-board-ui-final/checks.json` reports `fmt` 0, `clippy` 0, `workspace` 0, `build` 0.
All 125 entries of `source-files.json` were re-hashed against the worktree: 125 match, 0 differ,
0 missing, so those results are bound to this exact source. Reading the raw workspace log rather
than the summary:

| Measure | Value |
| --- | --- |
| test binaries run | 23 (plus 6 empty doc-test sections) |
| passed | 393 |
| failed | 0 |
| ignored | 2 |
| `ymp-tui` alone | 133 passed, 1 ignored |

The `ymp-tui` figure matches the author's 133 exactly. The workspace figure is six higher than
the 387 the author reported, which is expected: the author reported at their own head, and this
composition adds the accepted backend. Five of the six are directly attributable — the new
`long_metadata_home.rs` carries one test and the new `mcp/socket.rs` carries four. The sixth was
not attributed to a single file, and the author's head was not re-run for this review.

## The shared plan, in the running binary

Both fixtures were rebuilt through the real engine and team API by the author's own ignored
keeper, `keep_the_fixtures_an_interface_walk_reads`, so the directories the runs used are the
ones the walk reads and every session-to-project binding stays valid. The built binary was then
driven over a pseudo-terminal at 120x40 and 80x24, starting no run and sending no prompt.

What the plan fixture shows, read off the screen:

- The tasks page lists four tasks and then a `PROPOSALS TO CHANGE THIS PLAN` heading with three
  rows: `give Gamma note to one · committed`, `give Gamma note to one · rejected`,
  `give Delta note to one · committed`.
- Delta note carries `task version fdec1c9d`, `responsibility one · small at low · against task
  version f5ca0353`, `committed by proposal 0fe8795a from one`, and `put off commitment_busy ·
  the agent responsible for it was already working, so it keeps the commitment`. The captured
  model and effort are the ones the commitment was made under, and the task version the
  commitment names is deliberately the pre-commitment one, because committing changes the task
  version.
- The committed proposal's record reads `decided committed · the plan now carries it`,
  `decided by ymp.ordered-board 1 · the runtime, not an agent`, `resulting plan 1a9a9fad · the
  version the plan took on`, and `responsibility one · small at low · against task version
  4ac3fa58`.
- The rejected proposal's record reads `rejected · the plan was not changed`, `resulting plan
  1a9a9fad · unchanged by this proposal`, and the runtime's own words
  `stale_task: proposal describes an older task or commitment`. It carries no responsibility
  line, which is correct: a refused proposal created none.
- The team page and the tasks page agree. Member `one` reads `responsible for Delta note · small
  at low · committed by proposal 0fe8795a`; member `two` reads `no task on the plan is committed
  to this member`.

The decision is the authority in source as well as on screen. `proposal_outcome`
(`ymp-tui/src/views.rs:4170`) reads `Records::board_decision` first and falls back to the stored
status only to say that the reason was not read. `Records::pending_proposals`
(`provenance.rs`) requires both a pending status and the absence of a decision, so the
subtitle's count of waiting proposals cannot disagree with the rows. Two deferrals exist in the
fixture, one per task, and each appears once under its own task; the general wait list does not
repeat the one already named as put off.

## The correction, in the running binary

The knowledge fixture is two runs of the real runtime over
`ymp-evals/fixtures/universal/inputs`. Read off the screen:

- The superseded entry states `superseded: an accepted correction replaced this entry…`,
  `replaced by Record observation (82cb7a5e)`, `this entry is the one that was corrected ·
  82cb7a5e replaced 41c27a96 at version 857c49fd`, `authorised by acceptance baa52cee under
  trusted contract c17cec30`, `corrected by policy ymp.bound-correction version 1`, and the
  paragraph `Both are kept. A correction does not delete what it corrects…`. Its own evidence is
  `acceptance 75bd6034 · result f085943a version 1`, and its text ends
  `Artifact: claim-95.json (sha256 2f70268d…)` with the excerpt
  `{"row":"O04","site":"Hill","value":95,"week":"2026-W36"}`.
- The replacement states `current`, `replaces Record observation (41c27a96)`, `this entry is the
  correction`, the same acceptance, contract and policy, and its own evidence `acceptance
  baa52cee · result 4fe62214 version 1`, ending `Artifact: claim-60.json (sha256 8056cd46…)`
  with `{"row":"O04","site":"Hill","value":60,"week":"2026-W36"}`.
- Both sides are listed at once, each keeps its own acceptance and result, and the two
  acceptances are different records. That is the sense in which both sources are retained, and
  it holds.
- Scope is stated twice and consistently: the page answers under `dataset is observations, site
  is Hill, week is 2026-W36`, and each entry repeats what it requires. The subtitle
  `8 recorded · 3 current · 3 supported as context` matches the eight rows.

`entry_state` takes the store's standing when there is one and only falls back to a heuristic
when the store gave no answer, and `availability_words` / `availability_sentence` cover all nine
`KnowledgeAvailability` variants exhaustively.

## Width and read-only behaviour

At 80x24 no line of either fixture overflows the frame: fields wrap under their label, long ids
break across lines, the subtitle truncates on the right while the page title and command stay,
and the `▼ N more` affordance appears on both the list and the sidebar. The record overlay is
78 columns inside an 80-column terminal and reports what is still below it. At 120x40 the same
content is laid out without truncation. Nothing on either page starts a run; the walk sent no
prompt and read no credential.

## Findings

### F1 — a plan change and a correction read as "recorded without an outcome" in the decisions list

On `/decisions` the row for a `board_committed`, `board_rejected` or `retained knowledge
corrected` record shows `recorded without an outcome` on the right, while the same record's own
`outcome` field says `the plan took the change on`, `the plan was left unchanged` or `what was
retained was replaced`. Captured from the running binary and reproduced at unit level:

```
PROBE row  board_committed: "recorded without an outcome"
PROBE rec  board_committed: outcome the plan took the change on
PROBE row  board_rejected:  "recorded without an outcome"
PROBE rec  board_rejected:  outcome the plan was left unchanged
PROBE row  knowledge_correction: "recorded without an outcome"
PROBE rec  knowledge_correction: outcome present = true
```

The cause is local. `decision_row` (`views.rs:5006`) builds the right-hand word from a match on
`links.allocation`, `links.resource_allocation`, `links.workspace_wait`,
`links.acceptance_contract` and the three `workspace_access` kinds, and falls through to
`acceptance.word()` for everything else. `links.board` and `links.knowledge_correction` have no
arm, so they take the fallback, which is the grade field that was never written for them. Two
things show this is not the intent: the doc comment above `recorded_outcome` (`views.rs:5115`)
says in so many words that reading the grade field "would report them as decisions recorded
without an outcome while the row beside them says what they did", and `bounded_outcome`
(`views.rs:5161`) already includes `links.board`, which is why the row's marker is a correct
`✓` or `✗` next to the wrong words.

The user-visible effect is that the list contradicts the record it opens, and it contradicts the
tasks page, where the same proposal reads `committed` or `rejected`.

Why it survived: `a_decision_that_changed_the_plan_reads_as_its_own_outcome` (`tests.rs:5599`)
asserts only on `detail_of_key`. No test reads `right_of_key` for either kind.

Fix: add a `links.board` arm and a `links.knowledge_correction` arm to the right-hand match in
`decision_row`, mirroring `recorded_outcome`, and extend the existing test with `right_of_key`
assertions for both.

### F2 — the declared source of a claim is never named, including on the record that carries it

The criterion the interface prints for both sides of the correction reads "The O04 claim for
Hill in 2026-W36 **matches the declared source**", and the declared source is not named anywhere
in the interface. The captured contract record, which the interface does display, shows:

```
artifacts named claim-60.json
inputs recorded 1 file(s) by digest
```

Artifacts are named by path; inputs are reduced to a count. Probed directly, the contract detail
for both the 95 and the 60 contract contains no input path at all, and never says it is a
correction.

The author's own limits for this round say the replaced inputs are "named in the acceptance
contract, not in the correction record". That is true of the storage, but the acceptance
contract is itself on screen, and there it says only how many files there were. The data is
present in the record the page already renders: `AcceptanceContract.inputs` is a `Vec<PathBuf>`,
and `AcceptanceContract.knowledge_correction.source_replacement` carries `previous_input` and
`replacement_input` explicitly. So a reader can see that 95 became 60 and that a contract
authorised it, but not that the replacement read a different declared file, which is the fact
that makes the change a correction rather than a contradiction.

Fix: in `contract_detail` (`views.rs`), name the input paths the way artifacts are already
named, and add one line for `contract.knowledge_correction.source_replacement` when it is
present, saying which input replaced which. This is presentation of a record the page already
holds; it needs no backend change.

Secondary observation in the same area, offered as context rather than as a separate finding:
the `retained knowledge corrected` decision record names the result, the files and the task, but
not the entry it replaced or the replacement. That chain exists only on the memory page.

## Negative controls

The five controls the author listed for this round were re-run against a `git archive` snapshot
of `f6b58fa` at `/tmp/board-mut`, never against the reviewed worktree. Each inversion was
applied alone and the source restored and verified with `shasum -a 256 -c` afterwards.

| Inversion | Named test | Result |
| --- | --- | --- |
| `proposal_outcome` reads the stored status instead of the decision | `the_decision_and_not_the_stored_status_says_what_became_of_a_proposal` | FAILED |
| `board_task_lines` drops the deferral loop | `a_task_says_who_is_responsible_for_it_and_what_was_put_off` | FAILED |
| `entry_state` ignores the standing the store answered with | `both_sides_of_a_correction_are_kept_and_each_says_which_it_is` | FAILED |
| the memory page asks with an empty scope | `both_sides_of_a_correction_are_kept_and_each_says_which_it_is` | FAILED |
| the first clause of the naming rule is deleted | `a_finished_session_is_named_by_the_identity_its_turns_captured` | FAILED |

All four named tests pass unmutated on the same snapshot. The last row is the control from the
previous native-name round; it is still discriminating at this head, so the captured-identity
naming rule has not regressed under the board and knowledge work.

Two probe tests of my own were appended to the snapshot's `tests.rs`, run, and removed; they
produced the F1 and F2 evidence above. The three reviewed source files hash-verified identical
before and after every mutation and every probe.

## Limitations of this review

- A commitment is kept by the storage only while a task is ready, so a running or accepted task
  shows no responsibility. That is backend behaviour, it is documented in
  `assignment-and-confirmation-views.md`, and it was not treated as a UI defect. One wording
  consequence is worth noting without raising it to a finding: on an accepted task the page says
  "nobody holds this task; the runtime picks an executor at the next work boundary", and the
  second clause is a prediction that will not come true for a task that is already finished.
- Three of the nine `KnowledgeAvailability` sentences — `correction proposed`, `source changed`
  and `source unavailable` — are asserted by no test and were not reached by the walk, although
  the storage can produce all nine. The match is exhaustive so nothing is unhandled, but those
  three sentences have never been read in a consumer.
- The `board_unreadable` path is covered by a test that injects the failure; it cannot be
  reached from a walk without corrupting a store, and it was not reached here.
- The runs behind both fixtures were started by the author's harness, because a scripted
  execution backend cannot be installed from the command line. The walk itself started no run.
- The author's 387-test figure was not reproduced at their own head; only this composition was
  measured.
- No provider was probed, no model was asked anything, no credential was read, and
  `~/.ymp2` was not modified. The reviewed worktree is unmodified apart from this report.

## Evidence

| What | Where |
| --- | --- |
| fixtures rebuilt through the real runtime | `YMP_WALK_FIXTURE=/tmp/board-rev cargo test -p ymp-tui --lib keep_the_fixtures_an_interface_walk_reads -- --ignored`, exit 0 |
| plan fixture | home `…/T/.tmpmggEOL`, project `…/T/.tmpBDzglr`, session `9006b66a`, status `blocked` |
| correction fixture | home `…/T/.tmp52LS3h`, project `…/T/.tmp4FQZQF/project`, entries `41c27a96` → `82cb7a5e` |
| walks at 120x40 | `/tmp/board120.txt`, `/tmp/board-p1.txt`, `/tmp/board-p2.txt`, `/tmp/board-dec2.txt`, `/tmp/board-team2.txt`, `/tmp/know120.txt`, `/tmp/know-e8.txt`, `/tmp/know-ct.txt`, `/tmp/know-corr3.txt` |
| walks at 80x24 | `/tmp/board80.txt`, `/tmp/know80.txt` |
| mutation snapshot and restore proof | `/tmp/board-mut`, `/tmp/board-mut-baseline.sha` |
| parent checks re-bound to source | `/tmp/ymp-board-ui-final/{checks.json,workspace.log,source-files.json}` |
