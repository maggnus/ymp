---
id: W1-APP-02f
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T09:42:00+08:00
updated_at: 2026-08-12T10:03:00+08:00
started_at: 2026-08-12T09:40:09+08:00
accepted_at: 2026-08-12T10:03:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/a08bf5b2f9af7c6868dac8d4bd99a1d1c76b9583
closure_commit: https://github.com/maggnus/ymp/commit/4145a442b476afedca8d091de3a30b1eb1ad9b84
evidence: ["[a08bf5b](https://github.com/maggnus/ymp/commit/a08bf5b2f9af7c6868dac8d4bd99a1d1c76b9583)", "[4145a44](https://github.com/maggnus/ymp/commit/4145a442b476afedca8d091de3a30b1eb1ad9b84)"]
duration_minutes: 23
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

- [x] Ordered unique runtime events still reach candidate construction through the real supervisor path.
- [x] A duplicate, gap, or reordered event produces a typed `infrastructure_error`, creates no
  candidate, and leaves the run in a terminal state.
- [x] A regression test fails on the inherited implementation and passes on the corrected revision.

## Current state

Accepted. Duplicate, skipped, and reordered runtime progress is rejected before durable recording;
the run terminates with `InfrastructureError` and no candidate. Correct progress remains accepted.

## Next action

Proceed with the remaining required W1-APP-02 tasks.

## Guardrails

- Preserve the one-writer event history and typed failure model.
- Do not widen this task to environment binding or session resume.

## Findings

None.

## Closure

### Accepted outcome

The production supervisor enforces a contiguous event sequence and unique bounded event identifiers
before writing runtime progress. The same external product-path test that reproduced the inherited
defect now rejects duplicate, skipped, and reordered events without a candidate or active attempt,
including after application reopen.

### Residuals

None.

### Evidence

- [Reviewed correction](https://github.com/maggnus/ymp/commit/a08bf5b2f9af7c6868dac8d4bd99a1d1c76b9583).
- [Integration commit](https://github.com/maggnus/ymp/commit/4145a442b476afedca8d091de3a30b1eb1ad9b84).
