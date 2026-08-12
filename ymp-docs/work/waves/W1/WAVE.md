---
id: W1
kind: wave
state: blocked
areas: [EXP, APP, COR, EVL]
plan_review_state: pending
plan_review_evidence:
plan_review_at:
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T09:25:52+08:00
blocker: Owner gate G2 and independent plan review
---

# W1 — POC decision readiness

## Outcome

A controlled, matched-budget study can decide whether locally negotiated self-organization
improves independently accepted results and whether observed agent communication is causally
useful.

## Scope

This wave owns `POC-0` through `POC-3` from `ymp-docs/ROADMAP.md`: the falsification
package, foreground single-participant runtime, smallest self-organizing system, and controlled
evaluation. It runs only in an externally disposable environment.

It excludes MVP containment, ordinary-machine safety, OpenCode and Nemotron support, SQLite,
background service operation, server/client or web modes, PostgreSQL, Kubernetes, and any claim
about arbitrary projects or consciousness.

The interface design contract is prepared and reviewed in `W0`. That work may proceed in parallel
with `W1-EXP-01`; only the screen implementation tasks depend on its accepted result.

## Cards

- [W1-EXP-01](W1-EXP-01/CARD.md) — required
- [W1-APP-02](W1-APP-02/CARD.md) — required
- [W1-COR-03](W1-COR-03/CARD.md) — required
- [W1-EVL-04](W1-EVL-04/CARD.md) — required

## Plan review

In progress. A non-author reviewer is testing this tree for missing work,
false closure paths, dependency cycles, hidden owner decisions, uncheckable acceptance criteria,
and tasks that cannot be executed from a cold context. The review must return `ACCEPT` or
`RETURN`; no implementation work starts while this field is pending.
