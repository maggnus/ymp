# YMP-120 independent review

12/09 20:54 HKT — **R2(9/10) ACCEPT** for `d9a33174bb34233c939151ab3e5408f0fcecd0d8` (`dc81cd0` authority implementation plus A120-1 correction). Code: 9/10; work: 9/10; consumer experience: 9/10. No open finding remains in this outcome.

## R2 evidence and resolution

The complete five-file correction was independently reviewed. `run_turn` retains only the current known assignment capability for error redaction and wraps every return from `run_turn_inner`, including native, cancellation and timeout errors. Redaction renders the full error chain, replaces all occurrences and constructs a new sanitized error only when required; it does not leave the original secret-bearing source attached. Unchanged errors retain their original types. Method, error code and cause remain readable.

The exact R1 public-engine probe changed **101 → 0**, reporting `persisted_native_error_with_capability=0; visible_events=0; session_status=blocked`. The new permanent consumer fixture compares both actual synthetic capabilities, received through transient IPC, against stored board messages, the full trace/event history, emitted UI events and a reopened store's trace. It also verifies distinct capabilities, terminal failed invocations, revoked grants and preserved error diagnosis. It passes on the candidate and fails when only the boundary redaction is disabled.

Independent adverse controls ran outside the checkout, with the original runtime/storage modules read directly:

| Control | Real failing form | Restored result |
| --- | --- | --- |
| Permanent `capability_errors` consumer | Disable only `run_turn` redaction: **101**, `Current capability leaked through messages` | **0**, same permanent test |
| Three error-format/type tests | Retain the original source with `error.context(sanitized)` and flatten unchanged errors: **101**, all three fail; failures include `!display.contains(capability)` and failed `downcast_ref::<std::io::Error>()` | **0**, all three pass |
| Nested/repeated secret test | Replace sanitization with unchanged diagnostic: **101**, `!display.contains(capability)` | **0** after restoration |
| Concurrent replay control | Ignore the false result of request-ID insertion: **101**, two successful effects versus expected one (`left: 2`, `right: 1`) | **0** after restoration |

These controls distinguish current-capability error leakage, retained unsafe sources/type loss and duplicate committed requests. They do not establish OS isolation or correctness of unrelated provider output. The restored external helper suite passed all 17 tests: three redaction controls and all 14 MCP authority controls, including stale/terminal credentials, cross-session/assignment arguments, self-grant proposals, task reassignment, restart/drop, admission rollback and cancellation ordering.

Commands and actual exits:

| Command | Exit / result |
| --- | --- |
| `cargo fmt --all --check` | **0** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0** |
| `cargo test --workspace` | **0**, 167 Rust tests; includes permanent consumer, three redaction tests, native authority/settings and compatible-continuation coverage |
| R1 retained command below, unchanged | **0**, zero stored/UI leaks |
| `cargo test --offline --manifest-path "$ymp_review_falsifiers/ymp-runtime/Cargo.toml" --target-dir "$ymp_review_candidate/target" --test capability_errors -- --nocapture` | **101 → 0**, boundary disabled → restored |
| `cargo test --offline --manifest-path "$ymp_review_falsifiers/Cargo.toml" --target-dir "$ymp_review_candidate/target" redaction::tests -- --nocapture` | **101** with faulty redaction; restored tests **0** in full helper command |
| Same helper command with filter `concurrent_replayed_request_has_only_one_effect` | **101**, replay guard disabled |
| `cargo test --offline --manifest-path "$ymp_review_falsifiers/Cargo.toml" --target-dir "$ymp_review_candidate/target" --lib -- --nocapture` | **0**, all 17 restored helper tests |
| `git diff --check` | **0** |

Command path definitions (all disposable probes are retained):

```sh
ymp_review_candidate='/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/120'
ymp_review_falsifiers='/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp120-r2-falsifiers-wtjop43m'
```

The initial disposable manifest setup had one tooling-only exit 101 from an incorrect generated Rust-version field; it was corrected before compiling or counting adverse evidence. Fixture-only unused-function warnings do not occur in the candidate's passing Clippy check.

No real provider inference, native credential reads or provider credential output occurred. R1's passing Claude bridge check/build and 11 tests remain applicable: no bridge code changed in R2, so they were not repeated. Native permission observations remain bounded by the documented provider capabilities, without an OS-containment claim. Later combined budget/admission integration and YMP-122 are outside this verdict. This report remains the only source-checkout write.

Round ledger:

