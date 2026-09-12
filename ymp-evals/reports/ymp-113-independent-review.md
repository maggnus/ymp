12/09 23:09 HKT, 2026-09-12 — **RETURN R1(6/10)**. The incremental knowledge behavior works, but the new public outcome API can replace an actual captured artifact location with a location where the artifact never existed. Code 6/10; evidence 8/10; public consumer 6/10. The open location defect determines the score and verdict.

Reviewed outcome: `0bc18b0ba8389da4f56dee695b65f389174ca90e`, on separate parent execution-identity correction `676e122` (parent mapping `01b2053`). The complete sixteen-file outcome diff was read against current MAIN YMP-113, approved `intent.md`, YMP-116 runtime contracts, replaceable subsystem interfaces, the knowledge/evidence and location fixture contracts, and the team skill. No product source was modified. The sole checkout change is this report.

**F1 — Major, contracted outcome: `Store.outcomes` reports the edited project directory as the original result location.**

Location: `ymp-rust/crates/ymp-storage/src/knowledge.rs:352`, `:370`, and `:376`. The new API loads the current mutable `Project.path` and joins every captured relative artifact path onto it. It does not use the recorded session policy or producing assignment directory. The public relocation entry point is `ymp-rust/crates/ymp-storage/src/lib.rs:126`.

A public `Engine.run` probe creates and confirms `greeting.txt`, then deliberately fails the later sibling. The first outcome and its captured directory remain durable. Calling `Store.relocate_project(project_id, empty_existing_directory)` changes only the project association; no file is moved. Calling `Store.outcomes(source_session)` then returns:

| Recorded fact | Before relocation | After relocation |
| --- | --- | --- |
| `directory` | `/private/tmp/ymp113-independent-04rj1w98/relocate/project` | `/private/tmp/ymp113-independent-04rj1w98/relocate/relocated` |
| Artifact path | Original directory plus `greeting.txt` | Edited directory plus `greeting.txt` |
| Reported artifact exists | true | false |
| Original artifact still exists | true | true |
| Captured `SessionPolicy.cwd` | Original directory | Original directory |
| Result ID | `98a75d18-5ae9-4dba-88b4-e05893cb1c1c` | unchanged |
| Result version / artifact digest | 1 / `67b63638f5cf6adcdea67af75aee1086192da26aaf575687ea4ecd5c05b90a4f` | unchanged |
| Current flag / grade | true / confirmed | false / unconfirmed |

The unconfirmed flag does not make the invented historical path truthful. This breaks the supplied outcome-inspection requirement and the implementation document's explicit promise to return the original directory and absolute captured artifact paths. It can mislead the application location route before that consumer is integrated. Routing and a general prior-outcome router are unnecessary to reproduce the defect.

The external public probe exits **1** with `Error: Outcome original captured path replaced by mutable project relocation`. Its control is the same captured result before the one public project-association edit: the original path, result identity, confirmed grade and zero new source-session invocations all pass. Evidence is `relocate/relocation.json` in the external probe directory; its SHA-256 is `1ee0e32e43eadec42c917833cd843c3cf6476caf4828d8250802f2b427f1a5a9`.

Correction should keep original location metadata anchored to the actual captured runtime directory. Any current relocation mapping must be represented separately and must not impersonate the captured location. Current grade and freshness must also identify the source location they check. The follow-up check must exercise relocation to an empty directory while the original artifact still exists, preserve result IDs and digests, and retain the existing changed-input/artifact behavior without new inference. Historical records without a captured directory need an explicit absence or documented fallback, rather than an invented original location.

**Observed limitation, outside the return condition: MCP memory search has no finite response allowance.**

`ymp-storage/src/lib.rs:594` and `:596` remove the former twenty-row empty-query and ten-row FTS limits. `ymp-storage/src/authority.rs:236`–`:256` now collects and serializes every matching entry. Through a real admitted `TeamServer` socket call, `memory_search` with `query: "oversized", include_unconfirmed: true` returned twenty-five candidate records containing 30,000 characters each: **758,536 JSON characters in one response**, exit **0**. Those records are within the existing per-candidate size limit. More records increase the response without an application bound. This is an actual tool-consumer observation, not an extrapolation from the engine's automatic assembly.

The documented five-entry/8,000-character allowance explicitly concerns automatic memory excerpts added to an invocation. That allowance was verified separately and is not silently imposed here as a newly specified MCP limit. The release should track a bounded tool-search response/pagination policy and truthful omission information as its own decision. The present review does not claim that every knowledge access path is context-bounded. `mcp_bounds/mcp-bounds.json` retains the measured counts; SHA-256 `8990b4bba24a04d70623f3defa8746c0f23279ba4bd88d09cfdad15175e71e8b`.

**Passing behavior and adverse inputs.**

The independently written external crate depends on the reviewed public crates. Its execution wrapper rejects every non-Mock provider, records the exact prompts it receives, forwards ordinary work to the existing mock backend and fails only the later sibling. The public normal probe exits **0** and establishes:

