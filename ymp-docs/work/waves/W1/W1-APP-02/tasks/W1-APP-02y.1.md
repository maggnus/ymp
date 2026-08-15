---
id: W1-APP-02y.1
kind: subtask
wave: W1
card: W1-APP-02
state: ready
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02y]
blocks: []
created_at: 2026-08-15T08:05:36+08:00
updated_at: 2026-08-15T08:05:36+08:00
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

# W1-APP-02y.1 — The accounting assertion states what accounting guarantees

## Outcome

The durable-accounting test asserts a property the accounting actually guarantees instead of an
exact in-flight count that depends on scheduling: under load the suite stays stable. The negative
half is the measured flake (in_flight_excess.model_requests 1 vs expected 2 on a loaded first run,
green on three isolated repeats — W1-APP-02e re-review finding at codex_product_path.rs:487).

## Scope

### In

- The exact-equality assertion in ymp-rust/crates/ymp-cli/tests/codex_product_path.rs.

### Out

- The accounting semantics, accepted in W1-APP-02i and W1-APP-02y.

## Acceptance

- [ ] The assertion holds under a loaded parallel run and still fails on the mutation that drops
      in-flight tracking.

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
