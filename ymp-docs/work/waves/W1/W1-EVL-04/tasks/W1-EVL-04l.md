---
id: W1-EVL-04l
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04j]
blocks: [W1-EVL-04m]
created_at: 2026-09-01T15:57:45+08:00
updated_at: 2026-09-01T15:57:50+08:00
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

# W1-EVL-04l — Controller owns nonce read-back and probe attestation

## Outcome

The trusted foreground controller generates an unpredictable nonce and scoped disposable path,
reserves the separate probe budget, executes W1-EVL-04j, independently reads back the file and
persists an immutable `AttestedToolHostProbe` below the configured YMP data root only when every
identity, route, usage, effect and terminal condition matches.

## Scope

### In

- Controller-owned nonce generation, normalized relative destination, deadline, probe/invocation
  identity and separate resource reservation.
- Independent filesystem read-back and exact nonce comparison after the supervisor returns its
  untrusted trace; binding to runtime/profile/CLI/driver/tool schema, usage, event/output digests,
  wall time and cost availability.
- Immutable persistence and replay identity below the configured YMP data root.
- Exclusive write zone: optional new
  `ymp-rust/crates/ymp-application/src/tool_host_probe.rs`, narrow declarations and dispatch in
  `ymp-rust/crates/ymp-application/src/lib.rs`, narrow internal command wiring in
  `ymp-rust/crates/ymp-cli/src/internal.rs`, and optional new
  `ymp-rust/crates/ymp-cli/tests/tool_host_probe.rs`.

### Out

- No-touch: runtime API/supervisor/driver code, `ymp-corpus/**`, Cargo manifests/lockfile, public
  CLI/TUI commands, arbitrary caller paths, real user HOME, task/board/recruitment/candidate state,
  deployment and live model/network/money execution.

## Acceptance

- [ ] In a disposable evaluation root the controller generates nonce/path/identity, reserves exactly
      one probe resource vector, invokes the accepted runtime seam, reads the file itself and emits
      one immutable attestation bound to the complete trace and read-back digest.
- [ ] Missing file, wrong nonce, runtime- or model-chosen path/nonce/digest, stale or replayed
      invocation, incomplete usage, mismatched route/profile/version/schema, ambiguous terminal,
      timeout/cancellation, budget overrun or extra effect produces no attestation and no persisted
      success record.
- [ ] A syntactically valid raw trace or model-authored evidence cannot construct or serialize an
      accepted attestation without the controller-held nonce and read-back result.
- [ ] The probe budget is distinct from every arm/task/candidate/communication allocation and is
      charged once across start and terminal records; failure cannot refund or duplicate it.
- [ ] A product-path test with separate project, `HOME`, `YMP_HOME`, `TMPDIR`, build and export
      proves all durable probe state is below isolated `YMP_HOME`, with nothing in the repository,
      current directory or real `~/.ymp`.
- [ ] Focused Application/internal-CLI tests, strict affected-package Clippy, formatting and
      `git diff --check` pass; no real model/network/money call runs.

## Current state

W1-EVL-04h showed that schema-valid probe evidence is insufficient, while W1-EVL-04j now owns only
the untrusted runtime trace. The missing authority is controller nonce ownership plus independent
read-back, exact budget binding and isolated persistence. No live probe is authorized.

## Next action

After W1-EVL-04j is accepted, run a Critical contract check of this Application/internal-CLI seam.

## Guardrails

- Only the foreground controller may turn a runtime trace into an attestation.
- Internal CLI wiring is not user ceremony and exposes no version or project-directory controls.
- A real probe remains a separate owner model/money decision.

## Findings

- Created by R1 decomposition of W1-EVL-04j to isolate the authority, accounting and persistence
  boundary from runtime execution and corpus policy.

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
