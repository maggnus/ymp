# Optional test inventory for the kernel migration

Status: planning reference, 2026-09-15. Companion to the [migration plan](migration-plan-kernel-transition.md). No test was changed or deleted while preparing this inventory.

## Authority and use

The owner permits deletion of **any existing tests**, including entire suites, when this simplifies authorized migration work. Every recommendation below is optional. There is no protected category, minimum retained count, replacement obligation or requirement to complete a test-by-test audit first. The implementer may delete a whole listed file or suite without a separate owner approval. For inline tests, remove the test module/attributes and obsolete test-only helpers, not unrelated production code.

This inventory helps choose where reuse might save time. It is not a checklist that must be completed before deletion. Keep, port or delete are implementation choices; phase acceptance evaluates the intended behavior separately and does not require recreating the deleted tests.

## Baseline and counting method

Source revision: `aa98c242103416c978f441826fdad169cc4c7f64`. Counts include Rust files under `ymp-rust/crates`, including standalone test directories. Physical lines include blank lines, comments, examples and inline tests; they are not production-only LOC or an effort estimate. Test counts are source attributes matching `#[test]` or `#[tokio::test(...)]`, including arguments and ignored tests. They are not a fresh execution result.

The inventory covers 77 test-bearing Rust files and 668 test attributes. SDK bridge and external evaluation-script tests are outside this Rust count; the same deletion option applies to them. Their adapter-specific inventory can be consulted only if needed for the chosen change.

| Crate | Rust files | Physical lines | Test attributes | Optional default while code is unchanged |
| --- | ---: | ---: | ---: | --- |
| ymp-cli | 5 | 2,311 | 19 | Reuse inexpensive command/identity checks; drop or port affected cases in P4. |
| ymp-core | 21 | 4,606 | 28 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| ymp-eval-driver | 33 | 10,905 | 27 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |
| ymp-providers | 17 | 3,608 | 40 | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| ymp-runtime | 45 | 24,665 | 212 | Drop retired Engine fixtures; reuse or port only cases worth their adaptation cost. |
| ymp-storage | 18 | 9,635 | 39 | Reuse tests around retained atomic commands; adapt or drop when the boundary changes. |
| ymp-tui | 26 | 35,630 | 276 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| ymp-workspace | 7 | 2,629 | 27 | Reuse unaffected access/Git/preview tests; reassess or drop for P2/I1. |

Total: 172 Rust files, 93,989 physical lines and 668 test attributes.

## Concrete examples

| Existing test / source | What it currently checks | Optional treatment |
| --- | --- | --- |
| `allocation_startup_is_bounded_and_never_requires_pool_bids` in [engine.rs](../../../ymp-rust/crates/ymp-runtime/src/engine.rs) | Combines bounded startup with the current no-mandatory-bidding workflow. | Delete with the old Engine, or use only the bounded-admission idea if useful when P7 changes bidding. |
| `cancellation_during_learning_or_synthesis_still_pauses_the_session` in [engine.rs](../../../ymp-rust/crates/ymp-runtime/src/engine.rs) | Current ordering and owner cancellation before delivery. | Drop or rewrite when P6 moves learning after delivery; the old fixture is not a constraint on the new lifecycle. |
| `confirmation_failed_applicable_assertion_overrides_every_approving_agent` in [confirmation_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/engine/confirmation_tests.rs) | Failing applicable evidence overrides agreement. | Reuse if cheap, port to the changed consumer if valuable, or delete. The runtime rule is independent of this test file. |
| `uncertainty_and_unknown_accounting_block_replay` in [session_recovery.rs](../../../ymp-rust/crates/ymp-runtime/tests/session_recovery.rs) | Unknown effects/accounting prevent an unsafe repeat. | An optional diagnostic case for P2/P4; no preservation or equivalent-replacement requirement. |
| `native_unbounded_access_serializes_writers_and_readers` in [concurrency.rs](../../../ymp-rust/crates/ymp-runtime/tests/concurrency.rs) | Effective full-directory access controls conflicts. | Optional focused evidence for the invocation host; the existing test may be dropped independently. |

## File-level inventory

Counts below are exact for the stated source scan. A file can contain both workflow-specific checks and invariant checks; its count is not a classification of all its functions. Defaults describe a starting choice, not an approved bulk-deletion operation in the current documentation task.

