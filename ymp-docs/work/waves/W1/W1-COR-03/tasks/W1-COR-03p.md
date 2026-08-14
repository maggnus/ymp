---
id: W1-COR-03p
kind: task
wave: W1
card: W1-COR-03
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03n]
blocks: []
created_at: 2026-08-15T00:03:05+08:00
updated_at: 2026-08-15T00:03:21+08:00
started_at: 2026-08-15T00:03:21+08:00
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

# W1-COR-03p — A worker death by panic still drives the kernel to a terminal

## Outcome

A run whose worker dies by panic reaches a kernel terminal (infrastructure_error) and refuses later verdicts; the negative half is the measured terminal-less state.

## Scope

### In

- ymp-rust/crates/ymp-runtime-supervisor.

### Out

- The completion serialization accepted in W1-COR-03n.

## Acceptance

- [ ] A run whose worker dies by panic reaches a kernel terminal (infrastructure_error) and refuses later verdicts; the negative half is the measured terminal-less state.

## Current state

Ready. A panicking worker thread leaves the layer Running, the kernel without a terminal, the journal Cancelled, and cancel returning Ok — measured by the review; threatens INV-7 (honest termination).

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
