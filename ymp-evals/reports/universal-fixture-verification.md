# Universal fixture verification

Date: 2026-09-12. Task: YMP-119. Base revision: `59bece869aee9c1ded6e7a14e192c4b7a12b416b`. Treatment: synthetic validator controls, with no model/provider profile or inference. This report covers fixture preparation, not integrated runtime acceptance or model quality.

The [suite](../README.md) supplies four universal workflows and thirteen scripted protocol cases. The standard-library validators read actual delivered files and immutable input snapshots, and check normalized acceptance records or event projections independently of agent assertions. Reference artifacts and positive receipts in the tests are deliberately synthetic.

| Check | Observed result |
| --- | --- |
| `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s ymp-evals/tests -v` | 26 tests passed, including per-case positive controls and negative mutations |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | 100 unit tests passed; doc tests passed |

The negative controls reject missing and irrelevant objective outputs, changed agenda facts or Markdown heading structure, wrong refunds/void handling/counts/order, invented citations and incorrect source rows, changed input bytes, stale artifact evidence, unrelated passing commands, self-review, unsupported reputation credit, missing review usage and unknown usage reported as zero. Every protocol case rejects missing/extra events and a concrete policy regression. Restart checks also reject an unrelated replacement for the specified partial CSV; knowledge checks reject an altered corrected source.

The qualitative fixture confirms only formatting through deterministic code. Its accepted workflow receipt remains unconfirmed and awards no reputation. No deterministic usability score is assigned.

Full implementation diff reviewed, including source/reference consistency, event ordering, isolated effort-constraint failures, reservation arithmetic and charged restart inspection. All changes are under `ymp-evals`; runtime, provider, UI and task-registry implementation is unchanged by this assignment.

YMP-121 must still implement the trusted scripted-provider driver and exporter, run all seventeen cases against the integrated application, retain actual durable/native records and audit normalized projections. Standalone validators cannot authenticate a supplied receipt. Native compatibility, quality comparisons, provider resource use and elapsed-time advantages remain unmeasured. No real-provider quota was used.
