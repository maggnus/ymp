# Accepted runtime reservations integrated without the pending driver

13/09 05:51 HKT, 2026-09-13 (2026-09-12T21:51:07Z). This is an integration-preparation candidate from main `2c6bbab9dfb03ceb3a2eb90cb43a9ad69404dc50`. **The new composition awaits independent acceptance.** The integrator previously reviewed runtime R2 in another assignment; that earlier verdict does not independently accept this composition.

## Exact source selection

The accepted runtime source is `1bdea888169e680e69ecf47792abcc5f53838400`. Only its production/applicable-test changes relative to `897b4ebb0afe7ce95c64e759c7e786c563cab061` were selected: **15 files** in the existing CLI, core, runtime and storage packages. The patch was applied with three-way validation to the newer main base, not by replacing older source trees wholesale. Every selected final file is byte-identical to the accepted source. No conflict or additional implementation change was necessary.

The incomplete `ymp-eval-driver` crate, exporter/adapters, driver preparation documents and driver-only Cargo.lock addition were excluded. `cargo metadata --offline --locked --no-deps` still lists exactly the original seven application packages. Cargo.lock is byte-identical to the destination base and remains coherent; no unfinished driver is pulled into normal workspace checks.

Accepted YMP-128 `mcp/socket.rs`, TeamServer integration and the runtime tempfile dependency promotion are untouched. The accepted alternate WorkspaceAccessPolicy regression and all Opus UI follow-ups remain byte-identical to the base. No TUI fixture compilation gap appeared, and no TUI file was edited. Parent/Opus retain responsibility for visibility of the newly optional reservation fields. The existing aggregate sentence at `ymp-tui/src/views.rs:3254` reads `budget.reserved_tokens`; the TUI has no readers for `AssignmentRecord.token_reservation` or `SessionBudget.protected_review_tokens`. Separate requested/protected allowance labels, if required for the release, must be implemented and accepted by Opus.

## Runtime contract carried forward

The selected change adds optional per-assignment `token_reservation`, optional captured `review_reserve_tokens`, and current `SessionBudget.protected_review_tokens`. Admission validates variable requests against captured ceilings; outstanding reservations and review protection are accounted once, and observed spend is not refunded. Missing/partial usage retains its existing stop semantics. These are accounting allowances, not hard native expenditure ceilings.

Public workspace reservation requires an opaque `Arc<WorkspaceOwner>` created by `Engine::acquire_workspace_owner`. The owner holds the existing project OS lock and is retained by reservations. Actual prompt/instruction lengths and exact context evidence are bound before admission. Native work remains a one-shot bound request using existing settings, scope, budget and grant enforcement; capability cleanup precedes resource/owner release. Normal Engine work uses the same ownership and reservation paths.

Both prior independent reports are retained byte for byte: [R1 findings](ymp-126-runtime-independent-review-r1.md) and [R2 acceptance](ymp-126-runtime-independent-review-r2.md). R2 accepts the runtime delta and preserves the context, ownership-lifetime and variable-ledger evidence; it expressly excludes the seventeen-case driver. Their original external paths and hashes are recorded in the new ledger.

## Validation on this composition

All checks below exited **0** on one unchanged source fingerprint:

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --offline --locked --workspace --all-targets -- -D warnings` | Passed, including unchanged TUI compilation |
| `cargo test --offline --locked --workspace` | **392 tests passed** |
| Public `workspace_reservation` suite | Seven registered tests pass, including actual child-process ownership |
| Storage `variable_` and `live_review_reservation` controls | Three tests pass |
| Runtime `mcp::socket::tests` | Four real socket controls pass |
| Alternate WorkspaceAccessPolicy consumer | Passes on this ownership composition |
| `cargo build --offline --locked -p ymp-cli --bin ymp` | Passed |
| `git diff --check` | Passed |

The complete workspace suite includes the accepted long-home executable regression, board/knowledge integration, grant/pin/confirmation checks and UI tests. No suite failed or required a retry. Full command arguments, logs, hashes and timestamps are recorded in [the integration ledger](../../ymp-docs/research/evidence/runtime-reservations-integration.json). No full official MCP SDK or seventeen-case driver run is claimed for this composition.

Checks used this worktree's own target directory with `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and `CARGO_BUILD_JOBS=4`. Source fingerprint: `e41de6ae5befabf58751b5117bf1120b5e891e007e2a6784fa8018e5c8ba7e7a`. The exact selected source map and excluded file list are retained both in the ledger and beside the worktree under `runtime-reservations-integration-checks/`, along with the applied patch and raw logs.

No main, owner-request file, approved intent, registry, AGENTS.md, UI implementation, real application home or credentials were edited. All execution was mock/scripted. This candidate is ready for the parent's independent composition review; final driver/exporter acceptance and final release verification remain separate.
