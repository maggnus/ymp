# YMP-118: review follow-ups and token reservation visibility

Author: the delegated interface engineer. Date: 2026-09-13 (HKT).
Candidate: branch `ymp/task-118-reservation-views`. This report is committed on top of `b6eb516`.

## Sources

- The reviewed interface source `28c4299` and the reviewed composition `f6b58fa` are unchanged.
  Both are ancestors of the candidate.
- Composition steps:
  - accepted runtime reservations `fcbe5add`, merged as `4b3936b`;
  - the accepted unknown-usage API `d31d49f`, applied alone as `db13110` with `cherry-pick -x`
    (merging `d31d49f` itself would have brought 44 files of the then-pending driver);
  - accepted main `3c5013f`, merged as `0df6d71`, then `c21f93c`, merged as `b6eb516`.
- The candidate differs from main `c21f93c` in exactly four files: `ymp-tui/src/views.rs`,
  `ymp-tui/src/tests.rs`, `ymp-docs/guides/interface.md` and
  `ymp-docs/architecture/assignment-and-confirmation-views.md`. No core, storage, runtime,
  provider or policy file changes. `provenance.rs` is byte-identical to `f6b58fa`.
- Commits of this round:
  - `ab4697c8dd124793b8db5f9e0c93f89f4e8c261f`: the two review follow-ups;
  - `2302229243087a8cb11dcd4dfaaac0998163e3ea`: reservation and policy visibility;
  - `b6eb516e45dab307b9647c8f863d7a9f787ca845`: merge of main
    `c21f93c2d7101727854b47294cebb0a483d6aefe`, and the source under review;
  - `c01d2af9e9a1a515ab94af6663486b18dd7cbd1b` and the commit that finalises this report, which
    change only this file.
- Earlier composition commits: `4b3936bf817c65e2bac015028b595f6502601fea` (merge of `fcbe5add`),
  `db131109f4ca6acf7ea45eb598860d778562c19a` (cherry-pick of
  `d31d49f1e28e498fa85bb3d0e6a4287cbad8c0d2`) and `0df6d717b33709c1d664edaeeea71635bb78a75f`
  (merge of main `3c5013f`).
- SHA-256 of the changed files, read from the git objects of `b6eb516`:

  ```
  da48d9e5bfcf65bd3a9bab016dd8d914cdfcef6d4838af43be258090a10c4aae  ymp-rust/crates/ymp-tui/src/views.rs
  e04ccc4285d573ed7c046545252fdd7038b256193923f0cb27e73256321f35d7  ymp-rust/crates/ymp-tui/src/tests.rs
  51a767f5926f49c790e27ed4a77fcc3c880553574d3d870b2e1140d1cc23737a  ymp-docs/guides/interface.md
  cb33c8a99bc4156161bba8c7522df14a91ed49ae042806da14d8a809021e9328  ymp-docs/architecture/assignment-and-confirmation-views.md
  cc25fe6e8befea56cf00a9d45a7cbd47d1f50d640c1d173f04ad05199d2286e9  ymp-rust/crates/ymp-tui/src/provenance.rs (unchanged)
  ```

## F1: a decision's row and its record read one outcome

`recorded_outcome` returns both the row's word and the record's sentence, and `decision_row` uses
it for both. The separate right-hand match, which had no arm for the shared plan or a correction,
is removed. A committed plan change now reads `committed`, a rejected one `rejected`, and an
applied correction `entry replaced`, in the row and in the record alike. A correction is marked
from its own record, because the store writes one only once it has been applied. Its record also
names the entry it replaced, the replacement, the acceptance and contract behind it, and the
policy.

## F2: a contract names its declared inputs and the replaced source

`contract_detail` names each declared input with the digest it was captured at. For a correction
it also names the corrected entry, which declared source replaced which, and the criteria that
establish it. `field` now wraps a value in the room left beside a label longer than the usual
column, so a long label no longer pushes a line past its pane.

## Reservations and the captured policy for incomplete counts

The limits page reads the captured `ResourceLimits` and the store's `SessionBudget`, never the
configuration:

| Row | Field | What the record says |
| --- | --- | --- |
| token ceiling | `observed_tokens` | what reported counts leave, and "at most" that where coverage is partial |
| turn allowance | `invocation_tokens` | the allowance a turn inherits and the most one may request |
| review tokens | `protected_review_tokens` | from a captured reserve, or `invocation_tokens × protected_review_invocations` |
| incomplete counts | `unknown_usage` | `stop admitting` or `admit on reported` |
| strict bound | `strict_token_bound` | `not proved`, with the reasons that apply |

Every assignment names its token allowance: requested by the assignment (`token_reservation`),
inherited from the captured per-turn default, or none where no ceiling was captured.

Text that would have been false under `BoundedNative` has been corrected:

- `WHAT_LIMITS_ARE` said "nothing works around it". It now says that a ceiling bounds what is
  admitted rather than everything spent, and that going on without counts leaves a reported
  remainder, not a known one.
- The `tokens observed` detail said the figure was "as the budget counted it".
- The former `token bound` row read the unproved strict bound as a per-turn requirement. It is
  now the `strict bound` row.

`HOW_WORK_IS_ASSIGNED` gains the allowance sentence and keeps "unknown rather than zero".

## Tests and controls

New or extended tests, all reading records the real runtime wrote against the offline mock:

- `a_decision_that_changed_the_plan_reads_as_its_own_outcome` now reads the row and the marker.
- `a_correction_reads_as_applied_in_its_row_and_in_its_record` and
  `a_contract_names_the_inputs_it_declares_and_the_source_a_correction_replaced` use the
  95-to-60 correction. The contract test also checks the 44- and 30-column panes.
