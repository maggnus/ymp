---
id: W1-COR-03g
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03a]
blocks: [W1-EVL-04a]
created_at: 2026-08-13T16:27:50+08:00
updated_at: 2026-08-13T16:27:50+08:00
started_at: 2026-08-13T16:47:00+08:00
accepted_at: 2026-08-13T18:43:33+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/6b10304ccd0aa3ba958bf3eb2b8405ee6c926472
closure_commit: https://github.com/maggnus/ymp/commit/2b390922667426a48b01c9225ccee08c7d8e81f8
evidence: [`6b10304`](https://github.com/maggnus/ymp/commit/6b10304ccd0aa3ba958bf3eb2b8405ee6c926472)
duration_minutes: 96
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

- [x] A settlement whose recipient contract is already closed is refused, and the refusal leaves the
      registry byte-identical. The negative half is the reviewer's reproduction: a returned contract
      holding money from which an advertisement is currently accepted, which must fail with a
      captured non-zero exit.
- [x] No funded action can draw on an account whose contract has reached a terminal state, proved
      over generated schedules rather than one ordering.

## Current state

Accepted at Critical depth and integrated. A contract stops being an account at a terminal state and
cannot close while a return is owed to an offer funded from its escrow, so the settlement recipient
follows ownership of the escrow rather than whoever executes at the time. The reviewer's own
integration test over the public interface, with an oracle computed from emitted facts, accepts the
candidate and rejects the base.

## Next action

Refuse a settlement whose recipient is not a live account, then prove it over schedules.

## Guardrails

- Money that has been settled is not a pool the next command may reuse.

## Findings

- The defect is closed and independently re-measured: on the base the reviewer's falsifier accepted
  the spend-from-closed chain end to end and exited 101; on the candidate it passes.
- `minor`, refinement of the starting hypothesis: the refusal to settle into a closed account is
  unreachable on executable orderings, because closing sweeps the escrow first. What actually closes
  the defect is the paired rule that a contract cannot close while a return is owed. The unreachable
  half becomes reachable only through a path into a terminal state that bypasses that pairing.
- `minor`, independent product defect, already carried by W1-COR-03h: the aggregate conservation
  check still reads registry fields rather than emitted facts.

## Closure

Filled when the task is accepted.

### Accepted outcome

Escrow released by a settlement reaches only an account that can still spend, and no funded action
draws on an account whose contract has reached a terminal state, measured over the reviewer's own
interleavings rather than the author's alone.

### Residuals

None. The unreachable half of the rule is recorded as a refinement rather than carried as a
limitation, and the conservation oracle belongs to W1-COR-03h.

### Evidence

- [`6b10304`](https://github.com/maggnus/ymp/commit/6b10304ccd0aa3ba958bf3eb2b8405ee6c926472) —
  reviewed candidate.
- [`2b39092`](https://github.com/maggnus/ymp/commit/2b390922667426a48b01c9225ccee08c7d8e81f8) —
  integration into the release branch.
