# YMP-155 final functional session audit

Observed session: `6e286a3a-af64-4897-8209-87ee82026ee6` in the owner-provided
`~/Downloads/_ymp3`, using installed ymp 0.4.6. Observation was read-only. No
application command, provider call, file change, process cancellation, session
resume or credential read occurred. Monitoring stopped immediately after the
session was terminal with no active assignments.

At 2026-09-14T01:29:31Z the session remained blocked at event sequence 2579.
Five of six tasks were accepted but unconfirmed; the sixth remained in review.
All 14 outer invocations had ended. Their intervals occupied 44:04 of the 44:11
run and did not overlap. The task graph was nearly sequential and all 15 recorded
waits were dependency waits, so this case does not show lost useful parallelism.

The final GLM review message contains commentary with CSS braces followed by one
valid final review object with `approved: true`. The current parser tries the
first balanced brace block, `{overflow-x:hidden}`, and returns malformed JSON
before reaching the decision. Runtime acceptance therefore did not occur. YMP-159
owns the parser correction and its ambiguity/malformed controls.

Recovery source now in main can restore the completed invocation message into a
durable stage and parse it again. Consequently the fixed parser can consume the
stored review without another model call. The original review remains attributed
to GLM. Any explicit replacement of GLM by Sonnet changes the current team for
future work; it does not rewrite historical authorship.

Sonnet was not excluded during initial allocation. The preference-sorted eligible
list included it second, while GLM was last. The built-in final-review reservation
iterated candidates in reverse and selected GLM. YMP-160 owns a regression and
the product decision for equal-score role reservation. The new `/team` controls
also allow the owner to replace the current idle GLM participant explicitly in
the existing session.

Luna made 93 native model responses and 86 tool calls across seven outer
invocations. Native records exactly reconcile with ymp's stored per-invocation
counts: 11,084,090 input, 10,825,984 cache-read, 88,216 output and 36,991
reasoning tokens. Reasoning is contained in output; cache-read is contained in
input. There was no duplicate terminal-snapshot addition or cumulative-baseline
reset. The final verification alone made 33 responses and 32 tool calls, largely
around Chrome, screenshots and viewport checks, with a long reused context.
Monetary cost was not established and the owner deferred token-display changes.

All seven GLM usage observations are partial because ACP reports only the final
native request in each outer call. Their known figures are 239,963 input and
13,565 output, not complete usage. Luna reported native `xhigh` although requested
and sent effort were absent; the origin of that native default is unknown. GLM
reported a binary `on`, not a graded effort. These facts support YMP-157/158
session-statistics coverage requirements rather than a billing conclusion.

Five stale board proposals were delayed attempts by Luna to accept responsibility
for a task already assigned or completed by the time the proposal was processed.
They changed neither assignment nor settings. The UI audit separately confirmed
that accepted tasks are counted but filtered from sidebar rows, notices append
outside message chronology and some board events visually resemble failed work.
YMP-156 owns those projection corrections.

The GLM reviewer reported that its terminal/browser verification commands were
denied. This is consistent with the ACP read-only permission handler, but the
exact permission exchange was not retained. The browser evidence is therefore
not fully independently re-executed. Five earlier task acceptances remain
unconfirmed even though producer checks passed.

The recommended test progression is a new-directory single-file counter, then a
separate independent-work case for parallel scheduling, a fifteen-puzzle and the
fixed Holdem request. Each new native run needs its own bounded authorization.
The audit itself does not authorize one. Before the counter, complete YMP-159 and
YMP-160, package/install the accepted recovery UI and parser, then handle the real
poker session through one reviewable owner action.

Linux, real recovery after installation, Sonnet execution and actual user-facing
terminal rendering were not verified by this audit. Raw native paths and private
session projections remain outside the repository. This file retains only the
minimal findings needed for implementation and acceptance.
