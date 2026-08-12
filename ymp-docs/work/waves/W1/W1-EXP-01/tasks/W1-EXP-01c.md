---
id: W1-EXP-01c
kind: task
wave: W1
card: W1-EXP-01
state: accepted
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W1-APP-02a, W1-COR-03a, W1-COR-03b]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T12:38:52+08:00
started_at: 2026-08-12T11:02:25+08:00
accepted_at: 2026-08-12T12:38:52+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/6145b5daef4f6c5d01981a335c43a8a78fd9dbad
closure_commit: https://github.com/maggnus/ymp/commit/35ce2b171faa0609cfeb95081c7965049f5edddc
evidence: ["[6145b5d](https://github.com/maggnus/ymp/commit/6145b5daef4f6c5d01981a335c43a8a78fd9dbad)", "[35ce2b1](https://github.com/maggnus/ymp/commit/35ce2b171faa0609cfeb95081c7965049f5edddc)"]
duration_minutes: 96
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

- [x] Generated schedules preserve every declared safety invariant and reach a permitted terminal
  state when creation, lease, wake, and query budgets are finite.
- [x] Duplicate commands have one effect, and commands carrying a stale fencing generation cannot
  create a current candidate or close an obligation.
- [x] Mutating at least one reservation, fencing, or obligation-return rule produces a failing
  counterexample through the same model check.
- [x] A controller-crash schedule ends a POC run as `infrastructure_error`; it never resumes an
  ambiguous authority interval.

## Current state

Accepted. The executable model exhaustively enumerates 2,045,152 reachable states and 5,581,282
transitions with no non-terminal deadlock or cycle and a maximum terminal path of 41 transitions.
It fences stale result return and stores abstract results for 14 command identifiers spanning all
consequential command classes. Independent re-review reproduced the full graph and challenged both
corrected invariants with external checks of a different form.

## Next action

Use the accepted traces as the implementation contract for the production kernel tasks after their
remaining declared dependencies close.

## Guardrails

- The model validates mechanical effects only and must not encode a semantic allocator.
- A finite-state abstraction must document every omitted implementation behavior.

## Findings

None. Both returned findings were corrected and independently rechecked: expired attempts cannot
close obligations, and all consequential command classes participate in stored-result replay.

## Closure

### Accepted outcome

The bounded model proves conservation, fencing, exactly-once command effects, finite yield/wake,
obligation closure, honest crash termination, and eventual permitted terminal outcomes under its
declared finite schedules. Five rule mutations are detected by the same exhaustive check.

### Residuals

The model intentionally abstracts durable recovery, real transports, unbounded identifiers, and
production implementation behavior; these limits are documented in the accepted model and remain
owned by production tasks.

### Evidence

- [Reviewed candidate](https://github.com/maggnus/ymp/commit/6145b5daef4f6c5d01981a335c43a8a78fd9dbad).
- [Integration commit](https://github.com/maggnus/ymp/commit/35ce2b171faa0609cfeb95081c7965049f5edddc).
