12/09 20:29 HKT — **ACCEPT R1(9/10)**. YMP-106 meets its contracted outcome. No blocking, major, or minor finding remains. Code 9/10; work 9/10; consumer experience 9/10. The query follows the active task or request, the actual added memory is bounded, and the recorded sources identify what reached the invocation. This verdict covers deterministic retrieval and provenance, without claiming model-quality or resource-saving gains.

The complete diff of `6e5aa6aaa1eb5bd039ab5b6efe555c90b530c537` was reviewed in the authorized isolated checkout against AGENTS.md, YMP-106 acceptance, the runtime contract, approved intent, and the team skill. All three changed files were read, including the complete test changes. The intent SHA-256 remains `4479e919c5b47934e69e81ac493d16e90a0dc420a9a370382e79bca3f3c754d9`, matching the task register. Repository source stayed unchanged throughout the review; the only review deliverable is this report. No task-register, intent, UI, or provider-authentication changes were made.

The production caller chain was followed through bidding, execution, review, final review, planning, and conversation follow-up. Task calls supply their actual `TaskAttemptRef`; `memory_query` checks task identity and attempt before reading title/description. Conversation retrieval uses the latest `author='you', kind='user'` row, independently of the shared-message window. Session-wide work uses the captured goal or legacy stored request. The existing FTS expression, relevance order, active-status filter and project/global filter are retained. The assembler constructs the memory excerpt first, places that same string in `TurnRequest.prompt`, and commits its retrieval evidence before `run_turn` starts.

