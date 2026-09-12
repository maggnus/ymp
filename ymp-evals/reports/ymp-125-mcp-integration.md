# Trusted acceptance contracts through public MCP

2026-09-13 — Combined candidate prepared for independent review. This report records integration and verification; it does not grant combined acceptance or release approval.

The integration starts at main `3b4045f292845bb757aa14c8efd4afe74aea6abe`, including accepted concurrency, public stdio MCP and explicit StoreLock release. Only accepted YMP-125 commit `ef9a1254696baef5b1bdbf72a9c1745da3eacc37` was cherry-picked, as `894ab8679617f5d2bd5ab71433d22e5ac147cea6`. Its [independent acceptance](ymp-125-independent-review.md), 9/10, is retained unchanged.

The single source conflict was new-session creation in `Engine::run_internal`: the resolution keeps both `prepare_contracts` and the optional UUID supplied through `run_identified`. Contract capture remains atomic with session/policy creation. The main-to-candidate production delta otherwise consists of the accepted YMP-125 implementation. There is no alternate MCP contract installer or confirmation path.

The following accepted behavior remains present and exercised: preallocated session IDs and insert-only creation; explicit project-lock handoff to a distinct conversation task; concurrent scheduling constrained by backend workspace guarantees; task access and retained production responsibility; inspection of uncertain admitted work on resume; source session/result identities, supported knowledge and original locations; and explicit project/session lock release with inherited-descriptor controls. No TUI source changed.

## Actual executable consumer

The existing `ymp-evals/mcp/stdio_client.py` now includes a configured-contract walk, also selectable with `--contracts-only`. It extracts the documented TOML contract and launches actual stdio processes using the official MCP Python SDK 1.28.1. All workload agents use the built-in mock backend with fixed `mock` model and `low` effort. Configuration/state live outside fresh production directories; no native authentication or real provider inference is used.

The consumer establishes:

- A public `start` returns the same session identity later exposed by confirmed results, captures a trusted contract, observes the check and creates supported project knowledge.
- Launch configuration remains the facade's snapshot after the file changes. A new facade reads updated configuration; the tool cannot replace it.
- `acceptance_contracts`, `acceptance_contract` and `config` tool-argument injections are rejected without a session or invocation.
- A session cancelled after invocation admission retains its captured contract. A replacement facade with changed expected bytes records a failed resume operation with `immutable on resume`, **zero new invocations**, and exactly unchanged session, policy, assignment, invocation, decision and task rows.
- A subsequent facade omitting the contract field resumes using the original capture and produces confirmed results.
- A read-only facade inspects confirmed results and searches/resolves versioned supported knowledge with **zero invocation growth**. Foreign project references remain denied and do not expose that project knowledge.
- A distinct later session started through MCP receives the supported source entry before its deliberately failed mock planning response. The deliberate failure tests retrieval at admission; it is not represented as a completed task.

The complete SDK walk also preserves discovery, scoped bounded execution, usage, quantitative reply limits, qualitative/unconfirmed fallback, request deduplication, conflict rejection, cancel/resume, disconnect/reopen, SIGKILL recovery, internal bridge lifecycle, malformed framing and open-stdin SIGINT/SIGTERM handling. It passed **312 tool calls**, with a largest observed reply of **73,149 bytes**. The configured-contract walk passed again against the final `cargo build` executable, SHA-256 `98b31f9a0b9a729d3e78ffbb4d314d702c57b787d8ee77fba62a308482b34efb`.

## Discriminating failure and harness correction

The same corrected contract consumer was run against a separately rebuilt pre-integration executable. Its checked-out revision was `2cd0694572c8e5580278a5457e95a1f59525d255`; Cargo/bridge/Rust source equality with base `3b4045f292845bb757aa14c8efd4afe74aea6abe` was checked before and after that build. Differences at that revision are documentation and task records only.

That consumer exited **1** at the confirmation assertion: MCP returned `confirmation: unconfirmed` and an empty artifact inventory. The combined candidate passes the identical assertion. This is the actual executable integration failure, preserved in `/tmp/ymp125-integration-logs/mcp-contracts-before.log` with the baseline binary/build fingerprints.

The first new consumer run had a separate harness error: it expected `session_id` in a `StoredOutcome` row. The public DTO correctly uses `source_session`. The assertion was corrected to use that existing source identity; no production fix was required. Its initial log remains separately classified in the evidence file. Neither failure log is counted as a successful check.

## Validation and reproducibility

Targeted checks and the real MCP processes ran before the final formatting, Clippy, workspace and executable build checks. All successful commands below exited 0.

| Check | Result |
| --- | --- |
| CLI contract integration | 5 tests |
| Runtime contract ingress | 7 tests |
| Public concurrency, state and allocation consumers | 24 tests |
| Public MCP runtime tests | 2 tests |
| Explicit inherited-descriptor lock release | 2 tests |
| Atomic complete-contract capture | 1 test |
| Official SDK, contract-only and complete walks | Passed |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | **297 passed / 0 failed** |
| `cargo build -p ymp-cli` | Passed |
| Contract-only walk against final built executable | Passed |

Cargo used three build jobs, disabled incremental compilation and disabled dev/test debug information, with `CARGO_TARGET_DIR=/tmp/ymp125-integration-target`. Exact command strings, all log hashes, the final executable hash, source manifest, source-session observations and classified earlier failures are recorded in [ymp-125-mcp-integration.json](../../ymp-docs/research/evidence/ymp-125-mcp-integration.json). Its 116-file source/consumer/configuration-document manifest has SHA-256 `f218adef48d07a4b1abbd36fa174fa452a06b61ed5fbab125a1d41da696a971c` over compact sorted JSON.

Activation remains explicit: the selected application's configuration applies to every new team session using that home, while each MCP facade is restricted to its launch project. A dedicated `--home` selects project/workflow configuration; no project file is automatically trusted. Public tool arguments cannot supply acceptance authority. The direct-workspace MVP limitations, user-declared verifier meaning and existing confirmation semantics remain unchanged. These results do not establish native-provider quality, filesystem isolation or whole-release acceptance. Main source, task registry and intent were not modified by this assignment.
