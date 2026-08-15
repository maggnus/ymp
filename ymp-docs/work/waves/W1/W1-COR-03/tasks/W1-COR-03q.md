---
id: W1-COR-03q
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: routine
maturity: BUILD
relation: required
depends_on: [W1-COR-03o]
blocks: []
created_at: 2026-08-15T00:34:25+08:00
updated_at: 2026-08-15T01:04:05+08:00
started_at: 2026-08-15T00:34:43+08:00
accepted_at: 2026-08-15T01:04:05+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/b427ad0ce24d79fdf3e0c1ba509fa92ccb47175d
closure_commit: https://github.com/maggnus/ymp/commit/40c1095c3cc4b0b65868c74150cea54e610d6359
evidence: ["[40c1095](https://github.com/maggnus/ymp/commit/40c1095c3cc4b0b65868c74150cea54e610d6359)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-COR-03q — A poisoned journal lock still lets the run record its terminal

## Outcome

After a panic that poisons the journal lock, the journal still reaches its terminal record; the negative half is the measured Running journal beside a terminal kernel.

## Scope

### In

- ymp-rust/crates/ymp-runtime-supervisor.

### Out

- The terminal semantics accepted in W1-COR-03n, 03o and 03p.

## Acceptance

- [ ] After a panic that poisons the journal lock, the journal still reaches its terminal record; the negative half is the measured Running journal beside a terminal kernel.

## Current state

Ready. A panic under the held journal lock poisons it, record_infrastructure_failure silently exits, and the journal stays Running while the kernel holds InfrastructureError — measured by the 03o+03p review fault injection. Recovery must align with the take_terminal decision.

## Next action

None recorded.

## Guardrails

None recorded.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- whole-fact append before in-memory apply survives a poisoned lock; a repeated interrupted command
  is refused by sequence, no double write (verified in sources by the second look)
