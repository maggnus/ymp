---
id: W1-APP-02z.2
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02z
state: accepted
risk: routine
maturity: BUILD
relation: required
depends_on: [W1-APP-02z]
blocks: []
created_at: 2026-08-15T00:05:22+08:00
updated_at: 2026-08-15T12:33:57+08:00
started_at: 2026-08-15T12:18:33+08:00
accepted_at: 2026-08-15T12:33:57+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/318016a8cdca7102bb6bf18074c60c61e60467a5
closure_commit: https://github.com/maggnus/ymp/commit/163a7a027900b2b1ad08fed83d500aae2a1b4015
evidence: ["[163a7a0](https://github.com/maggnus/ymp/commit/163a7a027900b2b1ad08fed83d500aae2a1b4015)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02z.2 — The refusal names the actual obstacle

## Outcome

A non-executable entry point is named as the obstacle with its permission problem; the negative half is the measured refusal that names the wrong file.

## Scope

### In

- The verifier proposal in ymp-application (answer.rs) and its draft wording.

### Out

- The pinning accepted in W1-APP-02z.

## Acceptance

- [ ] A non-executable entry point is named as the obstacle with its permission problem; the negative half is the measured refusal that names the wrong file.

## Current state

Ready. From the final W1-APP-02z review: a scripts/test.sh present but not executable is silently skipped, and the refusal names the next recognized file (package.json) instead of the real obstacle.

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

- a non-executable test script is named as the obstacle with its missing permission; the negative
  half showed the old refusal naming package.json instead
