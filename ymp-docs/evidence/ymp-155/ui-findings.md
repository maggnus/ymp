# YMP-155 UI findings and follow-up

Claude reviewer 8d8b7748 completed a read-only projection audit. Five offline tests
on artificial events passed (exit 0); the test log is retained as ui-probe.txt.
The original detailed report remains at /tmp/ymp155-ui-audit/report.md. The actual
owner terminal was not captured, and runtime selection/native usage are a
separate audit by 52111c72.

Confirmed mechanisms: stored messages are chronologically consistent in the
observed session, but notices always append after messages; a chat message from
an author can prematurely close that author's live writing block. Arrival-order
message insertion has an additional latent ordering defect, not observed in this
real case. Accepted tasks remain in storage and /tasks but are excluded from the
sidebar list. Routine stale board proposals look like failures despite arriving
after accepted work. Native todos are currently discarded; they cannot be merged
with the authoritative task graph as if already integrated.

The parent accepts targeted follow-up: stable message sequence, sequence-anchored
notices, streaming lifetime bound to actual invocation completion, retained
accepted task rows with appropriate small-screen visibility, and consistent
board-event wording. These existing defects are separate from the recovery UI146
candidate and must not be used to falsely reject unchanged behavior in that review.

The audit proposed an inline total-plus-cache figure. The owner's subsequent
instruction supersedes that presentation: repeated cache traffic must not dominate
the headline or imply equal-price expenditure. The parent defines the accurate
follow-up in the usage presentation contract rather than labeling all noncached
tokens as money actually paid.
