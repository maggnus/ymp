---
id: W1-COR-03e.1
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-09-01T12:14:00+08:00
updated_at: 2026-09-01T12:26:00+08:00
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
review_rounds: 1
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
- The exact `ymp-testkit/src/lib.rs` call site that currently obtains `Application::object_store()`
  and a candidate path; it is replaced by a typed application verification operation.

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

R1 returned candidate `ec45bce`: payload ordering and resolution are correct, but pre-existing
public `Application::object_store()` and `Application::board()` still bypass the typed boundary.
The only non-test call site is the accepted-demo verifier in `ymp-testkit`; its exact replacement is
now in scope and no correction code has yet been written.

## Next action

Replace the testkit path extraction with a typed application verification operation, close both
public accessors, and return the corrected candidate.

## Guardrails

- Payload bytes remain inert object data and are never parsed by the kernel or board transition.
- An object is durable before its board fact; a later refusal may leave an unreferenced object but
  never a board record pointing to missing or partial bytes.
- No path, object store, `BoardLedger`, or `BoardStore` crosses the application API.

## Findings

- R1 scope break: `ymp-testkit::run_accepted_demo` depends on the public object-store/path accessor;
  only this call site is authorized outside `ymp-application/**` for the correction.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(7/10) RETURN 01/09 12:26 — public object-store and board access bypasses the typed seam → Sol
  correction assigned → candidate `ec45bce` returned

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
