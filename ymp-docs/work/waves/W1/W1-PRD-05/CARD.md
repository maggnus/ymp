---
id: W1-PRD-05
kind: card
wave: W1
state: active
risk: critical
maturity: DESIGN
relation: required
depends_on: []
blocks: []
created_at: 2026-08-15T02:07:10+08:00
updated_at: 2026-08-15T02:07:10+08:00
started_at: 2026-08-15T02:07:10+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
---

# W1-PRD-05 — The product is experienced as an autonomous collective

## Outcome

An operator gives ymp a natural-language goal and receives a verified result with evidence,
without authoring contracts, oracles, verifiers, teams, roles, model assignments or
decomposition plans. The collective is the intelligence; the kernel enforces mechanics; the
verification system proves the result; the TUI stays simple. Source of truth:
[PRODUCT-BRIEF-collective.md](../../../../design/PRODUCT-BRIEF-collective.md).

## Invariants

- Internal safety mechanisms move behind the product boundary; none is removed for UX.
- The kernel never makes semantic decisions; no hidden manager agent appears.
- A provider's model catalog never auto-instantiates participants.
- The operator is asked only about genuine intent ambiguity.
- Honest terminal states stay distinct; exhaustion is never presented as success.

## Scope

The complete redesign of the operator experience and the ownership boundaries per the brief's
deliverable list (16.1-16.24), the acceptance criteria (17.1-17.16) and the final design test
(18). Implementation cards are derived after the design is accepted and the owner resolves the
open decisions.

## Aggregate acceptance

The design deliverable is accepted after independent review and every one of the brief's
acceptance criteria is either satisfied by the design or carried as a named owner decision; the
final design test scenario walks through the design without any operator-authored machinery.

## Tasks
