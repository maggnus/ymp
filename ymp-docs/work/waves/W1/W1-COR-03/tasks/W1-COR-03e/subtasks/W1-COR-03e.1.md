---
id: W1-COR-03e.1
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-09-01T12:14:00+08:00
updated_at: 2026-09-01T12:15:03+08:00
started_at: 2026-09-01T12:15:03+08:00
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

# W1-COR-03e.1 — Application persists and resolves inert board payloads

## Outcome

The application atomically persists each inert collaboration payload before its board fact and
returns an owned operator projection whose visible messages resolve to the exact verified bytes,
without exposing storage, a mutable board, or authority to the TUI.

## Scope

### In

- The application publication seam for `Publish` plus exact payload bytes.
- Object-before-record ordering, digest and length validation, and owned resolved projections.
- Focused application tests for reopen, mutation isolation, and fail-closed mismatch or loss.

### Out

- TUI rendering, agent-facing tools, new board semantics, automatic migration, and any causal
  interpretation of messages.

## Acceptance

- [ ] Exact payload bytes survive publication, reopen, and read through the typed application API.
- [ ] The returned message and bytes are owned copies; mutating them changes neither a later read
      nor the persisted board or object.
- [ ] A digest or length mismatch, and a missing or corrupt object, fail without changing the board
      audit or durable record position.
- [ ] Existing board-section and board-projection checks, strict application Clippy, formatting,
      and `git diff --check` pass.

## Current state

Active on `codex/gpt-5.6-sol` at `xhigh`. The application owns and persists the board and exposes an
owned operator `ViewState`; this subtask now binds exact payload bytes to publication and resolves
them through an owned consumer projection.

## Next action

Return the reviewed publish/resolve candidate and its focused evidence.

## Guardrails

- Payload bytes remain inert object data and are never parsed by the kernel or board transition.
- An object is durable before its board fact; a later refusal may leave an unreferenced object but
  never a board record pointing to missing or partial bytes.
- No path, object store, `BoardLedger`, or `BoardStore` crosses the application API.

## Findings

- None.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
