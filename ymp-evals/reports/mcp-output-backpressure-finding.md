# MCP signal shutdown with an undrained output pipe

2026-09-13 — Parent consumer review: **R3(6/10) RETURN** for YMP-123's process-exit contract. The earlier input-side shutdown correction remains valid; this is a distinct output-side condition in the same transport lifecycle.

An actual default read-only MCP process was initialized with isolated mock/low configuration. The client sent 100 `tools/list` requests, stopped reading stdout while keeping both pipes open, then sent SIGTERM. The process did not exit within seven seconds. The parent killed only its own temporary subprocess afterward. No execution tool or model was invoked.

```json
{"stdin_open":true,"stdout_open_and_not_drained":true,"read_only":true,"exit":null,"timed_out":true,"elapsed_seconds":7.008}
```

Command: `/tmp/ymp-mcp123-sdk/bin/python /tmp/ymp-mcp-output-exit-probe.py /Users/maggnus/Code/ymp2/target/debug/ymp` — **exit 1**. Script and complete failure log are retained at `/tmp/ymp-mcp-output-exit-probe.py` and `/tmp/ymp-mcp-output-exit-probe.log`. The binary was built from accepted main `3b4045f`; its public MCP implementation is unchanged in current main `c46a454`. The new YMP-125 contract ingress does not alter that transport implementation.

Source inspection identifies uncancellable output waits as the condition to correct: signal selection surrounds the input wait, while response writes/flushes use Tokio stdout outside that selection. The runtime must remain able to cancel owned work and exit when output cannot progress. A normal SDK client continuously drains output, so the prior SDK and open-input-pipe controls do not distinguish this case.

Required correction: preserve bounded queues, full byte/UTF-8 framing and ordinary response delivery, while making output backpressure interruptible and preventing runtime teardown from waiting on a blocked output worker. Actual SIGTERM/SIGINT controls must keep output undrained for idle/read-only and active mock runs, require bounded exit and inspect terminal operation/invocation accounting. EOF, broken pipes, partial writes and the existing input/framing controls remain in scope. No new service or native inference is needed.

The parent authorizes one bounded review extension, at most two further returns, for this exact lifecycle condition. It does not lower acceptance or reopen unrelated protocol, configuration, authority or UI design. Earlier accepted source and failing observations remain recorded; YMP-123 is reopened until the corrected consumer passes independent review.
