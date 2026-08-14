---
id: W1-COR-03n
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03m]
blocks: []
created_at: 2026-08-14T23:29:02+08:00
updated_at: 2026-08-14T23:29:02+08:00
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

# W1-COR-03n — Cancelling a finished run reaches the kernel, and the two accountings agree

## Outcome

A cancellation issued after the runtime has completed reaches the kernel as its transition, so the
journal and the kernel name the same terminal: a run the journal records as Cancelled cannot later
be driven to Accepted by a verdict, and no state exists where the two accountings disagree.

## Scope

### In

- The cancel path of ymp-runtime-supervisor (lib.rs:402-416 at a2ddeb2), which today mutates only
  the journal after runtime completion.

### Out

- The verification-fact semantics accepted in W1-COR-03m.

## Acceptance

- [ ] Cancel after runtime completion drives the kernel terminal; a subsequent verdict is refused;
      the negative half is the measured divergence on the base: journal Cancelled, kernel None,
      then Accepted.

## Current state

Ready. Recorded from the W1-COR-03m review (major independent defect).

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
