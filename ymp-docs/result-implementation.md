# Immutable production candidates

Canonical status remains in `tasks/records/W1-0008.json`. This slice connects an
explicit one-item Plan to actual production, retained snapshots and ResultVersion
submission. [W1-0011](planning-implementation.md) connects accounted Planner output
and criterion-directed selection; independent acceptance is connected by
[W1-0010](acceptance-implementation.md).

## Plan and production scope

`AdmissionRuntime.results` creates a consumer sharing the original Gatekeeper.
It records a fixed initial Plan from a real admitted Planner's live grant,
deriving author and session from that assignment. The graph contains one WorkItem
with current criterion targets, required capabilities and nonempty write paths;
dependencies and parent are empty. W1-0011 adds paid initial planning and a shared
definition validator; general graph revision remains W3-0004. New explicit calls
emit PlanCommitted v2 with full criterion coverage; historical v1 replay retains
its original checks.

A WorkItem reference hashes its definition, excluding state, attempts and accepted
reference. Its Contribution therefore stays valid as the work changes state.
Typed Produce/Alternative contributions must match the definition and original
acceptance contract; admission also checks write paths. Other typed subjects and
independent reviewer admission remain with their owning tasks.

## Continuous snapshot protection

The baseline is captured after admission but before invocation authorization.
`WorkspaceGuard.snapshot_unstarted` requires the original mediated capability.
CaptureStarted records `protected_by` for the admitted, unstarted writer; its
root Read is combined with that writer's existing ownership. Invocation
authorization is forbidden until capture ends. The Write hold then remains
throughout production. An older snapshot of the same workspace cannot substitute
for this attempt's baseline.

`Results.prepare_attempt` binds that snapshot, assignment, workspace and fresh
attempt/after identities to a non-serializable PreparedAttempt. `begin` records
Pending; the retained preparation resolves its exact uncertain acknowledgement.
A typed producer cannot dispatch without its Pending Attempt. Replay cannot
recreate the local handle.

At termination the host closes and drains mediated access. For a Pending Attempt,
WorkspaceGuard commits Released and the matching CaptureStarted together before
capturing after bytes. The aggregate Journal transaction prevents another session's
writer entering between them. A release without its matching capture is rejected.
Capture completion uses the existing nonce-bound local state and content store.

The host keeps cessation evidence when capture or a later observation fails.
Completed capture I/O can retry publication through `resolve_capture` without
withdrawing already released access again. Abandonment first revokes production
authority and retains unresolved holds. Successful capture is not a prerequisite
for proving previously drained file access has ceased. Control reserves decrease
through transfer, capture completion, submission and abandonment.

These guarantees cover the existing mediator. After capture still reads the whole
workspace and may be blocked by another writer in a disjoint scope. This sequential
one-item slice does not establish parallel result capture or protection against
unrelated processes modifying files outside the mediator.

## Candidate and history

Submission derives producer, profile, work item and snapshot identities from the
recorded assignment and attempt. It requires confirmed completed execution,
validated scoped cessation and the exact transferred after capture. Artifact paths
must lie within the declared write paths, and digests must match retained after
bytes. The consumer checks both snapshots through the ContentStore port.

ResultSubmitted atomically stores a fresh immutable ResultVersion, sets Attempt
to Submitted and WorkItem to InReview. Identical repeated submission is idempotent;
rewriting a candidate is refused. Submission leaves `accepted` empty and does not
discharge an artifact commitment.

Abandoned retains its candidate, if any, and reopens unaccepted work. It does not
imply P2 Discharged or Cancelled. A retry still obeys responsibility/resource rules;
the integration scenario expires the original lease before admitting the same
producer again. W1-0010 adds Accepted/Rejected transitions through the independent
acceptance consumer and permits explicit abandonment of a rejected candidate.

## Evidence and remaining coverage

`crates/ymp-storage/tests/results.rs` runs an admitted Planner and two Scripted
productions against a temporary workspace. It checks exact before/after bytes
after later edits and SQLite reopening, actual producer/profile, retained abandoned
candidates and distinct retry identities. It rejects stale/missing baselines,
foreign consumers, premature submission, wrong retained digests and rewritten
candidates. Removing the baseline ownership guard makes the stale-snapshot
assertion fail.

Native execution, general graphs, recovery of a
lost PreparedAttempt, parallel result capture and injected after-capture
publication failures have not been exercised by this scenario. Failure handling
and control-reserve calculations receive independent review; that does not
establish those future execution checks.

Final verification: `make verify` exited 0, including the legacy scan and all four
required workspace commands. The task register and `git diff --check` passed.
Independent review returned R1(9/10) ACCEPT. One existing content-discrimination
test exceeded its incidental one-second process limit during the first broad run;
its separate rerun passed. That scenario now allows five seconds while dedicated
timeout controls retain their 50 ms limit; production limits are unchanged.
