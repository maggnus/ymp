---
id: W1-APP-02j
kind: task
wave: W1
card: W1-APP-02
state: deferred
risk: routine
maturity: BUILD
relation: follow_up
depends_on: [W1-APP-02c]
blocks: []
created_at: 2026-08-13T09:19:36+08:00
updated_at: 2026-08-13T09:24:00+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason: measured cost is tolerable for the POC comparison and the work is pulled by the trigger below
return_trigger: an operator-visible start exceeds two seconds, or one run admits more than two driver instances, or temporary-directory space becomes a constraint
deliberate_partial: false
---

# W1-APP-02j — Runtime admission stops duplicating the pinned executable per instance

## Outcome

Admitting a pinned runtime costs a bounded amount of time and temporary space regardless of how
large the executable is, while the property W1-APP-02c established — the bytes that execute are the
bytes that were measured — continues to hold.

## Scope

### In

- The admission and launch path in `ymp-rust/crates/ymp-runtime-codex` that materialises a private
  copy from the opened source file.
- A measurement of start cost and temporary-space use with the real pinned executable.

### Out

- Weakening admission: a source path replaced after measurement must still never execute.
- `ymp-rust/crates/ymp-tui` and the terminal presentation of start latency.

## Acceptance

- [ ] Start cost and temporary-space use with the pinned 220 MB executable are measured before and
      after the change, and the result meets a stated bound instead of an impression.
- [ ] Replacing the source file between measurement and launch still fails, proved by the existing
      substitution check with its captured non-zero exit.

## Current state

Deferred. The independent third review of W1-APP-02c measured about 0.74 seconds and 220 MB of
temporary space per driver instance, paid twice in one terminal session because starting the product
and starting a run each admit an instance. Fixtures of about one kilobyte hid this entirely.

## Next action

None until the return trigger fires. The measurement above is the starting evidence.

## Guardrails

- Any cheaper admission must still bind the executed bytes to the measured bytes; an identity check
  that trusts a path is not a substitute.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
