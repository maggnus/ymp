---
id: W1-COR-03r
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: routine
maturity: BUILD
relation: required
depends_on: [W1-COR-03o]
blocks: []
created_at: 2026-08-15T00:34:25+08:00
updated_at: 2026-08-15T00:34:25+08:00
started_at:
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

# W1-COR-03r — A limit expiry is not silenced by a simultaneous cancel

## Outcome

A run whose limit expired records that violation even when a cancel lands in the same window; the negative half is the measured vanishing violation.

## Scope

### In

- ymp-rust/crates/ymp-runtime-supervisor.

### Out

- The terminal semantics accepted in W1-COR-03n, 03o and 03p.

## Acceptance

- [ ] A run whose limit expired records that violation even when a cancel lands in the same window; the negative half is the measured vanishing violation.

## Current state

Ready. Cancellation took precedence over budget expiry (lib.rs:1054-1064 at 3ee3942): a cancel landing together with an expired limit writes Cancelled and the limit violation vanishes from accounting.

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
