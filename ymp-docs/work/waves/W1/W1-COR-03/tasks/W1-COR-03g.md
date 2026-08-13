---
id: W1-COR-03g
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03a]
blocks: [W1-EVL-04a]
created_at: 2026-08-13T16:27:50+08:00
updated_at: 2026-08-13T16:27:50+08:00
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

# W1-COR-03g — Settlement returns escrow only to an account that can still spend it

## Outcome

Escrow released by a settlement can only reach an account that is still able to spend, so funds
never return into a closed task contract and no former contractor can spend from it a second time.

## Scope

### In

- The settlement path in the commitment ledger and the state of the account it credits.
- The invariant that funding an offer requires a live funding account.

### Out

- The commitment rules accepted in W1-COR-03a beyond this one path.

## Acceptance

- [ ] A settlement whose recipient contract is already closed is refused, and the refusal leaves the
      registry byte-identical. The negative half is the reviewer's reproduction: a returned contract
      holding money from which an advertisement is currently accepted, which must fail with a
      captured non-zero exit.
- [ ] No funded action can draw on an account whose contract has reached a terminal state, proved
      over generated schedules rather than one ordering.

## Current state

Ready. The independent review of the commitment kernel reproduced a settlement that credits a task
contract already in a returned state, from which a further advertisement was accepted. No acceptance
item of that card is violated, so the defect stands on its own, and it can spend one reservation
twice, which the comparison cannot tolerate.

## Next action

Refuse a settlement whose recipient is not a live account, then prove it over schedules.

## Guardrails

- Money that has been settled is not a pool the next command may reuse.

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
