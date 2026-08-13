---
id: W1-APP-02t
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02q]
blocks: []
created_at: 2026-08-13T21:46:07+08:00
updated_at: 2026-08-13T21:46:07+08:00
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

# W1-APP-02t — Lifecycle tests are isolated from other test binaries

## Outcome

The integration check gives the same answer every time it runs, because a lifecycle test observes
only the processes and files of its own run and cannot be disturbed by another test binary executing
beside it.

## Scope

### In

- The lifecycle refusal and termination tests, and whatever state they share with concurrently
  running test binaries: process observation, fixed paths, temporary directories, markers.

### Out

- The admission gate and the refusals themselves, which are accepted and behave correctly.

## Acceptance

- [ ] The full workspace suite passes ten consecutive times; the negative half reintroduces the
      shared state and shows the failure returning.
- [ ] A lifecycle test observes only processes and files belonging to its own run, proved by a check
      rather than by naming convention.

## Current state

Ready. The cancellation refusal test fails during the full workspace run and passes when its binary
runs alone, including under a twelve-thread external load measured by an independent reviewer. The
distinguishing condition is therefore concurrency between test binaries, not machine load: cargo runs
them in parallel, and the test observes state another binary also touches. Until this is closed the
integration gate cannot be trusted to mean what it says.

## Next action

Reproduce by running the suite repeatedly, then isolate what the test observes.

## Guardrails

- A gate that reports red at random teaches the fleet to ignore it, which is worse than having no
  gate at all.

## Findings

- Measured twice on a quiet machine: the same test fails in the full run and passes alone.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
