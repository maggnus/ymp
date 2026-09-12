# MCP output-backpressure lifecycle correction

13/09 01:54 HKT — R3(6/10) RETURN from
[the parent consumer finding](mcp-output-backpressure-finding.md) is addressed in
an isolated development worktree based on `3d9c5808020bb106cf6ae8f18535ef889b175af3`.
This report records author evidence; independent acceptance remains pending.

## Bounded correction

The public transport writes through a dedicated standard thread with a capacity-one
frame channel. Every delivery acknowledgement follows `write_all` and `flush`, so
partial writes are completed in order and ordinary EOF cannot overtake the final
response. The protocol awaits each acknowledgement before admitting its next input
frame. Output frames retain the existing 128 KiB allowance plus a newline; input
read-ahead retains the existing one-frame queue and one bounded read.

Signal selection now covers the complete protocol exchange, including input waits,
output queueing, and output acknowledgements. SIGTERM/SIGINT can therefore drop a
stalled exchange, cancel owned runtime execution, and finish the existing five-second
cleanup path. Neither stdin nor stdout leaves a Tokio-owned blocking worker that
runtime teardown must join. A dedicated worker blocked in OS I/O ends at process exit;
a signal can leave an undelivered response incomplete, and durable operation recovery
retains its existing semantics.

No global CLI shutdown change, provider/runtime policy change, storage/configuration
change, new dependency, network service or UI change was introduced. The configured
contract ingress and current StoreLock/concurrency behavior come unchanged from the
base commit.

## Actual-process failing controls

Before source changes, the exact parent probe against the rebuilt base executable
reproduced the original failure:

```sh
/tmp/ymp-mcp123-sdk/bin/python /tmp/ymp-mcp-output-exit-probe.py target/debug/ymp
```

The read-only process had both pipes open and 100 `tools/list` requests with stdout
undrained. **Exit 1:** no process exit after seven seconds. No model was invoked.
The same parent probe with the corrected executable returned **exit 0** and process
exit 0 in **0.004 seconds**, without draining stdout.

The permanent control is:

```sh
/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp --output-only
```

For the final before/after control, only `public_mcp.rs` was temporarily restored from
the exact base commit; the identical permanent client checks were used in both builds.
The original output path produced **exit 1**: all four SIGTERM/SIGINT cases timed out
at 5.001–5.003 seconds. Read-only cases had zero invocations; active mock runs continued
rather than reaching the expected paused cancellation state. Restoring and rebuilding
the correction produced **exit 0** for the complete control.

The corrected cases observed:

| Consumer condition | Process result |
| --- | --- |
| SIGTERM, read-only, both pipes open, stdout undrained | Exit 0 in 0.004 s; no operations/invocations/grants |
| SIGINT, read-only, both pipes open, stdout undrained | Exit 0 in 0.004 s; no operations/invocations/grants |
| SIGTERM, active mock invocation, both pipes open, stdout undrained | Exit 0 in 0.020 s; operation paused, invocations ended, grants revoked |
| SIGINT, active mock invocation, both pipes open, stdout undrained | Exit 0 in 0.038 s; operation paused, invocations ended, grants revoked |
| 100 queued requests, stdin EOF, slow 1,024-byte stdout reads | All 100 complete responses and UTF-8 request IDs delivered in order, including final frame; exit 0 |
| Active mock run, broken stdout pipe, stdin left open | Exit 1 with a broken-pipe diagnostic in 0.009 s; operation paused, invocations ended, grants revoked |

Exact case output is retained in
[`output-backpressure-controls.json`](../mcp/output-backpressure-controls.json).
All execution fixtures use only deterministic mock providers pinned to `mock/low`.
Active pressure fixtures use repeated scripted review rejection to keep work pending
until a real running invocation is observed before the signal. No credential or native
inference is used. The slow reader forces pipe backpressure while checking exact
response framing; the broken control distinguishes process cancellation from merely
having saved terminal records.

## Final validation

| Command | Exit / result |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0; 297 passed, 0 failed |
| `cargo build -p ymp-cli` | 0 |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp` | 0; official SDK 1.28.1, 324 tool calls, largest normal reply 73,149 bytes |
| `git diff --check` | 0 |

The full SDK walk retains input framing/fragmented UTF-8, malformed and oversized
frames, read-only scope, project/client isolation, request conflicts, explicit bounded
execution, configured contracts, confirmed knowledge, immutable resume, cancellation,
EOF/disconnect/reopen, SIGKILL recovery, output bounds and internal socket-bridge
compatibility. It now includes both open-stdin signal controls and the undrained-output
controls. No separate bridge source changed; bridge package tests were not repeated.

This correction preserves the documented MVP limitations and does not claim rollback,
native-provider cancellation latency, real-provider cost/quality or independent review
acceptance. The parent granted at most two further review returns for this output-side
lifecycle condition; this report does not alter that review budget or its verdict.
