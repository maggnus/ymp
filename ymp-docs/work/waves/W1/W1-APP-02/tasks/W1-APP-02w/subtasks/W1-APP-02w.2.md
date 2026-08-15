---
id: W1-APP-02w.2
kind: subtask
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02w]
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

# W1-APP-02w.2 — A run identifier names the run, not only its contract

## Outcome

Re-authorizing the same contract after a finished run yields a run distinguishable from the first:
either the identifier gains a per-run component, or the record explicitly states that identity is
contract-scoped and the layout ordinal is the distinguisher — decided, implemented and pinned by a
test. The negative half is the measured pair: runs/0001 and runs/0002 both holding
run-fb7950580423 (W1-APP-02e re-review finding).

## Scope

### In

- The run identifier derivation from the contract and every surface that shows or records it.

### Out

- The store layout, accepted in W1-APP-02w.

## Acceptance

- [ ] Two authorizations of one contract yield distinguishable run records, proved by a test with
      the measured negative half.

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
