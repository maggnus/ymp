---
id: W0
kind: wave
state: blocked
areas: [UX]
plan_review_state: pending
plan_review_evidence:
plan_review_at:
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T09:25:52+08:00
blocker: Independent plan review
---

# W0 — Terminal interface design readiness

## Outcome

An independently reviewed screen and state contract makes the POC terminal interface implementable
without inventing product semantics inside widgets or treating an attractive mockup as acceptance
evidence.

## Scope

This wave owns the sole HTML design artifact, its POC-1 and POC-2 screen and state coverage, and the
feasibility review that binds the accepted artifact revision to the Rust/ratatui implementation
tasks in `W1`. No separate design request or external design package exists.

It excludes production Rust code, kernel or protocol changes, contract-oracle research, visual
implementation, interface polish beyond the POC, and any server, web, database, or cluster mode.
It may proceed in parallel with `W1-EXP-01`, but its accepted result is required before the
single-participant and multi-participant screens are implemented.

## Cards

- [W0-UX-01](W0-UX-01/CARD.md) — required

## Plan review

In progress. A non-author reviewer is testing whether the decomposition covers product-to-screen
traceability, the sole HTML artifact, and implementation-feasibility review without letting design
work redefine protocol semantics. Owner gate `G1` is resolved.
