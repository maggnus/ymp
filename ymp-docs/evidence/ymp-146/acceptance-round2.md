# YMP-146 recovery re-review and native continuation addendum

Candidate: `1dff2cbcb558639a74f46fe009372a71652cc4e7`, executable source `58c7957`.
Recovery reviewer: `52111c72-73e2-4ddc-b677-5573306ba1a9`.

## R4 accepted

The free actor executes T3 before inspection of failed T1/T2. Known-ended read-only
transport failures enter independent review/rework in the same run. T0 is neither
executed nor accepted again; unconfirmed results receive no reputation. The
write-capable control retains ownership and claim_busy. Thirty-one public tests
passed, and declared source/original-artifact hashes matched.

## R3 still requires one source correction

Completed native execution can produce MalformedResponse and no valid review.
The new inspection consumer only accepts Failed/Cancelled invocation state,
rejecting Completed+MalformedResponse before an inspector call, even with the
required local scope and BackendEnded evidence. The paired external control
accepts Failed+Unknown with otherwise equivalent sufficient evidence.

Accept a completed invocation only when the exact recorded malformed-response
failure, origin, terminal evidence and absence of a valid verdict justify that
case. Keep scope, independent-review, file freshness and binding checks. A negative
or already accepted verdict is not a malformed response or a reason to seek a
different reviewer. Preserve the external failing control and add a durable test.
Original evidence is in /tmp/ymp146-recovery-rereview/scoped-inspection.log and
review-result.json; do not edit reviewer-owned originals.

## Native applicability: separate fresh review from effect resolution

The strict new effect-resolution primitive works with sufficient declared input,
but all existing native adapters provide no complete local_effect_scope, and
genuine pre-146 records never captured it. Its synthetic positive therefore does
not establish a usable recovery path for the original ACP failure.

The parent authorizes a narrow additional local-owner action: **review the exact
saved plan with another eligible participant in a fresh read-only invocation**.
It satisfies the outstanding review obligation; it does not certify or erase the
failed invocation's effects. This follows the approved goal of resuming saved work
and the existing distinction between interruption review and blind replay.

### Contract for the fresh saved-plan review action

- Scope is pre-execution review_plan with an exact saved proposal, known-ended
  prior local execution and released conflicting access. A lock alone is not
  termination evidence. Unknown termination remains a refusal.
- Bind the command to session, stage, proposal/result version, expected state and
  idempotent command ID. Preserve the original proposal, failure history,
  objections, usage and applicable owner constraints.
- Choose an eligible independent non-author, excluding the failed actor. Require
  actually read-only backend access; a requested read-only mode on ACP is not
  enough. Apply ordinary allocation, grants, resource admission and cancellation.
- Start a fresh native context: never continue the failed actor's native session
  or revive its authority. Record the new origin and its connection to the
  original proposal/obligation. Do not backfill local_effect_scope or create
  effect_resolution for this operation.
- A valid prior negative verdict still requires revision/dispute handling. Do not
  allow reviewer shopping or duplicate accepted verdicts. Preserve current owner
  Wait/Pause; a membership change is not permission for this action.
- The action returns its recorded review/result or an explicit unmet condition.
  It does not execute production tasks or call Engine::run automatically. Further
  execution is a separate explicit continuation with ordinary access, dependency,
  budget and acceptance checks. No generic permission to replay uncertain writes
  follows from a successful read-only review.
- Unknown historical external effects remain explicit. Where subsequent work
  actually depends on unresolved effects, do not silently treat the handoff as
  their resolution; retain a concrete unmet condition and report the missing
  information. Do not claim the entire session is recovered from a plan-only test.

### Required evidence

Reconstruct a genuine old-format failed plan review with no local scope field.
Exercise the new public action using the normal native execution boundary and
controlled local protocol fixtures, or an explicitly justified equivalent with
local_effect_scope remaining None. No real provider inference is authorized.
Demonstrate one new independent read-only review, no duplicate planner call,
fresh native context, preserved uncertainty/objections and no production invocation
from the review action itself. Show the exact subsequent ordinary-continuation
behavior and any remaining write-dependency condition.

Negative controls include a live/unknown-ended prior actor, write-capable new
reviewer, self-review, existing negative verdict, stale/foreign/repeated commands,
owner holds and exhausted resources. Test current backend/app-version changes
without relabeling old records or reusing an incompatible native context.

The earlier strict inspection acceptance clarification remains valid: insufficient
scope is not enough for global effect resolution. The additional action has a
different bounded purpose and must not disguise a waiver of that guard.

## Rework coordination

The authority reviewer uses immutable /tmp/ymp146-authority-1dff2cb or committed
blobs. The same implementation fork may proceed with the corrections above; no
additional implementation agent is needed. Preserve accepted R1/R2/R4 behavior,
run focused controls, then the required final checks after source changes. Native
process guarantees, Linux and the actual owner session remain unverified unless
separately checked. Parent owns integration, minimal UI sequencing and acceptance.