- The source session stops as blocked after the later sibling fails. Ten captured source invocations include a confirmed first task; no final `learn` assignment runs. Reopening storage retains the active project projection, evidence/result links and exactly one supported competence observation.
- A distinct later session retrieves that source before its scripted planning failure. Both the `memory_retrieval` journal and the actual received provider prompt contain the retained greeting. This verifies delivery, not only inventory membership. Outcome inspection preserves the source invocation count and requires no inference.
- Public `save_memory` rejects five concrete writes: a new active arbitrary claim carrying valid source IDs, a proposed claim with confirmed provenance, a new proposed row carrying an existing source, stripping provenance from an existing protected row, and changing protected content while retiring it. The inventory is byte-equivalent as JSON before and after each rejection.
- `retain_knowledge` with a real confirmed acceptance ID and free text `Every greeting cures bugs` retains a proposed, unconfirmed policy-authored candidate. The source ID cannot bless that unsupported statement. Supported search excludes it; explicit MCP inspection includes it with `provenance.confirmation: unconfirmed`.
- Wrong serialized source versions, a foreign project and missing exact applicability keys do not resolve. A changed captured input makes the outcome unconfirmed/current=false and removes the source from direct `Store.memory`, CLI memory and authorized MCP default search.
- Actual CLI commands return one active matching result while its source is current, zero results for the unsupported candidate, and zero results after input drift. All three commands exit **0**. The CLI uses a prewritten mock-only configuration and does not perform discovery or inference.

The required workspace run also exercises the new default agreement-denial, scope/source/retirement, alternate inventory retrieval, alternate free-text proposal, partial confirmation, failed proposal-policy and MCP proposal-origin cases. Reading those complete tests against the production consumers confirms that alternate retrieval returns only IDs/version hints and is re-resolved outside the policy; the runtime limits actual automatic excerpts to five and 8,000 characters, records exact source/excerpt digests, and records a null effective FTS query for inventory selection. Replacement proposals cannot select a source or supply confirmed free text. Global projection content consists of bounded check-type procedures with actually observed passing checks, excludes project paths/bytes, and does not assert success for the deliberately inconclusive process check.

Legacy entries remain inspectable with absent provenance and their historical lifecycle values, while supported retrieval excludes them. Historical inventory snapshots and current `resolve_memory` availability are different contracts. This distinction was read for YMP-118 integration; no UI code was changed or independently reviewed in this assignment.

The adverse inputs above exercise the old unsupported-active write shape, current-input corruption, unsupported candidate text, wrong versions and wrong scope at public entry points. The author report's five temporary source-mutation controls were read, but those source mutations were not independently reintroduced into this read-only checkout. Their failing exit statuses are author evidence, not newly claimed reviewer observations. The independently reproduced relocation failure remains open on the submitted code. Mock/scripted behavior cannot establish language-model quality, memory benefit or resource savings.

**Commands and retained evidence.**

Commands in the reviewed checkout:

| Command | Actual exit / result |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo test --workspace` | 0; 236 passing tests |
| `cargo build -p ymp-cli` | 0 |
| `git diff --check` | 0 |

External crate: `/tmp/ymp113-independent-04rj1w98`, with `Cargo.toml`, `src/main.rs`, and separate captured stores/task directories for each mode. The harness preserves existing captures by choosing a fresh case directory on subsequent runs. The actual initial public normal and relocation commands were:

```sh
PROBE_DIR=/tmp/ymp113-independent-04rj1w98 cargo run --offline --manifest-path /tmp/ymp113-independent-04rj1w98/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/113/target -- normal
PROBE_DIR=/tmp/ymp113-independent-04rj1w98 cargo run --offline --manifest-path /tmp/ymp113-independent-04rj1w98/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/113/target -- relocate
```

They exited **0** and **1**, respectively. The already-built probe run with `mcp_bounds` exited **0** and printed the measured row/character counts. Initial harness-only unused-code/result warnings were removed outside the checkout; the final repository Clippy result remains clean.

The CLI walkthrough used the built `target/debug/ymp --home /tmp/ymp113-independent-04rj1w98/normal/state -C /tmp/ymp113-independent-04rj1w98/normal/project memory greeting`, with `input.csv` set first to `Hill,95\n` and then `Hill,60\n`; `memory bugs` tested candidate exclusion. `normal/cli-current.json`, `normal/cli-candidate.json`, and `normal/cli-stale.json` retain the actual stdout. The probe also retains complete source traces, received prompts, public API responses, rejected-write diagnoses, outcomes and drift captures. `normal/prompts.json` SHA-256 is `367c2cfd3f91cb454f2525678664d6bab596d0eb799dc25b64f2d1760b720e59`; `normal/public-memory.json` is `283528386546a09f5a3981048e4233753221c533c8ff7641d62cc44d880be4c2`.

No real-provider inference at any effort, native credential reads, UI edits/review, intent edits, task-registry edits, bridge build, or application location routing occurred. Live MCP capabilities stayed in process memory and were excluded from captures. The required unattended workspace suite includes existing scripted provider and TUI tests. Correction/supersession, the YMP-204 general router and combined release integration remain outside this verdict.

Ledger: `R1(6/10) RETURN 12/09 23:09 HKT — original outcome paths follow mutable project metadata → public relocation probe returns a nonexistent replacement path while captured cwd and original file remain → F1 requires correction before acceptance`.
