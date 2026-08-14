---
id: W1-COR-03s
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03q]
blocks: []
created_at: 2026-08-15T01:04:05+08:00
updated_at: 2026-08-15T01:55:01+08:00
started_at: 2026-08-15T01:04:32+08:00
accepted_at: 2026-08-15T01:55:01+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/1ef0796
closure_commit: https://github.com/maggnus/ymp/commit/ddd250c
evidence: reviewer fault-injection sweep (8 runs) green on the candidate and refusing at round zero with the defect returned; release_control_and_join proved on join and Drop of one handle; new findings recorded as W1-COR-03t and W1-COR-03u
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-COR-03s — An operator cancel survives a poisoned lock, and Drop never hangs

## Outcome

Cancelling a run whose journal lock was poisoned still sets the cancellation token and reaches a
terminal, and dropping the controller joins the worker without holding the control sender, so no
path leaves the controller hanging indefinitely.

## Scope

### In

- The cancel path at ymp-runtime-supervisor/src/lib.rs:449 (refuses on a poisoned lock without
  setting the token) and the Drop at lib.rs:541-548 (joins the worker while holding the control
  sender) — measured by the 03q+03r second look: cancel returned Err, journal stayed Running,
  the process was killed by timeout (exit 144).
- The minor sequence note: a panic exactly between append and apply leaves later journal commands
  refused by sequence while the execute error in record_infrastructure_failure is discarded.

### Out

- The whole-fact append order accepted in W1-COR-03q.

## Acceptance

- [ ] Cancel on a poisoned lock sets the token and the run reaches a terminal in both accountings;
      the negative half is the measured hang (exit 144).
- [ ] No Drop path joins the worker while the control sender is still held.

## Current state

Ready. Recorded from the 03q+03r second look (major independent defect).

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
