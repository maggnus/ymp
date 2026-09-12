# Public MCP, concurrent execution and lock release integration

2026-09-13 — Combined candidate prepared for independent review. The final production source is `43ba6ccac0212e6de65259270833fee2100f159d`; no combined acceptance or release is claimed here.

Accepted MCP commits `1fa6f735` and `610f99d` map to `704a43f` and `ced4aec`. The independent R2 report is retained by `e630154`. Merge `4c06a63` combines main `f02a8c2`, including accepted concurrency/state source `6eaf6e8`. Its two conflicts retain both module exports and the explicit project-lock handoff together with MCP's optional preallocated session ID. The accepted lock correction `861bf91` maps to `43ba6cc`. No other production change was introduced by this integration.

The main-to-candidate Engine delta is only the reviewed `run_identified` entry point, its validated optional ID and corresponding private call arguments. The location-only branch, workspace coordinator, grants, budget/assignment admission, confirmation and incremental knowledge logic are preserved. Shared session/project locks now release explicitly when their owner ends; a stale inherited descriptor cannot retain or release another owner's authority. Standalone facade R2 and lock R1 acceptance reports are included, each preserving its original failing observations.

| Final command | Exit / observation |
| --- | --- |
| `cargo test -p ymp-storage completed_ -- --nocapture` | 0; both inherited-descriptor controls |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0; 284 tests |
| `cargo build -p ymp-cli` | 0 |
| `/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp` | 0; official SDK 1.28.1, protocol 2025-06-18, 171 tool calls, maximum reply 73,149 bytes |

The complete suite includes the public concurrency/state consumers: confirmed work survives sibling failures, unresolved responsibility is inspected before retirement, knowledge reaches a later session and recorded locations remain unchanged after registration relocation. The actual executable client exercises scoped reads/start/resume, usage, cancellation, reconnect/SIGKILL, bounded/versioned projection, internal bridge compatibility and malformed framing. Open-stdin SIGINT/SIGTERM cases exit 0 with terminal accounting; active cases took 0.008 and 0.010 seconds, idle cases 0.003 seconds. These are mock observations, not native cancellation latency guarantees.

Exact source manifest and all six successful command-log hashes are recorded in [mcp-concurrency-lock-integration.json](../../ymp-docs/research/evidence/mcp-concurrency-lock-integration.json). Its 108-file source fingerprint is `904a43094a413f9ccbd7255a8538676d5f387b14522d36b49dd0a8c24d1fb7a3`. Earlier logs remain separately identified: ENOSPC interrupted one suite; after cleanup another suite exposed the independently reproduced pre-admission lock defect. Neither failure was erased or counted as a successful check. The historical redaction test's discarded error remains unclassified.

No real model inference, credential inspection or UI implementation occurred. Runtime providers are mock/scripted, with actual executable MCP workloads fixed to mock/low. This composition does not establish production isolation, rollback, model quality, comparative efficiency, or the remaining executable contract ingress/UI/board/correction/release outcomes. Main remains separate until independent combined review accepts the candidate.
