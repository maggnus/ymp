# Commitment lifecycle implementation

Canonical status remains in `tasks/records/W1-0007.json`. This note describes the
P2 consumer and its boundaries, not execution or artifact acceptance.

## Recorded inputs and time

`AwardPolicy.commitment_terms` proposes typed `CommitmentTerms` against the same
AwardView as the award. FirstOffer version 2 supplies its explicit selected
parameters. Awarded retains the proposal, effective PolicySelection, input digest
and outcome. There is no additional strategy port and no requirement to choose a
Method before the bootstrap Plan contribution.

New admission requires those recorded terms at both Gatekeeper.prepare and the
Journal append boundary. A caller-supplied lease must match the derived initial
lease: admission time plus min(allowance timeout, T_lease). Historical Awarded
records without terms and their already committed admissions remain replayable;
they cannot be used to append another admission.

All clock values and durations use milliseconds. ManualClock is monotone and
SystemClock performs checked conversion from Unix-epoch milliseconds. The kernel
receives time explicitly. P2 expiration is strictly now > lease.expires;
renewal at equality is allowed while the grant remains live. Grant expiry keeps
its existing exclusive boundary, now >= grant.expires. No renewal extends that
original grant or the task deadline.

## Responsibility and retained resources

Renewal checks the exact current commitment reference, holder, configured signal,
remaining renewal count, resources and original lifetime. It subtracts one renewal
and records the resulting lease. The 4000-renewal bound leaves history capacity
for creation and the terminal transition.

Gatekeeper checks the current Active commitment and lease on every grant request.
Token-based Arbiter commands bind that validation to the expected journal revision;
a concurrent revocation cannot leave a stale authorization usable at a newer
revision. Complete journal boundaries reject a live assignment without its own
Active commitment.

Arbiter.tick processes expired commitments in stable ID order. Each expiration
atomically revokes assignment, financial and path authority and records Expired.
It does not settle usage or release a physical hold. Expired work may subsequently
be reopened through an explicitly linked solicitation.

Arbiter.release takes the replacement solicitation as input and commits revocation,
Released and the linked SolicitationOpened together. The new solicitation preserves
the contribution, increments the reopening count and adds the recorded Δ_release
to the previous stimulus. A truncated release without its reopening is rejected.

Renewing or ending a predecessor withdraws its outstanding linked delegation
solicitations and cancels only their Proposed commitments, recording the source
transition in their histories. The admitted winning successor of a Delegated
transition is preserved.

Control capacity retains two terminal/reopening slots for every Active commitment,
including one whose assignment has already been separately revoked. Financial
settlement and physical release retain their independent reserves and evidence
conditions. A terminal commitment is not evidence that execution never started.

## Delegation

open_delegation records a new solicitation explicitly linked to the current
holder's commitment and history reference. Its offer and award are persisted
through the existing Arbiter path. A delegation award cannot use ordinary
unbound admission. The successor keeps the same contribution, creditor and
condition, receives its own assignment/grant/commitment and must name a different
agent. Budget, parallel limits and conflicting old path holds still apply.

prepare_delegation returns the original sealed PreparedAdmission to its caller.
The packet funds and activates the successor, revokes predecessor authority,
records Delegated and ends with AssignmentAdmitted. Every proper packet prefix is
incomplete. The old grant loses authority in the same commit; local successor
capabilities are delivered only after exact commit resolution. A denied admission
leaves the original responsibility and resources intact. Lost acknowledgement or
post-commit delivery failure is uncertainty about delivery, not admission refusal;
the caller retains the same plan and uses its Prepared/Committed/Delivered state.

## Completion and remaining producer boundaries

Heartbeat renewal has a real authenticated consumer. W1-0017 connects CheckRun
progress to an exact recorded check, responsibility target and workspace.
EvidenceAdded and ResultSubmitted renewal integration remains pending. Recording
their kernel facts does not by itself make them supported renewal sources.

Public discharge and cancel connect resolution, the shared typed transition
consumer and terminal packet preparation. W1-0017 connects confirmed bounded
non-artifact completion with settled accounting and scoped cessation to discharge.
W1-0019 includes completed Reviewer work bound to its exact ResultVersion subject;
that transition discharges the review responsibility, not the artifact producer.
Other unsupported completion/cancellation sources return commitment_basis_unsupported.
The pure consumer validates session,
assignment, contribution, original contract, exact artifact subject, covered
targets or the affected contribution of a plan revision. Its synthetic projection
tests cover Discharged and applicable Cancelled outcomes and reject mismatches.
They create no InvocationEnded, AcceptanceRecorded, satisfied ledger or plan fact
in a production journal.

The [execution implementation](execution-implementation.md) describes the actual
non-artifact consumer. [W1-0010](acceptance-implementation.md) connects exact
committed artifact acceptance to the original producer's commitment. Plan/criterion
producers retain their owning tasks. Their production transitions and native
execution are not claimed as integrated.

[W1-0012](progress-implementation.md) retries failed artifact work only after
the previous responsibility actually expires. It preserves the failed candidate
and costs, uses a new admission, and retains exact original production contracts.
Changed checks are compatible with the same Task and criterion definitions;
changing their meaning still invalidates the old work scope. Research(None)
completion supplies the bounded diagnostic handler. Candidate-scoped
Verify/Diagnose completion remains a W1-0014 integration boundary.

## Verification

Final independent review accepted the W1-0007 consumer scope with R1(9/10) on
2026-09-28. `make verify` exited 0, covering the legacy scan, offline workspace
build, formatting, Clippy with warnings denied and workspace tests. Task-register
validation and `git diff --check` also exited 0.

`ymp-storage/tests/commitments.rs` exercises the real Arbiter and Gatekeeper over
MemoryJournal and SQLite: alternative recorded terms change actual initial and
renewed expiry; renewal exhaustion and equality boundaries; release/reopen with
unsettled financial and file holds; complete delegation and prefix refusal;
budget/self-delegation denials; stale delegation invalidation; unsupported facts;
and a one-read stale authorization race against revocation.

`arbiter::lifecycle::tests` uses explicitly synthetic kernel projections for all
four progress signals and completion/cancellation validation. Clock tests check
milliseconds and monotonic manual time. Existing journal-capacity scenarios now
include expiration after separate revocation and atomic release/reopen while an
authorized invocation still needs financial closure.

The lease consumer failed with exit 101 before the grant lease guard and passed
with exit 0 after it. Removing the authorization revision guard caused the stale
snapshot scenario to fail with exit 101; restoring it passed with exit 0.
Release with an outstanding delegation solicitation also failed before linked
round invalidation and passed after it. These checks establish the named kernel
boundaries; none starts a backend or demonstrates product cooperation.

Compatibility with historical admissions without terms was checked by inspecting
the replay path and optional serialized fields; a persisted pre-change admission
fixture was not reopened during this task.
