# Captured location in the structured follow-up result

13/09 00:27 HKT, 2026-09-13 — Correction for the sole F1 in the combined integration's R1 review. Independent re-review is pending. Base: `addba636a56653b1f1d2f14932cd3672e528f515`.

The location-only `RunOutcome.workspace` now uses the captured outcome directory, falling back to stored workspace metadata or the immutable session policy. It never substitutes the project's mutable current registration. A session with no captured absolute location returns `outcome_location_unknown` explicitly. The existing summary still distinguishes recorded artifacts from directory inspection and unavailable metadata. No UI implementation, phrase routing, allocation, confirmation, grants or execution behavior changed.

The permanent public consumer now checks the structured directory as well as the absolute artifact in the answer. That added assertion failed **101** against the original implementation and passed **0** after correction. It also exercises an output-free failed session after relocation, loss of its workspace metadata with its policy still present, and a legacy session without any location capture. The location requests create no new invocations/backend requests. Original artifacts and supported observations remain unchanged.

| Command | Exit and evidence |
| --- | --- |
| `cargo test -p ymp-runtime --test state_integration -- --nocapture`, original implementation with added directory assertion | 101; `/tmp/ymp-state-workspace-f1-before.log` |
| Same command, corrected implementation and extended fallback/legacy consumer | 0; `/tmp/ymp-state-workspace-f1-after.log` |
| `cargo fmt --all --check` | 0; `/tmp/ymp-state-workspace-f1-fmt.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0; `/tmp/ymp-state-workspace-f1-clippy.log` |
| First `cargo test --workspace` | 101; pre-existing redaction test indexed an empty backend request list; `/tmp/ymp-state-workspace-f1-workspace.log` |
| `cargo test -p ymp-runtime backend_public_engine_redacts_nested_capability_errors -- --nocapture` | 0; `/tmp/ymp-state-workspace-f1-redaction-recheck.log` |
| Repeated `cargo test --workspace` on unchanged source | 0; 267 tests; `/tmp/ymp-state-workspace-f1-workspace-recheck.log` |

The redaction test uses the model-backed conversation branch, outside the changed literal location branch. The first failure's underlying pre-invocation error was hidden by that existing test's unchecked index. Its cause is not established; the successful reruns do not prove that it is impossible. No unrelated test or implementation was changed to make the rerun pass. This observation is passed to independent review and final integrated verification.

Verified source SHA-256:

- `ymp-runtime/src/engine.rs`: `8641ea700e68229a7ba1408c7f3e6d2ffa2a8dc579b6a694e3218b61147f7f80`.
- `ymp-runtime/tests/state_integration.rs`: `f3208e1658c16c1016b0e70f048a892350d0fab0cf640a474d80ea53ad0a7cb0`.
- Successful complete-suite log: `98412140d99814fe00c6c3f668a3ceac4659a0157d858a6150bd7686ed411be6`.

All execution is mock/scripted. No real model inference, credentials, main checkout, approved intent or task registry was changed by this correction. Application workspace behavior remains direct only for the MVP.
