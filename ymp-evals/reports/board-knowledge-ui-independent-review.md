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

---

# Follow-up review of `b6eb516`: both findings closed, reservation views accepted

Reviewer: independent, not the author. Date: 2026-09-13, 07:24 HKT (2026-09-12T23:24Z).
Subject: `b6eb516e45dab307b9647c8f863d7a9f787ca845`, read from a `git archive` snapshot. The
author's worktree was not modified.

**Verdict: ACCEPT for 0.4.0.** Score **9 / 10**.

- F1 and F2 above are closed: in the list row, in the record, and with the actual source paths.
- Every YMP-112 and YMP-114 interface criterion is complete.
- The new visibility of token reservations and of the captured policy for incomplete counts is
  complete for what the backend captures in 0.4.0.
- Four residuals remain, none of them blocking.

## Source and scope

- `b6eb516` merges author `2302229` with main `c21f93c`. Of its 150 files under `ymp-rust` and
  `ymp-bridges`, 123 are identical to both parents. Two come only from the author: `ymp-tui/src/views.rs`
  and `ymp-tui/src/tests.rs`. Twenty-five come only from main, all in `ymp-eval-driver`. None
  comes from anywhere else.
- Since the accepted `f6b58fa`, only `ab4697c` (the two findings) and `2302229` (reservation views)
  touch the interface: 2 files, 910 insertions, 95 deletions. The merges from main and the
  cherry-picked backend commit `db13110` change nothing in `ymp-tui`.
- `e9c0214` adds two commits that change only `ymp-evals/reports/ymp-118-reservation-views.md`.
- The 0.4.0 candidate `c3758aa` merges `b6eb516` with `685d1ca`. It differs from `b6eb516` only in
  `Cargo.toml` and `Cargo.lock`, and every differing line changes `version = "0.3.0"` to
  `"0.4.0"`. The `ymp-tui` tree `80cfb28a` and the `ymp-bridges` tree `43b1c4ff` are identical in
  `b6eb516`, `e9c0214` and `c3758aa`.
- `views.rs` hashes to `da48d9e5…` and `tests.rs` to `e04ccc42…`, as the author's report states.

## Checks reused and checks run

| Check | Where | Result |
| --- | --- | --- |
| author evidence bindings | 9 files named in `ymp-118-reservation-views.md` | 9 of 9 SHA-256 match |
| workspace tests at `b6eb516` | `/tmp/118-final/workspace.log`, `head.txt` reads `b6eb516` | 32 result sections (26 binaries, 6 doc-test), 419 passed, 0 failed, 2 ignored |
| `ymp-tui` in that log | same | 139 passed, 0 failed, 1 ignored |
| clippy and fmt in that run | `exit.txt`, `clippy.log`, `fmt.log` | exit 0; no warning line; empty format log |
| `ymp-tui` on my archive | `cargo test -p ymp-tui --lib` | 139 passed, 0 failed, 1 ignored |
| fmt on my archive | `cargo fmt --all --check` | exit 0 |

The workspace suite was not repeated. The parent's results for `c3758aa` were reported to this
review and were not hash-checked here; its interface and bridge bytes were verified identical.

## F1 closed: the row and the record read one outcome

- `recorded_outcome` now returns the row's word and the record's sentence together, and
  `decision_row` uses both. The separate right-hand match is gone. `bounded_outcome` now also
  marks an applied correction.
- On screen at 120x40, the plan fixture's list reads `✓ plan change committed · the runtime …
  committed` and `✗ plan change rejected · the runtime … rejected`.
- On screen at 80x24, the correction fixture's list reads `✓ retained knowledge corrected …
  entry replaced`. The record states `outcome what was retained was replaced`, `replaced retained
  entry c632fabc at version 8d5dca3b`, `replacement retained entry f08e50cf`, `authorised by
  acceptance f81bf255 under trusted contract c11e8d2f` and `corrected by policy
  ymp.bound-correction version 1`.
- The new order inside `recorded_outcome` cannot relabel an acceptance: only the
  contract-captured decision carries `links.acceptance_contract` (`ymp-storage/src/provenance.rs`),
  so acceptance decisions keep their grade words.

## F2 closed: a contract names its declared and replaced sources

On screen at 80x24, the correcting contract `c11e8d2f` states:

```
declared input inputs/observations-corrected.csv · captured at sha256 11a948d3
corrects       retained entry c632fabc at version 8d5dca3b
source change  inputs/observations.csv replaced by
               inputs/observations-corrected.csv
