---
id: W1-COR-03a
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-APP-02e, W1-EXP-01c]
blocks: [W1-COR-03b, W1-COR-03c, W1-COR-03d]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T19:34:25+08:00
started_at: 2026-08-13T15:10:00+08:00
accepted_at: 2026-08-13T16:27:50+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/1d62e79608cc0600252e7f75924605bc8dd50257
closure_commit: https://github.com/maggnus/ymp/commit/585242645d405ff1f76d6015a1141144eac4207b
evidence: [`1d62e79`](https://github.com/maggnus/ymp/commit/1d62e79608cc0600252e7f75924605bc8dd50257)
duration_minutes: 85
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-COR-03a — Local commitments conserve budgets and close obligations

## Outcome

Participants can advertise sponsor-funded work, bid, award compatible consent, transfer escrow,
execute under fenced leases, and return causally linked obligations without a kernel-selected
executor or semantic priority.

## Scope

### In

- Participants, budget vectors, creation authority, offers, bids, open acceptance, awards, task
  contracts, escrow, attempts, leases, fencing generations, obligation lineage, renewals, returns,
  cancellation, and typed non-success outcomes.
- Transactional transition tests and generated concurrent command schedules.
- Per-principal mechanical admission inputs without task-text inspection.

### Out

- Skill matching, grade-based eligibility, global priorities, permanent managers, prescribed
  decomposition, semantic bid ranking, or automatic escalation after failure.
- Collaboration payloads, runtime wake mechanics, and candidate synthesis details.

## Acceptance

- [x] Compatible offer and bid records form one task contract, escrow transfer, first lease, and
  child obligation atomically; incompatible or expired records form none.
- [x] Concurrent awards or open accepts cannot spend one reservation twice, exceed funded award
  count, or create duplicate obligations.
- [x] A stale fencing generation cannot submit a current result, renew ownership, or close an
  obligation after reassignment.
- [x] Deliberately removing a reservation, fencing, or child-return check makes the property suite
  produce a counterexample.
- [x] No transition reads skill, model quality, natural-language intent, bid persuasiveness, or
  conversation activity to decide admission or award.

## Current state

Accepted at Critical depth and integrated. A command is decided as a pure function of the registry:
a refusal changes nothing, an award fixes the contract, the escrow transfer, the first lease and the
child obligation as one set of facts, and contending commands reduce to one order, idempotent by
command identifier. The reviewer's own falsifier found no conservation violation on the candidate
and failed on three deliberately broken revisions.

## Next action

Implement the budget, offer, award, lease, and obligation core against the executable model traces.

## Guardrails

- Monetary, token, and count dimensions use integer smallest units and are not silently exchanged.
- Failure adds outcome evidence but never changes a global task-complexity score.
- Sponsor and contractor are temporary relations scoped to one contract.

## Findings

- The independent falsifier confirmed one award per funded slot out of three contenders, one open
  acceptance out of three, sixteen refusals that leave the registry byte-identical, conservation
  across all nine dimensions, and a fencing change that cuts off six commands from the displaced
  owner.
- `blocker`, independent product defect, continued as W1-COR-03g: a settlement returns escrow into a
  task contract that has already closed, and a further advertisement funded from it is accepted, so
  one reservation can be spent twice.
- `major`, additional work, continued as W1-COR-03h: an impossible debit is skipped silently, and the
  conservation check reads the same registry fields the code writes, so it cannot see the divergence
  it exists to catch.
- `major`, refinement of the starting hypothesis: the equivariance argument does not prove the
  absence of semantic reading. The permutation changes only the inert alphabet while budget vectors,
  deadlines, identifiers and record order stay fixed, so a kernel that ranks bids by price passes it.
  The negative is actually carried by the directed tests and schedules, which fail on that mutant.
- `minor`, additional work, continued as W1-COR-03h: removing the holder check on the return path
  leaves the whole suite green, because no generated schedule reassigns a contract to another
  participant.

## Closure

### Accepted outcome

Participants advertise sponsor-funded work, bid, award compatible consent, transfer escrow, execute
under fenced leases and return causally linked obligations, with no kernel-selected executor, no bid
ranking and no ordering by any property other than the mechanical ones the contract allows.

### Residuals

None carried on this card. The settlement defect and the two unfalsifiable guards became their own
required nodes rather than tolerated limitations, because the first can spend one reservation twice
and the second two hide a divergence a green suite would report as health.

### Evidence

- [`1d62e79`](https://github.com/maggnus/ymp/commit/1d62e79608cc0600252e7f75924605bc8dd50257) —
  reviewed candidate.
- [`5852426`](https://github.com/maggnus/ymp/commit/585242645d405ff1f76d6015a1141144eac4207b) —
  integration into the release branch.