- `12/09 20:17 HKT R1(5/10) RETURN — A120-1 native error persisted assignment capability → exact failing public-engine probe supplied → targeted provider-boundary correction requested.`
- `12/09 20:54 HKT R2(9/10) ACCEPT — full-chain redaction closes A120-1 → retained probe 101→0 and permanent consumer independently falsified/restored → no open scoped finding.`

## R1 record (historical; A120-1 resolved above)

12/09 20:17 HKT — **R1(5/10) RETURN** for commit `dc81cd0eb5262269e196ca73d016638abcc6533a`.

The complete 17-file commit diff was reviewed against YMP-120 acceptance, approved intent, runtime authority contract and the team skill. One critical contract defect remains: native protocol errors can persist the process-owned assignment capability. Code: 5/10; work: 5/10; consumer experience: 5/10. The otherwise passing tests omit this native-error consumer path.

## Finding A120-1: capability copied into stored notices and UI events

`ymp-providers/src/rpc.rs:173` forwards the entire native error with `bail!("{method}: {err}")`. `Engine::ask_scoped` returns it; `ymp-runtime/src/engine.rs:1377` stores it in the board as a proposal-unavailable notice and emits the same message to the UI. Thus an MCP setup diagnostic containing `YMP_MCP_TOKEN` bypasses the new `McpEndpoint` Debug redaction. The runtime contract explicitly requires live capability secrets to remain process-owned and out of logs.

A disposable offline fixture calls the actual public `Engine::run` with two Python Codex protocol processes. Each process returns its own freshly issued, synthetic team capability in an `initialize` error. No model prompt, real provider process, provider credential read or credential output is involved. Only leak counts are printed. The actual result was:

```text
persisted_native_error_with_capability=2; visible_events=2; session_status=blocked
assertion failed: Native error copied process-owned team capabilities into stored board messages
left: 2
right: 0
exit: 101
```

The exact retained probe is `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp120-review-4c835yyl` (`Cargo.toml`, `src/lib.rs`, `native_error.py`). Its dependencies reference the reviewed checkout read-only. Reproduction:

```sh
cargo test --offline --manifest-path /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp120-review-4c835yyl/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/120/target native_error_must_not_persist_capability -- --nocapture
```

Required correction, communicated directly to `authority_120`: sanitize the known current assignment capability from native errors before they cross the provider boundary, including nested error/context formatting. Preserve a usable diagnosis. Add a runtime consumer regression showing absence of the actual synthetic capability in stored messages/events/trace and emitted UI events. The exact probe above must change from exit 101 to exit 0. No unrelated acceptance or workspace-policy changes are requested.

## Other inspected boundaries and checks

The runtime path admits each invocation through `TeamServer::admit`, supplies its fresh token to the native adapter and revokes through `InvocationGuard` on completion, cancellation, failure or dropped futures. Durable admission and revocation share storage transactions with events; task state/attempt/assignee changes revoke previous grants. The socket checks the live capability and request ledger under one lock, then storage revalidates the bound assignment, operation and task in its effect transaction. Caller fields cannot override runtime identity. Board and memory proposals cannot activate knowledge or accept tasks. Restart constructs an empty live map, and runtime recovery closes open invocations before admission.

Codex start/resume carries explicit current MCP configuration through the current process environment; ACP load/new receives fresh MCP configuration and resets an advertised compatible permission mode before prompting. Claude's installed SDK transport fixture observes fresh credentials, a retained resume ID, read tool restrictions and permission mode. Compatible continuation remains covered by the runtime settings tests. Native filesystem/process containment limitations are recorded explicitly; no live provider enforcement claim is made.

Commands run in the reviewed checkout unless stated otherwise:

| Command | Actual exit and evidence |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0; 163 Rust tests, including 14 MCP authority tests and four native authority boundary fixtures |
| `npm run check && npm test` in `ymp-bridges/claude` | 0; TypeScript check/build and 11 tests |
| Adversarial public-engine command above | 101; two persisted notices and two visible events contain the synthetic capability |

The green native tests verify physical protocol arguments and offline permission rejection, not real provider inference or OS isolation. The failing consumer probe distinguishes native diagnostic leakage from Debug redaction; it would still pass unrelated authority defects. Further mutation/consumer checks are deferred to the corrected round. No source, intent, task registry or TUI changes were made; this report is the only checkout write.

Round ledger: `12/09 20:17 HKT R1(5/10) RETURN — A120-1 synthetic capability persisted through native error → exact consumer probe shared with author → awaiting boundary redaction and consumer regression.`
