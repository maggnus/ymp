---
id: W1-COR-03k
kind: task
wave: W1
card: W1-COR-03
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03b]
blocks: []
created_at: 2026-08-14T03:48:07+08:00
updated_at: 2026-08-14T04:07:27+08:00
started_at: 2026-08-14T04:07:27+08:00
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

# W1-COR-03k — The live controller drives the kernel lifecycle, not its own

## Outcome

The running supervisor advances invocations through the kernel's yield, cursor and wake model
accepted in W1-COR-03b, instead of keeping parallel cursors and wake bookkeeping of its own, so a
live run and a modelled run cannot disagree about what resumes and what terminates.

## Scope

### In

- ymp-runtime-supervisor's own cursor and wake bookkeeping, replaced by or reconciled with the
  kernel lifecycle.

### Out

- The lifecycle semantics themselves, accepted in W1-COR-03b.

## Acceptance

- [ ] A live managed run resumes and terminates through the same kernel transitions the generated
      schedules exercise; the negative half shows today's parallel bookkeeping diverging.

## Current state

Ready. Recorded from the W1-COR-03b builder return: the supervisor keeps its own cursors and wakes
beside the kernel, outside that card's write zone.

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
