---
id: W1-APP-02z.1
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02z
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02z]
blocks: []
created_at: 2026-08-15T00:05:22+08:00
updated_at: 2026-08-15T00:05:22+08:00
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

# W1-APP-02z.1 — The npm entry point is pinned to its interpreter choice, or stays refused

## Outcome

A proposed npm verifier pins the interpreter and its environment so candidate files cannot redirect it, and it accepts a working candidate under the executor environment; or the refusal stays. The negative halves are the two measured bypasses.

## Scope

### In

- The verifier proposal in ymp-application (answer.rs) and its draft wording.

### Out

- The pinning accepted in W1-APP-02z.

## Acceptance

- [ ] A proposed npm verifier pins the interpreter and its environment so candidate files cannot redirect it, and it accepts a working candidate under the executor environment; or the refusal stays. The negative halves are the two measured bypasses.

## Current state

Ready. Split from W1-APP-02z round 2: the candidate chooses the program that runs package.json scripts (.npmrc script-shell — a measured silent false accept), and under the executor PATH the generated npm verifier accepted nothing. Until the interpreter choice itself is pinned, npm projects receive the honest TestEntryPointNotSupported refusal.

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

None recorded.
