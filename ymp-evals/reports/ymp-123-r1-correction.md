# YMP-123 R1 signal-exit correction

13/09 00:35 HKT — R1(8/10) RETURN: SIGTERM/SIGINT cancelled saved execution but
left runtime teardown waiting on open stdin. The bounded correction replaces
`tokio::io::stdin()` with a dedicated standard reader thread and a capacity-one
channel. The common runtime, execution lifecycle, stdout writer and tool API remain
unchanged.

The reader is detached from Tokio's blocking pool, so an idle stdin read cannot keep
Tokio runtime teardown alive. It ends on EOF, read/send failure, an oversized frame,
or process exit. One frame can queue and at most one additional bounded read can be
in progress. The existing 65,536-byte frame allowance plus one overflow byte remains;
channel backpressure prevents unbounded read-ahead. Complete byte frames are parsed
as UTF-8 JSON, preserving split multibyte characters and newline framing.

## Discriminating subprocess check

The permanent `signal_exit_walk` in `ymp-evals/mcp/stdio_client.py` uses actual
subprocesses with separate temporary homes/projects and only fixed `mock/low`
providers. It tests SIGTERM and SIGINT, each after idle inspection and during active
execution. The parent's stdin file stays open until after the exit assertion, unlike
SDK context teardown. Each case requires exit 0 within five seconds. Active cases
also require a terminal paused operation, an end timestamp, and closed admitted
invocations; idle cases require no operations or invocations.

The exact same command ran against the original reader restored from `1fa6f735`,
then against the corrected reader:

```sh
/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp --signals-only
```

- Original reader: **exit 1**; all four cases timed out at 5.001–5.003 seconds,
  while terminal accounting was already preserved.
- Corrected reader: **exit 0**; all four cases exited 0 in 0.004–0.008 seconds,
  with the same terminal accounting assertions satisfied.

The broken source was restored only for the control and the correction was rebuilt
before final checks. Exact case output is retained in
[`signal-exit-controls.json`](../mcp/signal-exit-controls.json). This test distinguishes
saved cancellation from process exit with an open input pipe. It does not measure
real-provider cancellation latency or claim rollback of workspace writes.

## Final checks

| Command | Exit |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0; 270 tests passed, 0 failed |
| `cargo build -p ymp-cli` | 0 |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp` | 0 |
| `git diff --check` | 0 |

The full official-SDK walk now includes the four signal checks. It also retains
EOF/disconnect/reopen, SIGKILL recovery, internal socket-bridge compatibility,
malformed/oversized frames, response bounds, scope controls and execution lifecycle.
A new wire check splits a JSON request inside a UTF-8 code point and delays its
remaining bytes/newline; the response preserves its complete request ID. The largest
observed normal MCP response remains 73,149 bytes.

No provider, UI, registry, intent or unrelated backend source changed. Bridge tests
were not repeated because this correction changes only the public input reader;
the actual internal-bridge client lifecycle was exercised by the full client walk.
The independent R1 report is preserved. Independent R2 acceptance remains pending.
