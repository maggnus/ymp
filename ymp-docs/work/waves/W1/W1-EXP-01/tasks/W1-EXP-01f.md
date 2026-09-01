---
id: W1-EXP-01f
kind: task
wave: W1
card: W1-EXP-01
state: accepted
risk: critical
maturity: BUILD
relation: follow_up
depends_on: [W1-EXP-01e]
blocks: [W1-EVL-04e, W1-EVL-04f, W1-EVL-04a]
created_at: 2026-09-01T15:18:13+08:00
updated_at: 2026-09-01T16:27:38+08:00
started_at: 2026-09-01T15:53:23+08:00
accepted_at: 2026-09-01T16:27:38+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/a2a3de1288dfc4957b34bb6ecec7e5e8ee27452c
closure_commit: https://github.com/maggnus/ymp/commit/59e35d90981e78fbfe194ab7df41b4afab6d2d1b
evidence: ["[59e35d9](https://github.com/maggnus/ymp/commit/59e35d90981e78fbfe194ab7df41b4afab6d2d1b)"]
duration_minutes: 34
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 3
escalation_decision: bounded_retry
---

# W1-EXP-01f — Immutable v2 covers both strata in both splits

## Outcome

Without modifying accepted `weak-diagnostic-v1`, a new immutable v2 freeze supplies a complete
stratum×split matrix: decomposable and sequential/null tasks in both development and transfer, with
fresh task instances and exact protected oracles that make weak diagnosis and same-mechanism
strong-profile transfer jointly identifiable before any model call.

## Scope

### In

- A v2 manifest bound to accepted v1 manifest digest
  `085bdd3b915faf61545283ad1ec13e7e693398ebf28eded0460e51c122243696`, retaining the two accepted
  cells and adding one fresh sequential/null development task and one fresh decomposable transfer
  task.
- Four exact cells: development/decomposable, development/sequential-null,
  transfer/decomposable and transfer/sequential-null; each carries a stable task identifier,
  source/public-contract/protected-oracle/reference-candidate digests and its own seed namespace.
- Protected reference candidates, seeded invalid candidates and split/stratum/freshness mutations
  for the two new task instances; exact verification remains sibling-only.
- New executable types `DevelopmentV2Command`, `DevelopmentV2Manifest` and `CoverageCell`, exposed
  by exact CLI consumers
  `ymp-corpus development-v2 check --manifest <path> --digest <path>` and
  `ymp-corpus development-v2 validate --manifest <path> --digest <path> --root <path>`.
- Write zone: new
  `ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v2/**`, new
  `ymp-rust/tools/ymp-corpus/src/development_v2.rs`, optional new
  `ymp-rust/tools/ymp-corpus/tests/development_v2.rs`, minimal declarations/dependencies in
  `ymp-rust/tools/ymp-corpus/src/lib.rs`, `src/main.rs`, `Cargo.toml`, and mechanically forced
  `ymp-rust/Cargo.lock` only.

### Out

- No-touch: accepted `corpus/development/weak-diagnostic-v1/**` and
  `ymp-corpus/src/development.rs`; all `ymp-calibration/**`; primary `corpus/tasks/**`,
  `corpus/study/**`, `corpus/policies/**`, `corpus/registry.json`, primary seeds/budgets/outcomes;
  all agent, Application, runtime, verifier, TUI, research and work-record paths.
- Model calls, arm scheduling, profile selection, communication, primary or causal analysis, and
  changing a task after any assignment or oracle result.

## Acceptance

- [x] The v2 manifest binds the exact accepted v1 manifest digest and records `model_calls=0`; a
      changed parent digest, write to v1, or attempt to replace an accepted v1 cell is rejected.
- [x] `development-v2 check` reports exactly one task in each of the four stratum×split cells. A
      missing cell, duplicate cell, unknown stratum/split, or reuse of one task identifier in both
      splits fails before any candidate or model execution.
- [x] Within each stratum, development and transfer are fresh task instances: their task,
      source/public-contract/protected-oracle/reference-candidate digests and seed namespaces are
      distinct. Cross-split aliasing of any required identity fails compliance.
