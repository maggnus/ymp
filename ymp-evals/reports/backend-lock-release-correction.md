# Completed run lock release and backend admission diagnostics

2026-09-13 — Correction based on combined MCP candidate `ced4aec16825a962fd46dc42bf042254b465a72a`, developed only in `/tmp/ymp-backend-diagnose-6jy1sA/source` on `diagnose/backend-setup`. Independent integration review is pending.

## Reproduced failure and cause

The default-parallel runtime library group reproduced `backend_public_engine_cancels_a_noncooperative_future` failing before backend admission. Credential-safe diagnostics identified the exact static error `This session is already running in another ymp process`, IO error 35 (`EWOULDBLOCK` on this host), and an already completed setup session. Its final synthesis, final review and task review assignments were completed. The scripted request list was empty. See `runtime-default-01.log` below.

The store returned a raw `File` as the ownership guard and relied on closing that descriptor to release `flock`. A concurrent subprocess fork can temporarily retain the same open file description before exec closes the inherited descriptor. The completed owner then closes its descriptor, but the inherited reference retains the lock and an immediate follow-up sees false contention. The isolated `fork-lock-control.py` reproduces this operating-system behavior with `O_CLOEXEC`; deterministic storage regressions retain the same open file description through `File::try_clone` and reproduce the identical session and project lock errors. No setup timeout was involved in the captured failure.

The old cancellation test joined an early completed call with a cancellation partner that waited forever for admission. This concealed the real error behind its three-second outer timeout. The historical redaction failure discarded its actual error by indexing an empty list. Its individual cause cannot be recovered from that old evidence; the tests share the corrected pre-admission lock path, but this report does not retroactively claim a captured error for that occurrence. ENOSPC is separate and was not reproduced here.

## Correction and controls

`Store::lock_session` and `Store::lock_project` now return a `StoreLock` guard which explicitly unlocks on drop. Ownership still lasts for the same runtime scope. Both new regressions require exclusion while an owner is live, successful immediate acquisition after its completion while an inherited descriptor remains open, and continued exclusion when that stale descriptor closes during the next owner's lifetime.

Both regressions failed against the original storage implementation and passed after correction. A second control bypassed only the explicit unlock in the final guard and made both final regressions fail again. Source was restored before verification.

The cancellation harness now uses `select!` so an early call result terminates its waiting partner. Cancellation and redaction tests check admission before inspecting the request. Failure diagnostics expose only the IO number, session status and durable event kinds. Existing timeout values, cancellation/drop/usage checks, redaction renderings and chain checks, and capability/grant closure assertions remain intact.

## Verification

All logs and exact command details are retained under `/tmp/ymp-backend-diagnose-6jy1sA`; `commands.md` records the sequence. `CARGO_INCREMENTAL=0` bounded the independent cache. Except the initial thirteen diagnostic searches, all runs used the normal debug profile and default test concurrency.

| Command / control | Result | Log |
| --- | --- | --- |
| Initial runtime diagnostic search, debug info disabled | 13 bounded runs passed, 92 tests each; no cause inferred from passing | `runtime-diagnostic-01.log` through `runtime-diagnostic-13.log` |
| `cargo test --workspace -- --nocapture`, diagnostics only | 0; 271 tests | `workspace-diagnostic-default-01.log` |
| `cargo test -p ymp-runtime --lib -- --nocapture`, diagnostics only | 101; 91 passed, cancellation failed with IO 35 before admission | `runtime-default-01.log` |
| `cargo test -p ymp-storage completed_ -- --nocapture`, original implementation | 101; both new lock controls failed | `lock-control-before.log` and `lock-control-before.patch` |
| Same storage controls after correction | 0; 2 passed | `lock-control-after.log` |
| Same final controls with explicit unlock bypassed | 101; both failed | `lock-unlock-bypass.log` |
| `cargo test -p ymp-runtime backend_contract_tests -- --nocapture` | 0; 8 passed | `backend-contracts-after.log` |
| `cargo fmt --all --check` | 0 | `fmt.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | `clippy.log` |
| `cargo test --workspace`, restored final source | 0; 273 tests | `workspace-after.log` |

Only offline mock/scripted providers were used. No live capabilities, raw requests or credentials were printed. Candidate and main source, task registry, approved intent, native inference and UI were outside this correction. The isolated build cache remained below 2 GiB; no other target directory was deleted.

SHA-256 of retained evidence:

- `runtime-default-01.log`: `f4899c2a9b2aab3ac9db744f5e66af5f535c70ab59d9d1c5b8c7c88bc0a40f8e`.
- `diagnostics.patch`: `952376a3cae93b404989b94e8a493a6491f53a245afb4100a21fc8d6b6218e4f`.
- `lock-control-before.log`: `2e038499ffe193c6605892cf3a2efc1781cc9c579ef5a0a0967fb7053b4355e4`.
- `lock-control-before.patch`: `4ef9090c27a97a241c450abd5e9478344662eaea16314c1b88a2b3ed6c47ddd9`.
- `lock-control-after.log`: `841cff9c6baf6589c9ea4c9ebdd2691794307c240d86540df6d4b2c479dd5122`.
- `lock-unlock-bypass.log`: `806a2c24312eaa6230dc8437e6d25df325ccaadfb396607449f643df4268b52f`.
- `workspace-after.log`: `f3cbc2b9c214df43c72fa3cf935c7718e130327489479cf592051f6a8c0851d8`.
