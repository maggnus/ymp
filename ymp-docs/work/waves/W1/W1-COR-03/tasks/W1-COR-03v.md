---
id: W1-COR-03v
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-COR-03t]
blocks: []
created_at: 2026-08-15T13:30:03+08:00
updated_at: 2026-08-15T13:30:03+08:00
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

# W1-COR-03v — The interface states what a bounded shutdown actually ended

## Outcome

After a bounded controller shutdown the interface reports the measured fact — the worker was left
holding its session and process tree — instead of the unconditional "the managed process tree
was ended" (ymp-tui/src/app.rs:386-400 at 3bfd761); the negative half is the current unconditional
sentence.

## Scope

### In

- The cancel/shutdown reply in ymp-tui/src/app.rs, reading the controller's join outcome.

### Out

- The shutdown semantics, accepted in W1-COR-03t.

## Acceptance

- [ ] A bounded shutdown that left the worker running is reported as such; a clean end keeps the
      current sentence; pinned by a test.

## Current state

Ready. Recorded from the W1-COR-03t review.

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
