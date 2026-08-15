---
id: W1-COR-03n
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03m]
blocks: []
created_at: 2026-08-14T23:29:02+08:00
updated_at: 2026-08-15T00:03:05+08:00
started_at: 2026-08-14T23:29:49+08:00
accepted_at: 2026-08-15T00:03:05+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/bcfddbc2956c6bc8cdeffd19a9939518c776881c
closure_commit: https://github.com/maggnus/ymp/commit/b5a691b150930d12ad0e96c9b7783f44e2617c7c
evidence: ["[b5a691b](https://github.com/maggnus/ymp/commit/b5a691b150930d12ad0e96c9b7783f44e2617c7c)"]
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

- reviewer 48-run cancellation sweep (0-188 ms offsets): both accountings agree in every landing
  order, no deadlock, repeated cancels idempotent; negative half measured on the base by the same
  sweep (43/48 divergences); two pre-existing defects recorded as W1-COR-03o and W1-COR-03p
