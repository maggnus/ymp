# MCP output-backpressure independent review

**R4 ACCEPT, 9.5/10**, for exact commit **`e4607d775e4dd3b2ed3b532f5a384d837f7cfbff`**, based on **`3d9c5808020bb106cf6ae8f18535ef889b175af3`**. No blocking finding remains in the bounded output-backpressure lifecycle correction.

Reviewed by `/root/review_mcp_123` at **2026-09-12 18:07:57 UTC / 2026-09-13 02:07:57 HKT**, read from the actual clock. This verdict resolves the R3 output-side return at the stated commit. The [R3 finding](mcp-output-backpressure-finding.md), original failing probe log and earlier input-side review remain historical evidence; this report does not rewrite their verdicts or accept unrelated release work.

## Production change and ownership

The production delta is confined to `ymp-rust/crates/ymp-runtime/src/public_mcp.rs`. I read the complete diff, the correction report, permanent consumer changes and before/after evidence. The remaining changed files are public transport documentation, test consumers and evidence. No global CLI, storage, configuration, provider, dependency, authority or UI implementation changed.

At `public_mcp.rs:513`, signal selection now encloses the entire protocol exchange. Input receives, output-channel sends and delivery-acknowledgement waits are consequently interruptible. After the exchange completes or is dropped, `facade.shutdown()` still cancels owned execution and allows the existing bounded runtime cleanup. Dropping the transport does not skip that cleanup.

`StdoutFrames` uses a dedicated standard thread, a capacity-one channel and one acknowledgement per frame. The exchange awaits that acknowledgement before processing another input frame. The writer calls `write_all` and then `flush` before acknowledging success; IO errors reach the exchange instead of being treated as delivered output. Normal EOF therefore follows delivery of the preceding response, including responses on the parse/error paths.

The 128 KiB response allowance plus newline is checked before copying a frame into the output channel. The queue has capacity one, and the sequential acknowledgement protocol prevents accumulating additional output requests. An encoded response and its worker copy can coexist, but their sizes remain bounded. The existing input queue and `MAX_FRAME + 1` read bound are unchanged. There is no unbounded worker/task queue or background model work introduced by the transport.

Both workers are outside Tokio's blocking pool. On a signal, abandoning an acknowledgement does not require joining a thread blocked in operating-system output. A queued acknowledgement already closed by cancellation is checked before writing; an IO failure ends the writer loop. A signal may interrupt an undelivered frame, as the updated documentation states. Ordinary EOF delivery assumes the client continues draining stdout; the signal path can abandon delivery when that is no longer possible.

## Independently run commands

All commands ran in the clean corrected worktree at the exact commit above. The binary was rebuilt first.

| Command / consumer | Observed result |
| --- | --- |
| `cargo build -p ymp-cli` | Exit 0 |
| `/tmp/ymp-mcp123-sdk/bin/python /tmp/ymp-mcp-output-exit-probe.py target/debug/ymp` | Exit 0; original parent probe now exits 0 in 0.004 s with stdin open and stdout undrained |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp --output-only` | Exit 0; four blocked-output signal cases, complete slow-drain EOF delivery and active broken-pipe cleanup passed |
| `/tmp/ymp-mcp123-sdk/bin/python -` independent raw-process consumer | Exit 0; all six independent cases below passed |
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp` | Exit 0; official SDK 1.28.1, 321 tool calls, maximum measured normal reply 73,149 bytes |
| `git diff --check` | Exit 0 |

The author reports a passing **297-test** workspace run in [the correction report](mcp-output-backpressure-correction.md). I did not repeat the complete workspace suite for this transport-only review; formatting, clippy, the actual executable, the full SDK walk and the new independent lifecycle consumers were rerun as recorded above.

## Independent blocked-output and delivery consumer

The additional consumer used raw `subprocess.Popen` pipes, `select`/`os.read` and explicit process deadlines. It reused only the checked-in isolated mock configuration and launch helpers, not the permanent test assertions. Each case used a separate temporary home/project. Active fixtures pinned both agents to `mock/low` and used scripted review rejection to keep execution pending. Before each active signal, the consumer checked that the operation had not ended and an invocation was actually running.

After initialization and, where applicable, the start acknowledgement, the client sent **100 `tools/list` requests** with distinct Unicode request IDs. It stopped reading stdout and kept both pipes open for signal cases. The independent broken-pipe case closed stdout only after this pressure had already built, exercising an outstanding output write rather than merely rejecting a first write to a closed descriptor.

| Independent case | Exit | Elapsed after signal/closure/drain start | Durable observation |
| --- | --- | --- | --- |
| SIGTERM, read-only, stdout undrained | 0 | 0.0039 s | No operations, invocations or grants |
| SIGINT, read-only, stdout undrained | 0 | 0.0013 s | No operations, invocations or grants |
| SIGTERM, active run, stdout undrained | 0 | 0.0092 s | Paused operation; 2 ended invocations; 2 revoked grants |
| SIGINT, active run, stdout undrained | 0 | 0.0091 s | Paused operation; 2 ended invocations; 2 revoked grants |
| Close blocked stdout during active run, leave stdin open | 1 | 0.0088 s | Paused operation; 2 ended invocations; 2 revoked grants; broken-pipe diagnostic |
| Close stdin, drain stdout slowly in 257-byte reads | 0 | 0.1711 s | No execution records; all expected replies preserved |

For every active case, the operation had an end timestamp, every invocation was non-running with an end timestamp and fixed `mock/low` settings, and every durable grant was revoked. Read-only cases had no invocation or grant growth. Signal-case pipes remained open through the process-exit assertion. Signal and normal-EOF cases had empty stderr.

The slow-drain case appended a malformed JSON frame after the 100 Unicode discovery requests, then closed stdin. It received **101 complete JSON responses totaling 329,666 bytes**: all 100 original Unicode request IDs in order, each with the five public tools, followed by the expected null-ID `-32700` parse error. There were no missing, duplicate or truncated frames. This checks acknowledgements on both ordinary replies and the final error path, rather than checking only process exit after EOF.

## Falsifier and retained behavior

The original parent failure log at `/tmp/ymp-mcp-output-exit-probe.log` still records a 7.008-second timeout and the failed assertion that shutdown must not depend on draining stdout. The identical parent probe now passes against the rebuilt corrected executable.

I also inspected `ymp-evals/mcp/output-backpressure-controls.json`: with only the old `public_mcp.rs` restored, all four permanent signal cases time out at five seconds; restoring the correction makes the same command pass. The active broken cases fail the expected paused-accounting condition as well. Those mutations were not repeated during this read-only assignment. The existing failing observations and independently rerun corrected consumers provide the distinguishing evidence without replacing history with successful-only logs.

The full SDK walk retained open-stdin signal shutdown, malformed/oversized input, fragmented UTF-8, bounded projection, read-only scope, durable request conflicts, explicit start/resume, configured contracts, cancellation, EOF/reopen, SIGKILL recovery and internal bridge compatibility. Those passing integration controls do not broaden this review into a new configuration/provider/UI assessment.

## Review boundary

Only this report is added. No commit, main edit, registry/intent change, source mutation, credential inspection or real-provider inference occurred. The dedicated workers' process-exit behavior and the recorded cleanup latencies are actual local mock observations, not guarantees of native-provider cancellation time or rollback of workspace writes.

The correction satisfies the specified output-backpressure shutdown and normal-delivery conditions at the exact reviewed commit. No additional return is requested; the prior historical verdicts and the parent's bounded review policy remain intact.
