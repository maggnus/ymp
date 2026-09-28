# Accountable session integration

Canonical status remains in `tasks/records/W1-0014.json`. This document records
the implemented session integration and its checked boundaries. Native
experiments remain distinct from the Scripted evidence recorded here.

## Fixed session workflow

Dispatcher.start binds the supplied workspace, records discovered participants
and explicit policy selections, opens the finite budget and retains the owner's
SessionDefinition. The definition binds visible check specifications to exact
criterion references and an explicit bounded offer window. Dispatcher.tick
advances from recorded facts; Waiting carries the actual next deadline rather
than pretending that an offer or commitment has already expired.

The workflow connects two accounted planning stages, admitted production, a
captured ResultVersion, verification, paid independent candidate review, A7
acceptance and P2 discharge. Finalization then checks the retained aggregate,
consumes an independent final review and audits the report. Free processing of
already paid completion and evidence precedes the affordability check for new
verification. Acceptance does not imply that every required criterion is
Satisfied; ending with unmet criteria requires the recorded stop basis.

This is the W1 fixed workflow. It does not implement later agent initiative,
dynamic team growth or arbitrary automatic environment/check repair. Retry and
AddVerifier use the delivered bounded recovery services. A repair that requires
new owner input remains unavailable without that input: FixEnvironment needs an
actually corrected compatible runner, and ReplaceCheck needs the explicit
proposal and independent approval required by its owning service. Board and
consequence wakeups belong to later integration tasks. A planner-created
criterion without an explicitly bound visible check cannot acquire a fabricated
owner check. Native execution uses the supplied backend and the same admission
and accounting consumers; Scripted evidence below does not establish a native run.

## Owner control and recovery

Application.recover requires an explicit RecoveryIntent. Observe inspects retained
state, Continue authorizes eligible further work, and Report requests only the
deterministic completion of an explanation.
It issues local SessionControl and, when a budget already exists, BudgetControl
for the same retained session and journal. It does not append BudgetOpened,
reset spent/held amounts or restore any Grant. Observe does not append Continue;
the Dispatcher must preserve that distinction when deciding whether to run work.

SessionChanged records trusted Stop, explicit Continue or a phase from the
existing SessionStatus values. The kernel derives the stop/revocation packet
from the current state. MemoryJournal applies it under one mutex; SqliteJournal
uses its write transaction. An optimistic revision from the caller cannot keep
losing to unrelated appended events. Repeated Stop is idempotent. Continue checks
current phase legality before returning an earlier idempotent result.

The current owner's local stop latch closes before journal I/O. A controlled host
and its InvocationFiles check that latch as well as the recorded authority.
Revocation preserves unresolved financial and physical holds. Explicit continuation
before reporting can authorize new work; all earlier grants remain revoked.
Sticky Finalization Stop, ReportingStarted and delivered reports cannot be
reopened for model work.

An exact completed recovery invocation can advance the session once through
RecoveryConsumed. Repeating that request returns the original reference without
changing the current phase; an old recovery completion cannot repeatedly clear a
later Blocked phase. New consumption is denied after stop or reporting begins.

Report recovery cannot open production or start a model call. It preserves
Cancelled/Blocked outcomes and can reuse already recorded complete accounting
and completion facts. An already delivered report is returned without another
delivery. This path remains separate from continuation of active task work.

## Memory and physical identity

MemoryJournal.with_binding_identity owns a real temporary identity file with a
nonce, device and inode, outside the task workspace. File binding uses actual
physical identity, including overlap/replaced-path checks and the journal identity
on incoming bindings. Cleanup checks the owned file identity before removing it.
The marker does not store events or restore lost RAM history. The existing
constructor without physical binding remains available for non-filesystem uses.

Reopening Memory means a new Application over the same retained Arc<MemoryJournal>.
Actual recovery after process loss belongs to persisted SQLite history. Neither
kind reconstructs unresolved native handles or permits duplicate execution.

## Current evidence

Reviewer eligibility preserves A6: an explicit Executed or Browser requirement
requires the corresponding RunProcess or Browser assignment capability. A
files-only provider cannot silently satisfy that access rule by reading prior
evidence. The data-artifact scenarios use an explicit ExactBytes check without
requiring candidate-program execution; their real Executed byte-comparison
evidence remains recorded and failing checks still override approval. The runtime
does not weaken an existing criterion's needs_class to fit an available provider.

The first Dispatcher pass reached two paid planning stages and a retained plan
on both journals in 6.04 seconds. The first complete producer/reviewer path then
passed in 85.81 seconds, with final acceptance, ReportDelivered and six reconciled
invocations. A following scenario exercised typed Verify through ContributionPolicy
and the host, its own non-artifact discharge, and the complete session on both
journals in 113.61 seconds with seven reconciled invocations.

A paused, already started Planner was interrupted on both journals. The dispatcher
delivered a Cancelled deterministic report, retained the same budget and original
file, and the invocation count stayed one (2.01 seconds). An unavailable final
reviewer scenario disabled the actually independent participant, not a presumed
role by name: the report recorded exactly Blocked(final_review_pending), retained
the accepted source and accounted narration (29.46 seconds).

