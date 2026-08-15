---
id: W1-APP-02o.2
kind: subtask
wave: W1
card: W1-APP-02
state: active
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02o.1]
blocks: []
created_at: 2026-08-15T14:39:47+08:00
updated_at: 2026-08-15T14:41:59+08:00
started_at: 2026-08-15T14:41:59+08:00
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

# W1-APP-02o.2 — The unattested-runtime check parses items instead of truncating

## Outcome

ymp-cli/tests/unattested_runtime_is_unreachable.rs:128 stops truncating at the first test marker
and reuses the item-parsing helper of the accepted start guard; the negative half is the measured
pass of a post-marker module naming FakeRuntime beside a start.

## Scope

### In

- ymp-cli/tests/unattested_runtime_is_unreachable.rs and the shared parser helper.

### Out

- The guard semantics accepted in W1-APP-02o.

## Acceptance

- [ ] A post-marker start beside FakeRuntime is reported; pinned by test.

## Current state

Ready. Recorded from the W1-APP-02o.1 second look.

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