- [x] The retained v1 tasks remain byte-identical and digest-bound. Both new reference candidates
      pass their exact protected verifier; their seeded invalid candidates fail non-zero for
      distinct recorded reasons and blind spots.
- [x] Separate mutations for missing matrix cell, split alias, stratum relabel and protected-oracle
      substitution have immutable mutation/expected-result digests and each makes the v2 validator
      fail for its own reason with `model_calls=0`.
- [x] Preparation, digest reproduction, mutation validation and the CLI consumer walk execute in a
      fresh root with separate project, `HOME`, `YMP_HOME`, `TMPDIR`, build and export paths;
      protected bytes stay in a sibling verifier root unavailable to candidate projects.
- [x] A write-zone inventory fails on every v1, calibration, primary, recruitment, runtime,
      research or work-record change; focused tests, strict Clippy, formatting and
      `git diff --check` have no warnings or errors.

## Current state

Accepted and integrated as
[59e35d9](https://github.com/maggnus/ymp/commit/59e35d90981e78fbfe194ab7df41b4afab6d2d1b).
The immutable v2 supplies all four development/transfer × decomposable/sequential-null cells,
retains v1 by exact manifest and tree digests, and rejects every declared mutation before any model
path. No model has seen the freeze.

## Next action

Consume this exact v2 freeze from W1-EVL-04e and W1-EVL-04f without changing tasks, digests, strata,
splits, seeds or protected inputs.

## Guardrails

- Accepted v1 is historical evidence and immutable; v2 is additive and receives a new corpus ID,
  manifest digest and seed namespace.
- Fresh transfer tasks test transfer of a mechanism, not memorization of a task instance.
- A weak effect in one stratum advances only against the corresponding fresh transfer stratum; a
  null stratum remains a required control rather than an inconvenient exclusion.
- Any oracle weakness invalidates the freeze; it is not repaired by changing the task after model
  exposure.

## Findings

- Created from independent review of RUN-001/HYP-001/MAP-002 against the accepted v1 manifest and
  W1-EVL-04e/f. The 2×2 counterexample is a blocking scientific result, not a reason to rewrite v1.
- Independent review reached the product only after correcting its own tool-cache and corpus-layout
  errors; the final external mutation of the accepted v1 manifest failed on the expected digest
  mismatch with `model_calls=0` and an empty candidate project.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(7/10) ESCALATE 01/09 16:20 — the full 38-path diff and frozen identities satisfy the contract,
  but the independent mutation did not reach the product because the isolated tool cache was empty
  → CTO granted one proof-only retry with offline local tool caches → candidate unchanged
- CTO bounded_retry 01/09 16:20 — no product defect was established; the bounded evidence budget
  was extended only to let the same reviewer-owned v1 mutation reach the product offline
- R2(7/10) RETURN 01/09 16:24 — the CLI failed closed with `model_calls=0`, but the reviewer placed
  v1 outside the validator's expected sibling path and observed ENOENT rather than a digest failure
  → CTO granted a final sibling-layout proof inside the absolute review ceiling → candidate unchanged
- R3(8/10) ACCEPT 01/09 16:27 — the product read the altered accepted-v1 manifest and rejected it as
  `PARENT_V1_MUTATED` with `development manifest digest mismatch` → candidate project stayed empty,
  temporary state was removed and repository porcelain remained byte-identical

## Closure

### Accepted outcome

An additive immutable `weak-diagnostic-v2` binds the accepted v1 manifest and tree, retains its two
cells, and adds fresh development/sequential-null and transfer/decomposable tasks. The exact CLI
reports one task in every matrix cell, accepts four references, rejects two seeded defects and four
independent compliance mutations, keeps protected bytes outside candidate projects, and records
`model_calls=0`.

### Residuals

None.

### Evidence

- [59e35d9](https://github.com/maggnus/ymp/commit/59e35d90981e78fbfe194ab7df41b4afab6d2d1b)
  — integrated tree, byte-identical for the reviewed corpus paths to candidate
  [a2a3de1](https://github.com/maggnus/ymp/commit/a2a3de1288dfc4957b34bb6ecec7e5e8ee27452c).
