---
id: W1-COR-03i
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03h]
blocks: []
created_at: 2026-08-13T19:51:57+08:00
updated_at: 2026-08-13T19:51:57+08:00
started_at: 2026-08-13T20:50:00+08:00
accepted_at: 2026-08-13T21:41:56+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/f74221c171a943e9a58ebeea0480fb6e07c04502
closure_commit: https://github.com/maggnus/ymp/commit/67fe28d2fa21e993d3446de5c546a46c5451a4c1
evidence: [`f74221c`](https://github.com/maggnus/ymp/commit/f74221c171a943e9a58ebeea0480fb6e07c04502)
duration_minutes: 35
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-COR-03i — Emitted facts are proved against the command that produced them

## Outcome

A fact is checked against the command that produced it, not only against the other facts of the same
run, so a value corrupted at the moment of generation is detected rather than carried consistently
through every later check.

## Scope

### In

- The correspondence between an accepted command and the facts it emits, in the domain schedules.
- The conservation calculation that still reads registry fields in the application crate's tests.

### Out

- The commitment rules and the ledger's own guards, which are accepted.

## Acceptance

- [x] Reducing the amount of an escrow transfer at the point where the fact is generated makes a test
      fail; today the generated schedules stay green because they compare facts only with each other.
- [x] The application crate's conservation test is computed from emitted facts, and removing the
      covering check makes it fail with a captured non-zero exit.

## Current state

Accepted and integrated. The expectation is taken from the command a schedule issued, which the
kernel never records, while the compared value comes from the kernel's own facts, so a corruption
moves one side only. The reviewer corrupted a different command and a different field — a renewal's
wall-time consumption reduced by one — and the comparison produced the single violation naming both
values while every other check stayed silent.

## Next action

Bind each emitted fact to the command that produced it, then rewrite the application test's oracle.

## Guardrails

- A check that compares a system with itself proves nothing; the comparison must reach outside the
  code that produced the value.

## Findings

- The two sides of the comparison are produced by different code, verified by the reviewer following
  the derivation rather than the author's description.
- The application crate's conservation test now fails for the reason it claims: with the covering
  check neutralised, four of its six tests fail on a named account, where the previous version of the
  same test passed.
- `minor`, additional work, continued as W1-COR-03j: only the named amount is compared, not the
  account the value moved from, so an award's transfer redirected to another account passes every
  generated schedule and is caught only by a directed test.
- Four commands remain outside the comparison by a documented boundary: settlement, cancellation,
  return and attempt start.

## Closure

Filled when the task is accepted.

### Accepted outcome

A fact corrupted where it is generated is detected, because its expected value comes from the command
rather than from the fact stream, and the application crate's conservation test is computed from
emitted facts.

### Residuals

None. The unchecked source account became W1-COR-03j rather than a carried limitation, since a
comparison that verifies how much moved but not from where proves half of conservation.

### Evidence

- [`f74221c`](https://github.com/maggnus/ymp/commit/f74221c171a943e9a58ebeea0480fb6e07c04502) —
  reviewed candidate.
- [`67fe28d`](https://github.com/maggnus/ymp/commit/67fe28d2fa21e993d3446de5c546a46c5451a4c1) — integration into the release branch.
