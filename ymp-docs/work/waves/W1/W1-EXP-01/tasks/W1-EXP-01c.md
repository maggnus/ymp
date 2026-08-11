---
id: W1-EXP-01c
kind: task
wave: W1
card: W1-EXP-01
state: blocked
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-APP-02a, W1-COR-03a, W1-COR-03b]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T19:34:25+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker: Owner gate G1
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

The protocol is specified in prose only. No executable model, generated schedule, or counterexample
exists. Work is blocked by `G1`.

## Next action

After `G1`, choose the smallest executable model format and encode the budget and obligation core.

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
