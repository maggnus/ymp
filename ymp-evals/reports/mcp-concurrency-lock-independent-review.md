# Public MCP, concurrency and lock integration independent review

**ACCEPT, 9.5/10**, for exact HEAD **`376cdc4fc1ba54f96abe0a15d96c4e78e74ddf16`**, whose production source is **`43ba6ccac0212e6de65259270833fee2100f159d`**. No blocking integration finding remains in this composition.

Reviewed at **2026-09-12 17:17:52 UTC / 2026-09-13 01:17:52 HKT**, using the actual clock. Reviewer: independent assigned agent `/root/review_mcp_123`. This verdict accepts the composition of the previously accepted public facade, concurrency/state/location behavior and lock-release correction. It does not accept the complete release or the separate YMP-125 contract-ingress candidate.

## Source and evidence binding

I read `AGENTS.md`, the combined report and evidence ledger, the included YMP-115 combined independent review, the lock correction and independent review, and the relevant production merge diff. The worktree was clean at the stated HEAD.

All **108 files** in `mcp-concurrency-lock-integration.json` were independently hashed and matched. Recomputing SHA-256 over the compact, sorted manifest JSON reproduced **`904a43094a413f9ccbd7255a8538676d5f387b14522d36b49dd0a8c24d1fb7a3`**. All six referenced successful command-log hashes also matched. The successful workspace log's individual test summaries independently total **284 passed**.

The older ENOSPC and cancellation-test failure logs still exist separately and retain their failure output. I did not count either as a passing check or reinterpret the discarded historical redaction error. The independently accepted lock report supplies the bounded ownership diagnosis and discriminating old-unlock controls; this review checks its unchanged source in the final composition.

Independent byte comparisons confirmed:

- Public facade implementation, public schema/documentation and official client script are unchanged from accepted MCP R2 `610f99d6932a8f326bc315597faa3be2d6c440a7`.
- The storage implementation containing `StoreLock` is byte-identical to accepted correction `861bf918168125c526b241c860bad1cc741cf7d5`.
- Both workspace-access implementations, runtime confirmation, storage knowledge/confirmation/budget and storage provenance are unchanged from accepted main `f02a8c2`.
- The main-to-candidate Engine production diff adds only `run_identified`, validates its UUID, threads the optional new-session ID through `run_internal`, and retains the corresponding existing entry-point arguments. The distinct-task follow-up branch still drops context, session lock and project lock before calling `run_internal(path, &task, None, Some(previous), None)`. Module exports retain both public MCP and workspace coordination.

## Integration assessment

The public process still calls the shared Engine for start/resume. It does not allocate an alternate backend, change the captured execution identity, substitute a second continuation store, or bypass common allocation, permission, resource, budget or acceptance checks. Identified starts retain insert-only session creation. Resume obtains the shared project/session locks, interrupts historical open invocations, recovers workspace reservations and reads captured admission limits.

The combined Engine retains the workspace lease across produced-result capture and submission. Verification continues to hold its enclosing reservation through checks, review, acceptance and retention. Native requests must be covered by the trusted workspace-access declaration before admission. The facade's process lease and cancellation token therefore reach these same cleanup paths. Saved statuses remain history: foreign nonterminal request liveness is still reported as unknown, and no public client identity becomes an internal assignment capability.

`StoreLock` owns a private file and explicitly unlocks on drop. It introduces no clone or exposed-descriptor route. Its scope still covers each live run, while the explicit follow-up handoff ends old ownership before a new task acquires it. The independently rerun storage controls verify both release with an inherited descriptor still open and exclusion for the next owner when that stale descriptor closes.

Original locations still come from recorded outcomes/workspace/policy. The new session-ID parameter does not alter that branch. The public results tool continues to project `Store::outcomes` directly, including its actual confirmation and artifact inventory, instead of deriving an output location from mutable project registration.

## Independently run checks

