# YMP-126 trusted runtime driver

Review-ready source `a8493ac2a912b8f44fe5854a666607e4b7f6739b` passes all four workflows and all thirteen protocol cases in one source-bound run. The run took place from **2026-09-12 22:24:52 to 22:25:33 UTC** in `/tmp/ymp126-final-a8493ac-20260912`. `run.json.complete` is true; no named case was skipped. This is scripted boundary evidence, not a native model-quality, efficiency or cooperation comparison.

The source includes accepted main `bfba34b890672bdd03772030f7968f0a1457a6c8`, accepted runtime reservation/ownership corrections, and the independently accepted exporter corrections (`534ce54`/`bf7c3b4`, applied as `c051e43`/`d578f72`). The merge retained the two new incomplete-usage tests at the storage test file's tail; there was no production behavior conflict. The separately reviewed board/knowledge UI composition remains parent-owned and outside this snapshot.

## Reproduce

From the repository root, with the normal Rust toolchain and Python 3:

```sh
CARGO_INCREMENTAL=0 cargo build -p ymp-eval-driver -p ymp-cli
target/debug/ymp-eval-driver --output /tmp/ymp-universal-fresh-directory
```

The output must be fresh and outside Git. `--case CASE` runs one of the named cases for diagnostics; only all 17 passing cases set overall completion. Rebuild after source or checker changes. The build embeds the full Rust/Cargo/workload and fixture/checker inventories and rejects source drift before running a case. The actual executable digest and actual per-case backend identities/versions are retained; identities come from invocation records, including unknown metadata.

## Actual boundaries and retained evidence

- Document, transform, grounded and qualitative workflows execute through the public Engine with explicit mock-backed agents, actual artifact-producing subprocesses, native usage events, captured contracts, independent reviews and runtime acceptance. Objective output is independently recomputed from listed input files. Qualitative review has a separate deterministic relevance/usability rubric and stays accepted/unconfirmed with no reputation credit.
- Fixed size, fixed roster and adaptive membership exercise captured policy, actual allocation/board decisions and live team tools. The fixed-roster adverse setup uses reviewed public storage transitions and admitted native contributions before asking real Engine final-review admission to resolve an impossible independent-review roster.
- Resource and concurrency adapters use the same public Engine coordinator and retained `WorkspaceOwner`/reservation leases as normal execution. Immediate deferral creates no assignment, grant, spend or queued work. Real barriers hold native futures across independent/conflicting resource probes. Variable allowances, protected review spend and denial races go through the actual SQLite admission transaction.
- Authority runs the actual `ymp mcp` stdio executable and real TeamServer calls, including untrusted actor presentation, a foreign target and a runtime-only operation. The second native continuation receives a fresh grant and cannot restore the first. Its explicitly active endpoint and subsequent cleanup are both retained.
- Version and evidence adapters use actual admitted native production/review, captured input/result/checker versions, trusted check execution and Store acceptance invariants. They reject stale or self-authored review, unrelated commands, failing applicable checks and aggregate over-confirmation. No positive completion event is seeded by the provider.
- Knowledge uses actual confirmed 95 and 60 source computations, qualified retention, applicability filtering, a rejected agreement-only promotion, atomic correction and duplicate-observation handling. Query-only context IDs are preallocated before later session capture; that boundary distinction is documented. The projection walks the durable source sequence and retains extra matched transitions/credits rather than imposing call-script cardinality.
- Restart uses two distinct OS processes. SIGKILL follows persisted partial usage and actual partial file output. A fresh process revokes the old grant, denies the old capability and lets Engine inspect the actual two files before any production replay. The confirmed document stays accepted; totals remains partial. Capability transfer uses private inherited process pipes and is absent from retained evidence.

The final archive contains **232 files**, including selected non-Git task directories, separate metadata homes, full runtime/native journals, actual artifacts, projection sources/ID aliases, commands, usage and nullable cost metrics, and validator JSON. Expected traces, reference outputs and validator implementation bytes are never provider context or retained knowledge. Extra records outside the named projection remain in raw journals; authority cleanup has an explicit later endpoint. Provider identity is provenance, while usage stays attributed to session agent IDs.

The [manifest](ymp-126-final/manifest.json) inventories every retained file and identifies the [full evidence archive](ymp-126-final/runtime-evidence.tar.gz). Archive SHA-256: `ea6d989bb55b3d2e483bcc69d65a14ac80c9d1c33bcfbc554fb861dec5d8d2d2`. Compiled source-tree SHA-256: `535a26d0c3e1d885072a134f93677056ec3548a6115df907970ba9f151f759db`. Executable SHA-256: `4c2e619d441b13954aff4adbe8af58d83b745d658d6c177b186771ca64b94601`. Archived absolute paths identify the original observed run; use the reproduction command for a fresh portable run.

## Runtime corrections and failure controls

YMP-126 discovered gaps in earlier YMP-102 admission behavior. The already independently accepted reservation corrections and public context/project-ownership controls are described in [runtime corrections](ymp-126-runtime-corrections.md), [R1](ymp-126-runtime-independent-review-r1.md) and [R2](ymp-126-runtime-independent-review-r2.md).

The additional production delta is **`d31d49f`**, a captured `ResourceLimits.unknown_usage` enum: default `Stop`, explicit `BoundedNative`. The latter admits against reported spend and live reservations with all native/admission caps still enforced. It does not establish a strict true-spend bound or change unknown/partial records. This is separately subject to independent review; prior acceptance does not cover it.

The exact default-Stop restart failure is preserved in [the before archive](ymp-126-final/restart-stop-before.tar.gz): after a confirmed sibling and 40 complete units, the interrupted attempt contributes seven partial units. Fresh Engine inspection is denied `unknown_usage` at reported 47/100, starts no invocation and retains progress. The final scenario captures bounded mode **before** interruption, spends three units on actual read-only inspection, and ends at reported 50/100 with partial coverage and `strict_token_bound: false`. The [policy report](ymp-126-unknown-usage-policy.md) gives the before/after detail.

Discriminating controls passed for missing acceptance, extra actual usage/credit, duplicate and reordered knowledge transitions, missing projection sources, duplicate native/terminal closure and revocation, and early recovery closure. An unrelated qualitative file passes the structure validator but fails the separate scripted review rubric. Exporter controls also reject source/checker drift and duplicate/early/unattributed effort, location and partial-usage records. These controls mutate copies of real returned records; they never write fake runtime completion receipts.

## Validation

- `cargo fmt --all --check`: passed on the final source.
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `CARGO_INCREMENTAL=0 cargo test --workspace`: **402 passed**, zero failed. The one pre-existing default-ignored native catalog SDK fixture requires its separate npm setup; it is not one of the 17 evaluation cases and does not authorize native inference.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v`: **26 passed**.
- Actual final CLI: **17/17 passed**, four workflow and thirteen protocol validator results retained.

Logs are retained under [ymp-126-final](ymp-126-final/manifest.json). The initial final workspace compilation exhausted disk space before tests executed; its failure log is retained. Only this checkout's 3 GiB incremental cache was removed, then the full suite passed with incremental compilation disabled. No other active target or evidence was deleted.

All workload providers were mock/scripted with explicit identities and low supported values. Effort matrix values are protocol data only. No paid inference, credential reads, native account discovery or UI implementation occurred in this task. Final driver acceptance and release packaging/install checks remain the independent YMP-121 reviewer's responsibility.
