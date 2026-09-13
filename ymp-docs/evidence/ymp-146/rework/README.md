# YMP-146 required RETURN rework

Scope: the parent-owned `acceptance-round1.md`, candidate
`cfdceff06cc703352b1234892729fb72a85dbf09`, isolated branch
`fix/ymp146-review-findings`. This evidence supplements the preserved backend
evidence. It is not full backend acceptance; the parent must reconcile the
authority and parallel executable recovery reviews.

## Executed baseline controls

Commit `045741f` contains only tests and evidence on the unchanged candidate
production source. `baseline-controls.json` records an empty production diff,
the test SHA-256, exact command and exit **101**. All four tests reached the
accepted findings: both ready-commitment departures blocked with
`invalid_execution_choice`; membership changes after owner Wait and exhausted
recovery allowance admitted five additional calls (2 -> 7 and 3 -> 8).
There is no counterexample to either accepted finding in these controls.

The first R1 fixture incorrectly serialized full execution settings as
`ModelEffort`. Its rejection before a board proposal is retained in
`baseline-attempt1.log` and `baseline-r1-fixture.log`; those R1 failures are not
claimed as defect reproductions. The corrected fixture uses the actual bound
team socket to propose responsibility and the public runtime to commit it.

## R1: ready responsibility release

The accepted owner departure transaction appends a typed
`RecordLinks.board_release: BoardCommitmentRelease` before evaluating remaining
responsibilities. It records the command ID, previous task/commitment version,
and exact original commitment. Only Ready responsibility without execution
admission for that attempt is released. Original board proposals/decisions and
task definitions remain unchanged. Already admitted tasks, invocations and
workspace access retain their ownership and draining behavior.

Board snapshots include the release decision in the task version, reject stale
claims, and cease selecting the released commitment. Snapshots without releases
retain their exact historical version calculation. A later independently accepted
commitment has its own proposal identity and is not suppressed by an older release.
No provider-facing release operation or fabricated board proposal is introduced.

`r1-focused.json` binds source hashes to two commands, both exit **0**:

- `cargo test -p ymp-runtime --test session_recovery rework::r1_`: both removal
  and replacement complete through eligible participants, with no fresh call to
  the departing agent. Checks also cover unchanged tasks/plan, retained original
  decisions, one release on idempotent replay, receipt persistence, stale claims,
  stale/conflicting commands and atomic rejection.
- `cargo test -p ymp-runtime --test session_recovery live_replace`: admitted
  write ownership drains unchanged, while fresh departing-member admission and
  stale allocation remain rejected.

The initial successful R1 continuation check is in `r1-after-initial.log`.
The focused run was repeated only after adding version/history/CAS assertions.

## Limits and remaining work

R2 correction and its availability/explicit-continuation controls are pending.
The final fmt, strict Clippy and workspace-test sequence is reserved for the
final corrected source. Only scripted providers and temporary app homes are used.
The frozen original worktree is used solely through its ignored target build
cache; its source and Git state are untouched. Main's target cache is not used.

Team and recovery receipts have separate API namespaces: idempotence is scoped
to `(API, session_id, command_id)`. A pending replacement cannot currently be
cancelled through Add/Remove of its proposed replacement. Those limits are
documented, not redesigned. The distinct-connection deferred release-record race
remains unproven and is not changed here. Existing composed allocation provenance
and legacy interpretation remain intact. No native provider/platform, UI,
installed executable, user data, external-effect recovery or full backend
acceptance is claimed.