| Command / consumer on the exact HEAD | Observed result |
| --- | --- |
| `cargo build -p ymp-cli` | Exit 0; actual executable rebuilt |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp` | Exit 0; official SDK 1.28.1, MCP 2025-06-18, 190 tool calls, maximum measured response 73,149 bytes |
| `cargo test -p ymp-storage completed_ -- --nocapture` | Exit 0; both inherited-descriptor ownership controls passed |
| `cargo test -p ymp-runtime --test state_integration -- --nocapture` | Exit 0; combined membership, retained responsibility, knowledge-delivery and original-location consumer passed |
| `/tmp/ymp-mcp123-sdk/bin/python -` independent public relocation/reopen consumer | Exit 0; original projection and recorded counts preserved; old launch scope denied new execution |
| `/tmp/ymp-mcp123-sdk/bin/python -` independent process-lease/reopen consumer | Exit 0; expiry paused the run, explicit resume completed under the captured limit, all workspace reservations released |
| `git diff --check` | Exit 0 |

The official walk again covered default read-only admission, foreign references, request conflicts, progress/usage, cancellation/resume, graceful disconnect/reopen, SIGKILL recovery, bounded/versioned projections, fragmented UTF-8, malformed and oversized frames, internal bridge compatibility, and stdout framing. All four open-stdin signal cases exited 0 with preserved accounting: active SIGINT/SIGTERM took 0.006/0.007 seconds; idle cases took 0.002/0.003 seconds. The 190 tool calls versus the author's 171 reflect repeated progress polling in this script, not a fixed execution-count acceptance criterion.

The exact-source author formatting, clippy and full-workspace results were inspected through their verified logs rather than rerunning an otherwise unchanged complete suite. Those recorded commands all exited 0. The targeted tests and actual-process consumers above are newly observed reviewer runs.

### Public relocation and scope control

A fresh official SDK consumer completed a fixed `mock/low` run, saved the full public results page, and changed only the isolated metadata fixture's project registration to a different empty directory. Reading results through the still-open original facade returned exactly the original page, including its original absolute directory and source/result identifiers. A new start through that stale launcher was rejected with `project_relocated`.

Reopening the facade at the newly registered directory returned the same project identity and unchanged original results. Retrying the original operation returned its terminal handle without replay. Sessions/invocations/assignments/decisions stayed exactly **1/8/8/51**. The original `greeting.txt` still contained `Hello from ymp\n`; the newly registered directory remained empty.

This ordinary public-v1 start produced unconfirmed results with an empty artifact inventory. The test asserts the actual inventory and preserves it; it does not infer recorded artifact evidence from a file's existence. An initial overly strong harness assumption that this inventory must contain a path was corrected after inspecting `StoredOutcome` and its source. The separately rerun trusted Engine state consumer supplies the positive recorded-artifact-path and structured-location check. The tracked contract-ingress gap is not reopened or claimed implemented here.

### Lease expiry and shared-runtime recovery

A separate actual-process consumer started a fixed `mock/low` run with `max_turns: 40` and `max_seconds: 1`. After its first invocation was admitted, the harness deliberately suspended the process with SIGSTOP for 1.15 seconds and continued it. This induced expiry of the wall-clock process lease without a real model call.

The operation became terminal `paused`, retained its one admitted invocation, closed running invocation state, and preserved the captured 40-turn limit. After EOF and process exit, a new facade inspected the same handle without adding calls. Explicit resume under a new request ID reused the session and completed with **9 total invocations**, still under the captured **40-turn** limit. Every acquired workspace reservation ID had a corresponding release; all invocations were closed and requested fixed `mock/low`.

This is a controlled lease-expiry/recovery observation. It does not measure real-provider cancellation latency, promise exact native token spending, or establish rollback of selected-directory writes.

## Scope and handoff

No source, UI, intent, task registry, main branch or commit was changed. Only this report is added. All executable review workloads were mock/scripted; the real MCP processes used fixed `mock/low`. No credentials or native authentication were inspected and no real-provider inference ran.

Existing MVP limitations remain: direct selected-directory execution, no rollback guarantee, unknown foreign-process liveness after hard exit, explicit resume, bounded lossy metadata, non-snapshot pagination, and no inferred objective confirmation from qualitative agreement. Previously accepted falsifiers and the preserved R1 signal failure remain evidence; they were not silently replaced with passing-only histories.

**Accepted for integration at the exact HEAD above.** The separate contract ingress and final combined-release/UI verification remain their own assigned outcomes.
