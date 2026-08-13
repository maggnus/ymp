---
id: W1-COR-03j
kind: task
wave: W1
card: W1-COR-03
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03i]
blocks: []
created_at: 2026-08-13T21:41:30+08:00
updated_at: 2026-08-14T01:40:23+08:00
started_at: 2026-08-14T01:40:23+08:00
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
