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

## R2: membership is not recovery permission

`RecoveryStage.wait_reason: Option<RecoveryWaitReason>` separates owner Wait/Pause,
policy stops/ceilings, participant availability, admission, effect uncertainty,
cancellation and unbound legacy records. Missing legacy metadata remains unknown.
Membership edits clear a departing selection in non-running stages, retaining
owner/policy holds, failure history, attempt counters and manual permission state.
Only a typed participant-availability wait becomes Pending for reconsideration;
it receives no manual permit. The runtime still validates policy and admission.
Stage revisions and history change atomically with the owner command.

`r2-focused.json` binds source hashes to
`cargo test -p ymp-runtime --test session_recovery rework::r2_`, exit **0**
(four tests). Both remove/add and replacement retain owner Wait and exhausted
policy stops with zero calls until explicit continuation. Pause is retained too.
Availability replacement exercises both actual successful reassignment and a
policy-limit denial: the original provider failure and recovery count reach the
policy unchanged. Stale stage continuation is rejected; idempotent membership
replay does not change the stage. Explicit continuation admits exactly one saved
plan review, preserving the proposal and all prior failures/counters.
The initial two-control success is retained in `r2-after-initial.log`.

## R4: retain occupancy across failed waves and inspect before rework

Selection excludes participants with durable invocation, task or workspace-access
responsibility, in addition to the current wave's selections. Ordinary allocation
can therefore admit a free eligible participant on another provider for unrelated
ready work. No claim_busy or access check is relaxed.

After independent ready work drains, an atomic storage transition makes an exact
known-ended, read-only transport-failed attempt ready for the existing independent
interruption review. It requires matching failed assignment/invocation, terminal
records, failure access evidence and no remaining invocation/access ownership.
The original attempt/failure remains linked. Inspection and any rejection/rework
consume existing review, attempt, budget and acceptance boundaries in the same run.
Unknown write-capable effects do not take this transition.

`r4-before.log` reproduces claim_busy with no T3 call on the unchanged R4 source
at `83793dd` (exit **101**). `r4-focused.json` records the source hashes and
`cargo test -p ymp-runtime --test session_recovery execution_rework::`, exit **0**
(two tests): free T3 executes before failed-task inspection, all five results are
accepted without a restart, T0 is neither replayed nor reaccepted, and confirmation
remains unconfirmed with no reputation observations. The write-capable control
retains both responsibilities and explicitly proves claim_busy still rejects a
competing claim with unchanged tasks.

The reviewer artifacts were copied byte-for-byte and their hashes verified in
`independent-probe/copy-manifest.json`. Candidate probe changes only redirect
absolute dependency and output/temp paths; assertions are unchanged. The rerun
before R3/R4 repeats its original two failures and two passes (exit **101**),
including the saved revision/arbitration controls. The original R4 probe expects
failed tasks to remain Running until restart; same-run safe inspection intentionally
supersedes that intermediate expectation. Its original evidence is retained.

## R3: sufficient evidence enables independent inspection and continuation

The parent clarified the acceptance criterion: the original probe's code comment
about no remote effects is not an input to the runtime. Its unchanged automatic
completion expectation is not a valid positive safety oracle. The exact original
probe/results remain preserved. The real missing consumer is implemented by
`Engine::inspect_recovery`, with a separate positive public-API test and a durable
negative test of those insufficient historical records. See [API.md](API.md).

A narrow compiled-backend `local_effect_scope` enforcement contract is captured
with actual admitted access, backend identity and invocation origin. Missing scope
is never backfilled; native adapters, including Mock, default to unknown. The
positive fixture's actual implementation performs only one fixed Rust file write
and supplies its enforced complete local scope. The public historical-format
reconstruction retains that genuine access evidence without inventing resolution.

Inspection acquires ordinary ownership/access, allocates an independent reviewer,
uses normal grants/budget admission, and requires actual read-only execution. It
validates terminal records, bounded effect scope, exact task/plan/result binding,
observed files and the originating review response. The stored resolution preserves
all original uncertainty/failures. Owner holds, policy limits and counters remain;
Continue is separate. Both Continue and atomic replay admission reject changed
observed state. The replay itself must remain actually read-only.

`r3-focused.json` records six successful public runtime tests (exit **0**). They
cover sufficient local evidence, preserved stage Wait/Pause, explicit continuation,
no duplicate planning, no reputation, missing scope, unverified termination,
foreign/stale command and result binding, changed files during inspection/after
resolution/before admission, owner-pause races, model safe=true, reviewer rejection,
write-capable inspector denial, and exhaustion of the original four-call budget.
The initial compile and four-scenario test successes remain in their original logs.

`r3-observed-state-falsifier.json` records a source-bound mutation removing only
the current-file check from replay validation. The rejecting test exited **101**
because Continue incorrectly succeeded. Source restoration is byte-for-byte with
matching SHA-256; `r3-observed-state-restored.log` records the restored pass (**0**).

`complete-focused.json` records all **31** public session_recovery scenarios and
both saved revision/arbitration subcases of the unchanged independent probe,
all exit **0**. After that run, one narrow reason update was restricted to resolved
uncertainty/legacy holds so inspection also preserves policy-stop explanations.
The final required chain below checks that final source.

## Limits and remaining work

The four required corrections are implemented; full parent acceptance is separate. R3 and R4 were added by the completed parallel review. Their common model and
bounded evidence issue are in [responsibility-model.md](responsibility-model.md).
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
