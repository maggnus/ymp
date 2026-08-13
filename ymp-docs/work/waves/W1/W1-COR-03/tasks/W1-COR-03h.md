---
id: W1-COR-03h
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03a]
blocks: [W1-EVL-04a]
created_at: 2026-08-13T16:27:50+08:00
updated_at: 2026-08-13T16:27:50+08:00
started_at: 2026-08-13T18:52:00+08:00
accepted_at: 2026-08-13T20:02:59+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/f96e8b7be2066da31c65870b2b91afe0cc9bd5b3
closure_commit: https://github.com/maggnus/ymp/commit/ce0aeecbcdbf598b4badc72529873748efe3d966
evidence: [`f96e8b7`](https://github.com/maggnus/ymp/commit/f96e8b7be2066da31c65870b2b91afe0cc9bd5b3)
duration_minutes: 48
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

- [x] Removing the covering check makes the conservation oracle fail with a captured non-zero exit;
      today that mutant leaves the whole suite green while facts and registry diverge.
- [x] An impossible debit is refused rather than skipped, and the refusal is observable.
- [x] Removing the holder check on the return path makes a test fail; today it does not, because no
      schedule ever reassigns a contract to another participant.

## Current state

Accepted and integrated. Conservation is asserted by comparing accounts rebuilt from the emitted
facts with the accounts the registry holds, two values built by different code from the same facts,
so they can disagree. The reviewer's own mutant — a fact emitted but never applied — produces a
registry divergence naming the account and both amounts.

## Next action

Compute conservation from the emitted facts, then add the reassignment case.

## Guardrails

- A check that cannot fail is the defect this card closes; do not replace it with another of the
  same shape.

## Findings

- All three guards now fail on the mutants they name, with concrete counterexamples: an uncovered
  debit, a silently skipped debit of a billion units, and an obligation closed by a participant that
  never held the lease.
- The independent review added a fourth mutant of its own, dropping a fact after emission, and the
  oracle caught it, which establishes that the oracle does not re-derive its expectation from the
  code that applies facts.
- One assertion delivered by the first candidate was itself tautological — the projection started
  from the opening balances and every fact was zero-sum, so it held for any stream at all. It was
  removed inside the same round and replaced by a directed test in which an award recorded as one
  unit larger parts from the registry.
- `minor`, refinement of the starting hypothesis, continued as W1-COR-03i: a fact corrupted at the
  moment it is generated stays invisible, because the schedules compare facts only with each other
  and never with the command that produced them.
- `minor`, additional work, continued as W1-COR-03i: the same registry-field calculation survives in
  the application crate's tests.

## Closure

Filled when the task is accepted.

### Accepted outcome

Each guard fails when the property it names is removed, and the conservation oracle is computed from
emitted facts by code independent of the code that applies them.

### Residuals

None. The two remaining weaknesses became W1-COR-03i, because a check that compares a system with
itself is the defect this card was opened to remove and is not carried as a limitation.

### Evidence

- [`f96e8b7`](https://github.com/maggnus/ymp/commit/f96e8b7be2066da31c65870b2b91afe0cc9bd5b3) —
  reviewed candidate with the tautology removed.
- [`ce0aeec`](https://github.com/maggnus/ymp/commit/ce0aeecbcdbf598b4badc72529873748efe3d966) —
  integration into the release branch.
