---
id: W1-EXP-01c
kind: task
wave: W1
card: W1-EXP-01
state: active
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-APP-02a, W1-COR-03a, W1-COR-03b]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T11:25:15+08:00
started_at: 2026-08-12T11:02:25+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/2135c5c7532a23749d95363fcdf48e69fe6019dd
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-EXP-01c — Protocol model terminates under declared fault schedules

## Outcome

An executable state-machine model or simulator checks budget conservation, command idempotency,
lease fencing, obligation closure, cancellation, and eventual terminal outcomes under generated
duplicate, delay, expiry, and crash schedules.

## Scope

### In

- A bounded model of commands, control records, budgets, offers, awards, obligations, leases,
  candidates, verification, yield, wake, cancellation, and POC controller failure.
- Generated interleavings for duplicate delivery, stale writers, expiry races, ambiguous replies,
  and exhausted or abandoned work.
- Explicit fault assumptions and legitimate-state predicates corresponding to `PROTOCOL.md`.

### Out

- Semantic task success, model quality, host containment, distributed consensus, crash resume, and
  cluster behavior.
- Production Rust implementation of the kernel.

## Acceptance

- [ ] Generated schedules preserve every declared safety invariant and reach a permitted terminal
  state when creation, lease, wake, and query budgets are finite.
- [ ] Duplicate commands have one effect, and commands carrying a stale fencing generation cannot
  create a current candidate or close an obligation.
- [ ] Mutating at least one reservation, fencing, or obligation-return rule produces a failing
  counterexample through the same model check.
- [ ] A controller-crash schedule ends a POC run as `infrastructure_error`; it never resumes an
  ambiguous authority interval.

## Current state

The candidate exhaustively enumerates 385,605 reachable states and 1,927,370 transitions with
separate finite creation, start, wake, and protected-query budgets. It reports no non-terminal
deadlocks or cycles, bounds every terminal schedule at 30 transitions, and finds shortest
counterexamples for reservation, fencing, and obligation-return mutations. Independent review is
checking the graph construction and abstraction.

## Next action

Decide acceptance through independent graph and mutation checks on the exact candidate.

## Guardrails

- The model validates mechanical effects only and must not encode a semantic allocator.
- A finite-state abstraction must document every omitted implementation behavior.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
