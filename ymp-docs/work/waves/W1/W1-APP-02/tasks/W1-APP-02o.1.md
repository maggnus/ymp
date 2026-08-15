---
id: W1-APP-02o.1
kind: subtask
wave: W1
card: W1-APP-02
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02o]
blocks: []
created_at: 2026-08-15T14:21:36+08:00
updated_at: 2026-08-15T14:39:47+08:00
started_at: 2026-08-15T14:23:59+08:00
accepted_at: 2026-08-15T14:39:47+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/2844771
closure_commit: https://github.com/maggnus/ymp/commit/dbda61d
evidence: the retired structural guard was measured weaker (a post-marker start passes it, the accepted guard names root.rs:47); the schema half kept verbatim; the acceptance line was wider than the scope — the remaining truncating check lives in unattested_runtime_is_unreachable.rs and is queued as W1-APP-02o.2
duration_minutes: 0
blocker:
pause_reason:
return_trigger: a tools/ crate gains a dependency on ymp-application (its sources are then outside every guard)
deliberate_partial: true
---

# W1-APP-02o.1 — The old start guard is retired or subordinated

## Outcome

ymp-application/tests/one_start_path.rs — which still truncates at the first #[cfg(test)] and
excludes ymp-testkit by declaration — either becomes a thin caller of the item-parsing guard for
crates outside the shipped closure, or is retired with the reason recorded; the glob-reexport
boundary wording in the new guard is tightened to what is measured.

## Scope

### In

- ymp-application/tests/one_start_path.rs; the boundary section of ymp-cli/tests/one_command_path.rs.

### Out

- The accepted guard semantics of W1-APP-02o.

## Acceptance

- [ ] No guard in the workspace truncates at a test marker or excludes a crate by declaration;
      the negative half is the current old guard.

## Current state

Ready. Recorded from the W1-APP-02o reviews.

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
