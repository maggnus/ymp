---
id: W1-COR-03t
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03s]
blocks: []
created_at: 2026-08-15T01:55:01+08:00
updated_at: 2026-08-15T13:43:07+08:00
started_at: 2026-08-15T12:38:53+08:00
accepted_at: 2026-08-15T13:43:07+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/b34ceb99d1af201c76de10f934c48fddcc66369b
closure_commit: https://github.com/maggnus/ymp/commit/6996998
evidence: one return round; the interrupt is delivered each loop iteration and the 5 s shutdown decides by the recorded slice state, never over a committed terminal (reviewer window-hold scenario reproduced clean); residuals: a poisoned-lock join path leaves the journal on running, and a shutdown in the completion window cancels a finished run — both recorded as W1-COR-03w
duration_minutes: 0
blocker:
pause_reason:
return_trigger: the abandoned worker still holds its session and process tree after a bounded shutdown; the controller reports it — return when the RuntimeSession API gains a preemptive interrupt or the process tree must be reaped by the controller
deliberate_partial: true
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
