---
id: W1-APP-02e.1
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: [W0-UX-01c]
blocks: []
created_at: 2026-08-12T16:19:52+08:00
updated_at: 2026-08-12T16:46:02+08:00
started_at: 2026-08-12T16:19:52+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/7a54cc7fc35a86cde267edf629fab5e03c323602
closure_commit:
evidence:
duration_minutes: 0
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

- [ ] Deterministic buffer tests cover every declared POC-1 state at 80 × 24 and 120 × 40 and assert
  required regions, labels, focus, and actions without snapshotting incidental spacing.
- [ ] High-volume and constrained fixtures clip or scroll within their region without hiding the
  terminal reason, primary action, or current selection.
- [ ] A monochrome rendering produces the same state distinctions through text and symbols, and
  all required actions remain keyboard-accessible without mouse input.
- [ ] A deliberately wrong projection-state mapping or omitted required region makes the same
  test harness fail rather than accepting a visually plausible buffer.

## Current state

The first independent review returned the candidate for bounded rework. The package-local tests
pass, but an external consumer cannot construct the required buffers through the current private
interfaces. High-volume data, terminal actions, and executable keyboard coverage are incomplete.

## Next action

Expose a minimal package test interface, complete the 16-buffer and keyboard matrices, and rerun
the preserved external falsifier before re-review by the same reviewer.

## Guardrails

- Changes are limited to `ymp-tui` and its package-local tests and fixtures.
- Widgets display immutable application projections and do not own domain transition rules.
- This subtask makes no claim about real runtime availability, provider authentication, PTY
  shutdown, export, or host containment.

## Findings

Review of
[`ymp-tui/src/lib.rs`](https://github.com/maggnus/ymp/blob/7a54cc7fc35a86cde267edf629fab5e03c323602/ymp-rust/crates/ymp-tui/src/lib.rs)
found:

- The externally compiled test cannot access rendering, projections, or interface state.
- The high-volume fixture uses 257 runtimes instead of 4096 candidates and does not assert the
  terminal reason.
- Terminal action hints and key handling diverge for cancellation, search, detail, escape, help,
  and quit paths.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