Relevant source: [query selection](../../ymp-rust/crates/ymp-runtime/src/engine.rs#L397), [actual prompt assembly](../../ymp-rust/crates/ymp-runtime/src/engine.rs#L499), [retrieval event](../../ymp-rust/crates/ymp-runtime/src/engine.rs#L646), [latest user input](../../ymp-rust/crates/ymp-storage/src/lib.rs#L262), and [unchanged lookup policy](../../ymp-rust/crates/ymp-storage/src/lib.rs#L368).

Independent consumer evidence used a disposable Cargo harness at `/tmp/ymp106-review-othosme5/probe`. Its passing form includes the unchanged production engine and team-server source by path. A local Node JSON-RPC responder replaces inference and captures only `purpose` and `prompt`; it never records the full request, environment, or MCP capability. This exercises the actual provider request boundary. The public `Engine.run` path also completed a temporary invoice task through ten invocations and produced `invoice.txt` containing `12500 cents`. That synthetic deliverable establishes reachability of the retrieval path, not invoice-model quality.

| Walked case | Observed result |
| --- | --- |
| Invoice task behind a generic role prefix | Only `invoice` entered the prompt; `generic`, unrelated, retired and foreign-project rows were excluded. |
| Seven matching active project/global rows | Five included sources retained SQLite relevance order: `invoice`, `invoice-1`, `invoice-2`, `invoice-3`, `invoice-0`; the last is global. Actual memory was 234 characters. |
| Multibyte content and partial final entry | Included `invoice`, `invoice-3`, `invoice-4` in order. Excerpts were 58, 4,448 and 3,492 characters; two separators made exactly 8,000 characters and 23,672 UTF-8 bytes. Retrieved rows omitted by the allowance were absent from the event and assignment references. |
| Changed memory source records | Version hashes matched the serialized record actually used, including changes to the same entry ID. Excerpt hashes matched the included prefix, including truncation. |
| Latest conversation request beyond the history window | After 10,002 intervening messages plus misleading author/kind combinations, retrieval used `Invoice "totals" (ledger)`. The effective FTS expression was `"Invoice" OR """totals""" OR "(ledger)"`. |
| Legacy, empty and disabled retrieval | Legacy request was used when no captured policy existed. Whitespace-only requests and disabled retrieval produced zero included characters, no entry references and null query fields. |
| Stale task attempt | Rejected with `Memory query refers to a missing or stale task`; no provider invocation was created. |
| Public run and trace export | All ten invocation prompts matched their journaled sources, counts and digests; `ymp trace` exported the same structured trace as `Store.trace`. |

One concrete provenance chain from the invoice control is assignment `fc83c7b7-b306-47ea-86cd-4b7d06da1fd4`, invocation `f1686576-712b-4412-8162-7f046e381928`, task `544ca4c6-0bc6-425a-b368-b8aa1f2da5e8`, retrieval event sequence 5. Entry `invoice` is active in project `4aa83d9c-3a68-4626-871a-ce960d5ae7d6`, from `source-history`. Its exact 58-character excerpt is `Invoice totals: Sum invoice ledger amounts in exact cents.` Its source version is `94b3d4e61168677fca2d652e303c0332af3b45dbd8b9991521839ed30d29de09`; its excerpt SHA-256 is `828371fa452d570dc9af8e3ef57f91f791bcc06b4a2c1d093c0b7c5d5ee772ca`. The assignment context carries the same entry ID, source session, excerpt count and digest. A separate Python `hashlib` check reconstructed record field order from `MemoryEntry`, checked the hashes independently of the Rust digest helper, and compared all ten public-run captures with SQLite records and the CLI trace.

The same independent harness was run against seven deliberate single-change variants, generated outside the checkout. Each child command exited **101** at the relevant assertion; the control driver exited **0** after confirming these failures.

| Broken form | Actual discriminating failure |
| --- | --- |
| Search the role prompt again | Source query was the boilerplate instead of the task; captured context included the unwanted generic entry. The original repository invoice test was separately run against this variant and failed with `left: ["generic"]`, `right: ["invoice"]`. |
| Raise allowance to 80,000 | Captured context reached 17,870 characters; assertion `actual excerpt exceeds 8000 Unicode characters` failed. |
| Include six entries | Captured memory was 298 characters and included the sixth row; comparison with five expected excerpts failed. |
| Substitute another project for lookup | Event identified `foreign` instead of `invoice`; source-ID comparison failed even though the two entries had identical excerpt text. |
| Record a false excerpt digest | `wrong-digest` differed from the SHA-256 of the captured excerpt. |
| Record a false source version | Prefixed `wrong-…` value differed from the serialized-source SHA-256. |
| Reuse the earlier conversation request | Recorded `Old unrelated request` differed from the latest actual quoted invoice request. |

These checks distinguish wrong query origin, scope/identity, order/count, Unicode allowance, stale conversation selection and false provenance. They would still pass a lexically matched entry whose advice is substantively poor; FTS semantic relevance and provider output quality are outside this outcome. Empty, one, many, multibyte, truncated, disabled, stale-attempt, edited-source and long-history edges were walked. Native inference, timeout/cancellation, crash recovery and concurrent source edits were not exercised by this review.

Executed commands and results, with the reviewed checkout as working directory unless an absolute manifest is specified:

```text
cargo test -p ymp-runtime memory -- --nocapture
  exit 0; four matching tests passed
cargo fmt --all --check
  exit 0
cargo clippy --workspace --all-targets -- -D warnings
  exit 0
cargo test --workspace
  exit 0
cargo run --offline --manifest-path /tmp/ymp106-review-othosme5/probe/Cargo.toml --target-dir /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/106/target -- /tmp/ymp106-review-othosme5/good
  exit 0; seven assembly cases and a complete public run
python3 /tmp/ymp106-review-othosme5/run_controls.py
  exit 0; seven variant cargo-run children exited 101
cargo test --offline --manifest-path /tmp/ymp106-review-othosme5/probe/Cargo.toml --target-dir /private/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/106/target memory_retrieval_uses_task_content_and_records_bounded_sources -- --nocapture
  exit 101 with the role-query variant included; generic replaced invoice
cargo run --offline -p ymp-cli -- --home /tmp/ymp106-review-othosme5/good/state trace 3ab79574-5d3b-4525-9e86-6f8fb3446df2
  exit 0; redirected to good/cli-trace.json
python3 /tmp/ymp106-review-othosme5/verify_evidence.py
  exit 0; seven assembly cases and ten public-run prompts matched
```

The initial disposable-harness compile failed because `unwrap_err` required an unavailable `Debug` implementation; its assertion was corrected outside the checkout. Its first two launches also exited 1 because the harness manifest lacked the directory depth expected by `Engine::new`; moving the manifest and recompiling resolved that harness-only issue. These failures are not product findings or adverse-control evidence. The harness has two expected unused-public-function warnings; repository Clippy itself is clean.

Retained local evidence is under `/tmp/ymp106-review-othosme5`: the harness, local responder, variant sources, failing logs, captured prompts, SQLite state, and trace exports. `good/evidence.json` SHA-256 is `f9d5c56c0e88c7cf71134dd34246f395d45cbcd97f7cb2603ec8326c5e1decf6`; `good/project/captured.jsonl` SHA-256 is `e4f4e5c0ee6b7a15e8741106f230649aa81de3d00ee4fc0308f5677799af6c60`. These temporary paths are reproduction aids, not application storage or release artifacts. The five-entry/8,000-character allowance applies to newly added memory per invocation; it is neither a token limit nor a bound on retained native conversation. No correction is required for acceptance.
