# YMP-146 backend delivery evidence

Baseline: `15f7c2225cb74fa2dc9056bd55f9cf836288d44a`, exactly the initial HEAD
of `feat/ymp146-recovery-backend`. No fetch, rebase, other worktree, live home,
real session, installed binary or provider probe is involved.

## Delivery sequence

1. Saved-plan recovery was delivered first in local commit `51b8bd9`.
2. Trusted local live-team commands, the shared read model, further recovery
   consumers and race controls are implemented in the following backend outcome.
3. The full diff has been inspected and the final fmt/strict Clippy/workspace-test
   sequence passes. Parent owns independent acceptance and the later Claude UI.

## First slice: saved-plan review recovery

`Engine::run` restores the latest exact `PlanVersion` from `plan_proposed` before
considering a new planner call. `review_saved_plan` consumes a versioned durable
`RecoveryStage`. It records a saved response and its assignment/invocation, and
consumes a persisted positive or negative verdict without asking again. Negative
verdicts require the existing bounded revision path with the objection linked to
the revised proposal; malformed output records a separate failure and no verdict.
Plan commitment now rejects an existing graph, an obsolete proposal, a missing
accepted review, or a self-review. Duplicate verdicts cannot shop for approval.

The narrow `RecoveryPolicy` in `ymp-runtime/src/recovery.rs` receives immutable
`RecoveryInput`. `Engine::with_recovery_policy` captures its ID, version and
configuration. Proposals are recorded and validated before a delay or invocation.
`BoundedRecoveryPolicy` defaults to two recovery actions per stage, a shared
provider failure threshold of two, and a 250 ms cancellable delay. Engine also
caps injected strategies at eight actions per stage, eight failures before a
same-provider retry, and a 30,000 ms maximum delay. These are recovery ceilings,
not additional resource budgets. Every actual call retains normal resource,
assignment, independent-review, access and capability admission.

Transient transport resets retry safely; authentication, quota and unsupported
configuration can select an independent candidate on another provider using
`AllocationPolicy`. A generic connection diagnostic remains Unknown. Unknown,
timeout, absent independent replacement and uncertain effects have concrete
waiting/owner-action conditions. Actual write-capable access blocks automatic
replay even when the requested mode was read-only. A restart without terminal
backend evidence remains explicitly unverified. Existing interrupted execution
review is retained.

`Engine::recovery_stages` is the actual typed local read API.
`Engine::control_recovery(RecoveryControlCommand)` supports explicit retry,
continuation, wait and pause. Commands bind session, stage, expected revision and
durable command ID; repeated identical requests return the saved receipt, reused
IDs with different content and stale versions fail atomically. A manual retry
permits one ordinarily admitted attempt without erasing usage or failure history.
It cannot override uncertain effects. No provider tool exposes this owner ingress.
The later live-team API will compose with these actual types, not a UI DTO copy.

Stage revision checks and invocation binding share the admission transaction.
Completed messages restore through their originating invocation, never through an
actor's latest configuration. `RecordLinks.policy_chain` preserves recovery then
allocation provenance. The directly touched board composition also retains the
original allocation implementation followed by the board implementation instead
of overwriting the former. Parameterless policy entries omit configuration; recording their provenance
introduces no settings or generic configuration schema.

## Checks and failing controls

- Baseline control: `cargo test -p ymp-runtime --test session_recovery
  restart_retains_plan_without_duplicate_planning` exited **101**. It observed
  two planner calls where one was required. See `failing-before.log`.
- The final seven tests in `recovery-targeted.log` exercise restart, finite retry
  versus an injected manual-wait strategy through the same consumer, independent
  replacement, correlated failures, negative/malformed distinction, actual write
  uncertainty, token-cap unknown usage, durable pause, idempotence and stale CAS.
- `cargo test -p ymp-runtime --lib` initially exited **101**, with 111 passing
  tests and one budget-resume status regression. The regression was fixed by
  persisting the typed admission denial. Its focused rerun exited **0**; see
  `recovery-budget-regression.log`. The earlier run includes the existing
  interrupted-execution and allocation-substitution controls; no duplicate
  alternative allocation policy was introduced.
- The backend test fixture now declares its real read-only review access; its
  only write operation remains execution. This adapts its explicit trusted access
  contract, rather than treating requested read-only as an enforcement guarantee.
- Compilation iterations caught a missing Review field and incorrect message
  origin access (exit 101 each); neither was a passing behavior check.

At the first-slice commit, live-team changes, broader stage consumers and the
final required check sequence were still outstanding. They are completed in the
final backend outcome below; independent parent acceptance remains separate. No Linux/native-process termination,
actual provider error distributions, comparative model quality, UI, or installed
release behavior has been verified.

Observed slice exits: final `cargo test -p ymp-runtime --test session_recovery`
**0** (7 tests); `cargo test -p ymp-providers --lib failure::` **0** (1 test);
`cargo fmt -p ymp-core -p ymp-runtime -p ymp-storage -p ymp-providers` **0**;
`git diff --check` **0**. The full required final chain is reserved for final code.


## Final backend outcome

The second backend outcome extends the saved-plan slice to initial planning,
explicit plan revision, candidate review, existing dispute arbitration and final
review. It delivers trusted local live-team commands and the actual shared
`TeamControlView`, including versioned board/execution state. See [API.md](API.md)
for exact types, integration calls, desired/effective membership, pause semantics,
native metadata resolution, provenance and explicit recovery limits.

