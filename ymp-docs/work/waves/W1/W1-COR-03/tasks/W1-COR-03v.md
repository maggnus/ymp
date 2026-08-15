---
id: W1-COR-03v
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-COR-03t]
blocks: []
created_at: 2026-08-15T13:30:03+08:00
updated_at: 2026-08-15T14:07:02+08:00
started_at: 2026-08-15T13:49:39+08:00
accepted_at: 2026-08-15T14:07:02+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/61603f5451d3b2900ec8940742e75b527ecf3262
closure_commit: https://github.com/maggnus/ymp/commit/3978c253e6f8afed43621b5f226bd5c629ba164d
evidence: ["[3978c25](https://github.com/maggnus/ymp/commit/3978c253e6f8afed43621b5f226bd5c629ba164d)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: any later change to cancel_run or to the ManagedShutdown variants must add the live close-to-reply test (a FakeRuntime that stops answering plus an application fixture waiting past the shutdown limit)
deliberate_partial: true
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

- join error texts preserved verbatim; the interface maps ManagedShutdown to an honest sentence with
  the supervisor report; author mutation reproduced; residual: the close-to-reply link itself is not
  pinned (a revert of cancel_run leaves 60 tests green)
