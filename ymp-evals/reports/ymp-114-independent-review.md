# YMP-114 independent backend review

Verdict: ACCEPT, 9/10. Candidate: `6ed96c808090ff748764e619ad8118b411c5159f`; base: `9ad43dccb29840089a3f351c5651867aa808135d`. No blocking defect found. This accepts the backend candidate only; delegated TUI visibility and overall task acceptance remain separate.

The review read AGENTS.md, YMP-114 and YMP-125 criteria, the candidate diff, runtime/storage trust paths, focused consumers and the author report. The author checkout was not edited and remained clean. No native provider or credential access was used.

## Evidence

All paths below are relative to the candidate checkout.

- Trusted correction authority participates in the captured contract: `ymp-rust/crates/ymp-core/src/confirmation.rs:177`; exact target/version/project/applicability and actual old/new captured source bytes are resolved at `ymp-rust/crates/ymp-storage/src/knowledge_correction.rs:14` and `:49`. Existing complete contract equality on resume remains at `ymp-rust/crates/ymp-runtime/src/engine/confirmation.rs:48`.
- Independent review carries the exact correction relation as well as the immutable result: `ymp-rust/crates/ymp-storage/src/knowledge_correction.rs:109`, `:290`, `:327`; source acceptance and all passing criterion evidence are revalidated at commit. The source snapshot is preserved for review after normal retrieval filters it.
- Cross-file source capture invalidates the predecessor even with both input files retained: `ymp-rust/crates/ymp-storage/src/knowledge_correction.rs:142`, called from `ymp-rust/crates/ymp-storage/src/knowledge.rs:63`.
- Replacement projection stays pending at `ymp-rust/crates/ymp-storage/src/knowledge.rs:209` and `:329`. The correction transaction validates the stored target, pending replacement, current evidence and chain, then writes both states, link and receipt together at `ymp-rust/crates/ymp-storage/src/knowledge_correction.rs:187`. Its deterministic replay path returns the original receipt without mutating history or reputation.
- Proposal policy is injectable and outside validation; proposal acceptance substitution is denied at `ymp-rust/crates/ymp-runtime/src/engine.rs:2457`. Storage resolves all authority. Source-only confirmation, arbitrary policy targets and agreement cannot activate the correction.
- Shared inspection provides explicit availability and replacement links at `ymp-rust/crates/ymp-storage/src/knowledge_correction.rs:373`; scoped CLI and public MCP reads use it at `ymp-rust/crates/ymp-cli/src/main.rs:322` and `ymp-rust/crates/ymp-runtime/src/public_mcp.rs:251`. Internal scoped memory reads enforce the same applicability path in `ymp-rust/crates/ymp-storage/src/authority.rs:233`.

## Independently executed validation

Executed in the exact candidate checkout, using `CARGO_TARGET_DIR=../build114-final`:

- `cargo test -p ymp-runtime --test knowledge_correction`: all 5 public runtime tests passed. This includes the actual CSV validator producing 95 then 60, retained original/corrected files, paused immutable capture/resume, later provider prompt retrieval, denied trust/context/activation cases, competing proposals and replay without duplicate credit.
- `cargo test -p ymp-cli --test acceptance_contracts executable_correction_contract_scope_and_public_mcp_history_are_reachable -- --nocapture`: passed; this starts the real executable and public stdio MCP consumer.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, including accepted YMP-125 contract ingress/resume tests, internal scoped memory, runtime budgets/grants/locks and existing TUI tests.

The author-reported baseline and removed-guard controls were inspected but were not rerun because this assignment kept the author source read-only. The approval relies on inspected implementation and independently rerun production-boundary consumers, not solely on that report. No new service/database, time decay, timestamp promotion, automatic semantic trust, source-copy behavior or reputation reward was introduced in the reviewed diff.
