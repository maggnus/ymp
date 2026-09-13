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

## Additional observations

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
