---
id: W1-APP-02y
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02l]
blocks: []
created_at: 2026-08-14T02:46:00+08:00
updated_at: 2026-08-14T02:46:00+08:00
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

# W1-APP-02y — The run record itself carries the per-model spend

## Outcome

The serialized run record distinguishes an attributed cost from an unverified one by carrying the
per-model breakdown as a field, so attribution survives as data rather than only as an admission
rule. The shared Usage type in ymp-runtime-api gains the field; both runtime drivers fill it.

## Scope

### In

- The Usage type in ymp-runtime-api and its serialization in the run record.
- Both runtime drivers and the accounting tests that pin the field.

### Out

- The admission rules accepted in W1-APP-02l, which stay as they are.

## Acceptance

- [ ] A finished managed run's record names each model and its spend, summing to the recorded
      total; the negative half is the current record, which carries only the total.

## Current state

Ready. Recorded from the W1-APP-02l review (round 2, finding 3): attribution is enforced at
admission but the record does not distinguish an attributed cost from an unverified one.

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