- `a_reopened_session_shows_the_token_policy_it_captured_and_not_todays` works in both
  directions:
  - a captured `Stop` with a 10 000 reserve pauses on `unknown_usage` and is reopened under
    `BoundedNative` with other figures;
  - a captured `BoundedNative` on partial counts completes and is reopened under `Stop`.
- `an_assignment_says_whether_its_token_allowance_was_requested_or_inherited`,
  `a_session_without_a_token_ceiling_names_no_allowance_and_protects_nothing` and
  `a_session_that_captured_no_resource_limits_shows_no_token_policy_in_their_place`.

Controls: each inversion was applied alone to `views.rs`, the named tests were run, and the file
was restored from a byte copy and verified by SHA-256. Every inversion failed its named test.

| Inversion | Failing test |
| --- | --- |
| M1: a plan change or correction row falls back to the grade | plan outcome, correction row |
| M2: a correction's marker is not read from its record | correction row |
| M3: declared inputs reduced to a count | contract inputs |
| M4: the replaced source not shown | contract inputs |
| M5: a long label no longer bounds its value | contract inputs |
| R1: token rows read today's configuration | reopened policy |
| R2: a requested allowance read as inherited | allowance |
| R3: a captured reserve explained as the legacy calculation | reopened policy |
| R4: a remainder by reported counts presented as known | reopened policy |
| R5: incomplete counts no longer rule a strict bound out | reopened policy |
| R6: the captured policy always read as a stop | reopened policy |
| R7: an inherited allowance read from today's configuration | allowance |
| R8: going on without counts not said to leave the bound unproved | reopened policy |

## Terminal walk

`keep_the_fixtures_an_interface_walk_reads` kept the runtime's stores. Each home holds a
configuration with the opposite policy, a 50 000 ceiling, a 2 000 allowance and a 7 000 reserve.
The built binary was driven over a pseudo-terminal at 80x24 and then 120x40, starting no run and
sending no prompt: 78 screens, 402 keys, every row reachable, and no line wider than the terminal.

- **Captured stop, paused.** The captured rows read `stop admitting`, `100000`, `4000`, `10000`,
  `not proved` and `unknown_usage`. The ceiling record reads "No turn reported a count, so no
  spending is known, 0 are held for turns still open, and what is truly left is not known."
- **Captured go-on, completed.** The policy row reads `admit on reported`. The ceiling record
  reads "By reported counts, 360 were spent and 1500 are held for turns still open, which leaves
  98140. Not every count is complete, so what is truly left is not known: it is at most 98140."
- **Allowances.** The turn admitted through the store reads `1500 · requested by this assignment,
  within the per-turn ceiling of 4000 the session captured`. A runtime turn reads `4000 ·
  inherited`.

## Commands and results

All checks ran once, at `b6eb516e45dab307b9647c8f863d7a9f787ca845` as recorded in `head.txt`,
with `CARGO_INCREMENTAL=0` and debug information 0. The counts below are read from those logs.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | exit 0; the log is empty |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0; no warning or error line |
| `cargo test --workspace` | exit 0; 32 result sections (26 test binaries, 6 doc-test sections): 419 passed, 0 failed, 2 ignored |
| of which `ymp-tui` | 139 passed, 0 failed, 1 ignored (the fixture keeper) |
| of which `ymp-cli` `native_catalog` | 9 passed, 0 failed, 1 ignored |
| single-inversion controls | 13 of 13 failed their named tests; every restore matched its SHA-256 |
| walk at 80x24 and 120x40 | 78 screens, 402 keys, nothing unreached, no line wider than the terminal |

## Limits of this round

- The runtime at this composition never requests a per-assignment allowance: `engine.rs` writes
  `token_reservation: None`.
  - The requested case is therefore written through `Store::admit_invocation`, which checks it
    against the captured ceiling.
  - That turn lies outside a run, so the walked session's own counter reads 8 turns while
    admission counts 9.
- Where coverage is partial, the page states what reported counts leave. It does not say what is
  truly left.
  - `strict_token_bound` is always written false.
  - "Not every count is complete" follows `UsageTotals::is_partial`. The store's stop condition
    also checks finalisation, so the page states coverage and does not predict an admission.
- The overlay for a captured row is titled with the row key, such as `captured:token-ceiling`,
  as before.
- No provider was asked anything and no credential was read. The real application home, the task
  register and the intent were not touched.

## Evidence

SHA-256 of the existing logs, taken after the runs had finished. Nothing was rerun for this
report.

```
e6f0fea87e1e192947686b5e273372776f67d53dda4495d4c975b3d4a732eb5a  /tmp/118-final/exit.txt
97e37000420eccb43e60539a885200c6d5b11111c24601de7b4693f744d6d0e6  /tmp/118-final/head.txt
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  /tmp/118-final/fmt.log
7731c54bb186c9855f47b83e440bae1c72febe9c56ec40e3ccaad416d318acb8  /tmp/118-final/clippy.log
d49f884d677d65069c331abc0fe539b2363d06cfd4d06a21d2b4d8af5b3df177  /tmp/118-final/workspace.log
2e21c4ca8864fcc6991485cbda982449a5825200d964caa2102c24d58be4c0f1  /tmp/118-mut/followups.log
2190eeb04f3c4982357fce95be24dcfda3d2c76b152d2253c63e988f05df76d4  /tmp/118-mut/reservations.log
3314ae6145e76024b22b66b463fbd3a5a01951dc354b7959169cde868a640e1f  /tmp/118-walk/walk_reservations.txt
490cf35ac663196cda2fdcae6aea9af670a8d41f2d6124d5b46338a4afb1a735  /tmp/118-walk/reservations/manifest.json
```

The walk script is `/tmp/118-walk/walk_reservations.py`, and the mutation harness is
`/tmp/118-mut/mutate.py`.
