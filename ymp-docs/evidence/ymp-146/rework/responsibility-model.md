# Common responsibility and recovery model for R1-R4

Recorded before the overlapping R3/R4 implementation. The parent RETURN now
includes four accepted findings. This is a bounded implementation model, not an
amendment to the parent's product/recovery contract.

1. **Ready intent versus admitted ownership.** A board commitment is versioned
   selection intent. Accepted owner departure can release it atomically before
   execution admission (R1). A claimed task, admitted invocation, or unreleased
   workspace reservation is an actual outstanding responsibility. Removing its
   participant cannot erase it; selection must exclude that participant from
   competing work even after its backend future fails (R4).
2. **Durable obligation versus permission.** A recovery stage binds the exact
   task attempt, plan/result, saved response and reviews. Failure and attempt
   history are cumulative. Owner Wait/Pause, policy limits, admission denial,
   unavailable participants and effect uncertainty are distinct typed causes.
   Membership invalidates selection; only availability can be reconsidered
   automatically through ordinary policy/admission. Membership never creates a
   manual permit (R2). Explicit continuation cannot waive effect uncertainty.
3. **Inspection versus replay.** Inspection is an independently assigned,
   ordinarily admitted read-only operation. A model's approval is review evidence,
   not proof of termination or complete effect scope. Safe continuation requires
   trusted termination/effect facts, exact stage/result binding and retained
   resolution evidence alongside the original failures (R3). A local snapshot
   alone cannot exclude unobserved remote effects from historical WriteAll access.
   Unknown scope or termination therefore remains unresolved until applicable
   trusted evidence is available. Inspection must preserve owner holds and cannot
   manufacture acceptance, confirmation, reputation or a resource reset.
4. **Work-wave progress versus unresolved work.** After an execution failure,
   genuinely independent tasks may use free eligible participants through normal
   allocation, access and budget admission. Durable responsibilities, not just
   the current wave's temporary set, determine occupancy. Existing claim_busy and
   access checks remain the final transactional protection. Known-ended failed
   work reaches an explicit inspection/rework boundary; bookkeeping alone must
   not force an otherwise unnecessary run restart (R4).

## Verification sequence

R1, R2 and R4 are committed with focused checks. R3 now has a working public
inspection consumer, actual admitted local-scope evidence, six positive/rejecting
scenarios and a source mutation control. All 31 session recovery scenarios and
the preserved revision/arbitration probes pass. The final required
fmt/strict-Clippy/workspace-test chain follows the complete corrected source.

## Evidence gap in the unchanged R3 probe

The source comment states that its scripted failure has no remote execution, but
the historical public records contain WriteAll and BackendEnded without a typed
complete effect-scope assertion. The unchanged probe also calls no inspection API
(none existed). Its original assertions and artifacts remain immutable. A new
typed trusted evidence boundary must not infer local-only effects from a provider
name or accept a model's unsupported safe flag merely to make that probe finish.
The parent explicitly accepted this distinction: preserve the original probe as
insufficient-evidence history, and judge recovery by a separate positive API test
with a supported trusted evidence source plus a durable negative waiting test.
The implemented source is a compiled backend enforcement declaration captured
with real invocation/access records, not a model flag, Mock exception or fabricated
Store resolution. The original uncertainty remains linked to its inspection.
