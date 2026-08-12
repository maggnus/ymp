---
id: W0
kind: wave
state: ready
areas: [UX]
plan_review_state: accepted
plan_review_evidence: https://github.com/maggnus/ymp/commit/7468b37a25f3d5f0f2dec0a8a0482dddbdff1ca9
plan_review_at: 2026-08-12T09:30:05+08:00
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T09:30:05+08:00
blocker:
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

Accepted. Independent review returned the plan once because it allowed an unresolved assumption
affecting trust, authorization, verification, or a terminal outcome to be recorded as an ordinary
warning. Tasks `W0-UX-01b` and `W0-UX-01c` now make that condition block acceptance and require
negative controls for lost MCP replies and quiescence with unread inert messages. The repeated
falsifier no longer found a premature closure path.
