# YMP-146 independent authority review: RETURN

Reviewed immutable candidate: `cfdceff06cc703352b1234892729fb72a85dbf09`.
Reviewer: `332e0b99-79a7-4255-82ae-be642ff7707e`, Claude Code claude-opus-5 high.
The following findings were established by source review, not executed by this
reviewer. The maintainer inspected the relevant source and accepts them for
bounded rework with discriminating runtime tests. A separate recovery reviewer
continues executable checks on the frozen candidate.

## R1: departing participant retains an unexecutable ready commitment

A has a board commitment to Ready task T2. Owner Remove(A) or Replace(A,B)
marks A as departing because the commitment counts as responsibility. New
admission and allocation candidates exclude A, while eligibility retains A to
represent its current responsibility. `committed_executor` finds the commitment,
then `board_allocation` cannot find A among admissible candidates and returns
`invalid_execution_choice`. Departure cannot settle while the commitment remains;
no owner command releases or transfers that ready commitment. With T2 as the only
ready task the session blocks without a usable path to B.

Relevant source: storage/team_control.rs55–63,90–101,347–365;
runtime/engine/allocation.rs441–448; runtime/engine/board.rs453–476;
storage/board.rs36–51 (paths relative to ymp-rust/crates and their package).

Required outcome: atomically release or transfer a ready, not-yet-admitted
commitment when applying the explicit owner departure/replacement, preserving
version/provenance/history. Alternatively supply an actual typed transfer action
and truthful waiting consumer. Do not touch an admitted invocation's ownership.
Demonstrate both removal and replacement through the public runtime, with task
continuation and no new admission to A. Include stale/idempotent command checks
where that transaction changes existing guarantees.

## R2: membership change implicitly grants recovery permission

Owner explicitly sets a stage to Wait, clearing manual_permit. Later removal or
replacement of its selected reviewer causes commit_owner_team to set Pending and
manual_permit=true whenever recorded failures are read-only and ended. The same
branch can clear OwnerAction after recovery-policy/provider limits are exhausted.
review_recovering then skips the recovery-policy path and admits an attempt that
the owner did not explicitly request by changing membership.

Relevant source: storage/recovery.rs158–166; storage/team_control.rs382–417;
runtime/engine/recovery.rs424–433,534–537.

Required outcome: separate readiness after reviewer replacement from explicit
owner wait/pause and exhausted-policy conditions. Membership changes may invalidate
the selected reviewer but cannot manufacture a manual retry permit. Reconsider a
stage whose actual condition was reviewer availability through normal validated
policy/admission. Preserve explicit waits and require the appropriate continuation
command. Use a typed reason where needed, not fragile matching of display text.
Tests must observe actual invocation counts and retained state/counters.

## R3: legacy write-capable review has no usable inspection consumer

The recovery reviewer `52111c72-73e2-4ddc-b677-5573306ba1a9` executed a public-API
probe against the same immutable cfdceff. It reconstructs a pre-146 proposal,
invocations, access and membership. The scripted backend has ended, its single
local effect is known, and no remote execution exists. Nevertheless the stage
remains OwnerAction with only Wait/Pause; Continue and Retry reject with
uncertain_effects. Valid reviewer replacement and general continuation still
produce zero new invocations and zero tasks.

InspectEffects only records OwnerAction; the runtime has no consumer to perform
inspection and bind its outcome to the saved stage. The matched ReadAll control
continues and completes without duplicate planning, distinguishing this gap from
an intrinsically unverifiable remote effect.

Required outcome: an independently admitted, recorded inspection with applicable
termination/effect evidence and stage/result-version binding, followed by a real
safe continuation path. Retain historical uncertainty and its resolution evidence;
do not clear old failures, trust an unverified safe flag, or waive the guard.
Unknown external execution may still require waiting, but the reproducibly bounded
local case must be recoverable. Reuse the ordinary access, budget, attribution,
confirmation and independent-review boundaries. Preserve explicit owner pauses.

Source: runtime/engine/recovery.rs367 and storage/recovery.rs146. Retained probe
data: independent-recovery/legacy-true.json and legacy-false.json.

