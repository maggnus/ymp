---
id: W1-APP-02x
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02u]
blocks: []
created_at: 2026-08-14T02:19:00+08:00
updated_at: 2026-08-14T04:08:46+08:00
started_at: 2026-08-14T02:19:00+08:00
accepted_at: 2026-08-14T04:08:46+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/4e8c449ff027b0a4380dde4460ac2bf8b683c7ed
closure_commit: https://github.com/maggnus/ymp/commit/7e9883111e062d62e69cc4b2604345870f4bece2
evidence: ["[7e98831](https://github.com/maggnus/ymp/commit/7e9883111e062d62e69cc4b2604345870f4bece2)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02x — Entry validation cannot hold the interface

## Outcome

Validating a typed answer never freezes the dialogue: the negative-control run happens off the
event loop or under a short entry-time limit, the interface keeps redrawing, and the operator can
cancel a hanging verifier where they typed it.

## Scope

### In

- The synchronous call path app.rs -> draft answer validation with the verifier wall limit of
  60000 ms, measured by the W1-APP-02u review as a 60 s interface hold without redraw.

### Out

- The validation semantics themselves, accepted in W1-APP-02u.

## Acceptance

- [ ] A verifier that sleeps longer than the entry limit leaves the interface responsive and ends
      in a refusal the operator can see; the negative half is the current build's measured 60 s
      hold without redraw.
- [ ] Esc during a running entry check cancels it and returns to the answer.

## Current state

Active, dispatched in one batch with W1-APP-02v: one surface, one workspace, one review.

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

- hanging verifier left the interface redrawing for the full limit and Esc cancelled in 0.23 s; the
  negative half measured the prior 60.1 s hold on the base revision
