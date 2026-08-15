---
id: W1-PRD-05g
kind: task
wave: W1
card: W1-PRD-05
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05d]
blocks: []
created_at: 2026-08-15T23:35:00+08:00
updated_at: 2026-08-15T23:35:00+08:00
started_at: 2026-08-15T23:35:00+08:00
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

# W1-PRD-05g — Provider measurement runs off the interface thread

## Outcome

Enabling or refreshing a provider no longer freezes the interface: the measurement (an engine
probe of 15–40 s on the owner's host, measured in P2's review) runs on a worker thread; the
provider row and card show a measuring state and keep repainting; keys are handled meanwhile; the
observation lands when the probe returns and the pages re-read the records. No new operator act,
no confirmation.

## Scope

### In

- `ymp-tui` Session: `measure_enabled_providers` / `set_provider_enabled` / `refresh_provider_models`
  spawn the probe on a worker and deliver its result through the existing event channel; a
  `measuring…` state on `/providers`, the provider card and `/runtimes` while it runs; the input
  thread's buffered keys are processed, not replayed after the wait; a second Enable/Refresh while
  one is running is coalesced, not queued (stated on the row).
- CLI commands stay synchronous (they are one-shot).
- Bounded shutdown: quitting during a measurement does not leave an engine process behind
  (W1-APP-02k rule) — the worker is joined or the child killed within the existing 5 s bound.

### Out

- Any change to what is measured or when (P2's rules stand: only on Enable/Refresh).

## Acceptance

- [ ] A deterministic test with a slow fake engine (sleep ≥ 2 s) shows the interface repainting
      and accepting a key during the measurement, and the observation appearing afterwards;
      negative half: on the current tree the same test observes no repaint until the probe ends.
- [ ] Quitting during a measurement leaves no engine process (probe by pid file of the fake).
- [ ] Screen tests for the measuring state at both sizes.

## Current state

Active.
