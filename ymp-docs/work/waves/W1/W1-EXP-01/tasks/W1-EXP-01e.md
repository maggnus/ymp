---
id: W1-EXP-01e
kind: task
wave: W1
card: W1-EXP-01
state: accepted
risk: critical
maturity: BUILD
relation: follow_up
depends_on: []
blocks: [W1-EVL-04e, W1-EVL-04f, W1-EVL-04a]
created_at: 2026-09-01T13:41:52+08:00
updated_at: 2026-09-01T14:40:21+08:00
started_at: 2026-09-01T14:00:00+08:00
accepted_at: 2026-09-01T14:40:21+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/77ec6d0447b391206a725dbdcd7153c15e091b3c
closure_commit: https://github.com/maggnus/ymp/commit/58c20c3838fa3bb77e5b71c0f3af66528055d79f
evidence: ["[58c20c3](https://github.com/maggnus/ymp/commit/58c20c3838fa3bb77e5b71c0f3af66528055d79f)"]
duration_minutes: 40
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 1
escalation_decision:
---

# W1-EXP-01e — Held-out L4+ strata have frozen tasks and oracles

## Outcome

A frozen development-only L4+ ladder supplies held-out decomposable and sequential/null task
packages whose exact oracles, mutations, strata and digests can distinguish complementary synthesis
from independent selection without consuming a primary-study task or seed.

## Scope

### In

- At least one decomposable task with two independently checkable branches plus an integration
  property, and one strongly sequential or expected-null control.
- Public requirements, visible checks, protected oracle inputs, seeded invalid candidates, task and
  oracle digests, decomposability labels, and a frozen development/transfer partition.
- Executable preparation, exact verification, and manifest compliance in a fresh disposable root.
- Write zone: new files under
  `ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v1/**`, a new
  `ymp-rust/tools/ymp-corpus/src/development.rs`, and only the minimal module/CLI declarations in
  `ymp-corpus/src/lib.rs`, `src/main.rs`, its `Cargo.toml`, and mechanically forced `Cargo.lock`.

### Out

- Existing L1–L3 calibration cases, frozen primary corpus/seeds/outcome, model calls, arm scheduling,
  participant communication, and causal interpretation.
- No-touch: all `ymp-calibration/**`; existing `ymp-corpus/corpus/tasks/**`, `corpus/study/**`,
  `corpus/policies/**`, `corpus/registry.json`, primary-analysis code and artifacts; all agent,
  recruitment, runtime, TUI, work-tree and research-document paths.

## Acceptance

- [x] Every task package has a stable identifier, source digest, public-contract digest, protected
      oracle digest, stratum, split, and a seed namespace disjoint from calibration and primary use.
- [x] The decomposable package exposes at least two branch oracles and one integration oracle;
      removing either required branch or the integration step makes the protected verifier reject.
- [x] Its protected manifest names distinct branch-A omission, branch-B omission and integration
      bypass mutations with immutable mutation and expected-result digests; applying each mutation
      to the accepted reference candidate makes the exact verifier fail non-zero for its own reason.
- [x] The sequential/null package declares its dependency order and expected-null stratum before
      execution; a manifest that relabels it as decomposable is rejected by compliance validation.
- [x] A separate stratum-relabel mutation and digest is rejected before any model call; it cannot
      share the success condition of a branch or integration mutation.
- [x] Correct reference candidates pass and every seeded invalid candidate fails through the exact
      verifier with captured exits; the report states each oracle's blind spot.
- [x] Preparation, mutation validation, digest reproduction and split-integrity checks run without
      model calls in a fresh root with separate project, `HOME`, `YMP_HOME`, `TMPDIR`, build and
      export paths.
- [x] A write-zone inventory fails if the candidate changes recruitment, calibration, primary
      corpus/study/policy/seed/budget/outcome, or any path outside the declared additive seam.

## Current state

Accepted and integrated as
[58c20c3](https://github.com/maggnus/ymp/commit/58c20c3838fa3bb77e5b71c0f3af66528055d79f).
The frozen development-only corpus contains decomposable and sequential/null strata, separate
development/transfer assignments and protected exact-oracle bindings. L1–L3, primary paths and
model-call budgets remain untouched.

## Next action

Consume this exact freeze from W1-EVL-04e without changing its tasks, digests or protected inputs.

## Guardrails

- Task design cannot be changed after a model sees an assignment or an oracle result is revealed.
- A task that the strong single profile already solves at ceiling remains a labelled control rather
  than being replaced post hoc.
- No protected oracle path or expected value enters a participant workspace or public manifest.
- Each negative check executes against an isolated candidate copy; protected mutation bytes and
  expected values remain in the sibling verifier root.

## Findings

- Created from the scientific plan audit: without this owner, the project jumps from elementary
  calibration directly to an uninterpretable coordination comparison.
- Pre-dispatch check required an explicit disjoint write zone and distinct executable
  branch/integration/relabel mutations; both are now part of acceptance.
- Independent Critical review inspected all 46 paths and rejected a newly corrupted protected
  oracle byte by digest before candidate execution with `model_calls=0`.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(9/10) ACCEPT 01/09 14:40 — all 46 paths, frozen splits and protected bindings satisfy the
  Critical contract → the author's exact zero-model checks remained valid → an independent
  protected-oracle byte corruption failed on its digest before candidate execution

## Closure

### Accepted outcome

A development-only `weak-diagnostic-v1` freeze supplies one decomposable and one sequential/null
package with distinct development/transfer assignments, disjoint seed namespaces, exact public and
protected digests, and separate branch-A, branch-B, integration-bypass and stratum-relabel
mutations. The validator accepts both references, rejects every declared mutation for its own
reason, and performs no model call.

### Residuals

None.

### Evidence

- [58c20c3](https://github.com/maggnus/ymp/commit/58c20c3838fa3bb77e5b71c0f3af66528055d79f)
  — integrated tree, byte-identical for the reviewed corpus paths to candidate
  [77ec6d0](https://github.com/maggnus/ymp/commit/77ec6d0447b391206a725dbdcd7153c15e091b3c).
