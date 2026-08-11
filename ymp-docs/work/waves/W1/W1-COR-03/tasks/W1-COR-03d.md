---
id: W1-COR-03d
kind: task
wave: W1
card: W1-COR-03
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03a, W1-COR-03b, W1-APP-02b]
blocks: [W1-EVL-04a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-10T19:34:25+08:00
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

# W1-COR-03d — Competing submissions preserve immutable candidate ancestry

## Outcome

Concurrent participants can submit competing bundles and sponsor explicit synthesis without
sharing a writable repository or letting an expired, conflicting, or stale result overwrite an
existing candidate.

## Scope

### In

- Multi-parent candidate references, competing branches, stale-base evidence, typed integration
  conflicts, participant-sponsored rebase or synthesis tasks, fencing checks, and provenance from
  obligations through bundles to exact candidates.
- Race tests for simultaneous submissions, expiry, retry, integration failure, and synthesis.

### Out

- A mutable canonical branch during a run, shared writable Git metadata, automatic semantic merge
  resolution, or kernel selection of the preferred candidate.

## Acceptance

- [ ] Two concurrent attempts from one base produce distinct immutable bundles and candidate
  ancestry without modifying either workspace or candidate.
- [ ] A stale fencing generation, stale base, conflicting patch, repeated submission, or partial
  object cannot replace or mutate a current candidate.
- [ ] A semantic conflict creates typed evidence and becomes participant-sponsored work; the
  integrator does not choose a resolution or winning branch.
- [ ] A synthesized candidate records every contributing candidate, obligation, participant, base,
  and exact object digest required to reproduce its construction.
- [ ] Deliberately removing the stale-token or immutability check produces a failing race or digest
  test.

## Current state

The single-attempt candidate path is planned, but no multi-participant ancestry or synthesis path
exists. This task follows the accepted contract, lifecycle, and candidate foundations.

## Next action

Extend the candidate graph with competing submissions while keeping semantic synthesis outside the
integrator.

## Guardrails

- Candidate identity never depends on a workspace path, PID, branch name, or database row.
- Mechanical integration checks bytes and authority, not technical quality.
- Applying a result to the user's working tree remains a separate operator action outside POC
  acceptance.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
