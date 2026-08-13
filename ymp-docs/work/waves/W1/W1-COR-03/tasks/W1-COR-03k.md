---
id: W1-COR-03k
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03b]
blocks: []
created_at: 2026-08-14T03:48:07+08:00
updated_at: 2026-08-14T05:19:05+08:00
started_at: 2026-08-14T04:07:27+08:00
accepted_at: 2026-08-14T05:19:05+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/04500164486105267f301ddfddae1b0820436c83
closure_commit: https://github.com/maggnus/ymp/commit/05cec14
evidence: parallel bookkeeping removed and verified absent; kernel admission and closure mutations break their tests; reviewer runtime-error scenario matched kernel terminal and journal; reviewer ACCEPT with two minor findings moved to W1-COR-03m
duration_minutes: 0
blocker:
pause_reason:
return_trigger: the run ledger lives in run memory; the first requirement to survive a controller restart or share the ledger across attempts returns this boundary
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
