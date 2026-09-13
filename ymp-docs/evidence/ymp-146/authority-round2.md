# YMP-146 second-round authority acceptance

Reviewer: `332e0b99-79a7-4255-82ae-be642ff7707e`.
Candidate: `743543f`, executable source `edc779a`.
Verdict: ACCEPT for the statically reviewed authority boundary. No source changes,
builds, tests, native inference or live-session operations were performed by this
reviewer. Executable recovery acceptance remains a separate pending review.

The review found no route violating the approved current-files authorization.
It checked exact session/stage/proposal/team/board/budget/failure binding, runtime
file-context revalidation and transactional state checks; durable idempotency;
fresh-review admission markers preventing duplicate paid calls; CAS on racing
owner controls; and preservation of Wait/Pause through authorization and hold
release. ReleaseHold does not grant replay permission. Current-files authority
covers only the exact acknowledged failures and never fills effect_resolution.
Subsequent uncovered failures retain the ordinary work/admission refusal.
Native context reuse is refused at runtime and storage admission. Changed team
or board state invalidates the presented authorization context, preserving R1/R2.

Important evidence limits remain explicit. Failed pre-admission validation does
not create a receipt or incur a provider call; an admitted fresh-review attempt
is durably marked even when its response cannot commit. A CAS rejection after a
fresh-review call can require another explicitly requested paid review. ReleaseHold
may restore a conservative old reason after an effect inspection. Authorizing one
stage does not clear other stages' holds. Conversation-purpose calls retain their
existing separate admission behavior.

Current-file context construction has a 4096-entry bound and a 4 MiB per-file
snapshot limit; workspace listing exclusions apply. It binds the state presented
for the decision, not a permanent freeze against later external edits. The review
did not validate those exclusions, actual native-process behavior, implicit native
context reuse, Linux or executable scenario results. Backend APIs are not yet
available through the TUI. These limits must survive integration and UI delivery.
