---
id: W1-EVL-04k
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04i]
blocks: [W1-EVL-04m, W1-EVL-04e]
created_at: 2026-09-01T15:49:59+08:00
updated_at: 2026-09-01T17:07:17+08:00
started_at: 2026-09-01T16:39:00+08:00
accepted_at: 2026-09-01T17:07:17+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/9c1f27accc03c7972bfe6b5b0e63720f42d3f50d
closure_commit: https://github.com/maggnus/ymp/commit/77dab74116e9836795e07e9eb0e5b149d032d73d
evidence: ["[77dab74](https://github.com/maggnus/ymp/commit/77dab74116e9836795e07e9eb0e5b149d032d73d)"]
duration_minutes: 17
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 2
escalation_decision:
---

# W1-EVL-04k — Product fixtures bind the proven Codex runtime tuple

## Outcome

The command-line product fixtures and the zero-model admission manifest consume the exact Codex
runtime tuple proven by W1-EVL-04i, use an installed compatible runtime automatically, and reject
stale or unmeasured tuples without exposing version management to the user.

## Scope

### In

- Update exactly the five Codex product-path fixtures in
  `ymp-rust/crates/ymp-cli/tests/{cancelling_a_live_attempt.rs,cancelling_after_the_attempt_failed.rs,codex_product_path.rs,descendant_termination.rs,the_product_completes_and_exports_a_run.rs}`.
- Update `ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-admission-v1/manifest.json`
  and its `manifest.sha256` to the proven tuple without changing the frozen schedule, limits or
  outcomes.
- Update only `FROZEN_MANIFEST_SHA256` and the exact Codex-version/tool-schema fixture expectations
  mechanically forced by those manifest bytes in `ymp-rust/tools/ymp-corpus/src/admission.rs`;
  admission logic, schemas, schedules and decisions remain unchanged.
- Exclusive write zone is those seven files plus mechanically forced changes only in
  `ymp-rust/crates/ymp-cli/Cargo.toml`, `ymp-rust/tools/ymp-corpus/Cargo.toml` and
  `ymp-rust/Cargo.lock`, and the narrowly bounded `admission.rs` constants/fixture above.

### Out

- No-touch: every other `admission.rs` behavior, `ymp-runtime-codex/**`,
  `ymp-runtime-supervisor/**`, all research and calibration records, accepted admission report
  schemas and rehearsal semantics, primary/development corpus tasks, provider/model selection, TUI,
  storage, deployment and real model/network/money calls.
- Historical `ymp-docs/research/cal-001-calibration.md` and
  `ymp-rust/tools/ymp-calibration/results/**` remain immutable; they describe the profiles actually
  measured at their revisions.

## Acceptance

- [x] All five product-path fixtures exercise the exact tuple proven by W1-EVL-04i and preserve
      readiness, completion/export, resume, cancellation and descendant-termination behavior.
- [x] The admission manifest and digest bind that same tuple while retaining byte-identical
      schedule, read cap, accounting limits, S1-S3 expectations and `model_calls=0`.
- [x] The embedded frozen-manifest digest equals the new sidecar and computed manifest SHA-256; the
      previous embedded digest or a fixture still claiming 0.147 fails before admission evaluation.
- [x] A stale 0.147 fixture or manifest, a future unmeasured version, a digest mismatch and a tuple
      that disagrees with the runtime projection each fail closed before `model_ready=true` or an
      accepted task can start.
- [x] The command-line and TUI surfaces contain no version selector, project-directory ceremony or
      upgrade prompt; compatible installed runtime use is automatic and incompatibility produces
      one actionable product-owned error.
- [x] One fresh disposable-root product-path walk uses separate project, `HOME`, `YMP_HOME`,
      `TMPDIR`, build and export directories; no file reaches the worktree, current directory or
      real `~/.ymp`.
- [x] No real model/provider/network call runs; focused CLI and admission checks, strict affected
      package Clippy, formatting and `git diff --check` pass without warnings.

## Current state

Accepted and integrated as
[77dab74](https://github.com/maggnus/ymp/commit/77dab74116e9836795e07e9eb0e5b149d032d73d).
All five product fixtures and the zero-model admission manifest now consume the exact accepted
Codex 0.151 tuple; frozen schedules, limits and admission decisions remain unchanged.

## Next action

After W1-EVL-04l is accepted, run W1-EVL-04m to consume controller-attested probe evidence through
the now exact 0.151 admission manifest.

## Guardrails

- Product fixtures consume runtime conformance; they never define it.
- Version pinning remains internal reproducibility metadata, not a user workflow.
- The admission manifest changes only after the runtime tuple is accepted and remains fail-closed.

## Findings

- Created by R1 decomposition of W1-EVL-04i so runtime behavior and product-fixture migration have
  separate write zones, dependencies and acceptance stories.
- Builder preflight found the manifest's unavoidable embedded digest and exact test fixture; the
  contract was corrected without authorizing any admission-policy change.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(6/10) RETURN 01/09 17:06 — static bindings were correct but the independent external consumer
  run stopped in Rust toolchain setup → the candidate remained unchanged → the same reviewer was
  given an explicit installed toolchain/cache while retaining isolated product state
- R2(9/10) ACCEPT 01/09 17:07 — real `ymp-corpus admission check` with a fake stale 0.147 runtime
  returns typed incompatibility, `model_calls=0` and `model_ready=false` → the isolated root is
  removed and repository status remains byte-identical

## Closure

### Accepted outcome

The five Codex product fixtures automatically consume the exact accepted 0.151 runtime tuple for
readiness, completion/export, resume, cancellation and descendant termination. The admission
manifest, sidecar and embedded digest agree at `48f6107e…9947e24b`; a stale 0.147 or future 0.152
projection and schema mismatch fail before model readiness. No user-facing version control was
introduced.

### Residuals

An out-of-scope expanded author run observed `Protocol` failures in two Claude descendant fixtures.
Before a POC experiment uses Claude, or before the release integration suite, reproduce and classify
those failures in a separate focused task; they do not affect the accepted Codex-only gate.

### Evidence

- [77dab74](https://github.com/maggnus/ymp/commit/77dab74116e9836795e07e9eb0e5b149d032d73d)
  — integrated tree, byte-identical for the reviewed paths to candidate
  [9c1f27a](https://github.com/maggnus/ymp/commit/9c1f27accc03c7972bfe6b5b0e63720f42d3f50d).
