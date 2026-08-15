---
id: W1-APP-02e.7
kind: subtask
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02e]
blocks: []
created_at: 2026-08-15T02:53:45+08:00
updated_at: 2026-08-15T08:05:36+08:00
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
