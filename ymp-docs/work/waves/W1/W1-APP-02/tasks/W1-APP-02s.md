---
id: W1-APP-02s
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: routine
maturity: BUILD
relation: follow_up
depends_on: [W1-APP-02r]
blocks: []
created_at: 2026-08-13T21:21:27+08:00
updated_at: 2026-08-15T14:45:38+08:00
started_at: 2026-08-15T14:23:59+08:00
accepted_at: 2026-08-15T14:45:38+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/fa3ca2c7575b46d6b53c5ecb437f107eefc3385e
closure_commit: https://github.com/maggnus/ymp/commit/69506e74db422fbd7eca2a82881bf3fcd80e0c5b
evidence: ["[69506e7](https://github.com/maggnus/ymp/commit/69506e74db422fbd7eca2a82881bf3fcd80e0c5b)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: an unknown runtime name flows into contract assembly and is refused as a missing acceptance condition instead of "no such profile" (surface.rs:321) — fix when the runtime line is next touched
deliberate_partial: true
---

# W1-APP-02s — The fake runtime is neither linked nor offered as a profile

## Outcome

The shipped binary does not link the fake runtime, and the operator is not offered it among the
runtime profiles, so a test double cannot appear as a product capability.

## Scope

### In

- The profile list the terminal probes and displays.
- The dependency edge that keeps the fake runtime in the binary through the terminal crate.

### Out

- The admission gate and the holder lookup, both accepted.

## Acceptance

- [ ] The dependency graph of the shipped binary does not reach the fake runtime; the negative half
      restores the edge and the check rejects it.
- [ ] The runtimes page offers only profiles the product can actually start, and the change to what
      the operator sees is rendered and shown before acceptance.

## Current state

Ready. The command crate no longer reaches the test kit, but the fake runtime stays linked through
the terminal crate, whose probe list constructs it in production code. That crate was outside the
write zone of the card that found this.

## Next action

Remove the fake profile from the probe list, then prove the binary no longer links it.

## Guardrails

- A test double that reaches the operator's screen is a product claim nobody made.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- cargo tree shows the fixture runtime reachable only from the testkit and dev sections; /runtimes
  renders two engines instead of three on the built product; attempt --runtime fake refuses; both
  mutations reproduced; the unknown-profile-name wording queued as a residual
