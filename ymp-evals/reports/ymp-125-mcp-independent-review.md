# Independent YMP-125 and public MCP integration review

Verdict: **ACCEPT — 9/10**. No blocking integration finding. The combined executable exposes the accepted trusted-contract configuration through public MCP while preserving runtime authority, preallocated session identity, workspace concurrency, source-bound knowledge and explicit lock release. This accepts the combined implementation; it does not grant whole-release acceptance.

Reviewed HEAD: `127bfd19d6998a194504a982e0fed3954d1ba523`, based on main `3b4045f292845bb757aa14c8efd4afe74aea6abe`, with accepted YMP-125 `ef9a1254696baef5b1bdbf72a9c1745da3eacc37` integrated as `894ab8679617f5d2bd5ab71433d22e5ac147cea6`. Actual clock checks: review began `2026-09-12 17:36:51 UTC`; focused execution finished by `17:38:51 UTC`. The checkout was clean on entry. Only this report was added; no source, UI, registry, intent, main or credentials were changed or read as part of provider execution. All provider workloads were mocked with fixed `mock` model and `low` effort.

## Composition findings

| Boundary | Independent assessment |
| --- | --- |
| Common acceptance authority | The normal CLI configuration load passes the complete typed configuration into public MCP (`ymp-cli/src/main.rs:154`, `162`). `Facade` retains a launch snapshot and clones it for execution, changing only admitted budget limits (`ymp-runtime/src/public_mcp.rs:308`). `Engine::new` therefore reaches the same configured contract preparation, capture and checking path as CLI/TUI. There is no MCP-only contract installer. The strict `Run` DTO has no acceptance/configuration fields (`public_mcp.rs:44`); the actual SDK consumer rejects all three attempted authority arguments without creating a session or invocation. |
| Session identity and atomic capture | The conflict resolution preserves both `prepare_contracts` and `new_session_id` (`ymp-runtime/src/engine.rs:595`). `run_identified` validates the supplied UUID (`engine.rs:386`), while `create_session_with_contracts` uses insert-only session creation and one transaction for session/policy/contracts (`ymp-storage/src/provenance.rs:284`). The public operation's returned identity matches the confirmed result's `source_session`. The independently rerun identified-start test rejects duplicate/invalid IDs without new spend or overwritten goal. |
| Resume and target obligations | Explicit replacement is rejected by `validate_resumed_contracts` before another invocation. The actual MCP walk cancels after admission, restarts with changed expected bytes, and verifies byte-for-byte equality of session, policy, assignments, invocations, decisions and task rows. Only the separate failed MCP operation is new; the original authority stays unchanged. A later facade omitting the option resumes the original capture and reaches confirmed delivery. Initial/revised target validation and saved-task checks remain before production/recovery (`engine.rs:1490`, `1581`, `1610`); seven focused ingress controls passed on the combined source. |
| Locks and concurrent work | The accepted project/session `StoreLock` implementation is unchanged, including explicit `FileExt::unlock` on drop (`ymp-storage/src/lib.rs:38`). Conversation handoff drops both locks before creating a distinct task (`engine.rs:560`). The new capture runs under the existing project lock. Workspace grants, backend capability checks, uncertain-admission recovery and retained responsibility remain intact. Independent concurrency/allocation/state and inherited-descriptor tests passed. |
| Knowledge, source and inspection | The real MCP process returns confirmed results and supported versioned knowledge under the source session's identity. A read-only facade searches/resolves that knowledge with no added invocation; a foreign project cannot inspect it. A later distinct session retrieves the supported source before its deliberately failed mock planning response. That failure is correctly reported as blocked, not completed. Existing storage source/freshness/knowledge implementations are unchanged by this integration. Evidence replies omit expected/snapshot bytes. |

Production changes after mapped commit `894ab867` are absent. Compared with the integration base, the public MCP implementation, StoreLock, workspace-access subsystem, storage confirmation/knowledge implementations and TUI source are unchanged. The production delta is the accepted YMP-125 patch with the new-session conflict resolved as described above. There is no new service, permanent role, plugin ABI or model-authored confirmation authority.

## Actual MCP verification

The independently rerun consumer uses the installed official MCP Python SDK **1.28.1**, verified through package metadata, and protocol `2025-06-18`. It extracts the documented TOML contract and launches the final actual executable over stdio. It exercises configuration snapshot retention, forbidden tool arguments, confirmed output, read-only supported knowledge, cancelled-session capture, rejected replacement, capture-only resume and later retrieval.

Independent result: **PASS**, exit 0.

