---
id: W1-EVL-04h
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03z, W1-PRD-05j.1]
blocks: [W1-EVL-04e]
created_at: 2026-09-01T15:02:06+08:00
updated_at: 2026-09-01T15:41:27+08:00
started_at: 2026-09-01T15:06:13+08:00
accepted_at: 2026-09-01T15:41:27+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/4dafa512f737876c444ee285e99556d0899e7a31
closure_commit: https://github.com/maggnus/ymp/commit/694c79010f7417e1938d1e3d305817618c148657
evidence: ["[694c790](https://github.com/maggnus/ymp/commit/694c79010f7417e1938d1e3d305817618c148657)"]
duration_minutes: 35
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 1
escalation_decision:
---

# W1-EVL-04h — Zero-model admission and pull transport are executable

## Outcome

One zero-model CLI proves exact installed-route compatibility and the bounded S1-S3 pull-transport
mechanics through the production MCP/RPC/Application seam. It emits an immutable admission report
that keeps every weak-model arm closed until a separately budgeted no-task-output tool-host probe is
present and valid.

## Scope

### In

- A frozen `weak-diagnostic-admission-v1` manifest and digest naming exact runtime driver, installed
  CLI version, profile/route, tool schema, Git-trust/config policy, process windows, read cap and the
  separate no-task-output probe budget.
- New executable types `AdmissionCommand`, `AdmissionManifest`, `AdmissionReport` and
  `TransportSchedule` in a new `ymp-corpus::admission` module.
- Exact CLI consumers:
  `ymp-corpus admission check --manifest <path> --digest <path>` and
  `ymp-corpus admission rehearse --manifest <path> --digest <path> --root <path>`.
- S1 publish-before-read, S2 empty-read-before-publish then one remaining read, S3 reads exhausted
  or yield-before-publish with no board wake, plus an unauthorized-reader negative, all through the
  existing agent MCP, private RPC and Application path with the fake runtime.
- Write zone: new
  `ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-admission-v1/**`, new
  `ymp-rust/tools/ymp-corpus/src/admission.rs`, optional new
  `ymp-rust/tools/ymp-corpus/tests/admission.rs`, minimal declarations/dependencies in
  `ymp-rust/tools/ymp-corpus/src/lib.rs`, `src/main.rs`, `Cargo.toml`, and mechanically forced
  `ymp-rust/Cargo.lock` only.

### Out

- Any real model call, task output, experimental arm, profile promotion, model/money authorization,
  semantic listening or task-value claim.
- No-touch: every production crate including `ymp-agent-*`, `ymp-application`, `ymp-domain`,
  `ymp-runtime-*`, `ymp-verifier` and `ymp-tui`; all `ymp-calibration/**`;
  `ymp-docs/research/**`; all accepted L4 files under
  `corpus/development/weak-diagnostic-v1/**`; primary `corpus/tasks/**`, `corpus/study/**`,
  `corpus/policies/**`, `corpus/registry.json`, primary seeds/budgets/outcomes, and work records.

## Acceptance

- [x] `admission check` accepts the exact frozen manifest with `model_calls=0`; a CLI/driver version
      change, removed flag, missing Git trust, wrong tool schema, changed profile/route or missing
      tool-host binding fails before any task or provider invocation.
- [x] `admission rehearse` reaches MCP→private RPC→Application with the fake runtime. S1 and S2
      record the exact `MessagePublished`, `DeliveryRecorded`, predeclared later B action and honest
      terminal; the unauthorized reader receives nothing. S3 records no wake and remains a valid
      negative behavior, not an infrastructure success.
- [x] The manifest fixes process windows, their order and a hard read cap without depending on
      publication, message content or outcome. Rehearsal uses no retry-until-success polling; an
      extra read or schedule selected after observing a message fails compliance.
- [x] Admission has two distinct stages: zero-model compatibility and one separately budgeted
      no-task-output nonce write/read probe. Missing, refused, mismatched or unaccounted probe
      evidence deterministically keeps `model_ready=false` and emits no arm schedule or model call.
- [x] The probe schema records exact input, cached-input, output and reasoning tokens, wall time,
      route, CLI/driver versions, currency/cost availability and output/event digests. It cannot be
      counted as an arm observation or paid from an arm budget.
- [x] Removing a delivery receipt, advertising recruitment without its endpoint capability,
      accepting a stale version or replacing a failed nonce probe with success makes a focused
      negative check fail non-zero; each check states its blind spot.
- [x] The CLI walk runs in one fresh short root with separate `project`, `HOME`, `YMP_HOME`,
      `TMPDIR`, build and export paths, never the repository or real `~/.ymp`; strict affected-crate
      Clippy, formatting and `git diff --check` have no warnings or errors.

## Current state

Accepted and integrated as
[694c790](https://github.com/maggnus/ymp/commit/694c79010f7417e1938d1e3d305817618c148657).
The zero-model gate and S1-S3 rehearsal are executable; the installed `codex-cli 0.151.0` fails the
frozen `0.147.0` profile and no controller-attested nonce evidence exists, so
`model_ready=false` and no arm schedule is emitted.

## Next action

Complete W1-EVL-04i and W1-EVL-04j, then rerun this immutable gate before any weak-model arm.

## Guardrails

- Zero-model compatibility cannot by itself prove that a model can use workspace tools; only the
  separately accounted nonce probe can close stage two.
- A conservative refusal is useful failure behavior but never communication evidence.
- `DeliveryRecorded` proves availability only; listening and task value remain W1-EVL-04b work.
- Any missing production seam is returned as a finding; this task does not modify a production
  crate to make its fixture pass.

## Findings

- Split from W1-EVL-04e so engineering admission and transport conformance can advance while the
  scientific development/transfer task allocation is resolved independently.
- The exploratory pilot exposed layered readiness: authentication, model availability and isolated
  storage did not establish workspace tool-host availability.
- The accepted gate converted both missing production seams into explicit fail-closed results rather
  than weakening the manifest or accepting model-authored evidence.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(9/10) ACCEPT 01/09 15:41 — the full tool-only diff preserves strict schemas, caps,
  `model_calls=0` and production MCP/RPC/Application transport → author evidence proves S1-S3 and
  all focused negative mutations → exact runtime/probe gaps remain closed additional work, not a
  false admission

## Closure

### Accepted outcome

`ymp-corpus admission check/rehearse` validates a frozen route manifest, produces an immutable
admission report, and exercises S1-S3 plus unauthorized reading through the accepted collaboration
path without any model call. Stage two accepts only controller-attested, separately budgeted
no-task-output evidence. The current machine is correctly inadmissible and receives no schedule.

### Residuals

- [W1-EVL-04i](W1-EVL-04i.md) owns the installed `0.151.0` versus supported `0.147.0` runtime
  mismatch; return trigger: its accepted product-path conformance evidence.
- [W1-EVL-04j](W1-EVL-04j.md) owns controller-attested nonce-probe evidence; return trigger: its
  accepted fake-runtime and production-boundary proof.

### Evidence

- [694c790](https://github.com/maggnus/ymp/commit/694c79010f7417e1938d1e3d305817618c148657)
  — integrated tree, byte-identical for the reviewed admission paths to candidate
  [4dafa51](https://github.com/maggnus/ymp/commit/4dafa512f737876c444ee285e99556d0899e7a31).