### R3 evidence clarification before rework

The author identified, and the maintainer verified in the original probe, that
the no-remote-effects fact exists in Script's implementation/comment but is not
transmitted to the runtime as complete effect-scope evidence. The persisted facts
are WriteAll and ended execution; the original probe also calls no inspection API
because none existed. Its unconditional completion assertion is therefore not a
sufficient positive acceptance criterion by itself. Preserve that original probe
and its observed failure unchanged; do not weaken runtime safety to satisfy it.

Required rework remains an actual independently admitted inspection consumer and
usable continuation. Add a positive public-API case with verifiable trusted
scope/termination evidence and a durable insufficient-evidence negative control.
Evidence needs a supported acquisition/verification path, session/stage/result
binding and freshness checks. A raw Store safe setter, unsupported model claim
or Mock-only exception is not an implementation of that boundary. Historical
uncertainty remains, with linked resolution evidence rather than deletion.
Unconfirmed termination, unresolved external effects and stale/misbound/changed
observations must still refuse unsafe continuation. Inspection cannot silently
clear owner holds or reset resource accounting. This clarification authorizes
engineering work, not a safety attestation for the owner's real session.

## R4: failed executors remain occupied but are selected for independent work

An executed five-task probe accepts T0, then T1/T2 fail with transport errors and
retain Running responsibility. Independent T3 is ready; a free eligible executor
on another provider and an independent reviewer fit the team ceiling. Selection
nevertheless chooses a participant still responsible for a failed task. Storage
correctly rejects it with claim_busy, stopping the session before T3 executes.

Explicit resume inspects/reworks interrupted results and accepts all five tasks,
retaining the original T0 acceptance. These results remain unconfirmed as the
fixture's actual evidence requires. The control demonstrates retained work, but
does not make the first run's scheduling failure acceptable.

Required outcome: candidate selection accounts for retained responsibilities
across waves and failed execution, allowing genuinely independent ready work when
admission is feasible. Preserve claim_busy and write ownership; never permit one
actor to take conflicting work merely to satisfy the test. Integrate appropriate
interruption inspection/continuation boundaries without requiring an unrelated
manual resume solely to repair scheduler bookkeeping.

Source: runtime/engine.rs2471 and storage/board.rs319. Probe data:
independent-recovery/execute-n-first.json and execute-n.json.

The recovery reviewer ran all 17 public session_recovery tests and one failure
classification test successfully. Its four external probes have two passes and
the two expected acceptance failures above; additional revision/arbitration
continuation controls retain objections and origin. All 33 declared source hashes
matched. New access declarations in old fixtures match their executable behavior;
the reviewer found no weakened assertions there. Only macOS arm64 with scripted
providers was exercised. Linux/native provider behavior remains unverified.

## Additional authority observations

- Team and recovery command receipts use separate namespaces. Reusing an ID
  across the two APIs executes two commands; within one API, changed content is
  rejected. Document the namespace rather than implying global uniqueness.
- A pending replacement currently cannot be cancelled through Add/Remove of its
  replacement. Record the limitation; no unapproved broad interaction redesign.
- Composed allocation provenance changes the meaning of the primary implementation
  field for new records. Preserve legacy interpretation and the complete chain.
- Possible release-record loss with separate Store connections remains unproven:
  a deferred decision transaction also calls settle, while AccessLease::drop
  ignores recording errors. The reviewer found it unreachable for ordinary Store
  clones sharing one connection; distinct connections/processes need evidence
  before this becomes a required source change.

The review rejected owner-tool spoofing, stale-command partial writes, ordinary
departing-member admission, stale allocation, premature write release and native
profile substitution as counterexamples in the inspected paths. These confirmed
guards must survive rework. No UI or live provider behavior was exercised.

## Execution discipline

Keep the original candidate worktree frozen for the ongoing recovery probe.
Apply rework in an isolated fork based on cfdceff; preserve existing successful
checks and add only tests for accepted findings. Run the required final check
sequence after source corrections. P0 recovery remains unaccepted and uninstalled.
Any additional required finding from the parallel review must be reconciled before
final acceptance; do not present a two-finding fix as complete acceptance.
