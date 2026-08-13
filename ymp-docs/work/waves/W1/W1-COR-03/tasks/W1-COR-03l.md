---
id: W1-COR-03l
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03b]
blocks: []
created_at: 2026-08-14T04:06:57+08:00
updated_at: 2026-08-14T04:06:57+08:00
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

# W1-COR-03l — A stopped run sheds its yielded slices, and an attempt runs one slice

## Outcome

Stopping a run releases every yielded slice it still counts as a funded wake, so a stopped run
reaches its terminal instead of being held open by slices whose resumption is already refused;
and one attempt cannot hold two running slices at once.

## Scope

### In

- The 288 reachability states where a yielded slice survives StopRun as a funded wake, and the
  720 states where StartInvocation admits a second running slice of one attempt (measured by the
  W1-COR-03b review falsifier).
- The two check names the falsifier proved unreachable (AcceptedWithoutVerification,
  TerminalWhileOpen): each is made reachable, sharpened, or removed with the reason in code.

### Out

- The lifecycle semantics accepted in W1-COR-03b.

## Acceptance

- [ ] The reachability sweep finds no state where a stopped run is held open by a yielded slice;
      the negative half is the current model's 288 such states.
- [ ] StartInvocation refuses a second running slice of one attempt; the negative half is the
      current 720 admitting states.

## Current state

Ready. Recorded from the W1-COR-03b critical review falsifier.

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
