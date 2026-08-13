---
id: W1-APP-02t
kind: task
wave: W1
card: W1-APP-02
state: review
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02q]
blocks: []
created_at: 2026-08-13T21:46:07+08:00
updated_at: 2026-08-13T21:46:07+08:00
started_at: 2026-08-13T21:33:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/5118b7c59b18c80e381c6af7a6bbc8d13ecc7485
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

Under independent review. The card's premise is refuted: cargo runs these binaries sequentially and
shares no state between them. The measured cause is a race inside one test, whose session record the
profile could not accept, together with a cleanup that searched the whole process table. Four of ten
runs failed before the change and none after.

## Next action

Reproduce by running the suite repeatedly, then isolate what the test observes.

## Guardrails

- A gate that reports red at random teaches the fleet to ignore it, which is worse than having no
  gate at all.

## Findings

- The contract's assumption of concurrency between test binaries is refuted by measurement. My
  diagnosis of machine load, recorded earlier, is likewise refuted: neither load nor parallelism
  distinguished the failing runs.
- The failure came from the test racing its own driver over a session record the profile could not
  accept, and from a cleanup addressing processes by a command fragment rather than by the run's own
  process group.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
