# YMP-146 backend delivery evidence

Baseline: `15f7c2225cb74fa2dc9056bd55f9cf836288d44a`, exactly the initial HEAD
of `feat/ymp146-recovery-backend`. No fetch, rebase, other worktree, live home,
real session, installed binary or provider probe is involved.

## Delivery sequence

1. Deliver and commit saved-plan recovery first (this slice).
2. Finish trusted local live-team commands, shared read model, and remaining
   stage/recovery integration and race controls.
3. Inspect the full diff, run the final fmt/strict Clippy/workspace-test sequence,
   and commit the remaining backend outcome. Parent owns independent acceptance
   and the later Claude UI.

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
of overwriting the former. Existing policy interfaces have no configuration
method; `null` is retained honestly for that unavailable metadata.

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

This is a reviewable first slice, not final contract acceptance. Live-team
changes, broader stage consumers, the final required check sequence, and the
independent parent review remain outstanding. No Linux/native-process termination,
actual provider error distributions, comparative model quality, UI, or installed
release behavior has been verified.

Observed slice exits: final `cargo test -p ymp-runtime --test session_recovery`
**0** (7 tests); `cargo test -p ymp-providers --lib failure::` **0** (1 test);
`cargo fmt -p ymp-core -p ymp-runtime -p ymp-storage -p ymp-providers` **0**;
`git diff --check` **0**. The full required final chain is reserved for final code.