### ymp-cli

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/main.rs](../../../ymp-rust/crates/ymp-cli/src/main.rs) | 1 | P4 | Reuse inexpensive command/identity checks; drop or port affected cases in P4. |
| [tests/acceptance_contracts.rs](../../../ymp-rust/crates/ymp-cli/tests/acceptance_contracts.rs) | 6 | P4 | Reuse inexpensive command/identity checks; drop or port affected cases in P4. |
| [tests/long_metadata_home.rs](../../../ymp-rust/crates/ymp-cli/tests/long_metadata_home.rs) | 1 | P4 | Reuse inexpensive command/identity checks; drop or port affected cases in P4. |
| [tests/message_labels.rs](../../../ymp-rust/crates/ymp-cli/tests/message_labels.rs) | 1 | P4 | Reuse inexpensive command/identity checks; drop or port affected cases in P4. |
| [tests/native_catalog.rs](../../../ymp-rust/crates/ymp-cli/tests/native_catalog.rs) | 10 | P4 | Reuse inexpensive command/identity checks; drop or port affected cases in P4. |

### ymp-core

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/board.rs](../../../ymp-rust/crates/ymp-core/src/board.rs) | 1 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| [src/config/execution.rs](../../../ymp-rust/crates/ymp-core/src/config/execution.rs) | 2 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| [src/config.rs](../../../ymp-rust/crates/ymp-core/src/config.rs) | 1 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| [src/model.rs](../../../ymp-rust/crates/ymp-core/src/model.rs) | 15 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| [src/provenance.rs](../../../ymp-rust/crates/ymp-core/src/provenance.rs) | 1 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| [src/reputation.rs](../../../ymp-rust/crates/ymp-core/src/reputation.rs) | 1 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| [src/usage.rs](../../../ymp-rust/crates/ymp-core/src/usage.rs) | 2 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |
| [tests/pool_metadata.rs](../../../ymp-rust/crates/ymp-core/tests/pool_metadata.rs) | 5 | P1/P2, P6/Q1 | Reuse inexpensive type/rule checks; drop if they obstruct the chosen type changes. |

### ymp-eval-driver

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/bin/weak_pilot/consumer.rs](../../../ymp-rust/crates/ymp-eval-driver/src/bin/weak_pilot/consumer.rs) | 1 | P3/P4/J1 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |
| [src/bin/weak_pilot/measurement.rs](../../../ymp-rust/crates/ymp-eval-driver/src/bin/weak_pilot/measurement.rs) | 8 | P3/P4/J1 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |
| [src/bin/weak_pilot/mod.rs](../../../ymp-rust/crates/ymp-eval-driver/src/bin/weak_pilot/mod.rs) | 1 | P3/P4/J1 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |
| [src/driver_controls.rs](../../../ymp-rust/crates/ymp-eval-driver/src/driver_controls.rs) | 3 | P3/P4/J1 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |
| [src/export_tests.rs](../../../ymp-rust/crates/ymp-eval-driver/src/export_tests.rs) | 4 | P3/P4/J1 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |
| [tests/restart.rs](../../../ymp-rust/crates/ymp-eval-driver/tests/restart.rs) | 1 | P3/P4/J1 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |
| [tests/weak_pilot_consumer.rs](../../../ymp-rust/crates/ymp-eval-driver/tests/weak_pilot_consumer.rs) | 9 | P3/P4/J1 | Reuse useful drivers; drop obsolete event/workflow expectations in P3/P4/J1. |

### ymp-providers

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/discovery/catalog.rs](../../../ymp-rust/crates/ymp-providers/src/discovery/catalog.rs) | 1 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [src/discovery.rs](../../../ymp-rust/crates/ymp-providers/src/discovery.rs) | 5 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [src/failure.rs](../../../ymp-rust/crates/ymp-providers/src/failure.rs) | 1 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [src/redaction.rs](../../../ymp-rust/crates/ymp-providers/src/redaction.rs) | 3 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [src/usage.rs](../../../ymp-rust/crates/ymp-providers/src/usage.rs) | 4 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [tests/authority.rs](../../../ymp-rust/crates/ymp-providers/tests/authority.rs) | 4 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [tests/backend.rs](../../../ymp-rust/crates/ymp-providers/tests/backend.rs) | 1 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [tests/codex_retry.rs](../../../ymp-rust/crates/ymp-providers/tests/codex_retry.rs) | 11 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [tests/resource_limits.rs](../../../ymp-rust/crates/ymp-providers/tests/resource_limits.rs) | 2 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [tests/settings.rs](../../../ymp-rust/crates/ymp-providers/tests/settings.rs) | 7 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |
| [tests/tool_activity.rs](../../../ymp-rust/crates/ymp-providers/tests/tool_activity.rs) | 1 | P1/P2; changed adapters | Reuse protocol fixtures if interfaces remain stable; deletion remains available. |

