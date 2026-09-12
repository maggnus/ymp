# YMP-123 independent review

Reviewed commit: `1fa6f735f517d7437bdf0f9e56b0bb5c9ec8acdb`.
Review timestamp: **2026-09-12 16:26:33 UTC / 2026-09-13 00:26:33 HKT**, read from the actual clock.
Reviewer: independent assigned review agent `/root/review_mcp_123`.

**Verdict: REQUEST CHANGES. Score: 8/10.** One reproducible process-shutdown defect blocks acceptance of the documented signal/exit contract. The public protocol walk, authority boundaries, shared runtime execution, durable request handling, and bounded projections otherwise passed the checks below. This review covers the YMP-123 delta; it does not independently accept its backend ancestry or the combined release.

## Actionable blocker

### P2 — SIGTERM leaves the process alive while its stdin pipe remains open

Location: `ymp-rust/crates/ymp-runtime/src/public_mcp.rs:518` and the implicit runtime teardown in `ymp-rust/crates/ymp-cli/src/main.rs:116`.

With a real stdio process initialized and a mock run admitted, send SIGTERM while keeping the client's stdin pipe open. The process remains alive beyond the five-second cleanup allowance. Two independent subprocess consumers timed out after seven seconds. In the diagnostic reproduction, durable cancellation itself succeeded: the operation was terminal `paused`, and both admitted invocations were closed. Closing stdin then allowed the same process to exit **0**, with no stderr output.

Observed diagnostic output:

```json
{"stdin_open":true,"sigterm_wait_seconds":7.01,"timed_out":true,"exit":null,"operation_status":"paused","operation_ended":true,"invocations":2,"all_invocations_closed":true}
{"after_stdin_close_exit":0,"stderr":""}
```

The exchange loop drops a read from `tokio::io::stdin()` when it receives the signal. That API starts an uncancellable blocking read; dropping its future does not stop the read thread. The `#[tokio::main]` runtime subsequently waits for that thread at shutdown. This matches the documentation in the locally installed, lockfile-selected primary source, `tokio-1.53.1/src/io/stdin.rs`, lines 15–20. This is a process exit defect, not evidence of continued model execution after cancellation.

Use an input strategy whose shutdown does not leave a runtime-owned blocking read outstanding, or explicitly bound runtime teardown after owned execution cleanup. Add actual-process checks for SIGTERM and SIGINT with stdin deliberately left open, for both idle inspection and active mock execution. Assert process exit within the documented grace and preservation of terminal invocation/operation accounting. The existing official SDK context closes stdin during teardown and therefore cannot distinguish this defect.

Reproduction from the reviewed worktree, using the existing official SDK environment only for fixture helpers; this diagnostic intentionally expects the broken behavior:

```sh
/tmp/ymp-mcp123-sdk/bin/python - <<'PY'
import importlib.util, json, sqlite3, subprocess, sys, tempfile, time
from pathlib import Path
sys.dont_write_bytecode = True
sys.argv = ['review', 'target/debug/ymp']
spec = importlib.util.spec_from_file_location('fixture', 'ymp-evals/mcp/stdio_client.py')
f = importlib.util.module_from_spec(spec)
spec.loader.exec_module(f)
with tempfile.TemporaryDirectory(prefix='ymp123-', dir='/tmp') as tmp:
    home, project = Path(tmp)/'h', Path(tmp)/'p'
    project.mkdir()
    f.config(home)  # only mock provider, both agents fixed to mock/low
    params = f.params(home, project, True)
    p = subprocess.Popen([params.command, *params.args], env=params.env,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    def rpc(value, reply=True):
        p.stdin.write(json.dumps(value)+'\n')
        p.stdin.flush()
        if reply:
            return json.loads(p.stdout.readline())
    try:
        rpc({'jsonrpc':'2.0','id':1,'method':'initialize','params':{
            'protocolVersion':'2025-06-18','capabilities':{},
            'clientInfo':{'name':'review','version':'1'}}})
        rpc({'jsonrpc':'2.0','method':'notifications/initialized'}, False)
        rpc({'jsonrpc':'2.0','id':2,'method':'tools/call','params':{
            'name':'ymp_run_v1','arguments':f.start_args('sigterm')}})
        time.sleep(.06)
        p.terminate()
        try:
            p.wait(timeout=7)
            timed_out = False
        except subprocess.TimeoutExpired:
            timed_out = True
        with sqlite3.connect(home/'state.sqlite') as db:
            op = next(json.loads(v) for v, in db.execute(
                "SELECT value FROM kv WHERE key LIKE 'public_mcp:v1:%'"))
        print({'stdin_open_timeout':timed_out, 'operation_status':op['status']})
        p.stdin.close()
        print({'after_stdin_close_exit':p.wait(timeout=3)})
        assert timed_out and op['status']=='paused'
    finally:
        if p.poll() is None:
            p.kill()
            p.wait()
PY
```

