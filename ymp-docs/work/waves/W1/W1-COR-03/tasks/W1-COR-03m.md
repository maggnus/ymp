---
id: W1-COR-03m
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03k, W1-COR-03l]
blocks: []
created_at: 2026-08-14T05:19:05+08:00
updated_at: 2026-08-14T05:19:05+08:00
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
---

# W1-COR-03m — Candidate verification is a kernel fact, and a finished run reaches its terminal

## Outcome

A verifier decision enters the kernel as a committed fact, so a successfully finished managed run
closes its obligation and reaches Accepted or Exhausted instead of standing open; and the
admission-order check in the supervisor either reads the order after the agreement is written or
is removed, so it can actually refuse.

## Scope

### In

- The verification fact in the kernel lifecycle and its emission from the live controller.
- The inert admission-order check at ymp-runtime-supervisor/src/lib.rs:449-454 (measured unable to
  refuse: the agreement is written later, the queue reads empty).

### Out

- The lifecycle semantics accepted in W1-COR-03b and W1-COR-03l.

## Acceptance

- [ ] A live run whose candidate passes verification reaches the kernel terminal with a closed
      obligation; the negative half is the current build, where root_terminal stays empty.
- [ ] The admission-order check refuses at least one constructed out-of-order admission, or is
      removed with the reason in code; the negative half is the current check that no input fails.

## Current state

Ready. Recorded from the W1-COR-03k review (two minor findings and the supported child).

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
