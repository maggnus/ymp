# YMP-126 exporter corrections

Code checkpoint: `534ce54a80a7498795f58ff162d9eb40ae8dcb13`, based on author-coordinated `51b44fc8da828e27a8562467e5316e3e35b37ae4`. This is a bounded exporter correction for independent review, not final acceptance of all 17 evaluation cases. Runtime production APIs, other adapters, main, UI, intent, registry and immutable fixtures are unchanged.

The driver now embeds build-time source and fixture hashes. Source inputs include workspace Rust/Cargo files, runtime/provider/core/checker implementations and driver workloads; expected/reference/checker fixture files are classified separately and never added to agent context. Both the initial manifest and per-case metrics verify current bytes against the embedded snapshot, and identify the actual running executable by SHA-256. Backend identities, versions, invocation IDs and observed native versions come from each case's retained invocation records; missing metadata stays null. The former universal `ymp.evals.scripted` label is removed.

Effort and partial-usage backends record actual native observations in the same durable event sequence used by runtime transitions and trusted boundary return observations. A unique observation ID binds each raw native row to its exact durable payload; missing, duplicate, altered or inconsistently ordered sources fail explicitly. This replaces the UTC timestamp merge. Normalizers preserve extra native occurrences, unadmitted calls, duplicate runtime lifecycle events, changed order and extra grants. The partial strict-guarantee query has a complete source entry containing its actual returned budget/error.

Location projection iterates actual acceptance and answer records and all returned outcomes from the later metadata query. Additional occurrences are retained with their sources. Invocation counts for answers are derived from the observed interval since the requesting user message. Missing or ambiguous result/path attribution fails rather than inventing a binding. Selected artifact bytes, runtime acceptance and provider context remain governed by the original workflow.

Verification completed with debug information and incremental compilation disabled in this isolated checkout:

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0 |
| Four dedicated exporter tests | 4 passed, 0 ignored |
| Rebuilt executable: `effort-support` | Exit 0; 5 projected events and 5 source entries |
| Rebuilt executable: `location-retrieval` | Exit 0; 3 projected events and 3 source entries |
| Rebuilt executable: `partial-usage` | Exit 0; 4 projected events and 4 source entries |
| Stale executable/source control | Exit 1 before creating any case directory |

The dedicated tests first run the real affected case adapters, then mutate copies of the retained records. The unchanged independent validator rejects duplicate native closures, duplicate runtime completion, extra/unattributed calls, reordered closure/denial observations, additional answers/acceptances/query results, extra grants and duplicated guarantee observations. Equal UTC timestamps leave the durable order unchanged. Missing or altered raw provenance, contradictory source order, and source/workload/checker/reference drift fail. These are synthetic negative controls over actual captured records; they do not create runtime acceptance or claim model quality.

The separate actual-executable drift control appended a comment to the owned exporter test file after the binary was built. The executable returned `Evaluation source differs from the compiled build; rebuild before running` before starting a case. The original bytes were restored. The three successful CLI manifests bind revision `534ce54a80a7498795f58ff162d9eb40ae8dcb13` and executable SHA-256 `171e059c181f525f0adcd00ef2d8f6e271ce2ee9a022f502fa77a9920bab256e`; their source-tree digest is `59f44599ce0f694c281f086d66134405197b9678341630bba2b80bdce80be49e`.

Retained case bundles and the executable drift transcript are under `/private/tmp/ye-rx93zy00`. Full Cargo output and structured results are beside this checkout in `exporter-final-checks.log`, `exporter-final-checks.json` and `exporter-cli-evidence.json`. A machine-readable summary is retained in [ymp-126-exporter-corrections.json](ymp-126-exporter-corrections.json). Reproduce a selected case with `cargo run -p ymp-eval-driver -- --output /absolute/fresh/non-git/directory --case CASE_ID` after building the current checkout.

The standard workspace suite retains its existing single explicit Claude SDK catalog ignore; this report does not claim that separately gated bridge fixture run. All added workloads are scripted. No installed-provider inference, owner application-home changes or task-completion claims were made.
