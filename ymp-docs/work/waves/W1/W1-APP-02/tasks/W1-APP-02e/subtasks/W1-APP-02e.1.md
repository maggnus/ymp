---
id: W1-APP-02e.1
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W0-UX-01c]
blocks: []
created_at: 2026-08-12T16:19:52+08:00
updated_at: 2026-08-12T17:38:49+08:00
started_at: 2026-08-12T16:19:52+08:00
accepted_at: 2026-08-12T17:38:49+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/94a8e127b94ead4be3dcdee1339a6b29bd95ac0d
closure_commit: https://github.com/maggnus/ymp/commit/bdccf00825b6a93ff25ab10f34719e645bc8f8f9
evidence: ["[94a8e12](https://github.com/maggnus/ymp/commit/94a8e127b94ead4be3dcdee1339a6b29bd95ac0d)", "[bdccf00](https://github.com/maggnus/ymp/commit/bdccf00825b6a93ff25ab10f34719e645bc8f8f9)"]
duration_minutes: 59
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02e.1 — Deterministic TUI buffers enforce the accepted screen contract

## Outcome

Deterministic Ratatui buffer tests prove that the accepted reusable screen templates render the
required POC-1 projections, states, and terminal sizes without relying on colour, mouse input, or a
real model provider.

## Scope

### In

- `TestBackend` buffer assertions for 80 × 24 and 120 × 40 terminal sizes.
- First launch, mixed runtime readiness, running, cancellation, acceptance, candidate failure,
  budget exhaustion, and infrastructure failure projections.
- Structural assertions for named regions, typed state, focus, keyboard actions, clipping, and
  high-volume content.

### Out

- Real Codex or Claude Code invocation, PTY process-tree cancellation, candidate production,
  verifier execution, export, and multi-participant views.

## Acceptance

- [x] Deterministic buffer tests cover every declared POC-1 state at 80 × 24 and 120 × 40 and assert
  required regions, labels, focus, and actions without snapshotting incidental spacing.
- [x] High-volume and constrained fixtures clip or scroll within their region without hiding the
  terminal reason, primary action, or current selection.
- [x] A monochrome rendering produces the same state distinctions through text and symbols, and
  all required actions remain keyboard-accessible without mouse input.
- [x] A deliberately wrong projection-state mapping or omitted required region makes the same
  test harness fail rather than accepting a visually plausible buffer.

## Current state

Accepted. The public package test interface produces every required deterministic buffer and
keyboard transition through the production rendering and input paths without owning domain
transitions.

## Next action

Continue W1-APP-02e with PTY cancellation, typed failure, export, and controlled provider coverage.

## Guardrails

- Changes are limited to `ymp-tui` and its package-local tests and fixtures.
- Widgets display immutable application projections and do not own domain transition rules.
- This subtask makes no claim about real runtime availability, provider authentication, PTY
  shutdown, export, or host containment.

## Findings

None. The three findings from the first review were corrected and independently rechecked through
an external package and a fixed negative mutation.

## Closure

### Accepted outcome

The reusable screen contract is enforced by 16 deterministic buffers spanning eight POC-1 states
at 80 × 24 and 120 × 40. The external package confirms named regions, terminal reasons, primary
actions, current selection among 4096 candidates, monochrome distinctions, and the complete
keyboard map through production rendering and input handling.

### Residuals

None recorded.

### Evidence

- [Reviewed correction](https://github.com/maggnus/ymp/commit/94a8e127b94ead4be3dcdee1339a6b29bd95ac0d).
- [Integration commit](https://github.com/maggnus/ymp/commit/bdccf00825b6a93ff25ab10f34719e645bc8f8f9).