The bounded Retry scenario passed in 104.66 seconds. An actual failed candidate
check defeated approval, the original commitment expired at its real lease
deadline, and the recovered attempt was abandoned. A new admitted producer made
a distinct retained result, followed by verification, review and delivery; the
original result and costs remained visible. AddVerifier passed in 28.03 seconds
with an existing independent participant, a paid Researcher invocation, exact
diagnosis context, a receipt and non-artifact discharge. Its output was not
invented as check Evidence; exhaustion of the configured ladder preserved work
and led to reporting.

The paid-review integration adds CandidateVerdict and PaidReviewRecorded, retaining
the original Assignment, admission, completed invocation, settled receipt and
attributed prompt. It consumes the actual role output after grant revocation;
earlier ReviewRecorded shapes remain intact. Re-consuming the same invocation
returns the existing result rather than creating another review.

Typed Verify/Diagnose completion uses the same exact original responsibility and
completed/closed invocation boundary as typed Review. Recovering PreparedAttempt
uses its retained start and nonce under the current owner consumer. It restores
only the local Results capability; submission still requires the actual completed
producer and exact captured bytes. It restores neither a Grant nor a native start.

Candidate A7 version 2 waits for canonical Evidence for already recorded
conclusive runs in the exact selected result/check/criterion/environment context.
This closes the known Fail-without-Evidence case without adding mandatory new
checks or an A8 Satisfied precondition. Version 1 remains replay-only. The updated
existing report scenario passed with actual paid candidate review, recovered
attempt and repeated review consumption in 37.17 seconds. The existing result
scenario then passed in 31.84 seconds with independent Approve already present:
an exact Fail before Evidence returns candidate_evidence_pending without changing
the journal; after Evidence it produces Rejected despite applicable approvals.
New AcceptanceRecorded writes use version 2, and a new version 1 append is denied
with acceptance_version. Independent reading confirmed those exact assertions
and the paid-wrapper validator's exclusion of conflicting FinalReview identities.
Temporarily removing the new A7 guard caused this exact case to fail in 8.01
seconds: the known Fail incorrectly received Accepted with Confirmed(TrustedCheck).
The guard was restored, and the result scenario passed again in 34.16 seconds.

The first focused session-control consumer passed in 0.46 seconds after an
optimized test build with debug assertions and overflow checks enabled. It uses
both Memory with a real identity marker and SQLite, with an admitted assignment:
atomic stop at advanced journal state, retained financial/path holds, duplicate
stop without another event, new Application observation/continuation, old grant
still revoked, unchanged budget and denied Continue after ReportingStarted.
Independent reading confirmed the atomic storage boundaries and these fixes.
Its later extension raced Stop against another thread appending phase controls
and passed on both journals in 0.57 seconds. Stop retained the same financial and
physical holds, revoked the old grant, and stayed idempotent. This is bounded
concurrent evidence backed by the atomic storage implementation, not a claim
about arbitrary operating-system scheduling fairness.

The full session scenario reopened after production and during reporting on both
journals, preserving all seven invocations, their accounting, the accepted result
and discharged commitments (137.88 seconds). Report-only recovery returned an
already delivered report without another journal event or backend start.

The first combined-workspace run passed six of the seven Dispatcher scenarios,
including real check failure despite approval, insufficient verification funds
and the complete reopened session. The Retry scenario hit its external 120-second
test watchdog; the whole group took 162.89 seconds. Its earlier isolated run took
104.66 seconds. The watchdog was raised to 240 seconds to allow replay-heavy
scenarios to compete during the workspace run, with diagnostic state on expiry.
This changes neither the runtime lease nor the invocation or budget limits.
The repeated mandatory combined-workspace verification passed with exit status 0.
All seven Dispatcher scenarios passed together in 165.26 seconds, including
Retry and the two-journal reopened session. No runtime behavior or resource limit
was changed to obtain that result. No native model run or product experiment is
established by these checks.

Independent review of the stable source and model mappings returned ACCEPT,
R1 (9/10). The reviewer checked the completed-source-before-expiry ordering and
the report fallback when the selected narrator disappears or fails after
reporting begins. Those narrow interruption cuts were reviewed in code but were
not separately reproduced as full scenarios. The final combined-workspace run
passed the legacy scan, offline build, formatting check, Clippy with warnings
denied, workspace tests and documentation tests. The metadata-only installed
provider procedure remained ignored; the run made no native model calls.

The workspace verification uses the repository's unchanged required commands:

```sh
CARGO_PROFILE_TEST_OPT_LEVEL=1 \
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true make verify
```

Only test compilation is optimized; debug assertions and overflow checks remain
enabled. No Cargo profile or project dependency was changed for these runs.

## Native-pilot deadline follow-up

The first native pilot exposed a real-clock P1 ordering regression after both
paid planning stages completed: Dispatcher tried to submit a missing RuntimeProxy
offer after the deadline despite an already valid received offer. W1-0014 was
reopened through its task record. The narrow repair stops submission at the
deadline and invokes the existing AwardPolicy on received offers. A no_offers
denial follows the existing Blocked/report path; during narration it selects the
deterministic fallback. It does not extend windows or change admission authority.

The short consumer reproducer failed on old code in 0.14 seconds; the repaired
received-offer and empty-window cases passed together in 0.10 seconds. Independent
review accepted the repair (R1, 9/10). The full legacy scan and four required
Cargo checks passed; all nine Dispatcher scenarios passed together in 140.16
seconds. Normal verification made no native model calls.
The pilot's inputs, usage, failures and limitations are retained in
[its evidence directory](experiments/homogeneous-gpt-elementary/README.md).
