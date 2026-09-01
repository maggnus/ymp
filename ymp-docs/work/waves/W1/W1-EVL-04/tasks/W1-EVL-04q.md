---
id: W1-EVL-04q
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04k, W1-EVL-04p]
blocks: [W1-EVL-04n, W1-EVL-04m]
created_at: 2026-09-01T18:19:05+08:00
updated_at: 2026-09-01T20:09:43+08:00
started_at: 2026-09-01T19:35:30+08:00
accepted_at: 2026-09-01T20:09:43+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/4d3751f728ff8130bded3a5af371c73e7b33315c
closure_commit: https://github.com/maggnus/ymp/commit/3dab48d44e514f4c01c879c73afbbd8706c9f53f
evidence: ["[3dab48d](https://github.com/maggnus/ymp/commit/3dab48d44e514f4c01c879c73afbbd8706c9f53f)"]
duration_minutes: 25
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 3
escalation_decision: bounded_retry
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

- [x] V2 validates a compatible executable whose observed version differs from the historical
      reference, records exact version/executable digest and the frozen compatibility-contract
      digest, and leaves every experimental schedule/budget/outcome unchanged.
- [x] Same version with incompatible behavior, missing/wrong compatibility digest, malformed
      observed version/digest, stale v1 evidence and future behavior drift each fail before
      `model_ready` or an accepted task starts; version difference alone never fails.
- [x] All five product fixtures preserve their accepted behavior with automatic discovery and have
      no selector, upgrade prompt, project-directory ceremony or version acknowledgement.
- [x] V1 bytes and digest remain byte-identical and retrievable as historical evidence; v2 has a new
      identity/digest and exact parent binding, with no silent reinterpretation of the v1 freeze.
- [x] One fresh-root product/admission walk uses separate project, HOME, YMP_HOME, TMPDIR, build and
      export; focused tests, strict Clippy, formatting and `git diff --check` pass without model,
      network, money, warnings or writes to real state.

## Current state

Accepted and integrated as
[3dab48d](https://github.com/maggnus/ymp/commit/3dab48d44e514f4c01c879c73afbbd8706c9f53f).
Admission v2 and product fixtures consume behavioral compatibility; v1 stays immutable history and
the model gate remains closed until live attestation is accepted.

## Next action

Repeat the W1-EVL-04n contract check against the accepted transport, behavioral identity and v2
manifest, then implement and run the single live probe.

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

- R1(8/10) ACCEPT 01/09 20:05 — static and focused evidence showed no outcome defect, but the
  external parent mutation did not finish → the candidate remained unchanged → the reviewer was
  assigned the same scenario with a prebuilt binary
- R2(7/10) RETURN 01/09 20:07 — the harness accidentally failed v2 self-digest before reaching its
  copied parent → the exact sibling layout and pre-run v2 digest assertion were specified → no code
  change was made
- CTO bounded_retry 01/09 20:08 — no code defect was established and the remaining uncertainty was
  one deterministic harness path, so one final evidence-only retry was authorized without changing
  the candidate
- R3(9/10) ACCEPT 01/09 20:09 — v2 self-digest passes, one byte changed only in copied v1 causes a
  parent-loader refusal before report/model readiness, the root is removed and Git remains clean

## Closure

### Accepted outcome

Admission v2 adds a strict behavioral-compatibility schema and exact parent binding to immutable v1
manifest, sidecar and tree. Different observed Codex versions with the accepted contract pass and are
recorded with executable digests; behavior drift at the historical version fails. Five product
fixtures preserve readiness, export, resume, cancellation and descendant termination without any
version control exposed to the user.

### Residuals

Stage two deliberately remains `model_ready=false`: W1-EVL-04n must produce one live controller
attestation and W1-EVL-04m must consume it exactly once.

### Evidence

- [3dab48d](https://github.com/maggnus/ymp/commit/3dab48d44e514f4c01c879c73afbbd8706c9f53f)
  — integrated tree, byte-identical for the reviewed paths to candidate
  [4d3751f](https://github.com/maggnus/ymp/commit/4d3751f728ff8130bded3a5af371c73e7b33315c).
