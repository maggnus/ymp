---
id: W1-APP-02e.8
kind: subtask
wave: W1
card: W1-APP-02
state: active
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02e.5]
blocks: []
created_at: 2026-08-15T08:36:04+08:00
updated_at: 2026-08-15T10:35:25+08:00
started_at: 2026-08-15T10:35:25+08:00
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

# W1-APP-02e.8 — The key map fits the screen or scrolls

## Outcome

The ? key map shows every group at 80x24 — by scrolling or by a tighter layout; the negative half
is the measured render where the modals group (Enter confirm, Esc always the safe exit) is pushed
out (W1-APP-02e.5 second look).

## Scope

### In

- ymp-rust/crates/ymp-tui/src/overlay.rs.

### Out

- The start feed, accepted in W1-APP-02e.5.

## Acceptance

- [ ] Every key-map group is reachable at 80x24, pinned by a render test; the negative half is
      the current cut.

## Current state

Ready.

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
