---
id: W1-COR-03a
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-APP-02e, W1-EXP-01c]
blocks: [W1-COR-03b, W1-COR-03c, W1-COR-03d]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T19:34:25+08:00
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

- [ ] Compatible offer and bid records form one task contract, escrow transfer, first lease, and
  child obligation atomically; incompatible or expired records form none.
- [ ] Concurrent awards or open accepts cannot spend one reservation twice, exceed funded award
  count, or create duplicate obligations.
- [ ] A stale fencing generation cannot submit a current result, renew ownership, or close an
  obligation after reassignment.
- [ ] Deliberately removing a reservation, fencing, or child-return check makes the property suite
  produce a counterexample.
- [ ] No transition reads skill, model quality, natural-language intent, bid persuasiveness, or
  conversation activity to decide admission or award.

## Current state

The state transitions exist only in `PROTOCOL.md` and the planned model. Implementation follows the
accepted single-participant application path.

## Next action

Implement the budget, offer, award, lease, and obligation core against the executable model traces.

## Guardrails

- Monetary, token, and count dimensions use integer smallest units and are not silently exchanged.
- Failure adds outcome evidence but never changes a global task-complexity score.
- Sponsor and contractor are temporary relations scoped to one contract.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