The final integration suite has 17 public Engine scenarios. It adds late-review
restart with preserved production and aggregate versions, same-session native
member admission by an already running Engine, live writer draining, native
identity and pinned-roster validation, stale allocation rejection, no-reviewer
waiting, unknown usage under token caps, deliberate unsafe policy rejection,
interrupted-review termination uncertainty, initial-planner recovery, owner pause
across resume, and replacement racing completion. Existing allocation substitution,
workspace concurrency, authority and interrupted execution tests remain in place.

The owner-admission falsifier temporarily removed only the atomic owner admission
check. `live_replace` then exited **101**, admitting a departing actor's new
invocation; see `owner-admission-falsifier.log`. The original source was restored
byte-for-byte (SHA-256 recorded by the tool), and the focused restored test exited
**0** (`owner-admission-restored.log`). It has enough concurrency allowance that a
budget denial cannot substitute for the membership assertion.

`targeted-checks.json` records six successful targeted command exits, including
legacy acceptance-contract and native review-limit regressions and the scripted
Codex protocol test. `owner-races-final-targeted.log` records four passing owner
control/race tests. The existing contract fixture now declares its actual access
(native read-only except its deliberate file-changing mode) and uses explicit
owner continuation after its intentionally unknown initial-plan failure. Existing
failure-status assertions now expect the typed recovery condition; their blocked,
no-fallback, no-reputation and preserved-result assertions remain. Duplicate final
acceptance is explicitly rejected while a saved aggregate survives resume.

Intermediate regression runs and their actual failures are retained, including
`expanded-runtime-lib.log` (109 passed, 4 failed before corrections) and
`contracts-regression.log` (two fixture continuation placements corrected next).
No acceptance assertion was removed to obtain a passing result. The first two
strict-Clippy attempts stopped before workspace tests, with formatting/style
findings in the new code; their exits and logs are archived as
`final-checks-attempt1.json`/`final-checks-attempt2.json`. The final required
sequence and its actual exits are recorded in `final-checks.json`.

No fixture changes outside the allowed backend/provider/test zones were required.
No TUI, task-register, contract, intent, release, dependency, installed-binary,
main-branch or live-data change is part of this delivery. The remaining external
step is independent parent acceptance followed by the separately owned UI work;
Linux/native provider behavior and recovery of unverified external effects remain
explicitly unverified.


The first full workspace test attempt reached the native capability-diagnostic
control and exited **101**: planning recovery preserved the safe structured cause
but lost the original redacted diagnosis in user-visible messages. The planning
adapter now retains that diagnostic before following the recovery proposal.
`capability-diagnosis-regression.log` records its passing rerun (exit **0**), with
all credential-leak assertions intact. `sole-reviewer-regression.log` also exited
**0**, verifying that a sole remaining independent nonproducer can finish a saved
final review; production still requires independent reviewer eligibility.
`final-checks-attempt3.json` and `final-workspace-tests-attempt3.log` retain the
failed sequence. The complete successful checks preceding the legacy-compatibility falsifier are
archived in `pre-legacy-full-checks.json`. After fixing that demonstrated source
issue, the final code passed the required sequence again; `final-checks.json`
records its three zero exits.

The next full workspace attempt stopped at the existing knowledge-correction
resume fixture (exit **101**, archived as attempt 4). That fixture intentionally
paused its first planner with an unclassified error; it now declares its actual
read-only planning access and sends explicit owner continuation when its scripted
condition changes. All knowledge retention, supersession, retrieval and credit
assertions remain unchanged. The subsequent runtime/storage regression sweep ran
all targets without fail-fast: all runtime targets passed; storage's positive plan
fixture exposed its unrelated pre-existing task (exit **101** overall). Planning
fixtures now start without that task, so the negative body-mutation controls still
exercise actual content validation. A new duplicate-graph rejection assertion
checks unchanged state/history after the first commitment. The two focused plan
storage tests passed with exit **0** (`plan-storage-regressions.log`). A constructor
parameter placement error was caught at compilation (101) and corrected before
that passing run.


## Final acceptance check

The legacy-format control reconstructs pre-YMP-146 records through public Store
methods in a separate temporary home. Before its fix, the engine replayed an
unbound write-capable review and incorrectly completed (test exit **101**, see
`legacy-stage-failing-before.log`). The final consumer creates a truthful
owner-action stage and preserves actual access, invocation origin and uncertainty;
an absent legacy stage or unbound response is never evidence for replay or
acceptance. Already-bound attempts are recognized across every retry, and legacy
failed invocations contribute to shared provider accounting. Existing persisted
pauses take precedence over the cached-verdict path. The 17-scenario targeted run
passed (**0**), as did the focused unknown-budget resume check (**0**).

Final required commands, on the final source, all exited **0**:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Their actual timestamps and logs are in `final-checks.json`. No successful chain
was repeated without subsequent code/test changes or the demonstrated legacy
safety concern. The checked source hashes and write-boundary inspection are in
`source-verification.json`. Build output remains ignored. Delivery requires the
whole branch range from `15f7c22` through the final backend commit; the final API
uses `RecoveryAdmission` metadata without extending the TUI's context enums.

Final workspace totals: **596 passed, 0 failed, 2 ignored**. Test logs retain
command output with trailing blank lines normalized for Git whitespace checks.
The two default-ignored tests are the fixture-only Claude SDK scan (requires
`npm ci` and bridge build) and the optional fixture-retention helper. They were
not enabled or run as extra probes.
