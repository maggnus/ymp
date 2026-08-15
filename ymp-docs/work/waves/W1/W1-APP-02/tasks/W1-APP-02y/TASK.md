---
id: W1-APP-02y
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02l]
blocks: []
created_at: 2026-08-14T02:46:00+08:00
updated_at: 2026-08-14T22:59:14+08:00
started_at: 2026-08-14T05:19:32+08:00
accepted_at: 2026-08-14T22:59:14+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/f74ae1549abfb47e024049283f1a86bc16ad3f55
closure_commit: https://github.com/maggnus/ymp/commit/c63130c1a93afc4603f1f04d24e1fae465816d0d
evidence: ["[c63130c](https://github.com/maggnus/ymp/commit/c63130c1a93afc4603f1f04d24e1fae465816d0d)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: a real many-turn run whose record reads unverified only because the rounding bound is per model, not per accounted turn
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

- both drivers fill the field from runtime data only, proved by reviewer defect injections into
  product code; disclosed CTO fix made the rounding tolerance one-sided (an overshoot reads
  unverified); the per-model bound stays conservative for many-turn runs
