---
id: W1-APP-02e.8
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02e.5]
blocks: []
created_at: 2026-08-15T08:36:04+08:00
updated_at: 2026-08-15T10:50:41+08:00
started_at: 2026-08-15T10:35:25+08:00
accepted_at: 2026-08-15T10:50:41+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/66670c69c6340afe032c78ba32cd8ba7fdeed70a
closure_commit: https://github.com/maggnus/ymp/commit/025b22184a79780107c26e8c6895e698494caf0d
evidence: ["[025b221](https://github.com/maggnus/ymp/commit/025b22184a79780107c26e8c6895e698494caf0d)"]
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

- a spacing-concession ladder fits all four groups, the note and the full assurance text into 20
  body rows at 80x24; the reviewer took a direct product snapshot and independently probed the
  row-budget boundary; negative half pinned
