---
id: W1-APP-02f
kind: task
wave: W1
card: W1-APP-02
state: active
risk: critical
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T09:42:00+08:00
updated_at: 2026-08-12T09:42:00+08:00
started_at: 2026-08-12T09:40:09+08:00
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

# W1-APP-02f — Duplicate or out-of-order runtime events terminate without a candidate

## Outcome

The real supervisor path rejects duplicate, skipped, or reordered runtime progress before durable
recording or candidate construction, and terminates the run with a typed infrastructure failure.

## Scope

### In

- Runtime event sequence and event-identifier validation in the supervisor path.
- Terminal failure propagation and regression coverage through production wiring.

### Out

- Verification-environment binding, participant-session resume, the public protocol, and work records.

## Acceptance

- [ ] Ordered unique runtime events still reach candidate construction through the real supervisor path.
- [ ] A duplicate, gap, or reordered event produces a typed `infrastructure_error`, creates no
  candidate, and leaves the run in a terminal state.
- [ ] A regression test fails on the inherited implementation and passes on the corrected revision.

## Current state

Independent review reproduced a duplicate event that still created a candidate while the run
remained `Running`. The correction is active in an isolated workspace.

## Next action

Reject invalid runtime progress before recording it and rerun the reviewer-selected falsifier.

## Guardrails

- Preserve the one-writer event history and typed failure model.
- Do not widen this task to environment binding or session resume.

## Findings

- Duplicate runtime progress currently reaches candidate construction.

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger. Empty when there are none.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