established by observation-value
```

It ends with "It also binds a correction…". Each value wraps under its label inside the record
box. The first run's contract names `inputs/observations.csv` with its own digest and no source
change; the test asserts that, and controls M4 to M6 below confirm it.

## Reservations and the captured policy for incomplete counts

**Semantics checked against the store, not only against the tests.**

- The store recomputes the budget from records on every read. `budget::snapshot` serves both
  `trace` and `session_budget`, so no stale stored figure or legacy field can reach the page.
- Admission requires reported total, plus allowances held by open turns, plus the turn's own
  allowance, to fit under the ceiling. Work other than review must also leave the protected
  review tokens. The ceiling record states the first rule; the review row states the second.
- `Stop` refuses only where both a ceiling and an allowance exist. The sentence "this policy had
  nothing to act on" for a session without a ceiling is therefore true.
- With a captured reserve, review protection is the reserve less review spend and live review
  allowances while review is owed. Without one, it is `invocation_tokens ×
  protected_review_invocations`. Both explanations read exactly those fields.
- Storage and runtime always write `strict_token_bound: false`.

**Captured versus current, in both directions, on screen at 120x40.** Each home is kept with a
configuration that holds the opposite policy and a 50000 ceiling, a 2000 allowance and a 7000
reserve. None of today's figures appears.

| Row | captured `Stop`, paused | captured `BoundedNative`, completed |
| --- | --- | --- |
| incomplete counts | `stop admitting` | `admit on reported` |
| token ceiling | `100000` | `100000` |
| turn allowance | `4000` | `4000` |
| review tokens | `10000`, from the captured reserve | `0`, as `4000 × 0 = 0` |
| strict bound | `not proved` | `not proved` |
| last stop | `unknown_usage` | none recorded |

**Partial and strict-bound honesty.**

- The `BoundedNative` ceiling record reads "By reported counts, 360 were spent and 1500 are held
  for turns still open, which leaves 98140. Not every count is complete, so what is truly left is
  not known: it is at most 98140." The arithmetic is 100000 − 360 − 1500 = 98140.
- The `Stop` ceiling record reads "No turn reported a count, so no spending is known…".
- Both strict bound records give the in-flight reason and the incomplete-count reason. The
  `BoundedNative` one adds "Admitting on reported counts does not change that."

**Protected review tokens.** A probe on a real mock run with no reserve and reviews still owed
read `8000`, explained as `4000 × 2 = 8000`. The store's `protected_review_invocations` was 2.

**Explicit and default reservations, on screen at 80x24.**

- The turn admitted through the store reads `token allowance 1500 · requested by this assignment,
  within the per-turn ceiling of 4000 the session captured`.
- The runtime's plan turn reads `token allowance 4000 · inherited: this assignment requested
  none, so it took the session's per-turn default`. Today's 2000 appears nowhere.

**Absent and legacy fields.**

- A capture without resource limits shows `token policy · not captured` and no token rows.
- A session without a ceiling shows `none` for ceiling, allowance and review tokens, and says the
  policy had nothing to act on.
- Token fields absent from an older capture deserialise to `Stop` and `None`, which is what the
  store enforces.
- An assignment without `token_reservation` reads as inherited, which matches the store's own
  `allowance()`.

Every walk compared a SHA-256 of the store dump and of `config.toml` before and after. All were
unchanged.

## Controls

Eighteen single inversions were applied, one at a time, to `views.rs` in a separate archive
(`/tmp/b6eb-mut`). Each was run against its named tests, then restored and verified by SHA-256.
All nine named tests pass unmutated. Every inversion failed.

| Inversion | Failing test |
| --- | --- |
| M1 the row reads the grade instead of the recorded outcome | plan outcome; correction row |
| M2 an applied correction gets no outcome marker | correction row |
| M3 the correction record names the replacement as the replaced entry | correction row |
| M4 declared inputs reduced to a count | contract sources |
| M5 the source replacement direction swapped | contract sources |
| M6 a digest looked up for a different input | contract sources |
| M7 token rows read today's configuration | reopened policy |
| M8 a captured `Stop` labelled as admitting on reported counts | reopened policy |
| M9 a captured `BoundedNative` labelled as a stop | reopened policy |
| M10 a reported remainder under incomplete counts read as known | reopened policy |
| M11 the strict bound loses the incomplete-count reason | reopened policy |
| M12 review protection multiplied by the configured, not the owed, count | reopened policy |
| M13 an inherited allowance read as requested | allowance |
| M14 allowances held by open turns ignored | allowance |
| M15 a policy without a ceiling reads as if it acted | no ceiling |
| M16 absent resource limits replaced by defaults | legacy capture |
| M17 the captured-name clause deleted (earlier control) | captured name |
| M18 a proposal outcome read from its stored status (earlier control) | board authority |

