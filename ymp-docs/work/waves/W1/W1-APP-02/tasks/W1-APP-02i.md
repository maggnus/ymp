---
id: W1-APP-02i
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: routine
maturity: BUILD
relation: follow_up
depends_on: [W1-APP-02c]
blocks: []
created_at: 2026-08-13T09:19:36+08:00
updated_at: 2026-08-14T02:02:00+08:00
started_at: 2026-08-14T01:40:23+08:00
accepted_at: 2026-08-14T02:02:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/0f6f46ba125b0f503fcfe25262cf53a25d78111e
closure_commit: https://github.com/maggnus/ymp/commit/e896738e3d8d5d4b51ffed4a19beebc6896a56ff
evidence: ["[e896738](https://github.com/maggnus/ymp/commit/e896738e3d8d5d4b51ffed4a19beebc6896a56ff)"]
duration_minutes: 60
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02i — Terminal accounting reports only counters the runtime actually sent

## Outcome

A finished managed run records exactly the usage counters the runtime reported. A terminal report
that carries no in-flight field leaves no in-flight value behind, so the matched-budget comparison
in W1-EVL-04 reads measured numbers instead of a residue of the turn-start message.

## Scope

### In

- The usage merge on terminal runtime events in `ymp-rust/crates/ymp-runtime-codex`.
- The durable statement of which counters are runtime-reported in `ymp-rust/SCHEMA.md`, if the
  correction changes what the schema promises.
- Targeted accounting tests, including the existing product-path test under
  `ymp-rust/crates/ymp-cli/tests`.

### Out

- `ymp-rust/crates/ymp-tui`, which holds an unintegrated chat-first implementation and would
  conflict.
- Any change to lifecycle authority, admission, or launch evidence accepted in W1-APP-02c.
- Charging, capping, or interpreting budgets; this task only stops recording an unreported value.

## Acceptance

- [ ] A terminal report without an in-flight field leaves the recorded in-flight excess empty.
      The negative half is the accepted base
      [`d56b199`](https://github.com/maggnus/ymp/commit/d56b199ed1c8c7e13f479cfcac9647fa4f5abd0b),
      where the same input records `model_requests=1`; capture that failing exit before the fix.
- [ ] A terminal report that carries the field preserves it exactly across success, error, cancel
      and timeout, proved by the accounting test accepted in W1-APP-02c.
- [ ] The schema statement and the runtime agree on which counters are runtime-reported, and the
      evidence names the ones the product invents from its own state.

## Current state (closed)

Accepted. The terminal merge treats a turn's accounting record as closing the excess: stated
counters replace the running ones, and a record without the field leaves nothing. SCHEMA.md now
names the single product-derived counter and the case that preserves it. The reviewer confirmed
the pair of tests pins the behaviour from both sides; the cross-family property was lost and
compensated with a fault-injection falsifier.

## Prior state

Ready. The defect was measured by the independent third review of W1-APP-02c: the turn-start message
sets `model_requests=1` and a terminal report without the ymp-specific field never clears it, so
every successful managed run records an in-flight excess the runtime never sent. A real Codex build
does not send that field at all.

## Next action

Reproduce the recorded value on the accepted base, then correct the terminal merge so an absent
field clears rather than preserves.

## Guardrails

- Authority over lifecycle and candidate submission stays where W1-APP-02c placed it.
- A counter the runtime did not send is either absent or explicitly marked as product-derived; it is
  never written as if the runtime reported it.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- builder negative halves on the accepted base d56b199 (exit 101 before, 0 after); reviewer ACCEPT
  with a fault-injection falsifier of a different shape; byte-identical porcelain
