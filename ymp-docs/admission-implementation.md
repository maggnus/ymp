# Admission implementation notes

Canonical task status is in `tasks/records/W1-0006.json`. The current checkpoint
implements contribution, solicitation, RuntimeProxy offer and award persistence.
Gatekeeper admission/revocation and live grant issuance are still required.

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

## Verification and remaining work

`ymp-storage/tests/arbiter.rs` exercises the real consumer over MemoryJournal and
SQLite, alternate strategy selection, reopened records, stale proposal input, typed
subject refusal, offer-window timing, forged commitment data, incomplete/interleaved
packets and partial acknowledgement resolution. Domain validation tests cover invalid
probability values at construction and serialized input.

Gatekeeper must still bind the exact contribution/award/contract, fresh profile and
subject provenance, resource decisions, effective workspace permissions, assignment,
grant secret and active commitment in one attempt-bound atomic packet. It must check
limits and independence, cancel only the matching current Proposed commitment on a
confirmed denial, and retain unresolved financial/workspace holds after revocation.
Live access handles and grant secrets can be issued only after the whole packet is
confirmed; journal data alone cannot mint a second live capability. This remains the
W1-0006 completion boundary. W1-0017 owns actual starts, completion and recovery.
