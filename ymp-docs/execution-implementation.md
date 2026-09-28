# Bounded assignment execution

Canonical status remains in `tasks/records/W1-0017.json`. The runtime now executes
admitted assignments through `AdmissionRuntime.execution`, which shares the
original Gatekeeper issuer. The executable does not yet expose this workflow.

## Dispatch and authority

`Prompt` retains exact bounded text and committed context references. A prepared
dispatch fixes the admitted Assignment, provider, requested settings, selected
ExecutionBackend parameters, allowance, receipt identity, nonce and deadline.
Invocation and Assignment identities are distinct; a native session ID is an
optional observation, never a substitute for either.

The Journal commits dispatch, financial authorization, optional workspace
authorization and its exact Ready marker atomically. The host calls `start` only
after resolving that complete packet. It retains the original preparation across
a lost acknowledgement. Once a backend call has been attempted, that live
invocation cannot attempt another start. Replay cannot reconstruct its grant,
file capability or local handle. A new controller returns
`invocation_recovery_blocked` for unresolved execution without calling the backend.

Requested, sent and reported settings remain separate. Unreported observations
stay unknown; a conflicting reported setting fails the invocation. File operations
use the existing mediator and recheck current authority. Provider text and
OperationRequest arguments cannot choose another actor. The bounded reply channel
currently returns the owning authority's Denial or an explicit
`operation_transport_unavailable`; W3-0001 owns team-operation transport.

## Supervision and resource accounting

Backend calls run in separate local workers. Polling does not wait for a blocked
backend call. This bounds controller waiting; it does not prove termination of
arbitrary in-process backend code. Cancellation requests and stream loss do not
establish cessation. The host closes new mediated access immediately and releases
path holds only after the mediator drains admitted I/O and the kernel validates
the matching proof. Unknown financial usage can retain its hold independently.

Time, native turns, output and observed cost belong to the whole Invocation.
Events have stable invocation-local sequence numbers and cumulative usage.
Foreign identities, reset counters and oversized output are rejected. CostModel
proposes prices for observed counters and final settlement; partial counters do
not become complete usage. Stop/Estimate remain Treasury policy choices.
Only observed limits can be enforced: unreported provider usage is not a proven
upper bound, and discovered excess is charged honestly rather than truncated.

The host records backend termination separately from its final outcome. Completed
requires settled accounting and validated scoped cessation before the kernel can
record successful completion. Late receipt excess records `cost_limit` and
`Failed(Content)` while retaining the actual charge. Partial usage under Stop
remains blocked. Delayed accounting does not move the observed backend completion
time. A late backend response cannot promote a prior timeout or cancellation to
success. Cancellation cleanup is requested even when a terminal journal write
cannot currently succeed.

The implementation bounds one output chunk to 64 KiB, total output to at most
1,000,000 characters, the requested native-turn allowance to 4,096, the invocation
deadline to at most 24 hours and retained backend observations to 4,096. These are
implementation bounds, not evidence that a native provider enforces them. Journal
control reserves include unfinished start, terminal, receipt and price facts.

## Commitment and result boundaries

Confirmed bounded completion discharges its own non-artifact commitment through
Arbiter's existing P2 consumer. Produce, Alternative and Integrate remain pending
result submission and acceptance. An output saying “accepted” changes neither
authority nor acceptance.

Heartbeat has a real progress consumer. CheckRun progress resolves an exact
retained run, responsibility target and workspace. EvidenceAdded and
ResultSubmitted observations are retained but cannot renew a commitment until
their owning kernel facts exist. Replaying one progress observation cannot renew
the lease twice.

## Evidence and limits

`crates/ymp-storage/tests/execution.rs` exercises real admitted assignments through
the host with Scripted and a separate computing backend. Scripted performs bounded
local steps, including real mediated file writes; neither implementation invokes a
model. Scenarios cover attributable output and settlement, non-artifact discharge,
artifact non-acceptance, whole-invocation limits, late complete and partial usage,
ambiguous start, stream loss, foreign responses, delayed start, cancellation,
invalid public credentials, clock failure, lost dispatch acknowledgement and a
new controller over reopened SQLite. Cancellation is also exercised while the
journal is unavailable. The cost-limit regression fails when its guard is removed.

Real CheckRun progress and exhaustion of the Journal control reserve have been
reviewed but not separately exercised through this host. Native adapters, native
reattachment, external-process cessation and Linux execution have not been tested
by this slice. Those claims must wait for their owning tasks and actual runs.

Before integration, `make verify` completed with exit 0: legacy scan, workspace
build, formatting, Clippy with denied warnings and all workspace tests. The task
register and `git diff --check` also passed. Independent review returned
R2(9/10) ACCEPT after the completion/accounting and stop-classification corrections.
