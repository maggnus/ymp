---
id: W1-COR-03h
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
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

# W1-COR-03h — Conservation and ownership checks fail on the mutants they name

## Outcome

The checks that guard conservation and lease ownership fail when the property they name is removed,
so a green suite means the property holds rather than that nobody tested it.

## Scope

### In

- A conservation oracle computed from the emitted facts rather than from the same registry fields
  the code updates.
- A hard refusal when a debit cannot be covered, instead of silently skipping it.
- A directed case in which a participant who never held a lease attempts to close an obligation.

### Out

- The commitment rules themselves, which are accepted.

## Acceptance

- [ ] Removing the covering check makes the conservation oracle fail with a captured non-zero exit;
      today that mutant leaves the whole suite green while facts and registry diverge.
- [ ] An impossible debit is refused rather than skipped, and the refusal is observable.
- [ ] Removing the holder check on the return path makes a test fail; today it does not, because no
      schedule ever reassigns a contract to another participant.

## Current state

Ready. The independent review of the commitment kernel measured that two guards cannot fail on their
own mutants: the conservation check reads the same fields the code writes, and the holder check on
the return path has no case behind it. Both properties hold on the accepted revision; the proofs do
not.

## Next action

Compute conservation from the emitted facts, then add the reassignment case.

## Guardrails

- A check that cannot fail is the defect this card closes; do not replace it with another of the
  same shape.

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
