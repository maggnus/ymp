---
id: W1-APP-02e.9
kind: subtask
wave: W1
card: W1-APP-02
state: active
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02e.8]
blocks: []
created_at: 2026-08-15T10:50:41+08:00
updated_at: 2026-08-15T13:03:13+08:00
started_at: 2026-08-15T13:03:13+08:00
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

# W1-APP-02e.9 — Key descriptions wrap instead of truncating

## Outcome

A key description longer than the map column wraps with a hanging indent (or the column widens),
so no hint is cut mid-word; the negative half is the measured cut of the Esc description at
KEYS_WIDTH = 64 (overlay.rs:359), identical at both accepted sizes.

## Scope

### In

- ymp-rust/crates/ymp-tui/src/overlay.rs, composing with the accepted spacing ladder.

### Out

- The vertical fit, accepted in W1-APP-02e.8.

## Acceptance

- [ ] Every key description is fully readable at 80x24, pinned by a render test; the negative
      half is the current cut.

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
