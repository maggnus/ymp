---
id: W1
kind: wave
state: active
areas: [EXP, APP, COR, EVL]
plan_review_state: accepted
plan_review_evidence: https://github.com/maggnus/ymp/commit/b6ca7c8d589fb3143ee0aabc5afe3fda3e014ddf
plan_review_at: 2026-09-01T13:54:00+08:00
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-09-01T13:54:00+08:00
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

The plan was reviewed again after the scientific stage-boundary audit. R1 returned the tree because
the new held-out L4+ task owner contradicted a protocol that still ran L1–L3 as three-arm evidence;
it also required legacy validator debt to have its own non-blocking node. R2(9/10) accepted
[b6ca7c8](https://github.com/maggnus/ymp/commit/b6ca7c8d589fb3143ee0aabc5afe3fda3e014ddf):
L1–L3 are calibration only; W1-EXP-01e freezes L4+ tasks before any model call; W1-EVL-04e/f own
weak diagnostic and strong-profile transfer; W1-EVL-04a remains primary-only; W1-EVL-04g isolates
historical work-record debt. The reviewed closure graph is acyclic.
