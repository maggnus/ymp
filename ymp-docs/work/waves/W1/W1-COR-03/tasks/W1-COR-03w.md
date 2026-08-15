---
id: W1-COR-03w
kind: task
wave: W1
card: W1-COR-03
state: active
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-COR-03t]
blocks: []
created_at: 2026-08-15T13:43:07+08:00
updated_at: 2026-08-15T13:44:25+08:00
started_at: 2026-08-15T13:44:25+08:00
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

# W1-COR-03w — Shutdown edge cases keep the journal and the finished run honest

## Outcome

Two edges measured by the W1-COR-03t re-review: (1) on the join path with a poisoned ledger lock the
journal stays on running while nothing else moves — the journal must move or the refusal be
recorded with its reason; (2) a controller shutdown landing in the completion window cancels a run
that already finished and committed its candidate — a finished run keeps its terminal.

## Scope

### In

- ymp-runtime-supervisor join/shutdown paths.

### Out

- The interrupt delivery and bounded shutdown accepted in W1-COR-03t.

## Acceptance

- [ ] Both edges pinned by tests with their measured negative halves.

## Current state

Ready. Recorded from the W1-COR-03t re-review (two minor findings).

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

None recorded.
