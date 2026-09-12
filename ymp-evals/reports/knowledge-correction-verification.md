# YMP-114 backend verification

Author verification started from accepted `9ad43dccb29840089a3f351c5651867aa808135d`. All execution uses mock/scripted providers with explicitly fixed low effort. No native inference or credentials are involved. This records backend evidence; independent review and delegated TUI acceptance remain separate.

The runtime consumer uses the actual universal CSV files, a pinned Python validator outside the project directory, public Engine/Store APIs and real durable records. The validator checks the O04/Hill/2026-W36 claim against actual supplied rows: 95 percent initially and 60 percent after correction. Both original and corrected CSV files remain present. No expected protocol receipt is substituted for runtime behavior.

| Consumer | Observed behavior |
| --- | --- |
| `cross_file_correction_reopens_retrieves_and_replays_without_duplicate_credit` | Confirmed 95 is reachable only in the matching scope. Capturing the distinct corrected file excludes the old claim before confirmation. A paused session resumes from immutable capture, independently confirms 60, atomically links/supersedes, reopens with full history and supplies only the successor to a later actual provider prompt. Exact correction and observation replay add no duplicate credit. |
| `unrelated_confirmation_agreement_and_changed_authority_cannot_supersede` | A newer confirmed unrelated result cannot replace the target. Wrong target version, applicability, input relation and criterion fail capture without a partial session. Agreement without objective checks leaves a candidate and no new credit. A producer cannot submit its own independent review. |
| `policy_substitution_competing_stale_and_cyclic_corrections_remain_runtime_owned` | A distinct injected policy defers correction. Changed replacement files, misbound self/cycle targets and competing successors are rejected; one atomic successor remains after reopening. |
| `unrelated_review_context_cannot_approve_the_correction_relation` | A fresh completed independent review containing the result but omitting its correction context is rejected; the otherwise matching review including that relation is accepted. |
| `malicious_correction_policy_cannot_activate_an_uncommitted_replacement` | A forged target returned by the actual injected policy cannot activate a replacement, even though the new result was independently confirmed. |
| `executable_correction_contract_scope_and_public_mcp_history_are_reachable` | Real CLI configuration reaches correction capture and commit; scoped CLI search and actual public MCP stdio return the successor, historical predecessor and scope filtering. Read arguments cannot supply activation authority. This consumer checks the executable path using the built-in greeting output; numerical semantics are checked by the runtime CSV consumer above. |
| `scoped_internal_memory_inspection_preserves_applicability_and_confirmation` | The actual internal team API accepts matching query scope, filters mismatches and keeps unconfirmed inspection separate from supported retrieval. |

## Controls and final checks

The exact executable consumer compiled and failed with exit 101 on the accepted starting revision: its correction contract is rejected as an unknown field. On the implementation it passes. Removing each of three independent guards also produces exit 101 at the intended assertion: cross-file source invalidation, exact independent correction context, and withholding replacement activation until commit. Each mutation was restored. Compact failure evidence is retained in [knowledge-correction-controls.json](../../ymp-docs/research/evidence/knowledge-correction-controls.json).

Final checks use a dedicated fresh `CARGO_TARGET_DIR` at the sibling `build114-final` directory:

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 304 tests passed, plus doc tests.

An earlier full-workspace attempt used the same target directory as the pre-change executable control and reported pre-change core types. The fresh dedicated build passed without a source change to address those errors. Final validation does not rely on the shared control build cache.

This extends existing metadata storage, confirmation and observation semantics. It adds no time decay, automatic semantic classification, isolated production workspace, new service or reputation reward. The universal fixture exporter for the complete release is still YMP-121 work; these consumers validate the actual correction path directly. UI code and the task registry were not edited in this assignment.
