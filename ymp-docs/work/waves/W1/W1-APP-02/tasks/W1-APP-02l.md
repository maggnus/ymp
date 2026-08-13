---
id: W1-APP-02l
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: routine
maturity: BUILD
relation: follow_up
depends_on: [W1-APP-02d]
blocks: [W1-EVL-04a]
created_at: 2026-08-13T12:09:18+08:00
updated_at: 2026-08-13T12:09:18+08:00
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

# W1-APP-02l — Runtime evidence names the models that spent and the environment it passed

## Outcome

A finished managed run records which models consumed the recorded cost and which environment the
child actually received, and its live acceptance check distinguishes a product failure from a model
that simply declined to act.

## Scope

### In

- Usage parsing in `ymp-rust/crates/ymp-runtime-claude`, including the per-model breakdown the
  provider already sends.
- The environment allow list passed to a managed child and recorded in its profile record.
- Determinism of the live acceptance check in `ymp-rust/crates/ymp-cli/tests`.

### Out

- `ymp-rust/crates/ymp-tui`, and any change to the accepted lifecycle model.

## Acceptance

- [ ] The run record attributes the recorded cost to the models that produced it; the negative half
      is a fixture whose per-model breakdown disagrees with the total and is rejected.
- [ ] Only the allow-listed environment reaches the child and its profile record; the operator's full
      search path does not. The negative half injects an unexpected variable and fails.
- [ ] The live check fails for a product defect and reports a distinct, non-failing outcome when the
      model declines to act, so a red result always means the product.

## Current state

Ready. The independent review of the Claude Code profile found that only the total cost is read
while the provider's per-model breakdown is discarded, that the operator's full search path reaches
the child and its profile record, and that the live acceptance check cannot distinguish a product
failure from a model declining to submit.

## Next action

Parse the per-model breakdown, narrow the environment, then make the live check deterministic.

## Guardrails

- The matched-budget comparison reads these records; a number without its attribution is not
  evidence of a route.

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
