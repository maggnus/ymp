---
id: W1-COR-03w
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-COR-03t]
blocks: []
created_at: 2026-08-15T13:43:07+08:00
updated_at: 2026-08-15T14:18:49+08:00
started_at: 2026-08-15T13:44:25+08:00
accepted_at: 2026-08-15T14:14:47+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/6aa404b1011a554ecaaa16887e74c408e0ff2dfe
closure_commit: https://github.com/maggnus/ymp/commit/904cac0f27d069c7a4ad326fd60816607639e206
evidence: ["[904cac0](https://github.com/maggnus/ymp/commit/904cac0f27d069c7a4ad326fd60816607639e206)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: cancel_run returns on root_terminal()? before cancellation.cancel(), so a poisoned ledger lock leaves the runtime uninterrupted — reorder when a poisoned lock becomes reachable in a live path
deliberate_partial: true
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

- both edges pinned and mutation-sensitive; product-path suites (descendant_termination,
  lifecycle_admission_refusal) green; the poisoned-lock cause is unreachable from outside so the
  branch, not the cause, is measured — stated honestly
