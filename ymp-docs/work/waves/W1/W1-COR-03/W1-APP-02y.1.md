---
id: W1-APP-02y.1
kind: subtask
wave: W1
card: W1-APP-02
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02y]
blocks: []
created_at: 2026-08-15T08:05:36+08:00
updated_at: 2026-08-15T11:13:20+08:00
started_at: 2026-08-15T10:35:25+08:00
accepted_at: 2026-08-15T11:13:20+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/00ab47ec8e8484a98f28050caf38345bc940b26c
closure_commit: https://github.com/maggnus/ymp/commit/1e5a03e
evidence: both assertions restated from the runtime last settled statement (20/20 under load vs 20/20 base failures); mutations on both drivers still break; second-look drain gap closed by a disclosed CTO fix rerun twice (42 s each, both green); the deterministic-timeout-arm idea recorded as a low-value residual
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

- The exact-equality assertions in ymp-rust/crates/ymp-cli/tests/codex_product_path.rs and the timeout arm of durable_claude_accounting in claude_product_path.rs (reproduced identically on base 620147f by two reviews).

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
