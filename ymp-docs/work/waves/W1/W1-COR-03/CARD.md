---
id: W1-COR-03
kind: card
wave: W1
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-APP-02]
blocks: [W1-EVL-04]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T21:23:34+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
---

# W1-COR-03 — Bounded local commitments self-organize and terminate

## Outcome

Multiple participants can negotiate, delegate, challenge, compete, synthesize, abstain, and stop
without a semantic dispatcher, while the kernel preserves finite resources, attribution,
immutable candidates, and honest terminal states.

## Invariants

- Offers, bids, and awards form only through local consent and sponsor-owned escrow.
- Every delegation creates a finite, causally linked work obligation.
- Leases use fencing so expired attempts cannot advance current state.
- Collaboration messages are inert and scoped separately from control and verification.
- Event notifications can be lost or coalesced without losing committed state.
- No-solution runs terminate without being labelled accepted.

## Scope

This card owns `POC-2`: local contracts, budgets, obligations, leases, event cursors, yield and
wake, fair non-semantic admission, scoped collaboration, communication observability, competing
candidate ancestry, explicit synthesis, and quiescence detection.

It excludes grade-based eligibility, global task ranking, permanent roles, automatic escalation,
semantic merge resolution, and any metric that rewards influence, agreement, or message volume.

## Aggregate acceptance

All five required tasks are accepted. Generated race and fault schedules cannot overspend,
duplicate a shared effect, accept a stale submission, let a board payload exercise authority, or
keep a run active without a funded control object. At least one multi-participant run reaches each
applicable non-success terminal state without manual state repair, and the terminal interface
exposes these paths without assigning work or overstating communication evidence.

## Tasks

- [W1-COR-03a](tasks/W1-COR-03a.md) — required
- [W1-COR-03b](tasks/W1-COR-03b.md) — required
- [W1-COR-03c](tasks/W1-COR-03c.md) — required
- [W1-COR-03d](tasks/W1-COR-03d.md) — required
- [W1-COR-03e](tasks/W1-COR-03e/TASK.md) — required
- [W1-COR-03f](tasks/W1-COR-03f.md) — required
- [W1-COR-03g](tasks/W1-COR-03g.md) — required
- [W1-COR-03h](tasks/W1-COR-03h.md) — required
- [W1-COR-03i](tasks/W1-COR-03i.md) — required
- [W1-COR-03j](tasks/W1-COR-03j.md) — required