## Commands and observed results

All commands ran in the worktree at the exact commit above. No production source, registry, intent, or UI was changed; only this report is added. All execution fixtures used mock/scripted providers. No native authentication probe or real-provider inference ran.

| Command / consumer | Observed result |
| --- | --- |
| `git rev-parse HEAD` | Exact commit above; worktree initially clean |
| `cargo build -p ymp-cli` | Exit 0; actual executable rebuilt before client checks |
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0; all workspace test targets passed |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp` | Exit 0; official SDK 1.28.1, protocol 2025-06-18, 177 tool calls, largest measured reply 73,149 bytes |
| Independent `/tmp/ymp-mcp123-sdk/bin/python -` consumers described below | Scope/version and concurrent ownership controls passed; final short-path consumer exit 0 |
| Actual-process SIGTERM assertion with stdin open | Exit 1: seven-second `TimeoutError` |
| Diagnostic subprocess SIGTERM reproduction above | Exit 0 while proving timeout, saved cancellation, then exit after EOF |
| `target/debug/ymp mcp --help` | Public launch flags available; internal `--socket` dispatch retained |

The author's five mutation-control tails in `ymp-evals/mcp/failing-controls.json` were inspected: execution guard, project guard, payload fingerprint, cancellation signal, and knowledge projection removal each recorded exit 1 at the claimed distinguishing assertion. Those source mutations were not repeated in this read-only assignment. The author-reported 13 bridge tests were not separately repeated; the full Rust suite and actual internal-bridge client discovery were independently exercised.

## Independent consumer checks

The additional Python consumers reused the checked-in isolated mock configuration and SDK process-launch helpers, with new assertions and database fixtures. They did not invoke the author's complete walk as their test oracle.

- **Two simultaneous official clients, same project/client namespace:** client A started `owner`; client B retried identical arguments and received the same session ID with `interrupted_or_running_elsewhere`. B's cancellation was `not_owned`. A's second request was `execution_busy`. B's distinct new request reached a terminal `interrupted` operation with the shared runtime's working-directory lock error and no new session. A could cancel its original operation to `paused`; exactly one session existed.
- **Full-source version beyond the excerpt:** inserted an unconfirmed fixture whose content was 1,500 `a` characters followed by `OLD`. After reading its 1,024-character projection, changed only the unseen suffix to `NEW`. The old version was unavailable; resolving without the old version produced an identical visible entry and a different source digest. This discriminates full-source versioning from hashing the truncated response.
- **Claim/session-create gap:** after closing the owning facade, changed only the isolated saved operation's session pointer to a nonexistent UUID and its status to pending, retaining its real normalized fingerprint/key. Reopening and retrying identical arguments returned the orphan handle, null session, and unknown foreign liveness. It created no invocation or session.
- **Independent negative control:** a strict public discovery oracle accepted exactly the five public versioned tools and rejected the tool set returned by the actual internal `--socket` bridge. The bridge used only a synthetic string, never a live capability. This control checks consumer discrimination; it is not a replacement for the author's server mutations.
- **SIGTERM with an open input pipe:** the failing consumer and follow-up diagnostic above distinguish saved execution cancellation from actual process termination.

The official executable walk additionally passed foreign project/client references, read-only invocation counts, malformed frames, oversized input, duplicate RPC IDs, explicit payload conflicts, cancel/resume, accepted-but-unconfirmed outputs, persisted usage, graceful disconnect/reopen, SIGKILL recovery, pagination, stale versions, and stdout framing.

## Source assessment

`Facade::scoped_session` validates session ownership before inspection/resume. Execution input structs reject unknown fields; callers cannot add a project path, permission grant, provider override, or client identity through tool arguments. `--client-id` is correctly documented as a local deduplication/audit namespace, not authentication. The public tool dispatcher contains no internal assignment-grant operations.

`Facade::start` requires launch-time execution authority, enforces lower turn/timeout limits, and atomically claims a hash of the structured project/client/request tuple before execution. Its fingerprint covers normalized typed arguments. `Engine::run_identified` uses the same startup/allocation validation and insert-only session creation as the shared path. Resume checks the captured project directory and retains the existing budget. Runtime project/session locks, admission and grant issuance, evidence/acceptance, and usage accounting remain in the shared engine/storage paths; no separate execution backend or acceptance bypass was introduced.

Knowledge resolution checks the complete typed stored entry before projection. The shared storage projector bounds metadata, omits snapshot/check bytes, marks loss, and returns continuation for byte-shortened pages; public and internal memory search use it. The final public response guard accounts for text/structured duplication. The documentation honestly excludes a bounded SQL scan, frozen pagination, complete truncated text, and strict native token/currency guarantees. Saved session status is historical data; the operation observation uses process-owned task state and does not promote a foreign pending/running record to live authority.

## Non-blocking limitations and follow-up

- Cancellation preserves recorded progress and spend but cannot roll back writes in the selected directory. SIGKILL can leave unknown liveness and uncertain work; explicit resume uses a new request key. The claim/session-create gap remains documented and was independently checked.
- Version 1 starts do not install new objective acceptance contracts. The shared runtime preserves captured contracts on resume and leaves qualitative accepted results unconfirmed.
- Native provider quality/cost, installed desktop-client configuration, and combined-release interoperability remain outside this mock-only review. They are not inferred from successful SDK framing.

After the SIGTERM/SIGINT process-exit control passes at a new exact commit, the remaining evidence supports accepting the YMP-123 facade within these stated MVP limits.

## Author response to R1

13/09 00:35 HKT — The returned signal-exit defect is addressed by a dedicated
bounded stdin reader outside Tokio's blocking pool. The same four real-process
SIGTERM/SIGINT controls fail with the original reader and pass with the correction,
while stdin remains open and terminal accounting is preserved. See
[the correction evidence](ymp-123-r1-correction.md). This author response does not
change the R1 verdict or claim independent R2 acceptance.

## Independent R2 — accepted correction

Reviewed corrected commit: **`610f99d6932a8f326bc315597faa3be2d6c440a7`**.
Review timestamp: **2026-09-12 16:39:29 UTC / 2026-09-13 00:39:29 HKT**, read from the actual clock.

**R2 verdict: ACCEPT for YMP-123 integration. Score: 9.5/10. No remaining blocking findings in the reviewed facade and correction.** The R1 signal-exit finding is resolved at this exact commit. R1 and its failing observations above remain historical evidence. This acceptance does not extend to later combined-release changes or independently accept the backend ancestry.

### Correction and source assessment

The production diff is limited to the public input reader and byte-frame decoding. `stdin_frames` uses a named standard thread instead of Tokio's blocking pool, so an outstanding operating-system stdin read cannot hold Tokio teardown open. Dropping the join handle intentionally leaves the thread detached; process exit terminates a read that remains blocked. The exchange loop still runs owned execution cleanup before returning.

The `mpsc::channel(1)` allows one queued frame. The producer can hold one additional bounded frame while sending or reading; the consumer can hold its current frame. `Read::take(MAX_FRAME + 1)` bounds each read to 65,537 bytes, and the reader stops after sending an oversized frame. The standard buffered reader adds fixed buffering, not unbounded input accumulation. A blocked channel send ends when the receiving side closes. This preserves backpressure without introducing an unbounded task queue.

Complete frames are parsed from bytes. Split UTF-8 characters therefore survive partial operating-system reads; invalid UTF-8 becomes a JSON parse error and the next valid frame remains usable. EOF forwards a nonempty final frame before closing the channel. I found no changes to execution authority, captured limits, request fingerprints, runtime admission/acceptance, cancellation accounting, output bounds, or the internal socket bridge.

### Independently observed checks

The executable was rebuilt at the corrected commit. A fresh subprocess consumer used its own byte-oriented stdout reader with `select`/`os.read`, explicit five-second process waits, and closed SQLite connections. It reused only the checked-in mock configuration/launch helpers. Every signal assertion left the parent's stdin writer open until after the process had exited. All active cases required a paused operation with an end timestamp and closed admitted invocations with fixed `mock/low` settings. Idle cases required no operations or invocations.

| Independent signal case | Exit | Elapsed | Saved accounting |
| --- | --- | --- | --- |
| SIGTERM, idle | 0 | 0.0038 s | No execution records |
| SIGTERM, active mock run | 0 | 0.0088 s | Terminal paused; admitted invocations closed |
| SIGINT, idle | 0 | 0.0038 s | No execution records |
| SIGINT, active mock run | 0 | 0.0088 s | Terminal paused; admitted invocations closed |
| SIGTERM, active run, reader waiting inside an unfinished UTF-8 character | 0 | 0.0088 s | Terminal paused; admitted invocations closed |

The same independent consumer also passed these directly affected framing cases:

- Split a valid request inside a four-byte UTF-8 character, verified no premature response, and received the complete original Unicode request ID after the suffix/newline arrived.
- Sent malformed UTF-8, received `-32700`, then successfully consumed 100 pipelined valid requests in order with no dropped or duplicated IDs.
- Sent an exactly 65,536-byte newline-terminated valid request and received its normal result.
- Sent 65,537 bytes with no newline, received `-32600`, and observed exit 0 with stdin still open.
- Sent a valid final request without a newline, closed stdin, received its result, and observed exit 0.

Read-only wire cases created no invocations. The entire independent subprocess/framing consumer exited **0**.

| R2 command / evidence | Result |
| --- | --- |
| `git rev-parse HEAD` | Exact corrected commit above; initially clean worktree |
| `cargo build -p ymp-cli` | Independently rerun, exit 0 |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp --signals-only` | Independently rerun, exit 0; four cases exited in 0.005–0.008 s with stdin open and terminal accounting |
| `/tmp/ymp-mcp123-sdk/bin/python -` independent raw-process signal/framing consumer | Exit 0; results detailed above |
| `git diff --check` | Exit 0 before this report append |
| Author's exact-source fmt, clippy, 270-test workspace suite and full official SDK walk | Reviewed in `ymp-123-r1-correction.md`; not gratuitously repeated for this narrow R2 |

I inspected `signal-exit-controls.json`: restoring the original reader makes all four permanent signal cases time out with otherwise preserved accounting, while the correction makes the identical command pass. This agrees with the independently reproduced R1 failure and the R2 observations; the check distinguishes actual process termination from merely saved cancellation. The old reader was not restored again during R2.

Only this report was appended during R2; no production source, UI, intent, task registry, or main-branch change was made, and no real-provider inference ran. The mock-only interoperability and explicit MVP limitations recorded in R1 remain the limits of this acceptance. Combined integration verification is the next release step.
