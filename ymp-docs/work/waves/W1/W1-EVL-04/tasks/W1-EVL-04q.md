---
id: W1-EVL-04q
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04k, W1-EVL-04p]
blocks: [W1-EVL-04n, W1-EVL-04m]
created_at: 2026-09-01T18:19:05+08:00
updated_at: 2026-09-01T18:19:05+08:00
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

# W1-EVL-04q — Product and admission consume behavioral compatibility

## Outcome

The product fixtures and a new immutable admission-manifest revision consume the behavioral Codex
compatibility contract from W1-EVL-04p. They automatically accept any installed runtime that proves
that behavior, record its exact observed version/digest for reproducibility and never expose version
management to the user.

## Scope

### In

- Add `weak-diagnostic-admission-v2/manifest.json` and `manifest.sha256` beside immutable accepted v1.
  V2 keeps byte-identical schedules, S1-S3 expectations, read cap, budgets, arms and outcomes, links
  the accepted v1 digest, and replaces version authority with
  `compatibility_contract_sha256`; `reference_cli_version` is descriptive evidence only.
- Update the exact five product fixtures owned previously by W1-EVL-04k to exercise two compatible
  observed versions automatically and a behaviorally incompatible same-version negative. Preserve
  readiness, completion/export, resume, cancellation and descendant termination.
- Update only the strict v2 schema/loader, embedded v2 digest and compatibility evaluation in
  `ymp-rust/tools/ymp-corpus/src/admission.rs`; v1 code/data remain readable historical evidence but
  cannot open the new model gate.
- Exclusive write zone: the five `ymp-cli/tests` files named in W1-EVL-04k, new v2 manifest/digest,
  narrow `admission.rs` v2 definitions/fixtures, and mechanically forced changes only in
  `ymp-cli/Cargo.toml`, `ymp-corpus/Cargo.toml` and `Cargo.lock`.

### Out

- No-touch: accepted v1 manifest/digest, runtime drivers/supervisor, Application, research and
  calibration records, task corpus/oracles, schedules, primary/development seeds/budgets/outcomes,
  TUI, deployment and real model/network/money calls.

## Acceptance

- [ ] V2 validates a compatible executable whose observed version differs from the historical
      reference, records exact version/executable digest and the frozen compatibility-contract
      digest, and leaves every experimental schedule/budget/outcome unchanged.
- [ ] Same version with incompatible behavior, missing/wrong compatibility digest, malformed
      observed version/digest, stale v1 evidence and future behavior drift each fail before
      `model_ready` or an accepted task starts; version difference alone never fails.
- [ ] All five product fixtures preserve their accepted behavior with automatic discovery and have
      no selector, upgrade prompt, project-directory ceremony or version acknowledgement.
- [ ] V1 bytes and digest remain byte-identical and retrievable as historical evidence; v2 has a new
      identity/digest and exact parent binding, with no silent reinterpretation of the v1 freeze.
- [ ] One fresh-root product/admission walk uses separate project, HOME, YMP_HOME, TMPDIR, build and
      export; focused tests, strict Clippy, formatting and `git diff --check` pass without model,
      network, money, warnings or writes to real state.

## Current state

W1-EVL-04k correctly bound the reference 0.151 fixtures and v1 manifest, but that exact version must
remain historical evidence rather than product authority. W1-EVL-04p will expose a behavioral
contract digest; no v2 admission manifest exists yet, and the live model gate stays closed.

## Next action

After W1-EVL-04p is accepted, run a Critical contract check of the additive v2 consumer and dispatch
one Sol xhigh builder; W1-EVL-04n remains blocked until v2 is accepted.

## Guardrails

- Never rewrite accepted v1 evidence to pretend it measured a different policy.
- Reference version is metadata, not an acceptance predicate or user workflow.
- This task changes compatibility metadata only, not experiments or runtime behavior.

## Findings

- Created from the owner correction that users and maintainers cannot be required to coordinate on
  one exact Codex CLI version.

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
