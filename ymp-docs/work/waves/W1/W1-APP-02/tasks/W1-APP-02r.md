---
id: W1-APP-02r
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: routine
maturity: BUILD
relation: follow_up
depends_on: [W1-APP-02q]
blocks: []
created_at: 2026-08-13T21:11:27+08:00
updated_at: 2026-08-13T21:11:27+08:00
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

# W1-APP-02r — An unreadable marker is not read as an absent holder

## Outcome

A holder lookup distinguishes "no one holds this" from "this could not be read", so a permission
failure never passes as an empty answer, and the test kit is not a dependency of the shipped binary.

## Scope

### In

- The holder lookup that reads the process utility's exit code and output.
- The dependency entry that keeps the fake runtime in the command crate.

### Out

- The admission gate accepted in W1-APP-02q.

## Acceptance

- [ ] With the marker present but unreadable by the account, the lookup reports an error rather than
      an empty holder list; the negative half denies access to the marker and shows today's build
      reporting no holders.
- [ ] The fake runtime is a development dependency of the command crate, and the shipped binary does
      not link it.

## Current state

Ready. The review of the admission gate measured that the process utility exits with code one and no
output both when nothing holds the marker and when access is denied, and the lookup reads both as
nobody. The same review found the fake runtime still listed as an ordinary dependency of the command
crate while no source file names it.

## Next action

Confirm the marker exists and is readable before accepting an empty answer.

## Guardrails

- An error and an empty result are different answers; a check that merges them reports health it has
  not established.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
