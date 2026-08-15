---
id: W1-COR-03t
kind: task
wave: W1
card: W1-COR-03
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03s]
blocks: []
created_at: 2026-08-15T01:55:01+08:00
updated_at: 2026-08-15T12:38:53+08:00
started_at: 2026-08-15T12:38:53+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-COR-03t — A cancel interrupts a runtime that is still working

## Outcome

A cancel reaches a working runtime: the interrupt is delivered outside the wake wait, and controller shutdown is bounded; the negative half is the measured unbounded wait.

## Scope

### In

- Recorded from the W1-COR-03s review. session.interrupt() is called only while waiting for a wake (lib.rs:1059,1075 at 1ef0796) and the token is read there and at Ok(None); a runtime that stopped answering after a resume left controller shutdown unbounded (measured: 8 s test timeout, no product bound).

### Out

- What the sibling terminal-edge nodes accepted.

## Acceptance

- [ ] A cancel reaches a working runtime: the interrupt is delivered outside the wake wait, and controller shutdown is bounded; the negative half is the measured unbounded wait.

## Current state

Ready.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

None recorded.
