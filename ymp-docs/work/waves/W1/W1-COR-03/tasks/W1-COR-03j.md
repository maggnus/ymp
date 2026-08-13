---
id: W1-COR-03j
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03i]
blocks: []
created_at: 2026-08-13T21:41:30+08:00
updated_at: 2026-08-14T02:11:00+08:00
started_at: 2026-08-14T01:40:23+08:00
accepted_at: 2026-08-14T02:11:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/bbe79c7
closure_commit: https://github.com/maggnus/ymp/commit/2378798
evidence: redirected award transfer caught by all 192 generated schedules (green on base, FactContradictsCommand on candidate); four excluded commands included or justified; reviewer ACCEPT with production-code movement mutations as different-shape falsifiers
duration_minutes: 95
blocker:
pause_reason:
return_trigger: a scenario set that seeds ledger state outside the observed command prefix makes unresolvable accounts reachable; the comparison-wide skip in movements()=None must then be narrowed
deliberate_partial: false
---

# W1-COR-03j — A fact is proved against its source account, not only its amount

## Outcome

A fact carries the account it moved value from, and that account is compared with the one the command
named, so money redirected to another account is detected by the generated schedules rather than by a
single directed test.

## Scope

### In

- The comparison between an accepted command and the facts it emits, extended from the amount to the
  source and destination accounts.
- The commands currently outside that comparison by documented boundary: settlement, cancellation,
  return and attempt start.

### Out

- The commitment rules themselves, which are accepted.

## Acceptance

- [ ] Redirecting an award's transfer from the offer's account to the sponsor's account makes the
      generated schedules fail; today they stay green and only a directed test with fixed
      expectations catches it.
- [ ] Each command currently excluded from the comparison is either included or its exclusion is
      justified in the code with the reason.

## Current state

Ready. The independent review of the fact-to-command comparison measured that only the named amount
is compared, so a transfer redirected to another account passes every generated schedule. The same
review recorded four commands outside the comparison by a documented boundary.

## Next action

Add the source and destination accounts to the comparison, then revisit the excluded commands.

## Guardrails

- A comparison that checks how much moved but not from where proves half of conservation.

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
