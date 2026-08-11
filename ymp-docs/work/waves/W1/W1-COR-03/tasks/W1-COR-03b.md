---
id: W1-COR-03b
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03a, W1-APP-02a]
blocks: [W1-COR-03c, W1-EVL-04a]
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

# W1-COR-03b — Yielded participants resume finitely and runs terminate honestly

## Outcome

Participants can yield without polling, resume from bounded event cursors when funded conditions
occur, and eventually reach an honest terminal state through finite leases, wake counts, starts,
queries, and obligation accounting.

## Scope

### In

- Invocation lifecycle, event cursors, mechanically typed wake conditions, coalescing, deadlines,
  session capsules, invocation-start budgets, lease expiry, per-principal round-robin admission,
  participant loss, cancellation, quiescence, and root terminal states.
- At-least-once event delivery and exactly-once command effect during a live controller interval.

### Out

- Semantic message interpretation, guaranteed process slots, infinite dormant sessions, automatic
  crash resume, central work prioritization, and cluster scheduling.

## Acceptance

- [ ] A yielded invocation has no live model process, resumes only after a matching committed event
  and admission, and consumes one finite start unit.
- [ ] Coalesced, duplicate, delayed, or lost notifications do not lose committed events; the
  participant resumes from its authoritative cursor.
- [ ] An unread message, stale help request, expired audience projection, or unfunded wake cannot
  keep a run active.
- [ ] Generated schedules with participant loss, lease expiry, no solution, cancellation, and
  verifier failure reach `exhausted`, `abstained`, `cancelled`, or `infrastructure_error` as
  specified and never alias quiescence to `accepted`.
- [ ] Removing one finite creation, wake, or obligation-return condition produces a liveness or
  conservation counterexample through the same test suite.

## Current state

Yield, wake, cursor, and quiescence semantics exist only in documentation. Implementation follows
the accepted local-contract core and event-history interfaces.

## Next action

Implement invocation yield and cursor recovery with the fake runtime before enabling multiple real
participants.

## Guardrails

- Notification channels never own durable state or delivery correctness.
- A wake condition refers only to a typed committed event, never natural-language meaning.
- Expiry may duplicate computation but cannot authorize a stale write.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