- Source session: `8d1c8ec9-86b9-441d-9e99-403e2345ca2e`.
- Resumed session: `ca263828-b97b-47b8-864a-939ef73b86dd`.
- Later retrieval session: `e2a2f423-3407-42ca-8c48-983e653fc365`.
- Supported source entry: `knowledge:8a3919d9640461750d37e992e5875eb95c7747474d49d9aeeaa7e130ec2765b6`.
- Tool authority injections rejected: **3**.
- Added invocations for read-only inspection and rejected replacement: **0** in each case.

The consumer's temporary stores are removed normally. The retained independent log records these identities and outcomes; its exact assertions and SQLite read-only comparisons are in the fingerprinted `ymp-evals/mcp/stdio_client.py`.

## Exact-source evidence and commands

Independently verified all **116** source/consumer/configuration-document file digests against `ymp-docs/research/evidence/ymp-125-mcp-integration.json`, before and after focused execution. No mismatches. Compact sorted-JSON manifest SHA-256: `f218adef48d07a4b1abbd36fa174fa452a06b61ed5fbab125a1d41da696a971c`.

The executable `/tmp/ymp125-integration-target/debug/ymp` independently matched SHA-256 `98b31f9a0b9a729d3e78ffbb4d314d702c57b787d8ee77fba62a308482b34efb` before and after the focused consumer. All **15** retained success/failure log digests matched. The supplied full workspace log was read and counted as **297 passed / 0 failed**. The supplied complete SDK walk records **312 tool calls**, maximum reply **73,149 bytes**, plus signal, lifecycle, cancellation, scope and malformed-frame controls. These complete-suite results were verified from exact-source evidence, not claimed as independently rerun here. Formatting, clippy and final build evidence also matched.

Independent commands ran from the reviewed checkout:

```sh
export CARGO_TARGET_DIR=/tmp/ymp125-integration-target
export CARGO_BUILD_JOBS=3
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0

/tmp/ymp-mcp123-sdk/bin/python ymp-evals/mcp/stdio_client.py /tmp/ymp125-integration-target/debug/ymp --contracts-only > /tmp/ymp125-mcp-review-client.log 2>&1
cargo test -p ymp-runtime --test concurrency --test state_integration --test allocation_admission > /tmp/ymp125-mcp-review-composition.log 2>&1
cargo test -p ymp-runtime public_mcp > /tmp/ymp125-mcp-review-public.log 2>&1
cargo test -p ymp-runtime contract_ingress_tests > /tmp/ymp125-mcp-review-bindings.log 2>&1
cargo test -p ymp-storage completed_ > /tmp/ymp125-mcp-review-locks.log 2>&1
```

All five commands exited 0: the actual SDK walk and **35 focused Rust tests** passed. No full-suite rerun was necessary. Independent manifest/log validation is retained at `/tmp/ymp125-mcp-review-manifest.json`.

| Independent log | SHA-256 |
| --- | --- |
| `/tmp/ymp125-mcp-review-client.log` | `b20b0f8751f9adec6190b8e341b079f3b86184296d7d51d3532ac0eea9d419aa` |
| `/tmp/ymp125-mcp-review-composition.log` | `413b89e62bbf38f6163ea19d2a2c11d1f620af407afc453c6b3f2c057dbfb3b4` |
| `/tmp/ymp125-mcp-review-public.log` | `2b2dd357ffde769596096e19a2dd69c172448b3482968515c7ecb0a51a7e1ebf` |
| `/tmp/ymp125-mcp-review-bindings.log` | `429ca3dd8d319b3fa08d0cd439fa56de60929fc7aebd17f65a98c36d37eb914f` |
| `/tmp/ymp125-mcp-review-locks.log` | `98a96948652745df92428a36ad1a2fe226c6d624496b1aec9f48731deb2cf282` |

The retained pre-integration failure was inspected: the identical corrected consumer fails the confirmation assertion with `unconfirmed` and an empty artifact inventory. Independent `git diff --quiet 2cd0694572c8e5580278a5457e95a1f59525d255 3b4045f -- Cargo.toml Cargo.lock ymp-rust ymp-bridges` exited 0, confirming the documented production-source equivalence of that baseline. The initial consumer's `session_id` versus `source_session` DTO assumption is retained separately and correctly classified as a harness error. Neither failure is counted as passing validation.

## Disposition

Accept the combined YMP-125 integration at this exact source. The original gap remains closed through CLI/TUI's shared configuration and now has direct official-client evidence through public MCP. Existing unconfirmed/partial/failing-evidence behavior, declared verifier responsibility and direct-workspace MVP limitations remain in force. This review establishes integration and authority preservation, not native model quality, resource savings, filesystem isolation or YMP-121 release completion.
