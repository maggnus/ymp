---
id: W0
kind: wave
state: blocked
areas: [UX]
plan_review_state: pending
plan_review_evidence:
plan_review_at:
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-10T21:23:34+08:00
blocker: Owner gate G1 and independent plan review
---

# W0 — Terminal interface design readiness

## Outcome

An independently reviewed screen and state contract makes the POC terminal interface implementable
without inventing product semantics inside widgets or constraining Claude Design to a preconceived
layout.

## Scope

This wave owns the Claude Design request, the resulting POC-1 and POC-2 terminal mockups and state
variants, and the feasibility review that binds those artifacts to the Rust/ratatui implementation
tasks in `W1`.

It excludes production Rust code, kernel or protocol changes, contract-oracle research, visual
implementation, interface polish beyond the POC, and any server, web, database, or cluster mode.
It may proceed in parallel with `W1-EXP-01`, but its accepted result is required before the
single-participant and multi-participant screens are implemented.

## Cards

- [W0-UX-01](W0-UX-01/CARD.md) — required

## Plan review

Pending. Before the card starts, a non-author reviewer must test whether the decomposition covers
the design request, independent Claude Design output, and implementation-feasibility review without
letting design work redefine protocol semantics. Owner gate `G1` must first establish the Git and
durable-evidence conventions needed to record that verdict.
