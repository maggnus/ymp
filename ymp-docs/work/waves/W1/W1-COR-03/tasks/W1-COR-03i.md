---
id: W1-COR-03i
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03h]
blocks: []
created_at: 2026-08-13T19:51:57+08:00
updated_at: 2026-08-13T19:51:57+08:00
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

- [ ] Reducing the amount of an escrow transfer at the point where the fact is generated makes a test
      fail; today the generated schedules stay green because they compare facts only with each other.
- [ ] The application crate's conservation test is computed from emitted facts, and removing the
      covering check makes it fail with a captured non-zero exit.

## Current state

Ready. The independent review of the ledger proofs measured that a fact corrupted at generation is
invisible to every current check, and that the registry-field calculation removed from the domain
still lives in the application crate's tests.

## Next action

Bind each emitted fact to the command that produced it, then rewrite the application test's oracle.

## Guardrails

- A check that compares a system with itself proves nothing; the comparison must reach outside the
  code that produced the value.

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
