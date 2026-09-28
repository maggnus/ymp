# Stable final acceptance and audited reporting

Canonical status remains in `tasks/records/W1-0013.json`. Application exposes
Finalization under the original SessionControl. Strategies propose context,
reviewer selection, narration and claim audits; the kernel validates their exact
inputs, effective policies and state transitions, including replay.

## Final scope and acceptance

Capture first checks physical write ownership affecting the selected target.
Revoked, disconnected and pending work with unresolved overlapping effects yields
Blocked(effects_uncertain), before a snapshot or final acceptance. Work in an
unrelated physical root does not impose a global stop. A canonical read fence
holds the target from capture through report delivery. Admission and persistent
workspace ownership reject overlapping writers, including attempts to reuse the
fence identity as an assignment. These guarantees concern managed ownership;
they do not certify the absence of arbitrary external filesystem writers.

The current slice supports one Direct target. Its captured tree must match a
retained accepted tree, and every accepted source artifact must still have the
same digest. FinalAggregate records the complete accepted source set and producer
union, exact snapshot, current contract, criterion/check versions and selected
check environments. A common source baseline is retained only when unambiguous.
Missing integration is an explicit denial, not a synthetic merge. The aggregate
is a final acceptance subject; it is not a fabricated production ResultVersion.

All active checks are rerun against that immutable final snapshot in their
recorded environments. Every conclusive final run needs canonical Evidence;
an Error is not artifact contradiction. Candidate evidence and earlier review
cannot establish final acceptance. A shared iterative applicability traversal
propagates the same exact subject, criterion, check and environment boundary
through evidence and review dependencies. Final A8 assessments require the exact
aggregate context; candidate assessment contexts retain their existing behavior.

ReviewerPolicy chooses an eligible existing team member outside the entire
producer union. AnyNonProducer orders by stable identity; LeastUsedReviewer orders
by actual prior review count and identity. No candidate records
Blocked(final_review_pending); this slice does not grow the team. FinalReviewer
uses a separately admitted Verification assignment, with target-scoped read access
and an exact recorded context. Its verdict is consumed only from the original
completed, settled, closed invocation and retained output. A generic approval or
candidate review is insufficient.

A7 records final acceptance separately from A8 satisfaction and confirmation
grade. Applicable executable contradiction defeats approval. Satisfied is not
an additional acceptance precondition; weak evidence can leave an independently
accepted result Unconfirmed, with unmet criteria still visible. Final checks and
review precede ReportingStarted, which cannot reopen production or verification.

## Context and report authority

CriteriaProjection includes actual assignment targets, criterion texts, visible
check definitions and bounded planning-decision references. The input and Decision
retain the journal digest; it is not repeated in the prompt. CompactContext retains exact
check references instead of full definitions. Both preserve attributable agent
instructions as quoted untrusted content, with no user or grant authority.
The public context builder includes retained snapshot file references and at most
8 KiB of file bytes in total; replay validates each retained file against its
snapshot and the finite total Prompt bound. Remaining content has exact references and target read
access. Hidden checks are excluded from this visible projection; the complete
hidden-content boundary remains W2-0001.

Continuation is controlled by the original owner capability. Stop is sticky:
Stop → NoAuthority → Continue cannot resume model calls. Resolved ordinary
blocking can clear when the corresponding final stage succeeds; cancellation
persists. StopPreserving from W1-0012 already enters deterministic reporting and
cannot be upgraded to narrated reporting.

Narrator and DeterministicReport use the same NarrativeComposer consumer.
Narration is an admitted Reporting assignment with an exact context, estimate,
reservation and original paid output. At most one correction can reference an
Unsupported audit of the initial paid draft. Admission and execution recheck
current policy, continuation, profile and finite bounds; a prepared request cannot
outlive its authority. No authority, stop, failed/unavailable narration or
indefensible funds select the zero-call deterministic path. The dispatcher that
orders these consumers remains separate work.

EvidenceClassRules validates typed assertions against canonical wording and
applicable facts. Accepted requires the current final Acceptance and grade;
Executed requires a passing applicable run on the final snapshot. ExactBytes
supports a byte-comparison statement, not a claim that the candidate program
started. Causal requires a failing baseline and passing final run of the same
check/version and environment. Recommendations reference a recorded diagnosis or
unmet criterion. Browser and universal Property assertions have no delivered
evidence source here and are rejected. ConservativeAudit additionally excludes
execution and causal statements, providing a materially different policy through
the same consumer. This is a bounded typed report format, not an unrestricted
natural-language truth detector.

Every claim is audited. Unsupported claims remaining after correction or in the
fallback become fixed uncertainty statements. Delivery rechecks previously Valid
claims against current scope, so contract changes after an audit cannot publish
stale acceptance. Free text prefixed with “Uncertain” does not bypass the audit.

ReportDelivered derives outcome, criterion texts, unmet requirements, assumptions,
retained result and acceptance references, spent/held resources, receipt references
and cost uncertainty directly from current kernel state. Narration and correction
are accounted before delivery, without another call to describe expenses.
Verified never-started/released work is not mislabeled as unknown cost. Blocked
and Cancelled outcomes are preserved. A deterministic report without a final
aggregate still explains retained work; it cannot replace an unfinished artifact.
Report delivery does not wait for learning.

## Evidence and limits

`crates/ymp-storage/tests/final_report.rs` exercises actual SQLite, accepted
production, stable final capture, fresh checks, independent paid final review,
final A7/A8, paid narration, one correction and audited delivery. It exercises
both implementations of all four ports through their consumers. The initial
successful combined run took 302.10 seconds and settled 12 fixture cost units.
This is Scripted execution with real kernel/storage/check boundaries, not native
inference or evidence of a self-organization benefit.

A separate 9.86-second scenario rejects a pending Narrator assignment after policy
change with the exact narration_authority denial, then executes a failed Narrator
and delivers deterministic facts: spent 2, held 0, no unknown cost, unmet criterion
retained, one actual invocation. `tests/finalization.rs` checks scoped effects,
the final fence, sticky stop and zero-call Cancelled reporting. Removing the stop
guard caused the expected failure in 1.90 seconds; restoring it passed in 2.56
seconds. This negative control concerns stop authority, not a demonstrated write
bypass. Independent code and documentation review returned R1(9/10) ACCEPT. In
the final mandatory workspace run, both report scenarios passed on the final code
in 298.60 seconds, including the mismatched causal-environment assertion; the
scoped finalization scenario passed in 2.56 seconds. Final `make verify` exited 0:
legacy scan, offline workspace build, formatting, Clippy with denied warnings and
all workspace tests. Existing planning, progress/recovery, restart and
result/acceptance scenarios also passed. The canonical task register and staged
diff checks passed. No native inference ran for this task.

Native execution, automatic Dispatcher operation, multi-workspace merges,
hidden/mutation checks, Browser/Property evidence and learning are not delivered
by this task. W1-0014 still owns ordinary candidate-scoped Verify/Diagnose
non-artifact commitment completion. FinalReviewer Review(None) and Narrator
use the already connected non-artifact path. Earlier journal shapes and references
remain unchanged; new finalization facts use their own recorded projection.
