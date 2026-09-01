---
id: W1-EVL-04k
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04i]
blocks: [W1-EVL-04m, W1-EVL-04e]
created_at: 2026-09-01T15:49:59+08:00
updated_at: 2026-09-01T16:40:38+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 0
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

- [ ] All five product-path fixtures exercise the exact tuple proven by W1-EVL-04i and preserve
      readiness, completion/export, resume, cancellation and descendant-termination behavior.
- [ ] The admission manifest and digest bind that same tuple while retaining byte-identical
      schedule, read cap, accounting limits, S1-S3 expectations and `model_calls=0`.
- [ ] The embedded frozen-manifest digest equals the new sidecar and computed manifest SHA-256; the
      previous embedded digest or a fixture still claiming 0.147 fails before admission evaluation.
- [ ] A stale 0.147 fixture or manifest, a future unmeasured version, a digest mismatch and a tuple
      that disagrees with the runtime projection each fail closed before `model_ready=true` or an
      accepted task can start.
- [ ] The command-line and TUI surfaces contain no version selector, project-directory ceremony or
      upgrade prompt; compatible installed runtime use is automatic and incompatibility produces
      one actionable product-owned error.
- [ ] One fresh disposable-root product-path walk uses separate project, `HOME`, `YMP_HOME`,
      `TMPDIR`, build and export directories; no file reaches the worktree, current directory or
      real `~/.ymp`.
- [ ] No real model/provider/network call runs; focused CLI and admission checks, strict affected
      package Clippy, formatting and `git diff --check` pass without warnings.

## Current state

The accepted runtime proves 0.151 behavior. The first builder preflight showed that the admission
manifest is also bound by `FROZEN_MANIFEST_SHA256` and an exact Codex fixture in `admission.rs`, so
the original seven-file zone could not produce a valid candidate. That mechanical coupling is now
explicitly allowed; admission behavior remains out of scope.

## Next action

Continue the original Sol xhigh builder from the current `main` with only the newly explicit
embedded-digest and fixture allowance.

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

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger. Empty when there are none.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
