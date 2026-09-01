---
id: W1-APP-02e.7
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: accepted
risk: significant
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02e]
blocks: []
created_at: 2026-08-15T02:53:45+08:00
updated_at: 2026-08-15T12:14:44+08:00
started_at: 2026-08-15T11:26:50+08:00
accepted_at: 2026-08-15T12:14:44+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/8467ec00303c322cf96272d42d591011fc23fae4
closure_commit: https://github.com/maggnus/ymp/commit/2c91f16bebe8abbda8d746b92b25f5e324a01fbc
evidence: ["[2c91f16](https://github.com/maggnus/ymp/commit/2c91f16bebe8abbda8d746b92b25f5e324a01fbc)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02e.7 — The product itself tells the runtime how a candidate is published

## Outcome

A managed attempt is told by the product — not by the operator's prompt wording — that the result
is published through the submit coordination tool, so a run cannot burn its budget writing files
that never become a candidate. The operator's request stays untouched; the publication requirement
travels as the product's own instruction to the runtime.

## Scope

### In

- The instruction path from the supervisor/profile to the managed runtime.

### Out

- The prompt content of the operator, which stays verbatim.

## Acceptance

- [ ] A live attempt with a request that never mentions submit still publishes a candidate; the
      negative half is the measured first run of W1-APP-02e (46442 microusd spent,
      infrastructure_error, no candidate, because the hint did not mention publication).

## Current state

Ready. Recorded from the W1-APP-02e live scenario.

## Next action

None recorded.

## Guardrails

None recorded.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- the request stays verbatim and the publication instruction follows as the product's own line only
  when the bridge is attached; contract bytes and intent digest untouched; live Claude run with no
  publication mention published a candidate (118783 microusd, recorded in research/RES-001-calibration.md); reviewer
  fault injections reproduced the historic no-candidate outcome; prompt-policy pins bumped to v2 by
  a disclosed CTO fix with the boundary recorded