### ymp-runtime

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/allocation_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/allocation_tests.rs) | 13 | P2/P3 | Reuse unchanged policy/identity cases; port or drop when APIs change. |
| [src/assignment_settings_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/assignment_settings_tests.rs) | 3 | P2/P3 | Reuse unchanged policy/identity cases; port or drop when APIs change. |
| [src/backend_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/backend_tests.rs) | 15 | P1/P2/P3 | Reuse while the retained boundary is unchanged; drop or port as useful. |
| [src/budget_authority_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/budget_authority_tests.rs) | 3 | P1/P2/P3 | Reuse while the retained boundary is unchanged; drop or port as useful. |
| [src/budget_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/budget_tests.rs) | 7 | P1/P2/P3 | Reuse while the retained boundary is unchanged; drop or port as useful. |
| [src/engine/confirmation_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/engine/confirmation_tests.rs) | 11 | P3/P4, P6/Q1 | Port only if cheap and useful; otherwise drop Engine-bound fixtures. |
| [src/engine/contract_ingress_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/engine/contract_ingress_tests.rs) | 7 | P3/P4, P6/Q1 | Port only if cheap and useful; otherwise drop Engine-bound fixtures. |
| [src/engine/knowledge_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/engine/knowledge_tests.rs) | 6 | P3/P4, P6/Q1 | Port only if cheap and useful; otherwise drop Engine-bound fixtures. |
| [src/engine.rs](../../../ymp-rust/crates/ymp-runtime/src/engine.rs) | 24 | P3/P5, P6 | Drop inline tests with the retired Engine; reuse only useful setup/scenarios. |
| [src/mcp/socket.rs](../../../ymp-rust/crates/ymp-runtime/src/mcp/socket.rs) | 4 | P2/P4 | Reuse stable protocol cases; drop or port affected authority/schema fixtures. |
| [src/mcp_tests.rs](../../../ymp-rust/crates/ymp-runtime/src/mcp_tests.rs) | 18 | P2/P4 | Reuse stable protocol cases; drop or port affected authority/schema fixtures. |
| [src/public_mcp.rs](../../../ymp-rust/crates/ymp-runtime/src/public_mcp.rs) | 2 | P2/P4 | Reuse stable protocol cases; drop or port affected authority/schema fixtures. |
| [src/workspace_access.rs](../../../ymp-rust/crates/ymp-runtime/src/workspace_access.rs) | 1 | P2/I1 | Reuse useful access/ownership cases; drop or port for the new host. |
| [tests/allocation_admission.rs](../../../ymp-rust/crates/ymp-runtime/tests/allocation_admission.rs) | 13 | P2/P3 | Reuse unchanged policy/identity cases; port or drop when APIs change. |
| [tests/board_coordination.rs](../../../ymp-rust/crates/ymp-runtime/tests/board_coordination.rs) | 13 | P3/P7 | Drop obsolete workflow expectations; selectively reuse proposal/version cases. |
| [tests/capability_errors.rs](../../../ymp-rust/crates/ymp-runtime/tests/capability_errors.rs) | 1 | P1/P2/P3 | Reuse while the retained boundary is unchanged; drop or port as useful. |
| [tests/concurrency.rs](../../../ymp-rust/crates/ymp-runtime/tests/concurrency.rs) | 11 | P2/I1 | Reuse useful access/ownership cases; drop or port for the new host. |
| [tests/fresh_plan_review.rs](../../../ymp-rust/crates/ymp-runtime/tests/fresh_plan_review.rs) | 12 | P2/P3/P4 | Drop or rebuild selected lifecycle cases around the new consumer; no full-suite port. |
| [tests/knowledge_correction/board_integration.rs](../../../ymp-rust/crates/ymp-runtime/tests/knowledge_correction/board_integration.rs) | 1 | P3/P6/E1 | Reuse helpful correction/evidence cases; drop or port coupled fixtures. |
| [tests/knowledge_correction.rs](../../../ymp-rust/crates/ymp-runtime/tests/knowledge_correction.rs) | 5 | P3/P6/E1 | Reuse helpful correction/evidence cases; drop or port coupled fixtures. |
| [tests/pool_identity.rs](../../../ymp-rust/crates/ymp-runtime/tests/pool_identity.rs) | 1 | P2/P3 | Reuse unchanged policy/identity cases; port or drop when APIs change. |
| [tests/session_recovery/execution_rework.rs](../../../ymp-rust/crates/ymp-runtime/tests/session_recovery/execution_rework.rs) | 2 | P2/P3/P4 | Drop or rebuild selected lifecycle cases around the new consumer; no full-suite port. |
| [tests/session_recovery/inspection_rework.rs](../../../ymp-rust/crates/ymp-runtime/tests/session_recovery/inspection_rework.rs) | 8 | P2/P3/P4 | Drop or rebuild selected lifecycle cases around the new consumer; no full-suite port. |
| [tests/session_recovery/rework.rs](../../../ymp-rust/crates/ymp-runtime/tests/session_recovery/rework.rs) | 6 | P2/P3/P4 | Drop or rebuild selected lifecycle cases around the new consumer; no full-suite port. |
| [tests/session_recovery.rs](../../../ymp-rust/crates/ymp-runtime/tests/session_recovery.rs) | 17 | P2/P3/P4 | Drop or rebuild selected lifecycle cases around the new consumer; no full-suite port. |
| [tests/state_integration.rs](../../../ymp-rust/crates/ymp-runtime/tests/state_integration.rs) | 1 | P3/P7 | Drop obsolete workflow expectations; selectively reuse proposal/version cases. |
| [tests/workspace_reservation.rs](../../../ymp-rust/crates/ymp-runtime/tests/workspace_reservation.rs) | 7 | P2/I1 | Reuse useful access/ownership cases; drop or port for the new host. |

