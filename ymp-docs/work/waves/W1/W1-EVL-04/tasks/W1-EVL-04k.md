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
blocks: [W1-EVL-04j, W1-EVL-04e]
created_at: 2026-09-01T15:49:59+08:00
updated_at: 2026-09-01T15:50:30+08:00
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
- Exclusive write zone is those seven files plus mechanically forced changes only in
  `ymp-rust/crates/ymp-cli/Cargo.toml`, `ymp-rust/tools/ymp-corpus/Cargo.toml` and
  `ymp-rust/Cargo.lock`.

### Out

- No-touch: `ymp-runtime-codex/**`, `ymp-runtime-supervisor/**`, all research and calibration
  records, accepted admission report schemas and rehearsal semantics, primary/development corpus
  tasks, provider/model selection, TUI, storage, deployment and real model/network/money calls.
- Historical `ymp-docs/research/cal-001-calibration.md` and
  `ymp-rust/tools/ymp-calibration/results/**` remain immutable; they describe the profiles actually
  measured at their revisions.

## Acceptance

- [ ] All five product-path fixtures exercise the exact tuple proven by W1-EVL-04i and preserve
      readiness, completion/export, resume, cancellation and descendant-termination behavior.
- [ ] The admission manifest and digest bind that same tuple while retaining byte-identical
      schedule, read cap, accounting limits, S1-S3 expectations and `model_calls=0`.
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

The repository has five product fixtures and one admission manifest pinned to 0.147. W1-EVL-04i
now owns proving 0.151 behavior in the runtime and supervisor first. This task may migrate only the
consumers of that accepted proof; a version-number substitution without the proven tuple is refused.

## Next action

After W1-EVL-04i is accepted, run a Critical contract check of these exact seven product files and
dispatch one Sol xhigh builder from the then-current `main`.

## Guardrails

- Product fixtures consume runtime conformance; they never define it.
- Version pinning remains internal reproducibility metadata, not a user workflow.
- The admission manifest changes only after the runtime tuple is accepted and remains fail-closed.

## Findings

- Created by R1 decomposition of W1-EVL-04i so runtime behavior and product-fixture migration have
  separate write zones, dependencies and acceptance stories.

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
