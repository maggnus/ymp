---
id: W1-APP-02o.2
kind: subtask
wave: W1
card: W1-APP-02
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02o.1]
blocks: []
created_at: 2026-08-15T14:39:47+08:00
updated_at: 2026-08-15T15:04:22+08:00
started_at: 2026-08-15T14:41:59+08:00
accepted_at: 2026-08-15T15:04:22+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/534a0c4
closure_commit: https://github.com/maggnus/ymp/commit/d5e1df6
evidence: composition with 02s merged clean; the post-marker FakeRuntime start is reported and the truncation revert makes it pass again; the start guard semantics survived the parser extraction (four prior mutations re-run); the fixture-crate closure checks live; residual: the truncation removal is pinned only through the report function, not through shipped_sources
duration_minutes: 0
blocker:
pause_reason:
return_trigger: a change to collect() or shipped_sources() in the unattested-runtime check must add the source-reading negative half
deliberate_partial: true
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