### ymp-storage

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/budget_tests.rs](../../../ymp-rust/crates/ymp-storage/src/budget_tests.rs) | 14 | P1/P2/J1 | Reuse tests around retained atomic commands; adapt or drop when the boundary changes. |
| [src/knowledge.rs](../../../ymp-rust/crates/ymp-storage/src/knowledge.rs) | 2 | P1/P2/J1 | Reuse tests around retained atomic commands; adapt or drop when the boundary changes. |
| [src/lib.rs](../../../ymp-rust/crates/ymp-storage/src/lib.rs) | 6 | P1/P2/J1 | Reuse tests around retained atomic commands; adapt or drop when the boundary changes. |
| [src/projection.rs](../../../ymp-rust/crates/ymp-storage/src/projection.rs) | 1 | P1/P2/J1 | Reuse tests around retained atomic commands; adapt or drop when the boundary changes. |
| [src/provenance_tests.rs](../../../ymp-rust/crates/ymp-storage/src/provenance_tests.rs) | 13 | P1/P2/J1 | Reuse tests around retained atomic commands; adapt or drop when the boundary changes. |
| [src/usage.rs](../../../ymp-rust/crates/ymp-storage/src/usage.rs) | 3 | P1/P2/J1 | Reuse tests around retained atomic commands; adapt or drop when the boundary changes. |

### ymp-tui

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/diff.rs](../../../ymp-rust/crates/ymp-tui/src/diff.rs) | 5 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/files.rs](../../../ymp-rust/crates/ymp-tui/src/files.rs) | 9 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/highlight.rs](../../../ymp-rust/crates/ymp-tui/src/highlight.rs) | 9 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/label.rs](../../../ymp-rust/crates/ymp-tui/src/label.rs) | 6 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/table.rs](../../../ymp-rust/crates/ymp-tui/src/table.rs) | 9 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/tests/recovery.rs](../../../ymp-rust/crates/ymp-tui/src/tests/recovery.rs) | 5 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/tests.rs](../../../ymp-rust/crates/ymp-tui/src/tests.rs) | 226 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/theme.rs](../../../ymp-rust/crates/ymp-tui/src/theme.rs) | 4 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |
| [src/views.rs](../../../ymp-rust/crates/ymp-tui/src/views.rs) | 3 | P4 | Reuse unaffected presentation tests; drop or port affected Engine-bound fixtures in P4. |

### ymp-workspace

| Source file | Test attributes | Relevant phase | Optional starting choice |
| --- | ---: | --- | --- |
| [src/git_tests.rs](../../../ymp-rust/crates/ymp-workspace/src/git_tests.rs) | 12 | P2/I1 | Reuse unaffected access/Git/preview tests; reassess or drop for P2/I1. |
| [src/lib.rs](../../../ymp-rust/crates/ymp-workspace/src/lib.rs) | 5 | P2/I1 | Reuse unaffected access/Git/preview tests; reassess or drop for P2/I1. |
| [src/preview.rs](../../../ymp-rust/crates/ymp-workspace/src/preview.rs) | 5 | P2/I1 | Reuse unaffected access/Git/preview tests; reassess or drop for P2/I1. |
| [src/repository.rs](../../../ymp-rust/crates/ymp-workspace/src/repository.rs) | 5 | P2/I1 | Reuse unaffected access/Git/preview tests; reassess or drop for P2/I1. |

## Limits and maintenance

The 212 runtime test attributes are not 212 pure pipeline tests. This file provides a complete file-level count and selected semantic examples, not an exhaustive verdict on every function. That distinction does not restrict the owner-authorized deletion option.

Recount affected files when the implementation baseline changes. If deletion is selected, remove obsolete test-only helpers/dependencies as part of the same scoped change where appropriate, then run the required Cargo commands on the resulting workspace. No minimum remaining-test count is imposed. Keep actual acceptance evidence and any missing coverage explicit; no successful empty suite is called proof of behavior.
