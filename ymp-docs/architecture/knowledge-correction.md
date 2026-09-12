# Evidence-bound knowledge correction

YMP-114 extends the existing memory, FTS5 and decision/event tables. A correction is an explicit relation between an exact stored knowledge version and a new independently reviewed, confirmed result. Neither a newer timestamp nor agent agreement establishes that relation.

## Trusted declaration and capture

The user declares optional `knowledge_correction` in a YMP-125 acceptance contract. It contains:

- `target`: the predecessor's `id` and full serialized-record `version` digest returned by knowledge inspection.
- `projection`: `project_outcome` (default) or `check_procedure`. Free-text candidate content cannot be a confirmed replacement.
- `applicability`: exact key/value constraints, equal to the predecessor's constraints. Project/global scope must also remain equal. Partial overlap does not retire a broader claim.
- `criterion_ids`: declared criteria whose trusted checks establish this correction for the named claim. A confirmed unrelated result without this captured binding cannot supersede it.
- Optional `source_replacement`: `previous_input` from the predecessor's actual captured input contract and `replacement_input` from the new contract's declared inputs. Runtime capture resolves both snapshots and requires changed bytes.

For example, add this to a configured acceptance contract whose criteria and checks verify the corrected O04 claim from `inputs/observations-corrected.csv`:

```toml
[acceptance_contracts.knowledge_correction]
projection = "project_outcome"
criterion_ids = ["observation-value"]

[acceptance_contracts.knowledge_correction.target]
id = "knowledge:REPLACE_WITH_STORED_ID"
version = "REPLACE_WITH_FULL_ENTRY_DIGEST"

[acceptance_contracts.knowledge_correction.applicability]
dataset = "observations"
site = "Hill"
week = "2026-W36"

[acceptance_contracts.knowledge_correction.source_replacement]
previous_input = "inputs/observations.csv"
replacement_input = "inputs/observations-corrected.csv"
```

This is part of the authoritative configuration, never a model-provided or automatically discovered manifest. The binding participates in the complete contract digest and atomic initial session capture. Existing immutable-resume and exact task-title rules remain in force. A malformed target, foreign scope or undeclared input rolls back session creation.

The sanitized requirements sent to planning and review include the correction relation, criteria and paths, but no expected bytes or verifier code. The predecessor is captured atomically with the contract; reviewers receive up to 4,000 characters of its historical claim even after current retrieval excludes it. Its complete record must match the bound target digest. Independent review must retain an exact correction-context digest as well as the exact result version. The runtime excludes the producer from review and independently verifies criterion coverage, actual snapshots, fresh source versions and the accepted result. Trusted check semantics remain the client's explicit responsibility; the runtime does not infer arbitrary prose or row semantics.

## Source change, activation and history

Capturing an explicitly bound source replacement makes the predecessor unavailable to current retrieval immediately, even if the old file remains intact at its original path. This fact comes from the captured contract and its immutable snapshots. It does not confirm the replacement value. A failed or interrupted correction can therefore leave the old claim unavailable and the replacement unconfirmed or pending.

The replacement projection is retained during execution. Even a confirmed source leaves that projection `proposed` until correction commit. `KnowledgeCorrectionPolicy` receives read-only accepted-result/binding data and returns proposals. The default proposes the captured relation; substituted policies may defer or nominate differently. They cannot provide arbitrary confirmed text, change source acceptance or write storage. Policy ID/version and resolution or denial are journaled.

`Store.commit_knowledge_correction` resolves the stored target/version, bound source acceptance, independent review and actual passing evidence inside one transaction. That transaction marks the old entry `superseded`, activates the replacement, sets its `supersedes` link and records the correction receipt/event. Stale targets, retired entries, conflicting successors, self-links and cycles are rejected. The deterministic proposal receipt makes exact replay return `already_applied`; replay never revives a historical entry or adds a second correction.

Normal FTS search, selected-ID resolution, automatic Engine context and internal/public MCP search exclude superseded entries and explicitly changed sources. Original content, origin and evidence remain in inventory. `Store.inspect_knowledge(project, scope)` returns each historical entry and digest, explicit current availability, its successor and a correction receipt. A recorded confirmation is historical evidence, not proof of current availability. Availability distinguishes pending corrections, unconfirmed candidates, changed sources, missing/stale evidence, applicability mismatch and lifecycle states.

Qualified reputation remains exclusively on the existing `Store.observe_confirmed` path, keyed by result/version/producing agent. A supported producing outcome can earn its one existing observation; correction, search, history inspection and replay award no additional credit. Agreement alone still earns none.

## Scoped consumers

Optional top-level `[knowledge_scope]` in trusted `config.toml` supplies the Engine's exact applicability for automatic retrieval and newly retained findings. For a bound correction, retained replacement scope comes from the captured correction binding. Scope maps allow at most 16 nonempty keys (128 bytes each) and nonempty values (1,024 bytes each).

`ymp memory observation --scope dataset=observations --scope site=Hill --scope week=2026-W36` searches supported current knowledge. Add `--history` to inspect lifecycle and availability. Public `ymp_knowledge_v1` accepts a `scope` object and explicit `history: true`; normal `id`/`version` lookup still enforces current availability. Internal `memory_search` accepts the same bounded scope map, with unconfirmed inspection still explicit. These query parameters never modify project authority or install corrections.

See [verification evidence](../../ymp-evals/reports/knowledge-correction-verification.md) for actual runtime and executable consumers. TUI presentation remains separately owned by the delegated UI work and consumes the same inspection DTO.
