# Admission implementation notes

Canonical task status is in `tasks/records/W1-0006.json`. Contribution, solicitation,
RuntimeProxy offer, award persistence and the atomic Gatekeeper admission boundary are
implemented. W1-0007 owns later commitment transitions; W1-0017 owns actual starts
and completion.

## Values and provenance

The domain defines Contribution, Forecast, typed ContributionSubject, Assignment,
Grant, Invocation, RoleKind, TeamOperation and the coordination values needed for
awarding work. Prob validates finite values in [0, 1] during construction and
serialized input. ContributionKind reuses the existing resource-purpose mapping
and now supplies the model's competence mapping. ExecutionProfile validation keeps
native settings explicit; Invocation uses the existing requested/sent/reported
settings value and validates agreement between end time and terminal status.

These values do not create an admitted assignment or start an invocation. Their
kernel consumers establish authority and lifecycle separately. W1-0007 owns the
remaining commitment transitions; W3 owns voluntary bidding and participant transport.

ContributionSubject retains both the subject kind and versioned Ref. This checkpoint
rejects unsupported subjects: a generic existing Ref cannot masquerade as a WorkItem,
ResultVersion or Objection. Their owning services must supply typed resolution before
those paths become available. ContributionRecord retains the exact original acceptance
contract reference so a later admission can detect changed criterion interpretations.

## Persisted Arbiter

Arbiter.propose records a bounded immutable Runtime contribution against the current
acceptance contract. open records an eligible solicitation. submit validates a
RuntimeProxy offer's current Registry profile, agent, timing, source and target scope.
Agent-authored offers require the later grant-checked participant interface.

CoordinationView derives recorded contributions, solicitations, offers, awards and
commitments from Journal. It retains event references and timestamps. A new coordination
event cannot predate the latest time already represented in the session. Offers cannot
predate solicitation opening or arrive after its deadline. Awarding requires the offer
window to have closed, consistent with P1; no implicit early-close operation is added.

AwardPolicy receives an immutable AwardView and proposes an Award. FirstOffer implements
the approved earliest-offer alternative with stable ID tie-breaking. Arbiter validates
the exact current view digest, selected implementation/version/parameters, chosen offer,
agent and Proposed commitment, and retains the decision and basis. Tests connect a second
fixture strategy to the same consumer and verify that it selects a different agent.

Awarded and CommitmentChanged(Proposed) form one atomic append. No other event may
interrupt them; incomplete state cannot be stored or exposed as a replayed view. The
commitment history records the award and creation references. Exact lost-acknowledgement
resolution validates both packet boundaries, so matching only the first event of a
stored atomic packet cannot be reported as a complete committed operation.

## Gatekeeper admission

`Gatekeeper.prepare` validates the exact recorded award, offer, contribution, source
acceptance-contract reference, current Registry profile and task constraints. It
rejects unsupported typed subjects until their owning services provide typed
resolution. File access is represented by a sealed `PreparedMediation`; an empty
declared capability set is accepted only when the recorded provider has no actual
file capabilities and no file access is requested.

The prepared attempt generates an admission nonce and a grant secret locally. The
journal stores only the nonce, grant digest and immutable admission intent. A complete
packet begins with the resource reservation, optionally records prepared physical path
ownership, issues the grant, activates the Proposed commitment and ends with
`AssignmentAdmitted`. Replay rejects incomplete or interleaved packets. Final
capability delivery compares the complete stored intent with the sealed plan, so a
modified packet cannot expand operations or access. A delivered plan cannot mint
another token or handle, and restart cannot recreate the local secret.

Admission counts Admitted and Running responsibility toward `parallel_limit`, preserves
attempt limits across new contribution IDs for the same work, enforces member and pin
limits, and allows independent producers to share a role when effective paths are
disjoint. An assignment ID already present in path ownership cannot be reused, even
when the new admission requests no file access. Every denial leaves no partial
reservation, path ownership or assignment.
A denied attempt cancels only its exact current Proposed commitment; a competing CAS
winner is left untouched. Uncertain cancellation retains the local evidence instead
of claiming that work was never admitted.

`Gatekeeper.revoke` atomically revokes held financial and path authorities and marks
`AssignmentRevoked`. It does not settle usage, release a workspace hold or claim that
external effects ended. Journal capacity for revoke and later release/settlement is
reserved before admission. Grant authorization rechecks its digest, pinned admission
reference, operation set, expiry, reservation and path state on every request.

`ymp-runtime/src/admission.rs` exposes this boundary to the application without
starting a backend, copying credentials or inferring native model settings. Its
application command builds cost and allowance proposals from the current view and
passes any prepared workspace plan to the kernel; it does not bypass Gatekeeper.

## Verification and remaining work

`ymp-storage/tests/arbiter.rs` exercises the real consumer over MemoryJournal and
SQLite, alternate strategy selection, reopened records, stale proposal input, typed
subject refusal, offer-window timing, forged commitment data, incomplete/interleaved
packets and partial acknowledgement resolution. Domain validation tests cover invalid
probability values at construction and serialized input.

`ymp-storage/tests/admission.rs` exercises MemoryJournal and SQLite admission, real
mediated file ownership, no-file boundaries, stale contract pins, duplicate and
cross-session races, capability and budget denials, parallel/member/attempt limits,
sealed-intent tampering, lost acknowledgements, post-commit capability delivery,
revocation and financial/path release. Capacity tests cover revoke and final
settlement at the journal limit.

The final review found that a no-files admission could reuse the assignment ID of
standalone mediated path ownership. The SQLite regression
`no_files_admission_cannot_reuse_standalone_path_ownership` failed before the
uniqueness guard (exit 101) and passed after it. The guard preserves the previous
ownership and Treasury state, creates no assignment and cancels only the current
Proposed commitment. Separately, removing the grant operation check in a temporary
source copy made `memory_admission_commits_one_funded_no_files_assignment` fail on
an unauthorized `NoticePost`; restoring the guard and rebuilding passed. These
checks exercise admission authority, not execution or native confinement.

Final independent review accepted this boundary on 2026-09-28 after the ownership
correction. `make verify` completed with exit 0, covering the legacy scan and the
required offline build, formatting, Clippy and workspace tests. Task-register
validation also completed with exit 0. W1-0007 owns renewal, delegation and the rest
of the commitment lifecycle; W1-0017 owns actual starts, completion and recovery.
