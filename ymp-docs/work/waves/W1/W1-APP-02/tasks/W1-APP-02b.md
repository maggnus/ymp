---
id: W1-APP-02b
kind: task
wave: W1
card: W1-APP-02
state: rework
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-APP-02a, W1-EXP-01a]
blocks: [W1-APP-02e]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T18:27:00+08:00
started_at: 2026-08-12T16:14:11+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/38a1f4a6b327132667faba2de23ef337f95a8a9b
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02b — Private attempt produces independently verified immutable candidate

## Outcome

A bounded attempt works in a private directory derived from an immutable source digest, submits a
content-addressed bundle, receives a mechanically constructed immutable candidate, and obtains
verifier evidence for that exact candidate and contract package.

## Scope

### In

- Source snapshots, attempt workspaces, path authority, submission bundles, object digests,
  staging integration, candidate ancestry, typed conflicts, and retention metadata.
- Same-executable verifier mode invoked by the outer study harness against a clean candidate copy,
  protected-query accounting, bounded diagnostics, and negative controls.
- Logical separation between producing attempt, control state, and protected oracle.

### Out

- Hostile-code containment implemented by ymp, semantic merge resolution, applying a candidate to
  the user's working tree, and crash continuation.

## Acceptance

- [ ] A fake attempt modifies only its private workspace, submits an exact-base bundle, and produces
  a candidate whose digest and ancestry reproduce from clean inputs.
- [ ] Re-running the approved verifier against the exact candidate, environment, contract, and
  oracle digests reproduces the recorded evidence within declared nondeterminism bounds.
- [ ] A stale base, out-of-scope path, altered object, partial object write, integration conflict,
  or mismatched digest is rejected without changing an existing candidate.
- [ ] If a protected negative control passes, verifier execution yields `infrastructure_error`
  rather than candidate acceptance.
- [ ] The producing attempt and collaboration board cannot write verifier state or read protected
  oracle bytes.

## Current state

The second Linux review returned the candidate: separate namespaces retained a shared mutable root,
allowing negative-control state to produce false acceptance; launcher failure was also misclassified
as oracle rejection. A third bounded correction round is authorized because this silent failure
cannot be accepted as residue. The same author, reviewer, and external test are retained.

## Next action

Give each verifier phase a private root filesystem containing only a read-only oracle, the private
subject copy, and private scratch storage. Distinguish launcher and `exec` failures from an oracle
exit code, then repeat the same external review with the preserved reviewer.

## Guardrails

- Directory separation is experimental integrity, not a POC host-security claim.
- The verifier consumes exact objects, never producer conversation or mutable workspace paths.
- Integration reports semantic conflicts and does not resolve them centrally.

## Findings

- Separate namespaces still expose a shared mutable root filesystem outside the private scratch
  mounts, which permits state transfer and a silent false acceptance.
- A namespace-launch failure with exit code 1 is indistinguishable from a genuine oracle rejection.
- Communication is limited to exact revisions, reproducible evidence, findings, and an
  evidence-based response without negotiating the verdict.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
