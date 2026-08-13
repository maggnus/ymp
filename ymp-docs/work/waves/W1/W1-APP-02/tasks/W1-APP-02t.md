---
id: W1-APP-02t
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02q]
blocks: []
created_at: 2026-08-13T21:46:07+08:00
updated_at: 2026-08-13T21:46:07+08:00
started_at: 2026-08-13T21:33:00+08:00
accepted_at: 2026-08-14T01:16:07+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/47fdf00e2c76498119d3ef40572f8fc244a07ba3
closure_commit: https://github.com/maggnus/ymp/commit/51a7ed719535e34baed43f04d148629ffa1452de
evidence: [`47fdf00`](https://github.com/maggnus/ymp/commit/47fdf00e2c76498119d3ef40572f8fc244a07ba3)
duration_minutes: 258
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

- [x] The full workspace suite passes ten consecutive times; the negative half reintroduces the
      shared state and shows the failure returning.
- [x] A lifecycle test observes only processes and files belonging to its own run, proved by a check
      rather than by naming convention.

## Current state

Accepted and integrated. The failure came from a race inside one test, not from concurrency between
binaries, and survival is now read against a two-second deadline instead of at one instant. The
reviewer restored the old cleanup and the check failed ten times out of ten, passed ten times without
it, and the full workspace suite ran clean at the accepted revision.

## Next action

Reproduce by running the suite repeatedly, then isolate what the test observes.

## Guardrails

- A gate that reports red at random teaches the fleet to ignore it, which is worse than having no
  gate at all.

## Findings

- The card's premise is refuted by direct measurement: sampling the process table every two hundred
  milliseconds across a full run, 1477 samples, showed exactly one test binary at a time, and the
  failure reproduced on the parent revision without any neighbouring binary.
- My own diagnosis of machine load is likewise refuted; neither load nor parallelism distinguished
  the failing runs.
- The cause was the test's session record declaring one built-in capability where the profile
  requires ten, so the driver rejected it and cancellation only sometimes arrived first, together
  with a cleanup that searched the whole process table by a command fragment.
- The first correction left a check that could not fail: liveness was read once, immediately after
  the signal, and a killed but unreaped process reads as alive. Reading against a deadline makes the
  same mutation fail ten times out of ten.

## Closure

Filled when the task is accepted.

### Accepted outcome

The integration gate answers the same way every time it runs: ten consecutive full-workspace runs
pass, and each lifecycle assertion decides on an observed event within a bounded window rather than
at a single instant.

### Residuals

None. Behaviour on another platform and under a different machine load is unmeasured, which the
review records as unverified rather than as a carried limitation.

### Evidence

- [`47fdf00`](https://github.com/maggnus/ymp/commit/47fdf00e2c76498119d3ef40572f8fc244a07ba3) —
  reviewed correction.
- [`51a7ed7`](https://github.com/maggnus/ymp/commit/51a7ed719535e34baed43f04d148629ffa1452de) — integration into the release branch.
