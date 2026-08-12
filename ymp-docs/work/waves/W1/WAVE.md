---
id: W1
kind: wave
state: active
areas: [EXP, APP, COR, EVL]
plan_review_state: accepted
plan_review_evidence: https://github.com/maggnus/ymp/commit/dd2cadb7d34a3698bfbe6011b4f502506173aa0a
plan_review_at: 2026-08-12T09:35:13+08:00
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T11:56:07+08:00
blocker:
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

Accepted. Independent review returned the plan once because `W1-EXP-01b` could freeze an
unapproved budget and blinded selection lacked an early-disclosure falsifier. The corrected plan
made budget authority explicit, and `W1-COR-03c` plus `W1-EVL-04a` reject early disclosure from the
primary comparison. Repeated falsifiers and the structural check passed.

Owner gates `G2` and `G3` were resolved on 12 August. External public packages have no
owner-imposed numeric ceiling. A monetary limit is optional and, if configured, applies to the
project as a whole. Corpus size and resource opportunity are frozen by the preregistered
statistical design, while actual usage and cost remain fully accounted.