## Residuals, none blocking

1. **`0 past the bound` in the tokens observed record.** `observed_words` appends the overshoot
   whenever a ceiling exists and any count is known, including when it is zero. It calls the
   ceiling "the bound", on the page where this round reserved "strict bound" for a proved bound.
   Under incomplete counts the figure is by reported counts and is not qualified. Seen on screen:
   `at least 360 over 9 turn(s) · 6 reported nothing · 0 past the bound`. The helper predates this
   round, and the new `observed_row` reuses it. Fix: omit it at zero, say "past the ceiling", and
   add "by reported counts" where counts are incomplete.
2. **A latent strict-bound branch.** `strict_row` would read `proved` without looking at
   incomplete counts, while `SessionBudget::require_strict_token_bound` refuses a strict claim on
   partial counts. It is unreachable in 0.4.0 because the flag is always written false.
3. **No product path requests an allowance in 0.4.0.** The runtime writes `token_reservation:
   None`, so the requested case exists only through `Store::admit_invocation`. The author states
   this limit. It is also why the walked session shows 9 admitted turns against 8 session turns.
4. **Carried over from the `f6b58fa` review.** The availability sentences `correction proposed`,
   `source changed` and `source unavailable` are still asserted by no test and reached by no walk.

## Criteria for 0.4.0

- **YMP-112, existing views consistent with the new state.** The board views were accepted at
  `f6b58fa`, and F1 removes the last disagreement between the decisions list and its records.
  Complete.
- **YMP-112, the shared-interface follow-ups.** At `b6eb516` the labels still read `post to the
  team chat`, `read the team chat` and `read the shared board`. The captured-name regression is
  still discriminating (M17). Complete.
- **YMP-114, existing views consistent with the new state.** The current and superseded views
  were accepted at `f6b58fa`, and F2 names the declared and replaced sources. Complete.
- **Reservation-policy visibility.** Complete for every field the backend captures in 0.4.0, with
  residuals 1 to 3 above.
- The backend criteria of YMP-112 and YMP-114 were not judged here.

No provider was asked anything and no credential was read. The real application home, main, the
task register and the intent were not modified.

## Evidence

SHA-256 of the files this follow-up rests on, taken after the runs had finished:

```
abb0897f7bda7c8288a12da9ce491f1ee41d00d5087f69a6a8fabedcc6ad26ff  /tmp/b6eb-ymp
ddfc6b9563c2329c0f6dccee1d5fc892d17a451ad3ae14ccd69551d799c5e8ae  /tmp/b6eb-walk/manifest.json
ca93bd34ebe3d11dd9b33b45c710826710de3462fab8b6f8b56923df5dcb06d3  /tmp/b6eb-tui-tests.log
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  /tmp/b6eb-fmt.log
112dce76f8a5a0c0b13ef2d6e9a3dcadd5f386aac1fbb9dfcb04ed95d8b832e4  /tmp/b6eb-mutate.py
8243ab207d0c478058f9f0a572870d2ae56613e2b354a565c13ceb90172ea5fe  /tmp/b6eb-mutations.json
e45f07c5fae2602e84982239d7dad28699ebabd31262784b48fe095be3320413  /tmp/b6eb-mutations.log
3dabc8648062c71c840607e836b6995979368134282316c0ed8b5b3a14be766d  /tmp/board-walk.py
9a36fdf216c997933a3e88384ad0c9dff74a24ecc79983a5eb08a6cec83db797  /tmp/b6eb-stopped120.txt
68e387507104fb537e1efa29fa922c644dfd05db26e53ba86537e4b16a111bd0  /tmp/b6eb-bounded120.txt
e53b13ab4d17f3c1cb65ba77dd2b1d63ac7f4f0618b011733736232aa0602981  /tmp/b6eb-boarddec120.txt
1514d508f13432b28436aa9f3a01d16dab08c7f7ec8b94d7fbb1ba300a9c39a2  /tmp/b6eb-assign120.txt
29d20c6cdeddc1ada7d59e562964d008b50bcc9e38a13b0aabe974ca4920b5a0  /tmp/b6eb-know80.txt
6b024b90c6152eb97142dbd3750087b26f6a65f3f71623f59cd106697ee5e484  /tmp/b6eb-bounded80.txt
6393e5515c46469705f6f498d16d34fc4dc0c1cae39c7bd0812dc8aa57c2c1de  /tmp/b6eb-assign80.txt
```

The binary is the one built from the archive of `b6eb516`. The fixtures were rebuilt through the
real runtime by the author's ignored keeper, `keep_the_fixtures_an_interface_walk_reads`.
