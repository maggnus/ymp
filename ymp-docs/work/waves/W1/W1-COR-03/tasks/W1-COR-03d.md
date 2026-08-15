---
id: W1-COR-03d
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-COR-03a, W1-COR-03b, W1-APP-02b]
blocks: [W1-EVL-04a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-15T18:40:00+08:00
started_at: 2026-08-15T16:45:00+08:00
accepted_at: 2026-08-15T18:40:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/f4e9fbf89e869e2505ba642784a145d598f66fd3
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger: a test separating two constructions differing in exactly one of participant/contract_id/generation fails when that field leaves CandidateRecord::identify; verification coverage in the generated pool back to ≥4 of 192 seeds
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

- [x] Two concurrent attempts from one base produce distinct immutable bundles and candidate
  ancestry without modifying either workspace or candidate.
- [x] A stale fencing generation, stale base, conflicting patch, repeated submission, or partial
  object cannot replace or mutate a current candidate.
- [x] A semantic conflict creates typed evidence and becomes participant-sponsored work; the
  integrator does not choose a resolution or winning branch.
- [x] A synthesized candidate records every contributing candidate, obligation, participant, base,
  and exact object digest required to reproduce its construction.
- [x] Deliberately removing the stale-token or immutability check produces a failing race or digest
  test.

## Current state

Accepted 2026-08-15 (candidate f4e9fbf, review ACCEPT WITH RESIDUE, merged into main).

## Next action

Extend the candidate graph with competing submissions while keeping semantic synthesis outside the
integrator.

## Guardrails

- Candidate identity never depends on a workspace path, PID, branch name, or database row.
- Mechanical integration checks bytes and authority, not technical quality.
- Applying a result to the user's working tree remains a separate operator action outside POC
  acceptance.

## Findings

- Candidate identity = digest of the construction (base, per-path objects, declaring bundle,
  contributing candidates with obligations and participants); verified independent of source path,
  branch, HOME, cwd and process; order-independent on the committed fact set.
- Kernel commands record_object / submit_bundle / record_conflict; no ranking or preference path;
  one contract seals one candidate. Journal version 4 (v3 stores refused explicitly).
- R1 (residue): dropping participant, contract_id or generation from `CandidateRecord::identify`
  leaves the domain suite green — the triple is correct but unproven individually.
- R2 (residue): generated-schedule verification coverage fell from 4 seeds to 2 after the pool
  split (resume coverage stayed at the single pre-existing seed 145 — fragile, deserves its own node).
- Proposed child: move the live controller path (`ymp-runtime-supervisor::submitted`) from opaque
  `SubmitResult` onto `submit_bundle` so provenance reaches the journal on the product path.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
