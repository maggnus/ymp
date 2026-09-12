# Incremental knowledge verification

12/09 22:35 HKT, 2026-09-12. YMP-113 backend outcome: an accepted intermediate result becomes retrievable, source-bound knowledge before a later sibling runs; failure of that sibling preserves the outcome, its evidence-backed reputation and its knowledge. Unconfirmed findings remain inspectable candidates. Independent review and the coordinated YMP-118 UI consumer are separate acceptance work.

The development base was `c24b1ca`. Parent integration correction `01b2053` was applied separately as `676e122`; it resolves the pre-existing v1/v2 effective-execution identity mismatch between YMP-117 credit validation and YMP-122 invocation identity. No confirmation-grading or reputation logic is changed by YMP-113.

## Actual failing forms

Each command below exited **101** with the stated defect present. Mutations were restored immediately before subsequent checks; they are not part of the implementation.

| Command | Defect present | Observed failure |
| --- | --- | --- |
| `cargo test -p ymp-runtime knowledge_agreement_never_activates_general_claims -- --nocapture` | Original agreement-based global activation on the development base | `Agent agreement alone must not activate a general procedure` |
| `cargo test -p ymp-runtime knowledge_intermediate_survives -- --nocapture` | Incremental retention disabled, leaving final-only learning | `Intermediate result must have an active evidence-linked knowledge projection` |
| `cargo test -p ymp-runtime knowledge_scope_source_drift -- --nocapture` | Source file/grade checks removed from retrieval resolution | The post-input-change assertion requiring an omitted entry failed |
| `cargo test -p ymp-runtime knowledge_policy_substitution -- --nocapture` | Runtime applicability check bypassed after policy selection | Three excerpts were included instead of the single supported source (`left: 3`, `right: 1`) |
| `cargo test -p ymp-runtime knowledge_scope_source_drift -- --nocapture` | Shared procedure included a declared validator without an observed process result | `Inconclusive checks cannot become shared successful experience` |

These checks distinguish persistence, independent evidence, current source resolution and runtime-enforced selection from agent or policy assertions. They would still pass an inferior language-model answer or an inefficient retrieval strategy; no model-quality or cost-saving claim follows.

## Passing consumers

`cargo test -p ymp-runtime knowledge_ -- --nocapture` exited **0** with six tests after restoration and the observed-check correction:

- A public `Engine.run` uses the existing mock backend for normal work and a replacement backend that fails only the later sibling. The first task is accepted with exact-byte confirmation, no final learning assignment runs, the database is reopened, and a separate later session receives the original knowledge ID, serialized source version, result version and actual excerpt. Exactly one supported competence observation survives.
- `Store.outcomes` returns the first task's original absolute path and file digest from a non-Git directory. Reading it starts no invocation. The same source is available to the later session without rerunning production.
- Project, global and exact applicability constraints are exercised with Hill/Harbor and an absent scope. Wrong versions, foreign projects, changed input bytes and retired entries are filtered. A retirement replay stays retired. A changed source also invalidates the global projection and the current outcome view.
- A declared extra process kills itself, so it provides no exit result. The result remains confirmed by the separate exact-byte check, while shared experience includes only the actual passing check. No project path or dataset value enters global text.
- A substitute proposal policy supplies unsupported free text; it remains proposed and unconfirmed with the policy's identity. A substitute inventory policy nominates missing IDs, wrong versions, duplicate IDs, unknown candidates and a valid source. Only the resolved valid source enters the prompt, its exact excerpt reaches but does not exceed 8,000 characters, and the journal records the substitute implementation with a null FTS expression.
- Partial criterion coverage retains candidates and passing-evidence links without activation or reputation. A failed proposal policy preserves the checked project fact and records the attempted implementation.

`cargo test -p ymp-runtime memory_proposal_records_bound_origin_without_activation -- --nocapture` exited **0**. The real local team API rejects self-activation, binds the candidate's origin to the active assignment/invocation, supports explicit unconfirmed search, and preserves the candidate after invocation failure and storage reopening. Default search remains empty.

The original YMP-106 tests continue to exercise task-specific query selection, quoted FTS syntax, latest user requests, project/global scope, source-record and excerpt digests, exact character accounting, stale task references and disabled memory. Their legacy-shaped fixtures now use explicit unconfirmed inspection mode. A storage test directly loads a historical active record, verifies that it remains visible as unknown in inspection, denies supported retrieval, and rejects manufacture of new activation through ordinary saves.

## Final checks

All final commands exited **0**:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` — 236 passing tests, no failing suite.
- `git diff --check`

One earlier full workspace run encountered a busy session lock in the existing follow-up test; its isolated rerun and both later full workspace runs passed. No lock behavior was changed. An early broad runtime run before the parent identity correction could not reach expected narration and was terminated; it is not used as passing evidence.

Native-provider inference and credential reads were not performed. Bridge code was unchanged, so no bridge build was run. Cargo's provider checks use existing offline/scripted protocol fixtures.

## Integration and limits

The [runtime contract](../../ymp-docs/architecture/incremental-knowledge.md) documents the public DTOs and policy seams. The memory inventory exposes historical lifecycle snapshots; UI consumers must use `resolve_memory` for present supported availability. New automatic retrieval excludes unknown and unconfirmed sources, while explicit candidate inspection retains their labels. Plain memory saves cannot activate entries.

Outcome paths come from actual YMP-117 artifact snapshots. A result with no declared artifact snapshots has no guessed location. This change supplies deterministic inspection; the application location-question route is a separate integration consumer. Source correction/supersession, semantic scope inference, embedding services and comparative native-model evaluation are not implemented here. The existing source/version links and `supersedes` field are available to YMP-114.
