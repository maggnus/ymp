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
updated_at: 2026-08-12T17:35:00+08:00
started_at: 2026-08-12T16:14:11+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/08c18338ad36a94a5c49b1036f96e1896343d744
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

The independent Linux review returned the candidate for bounded rework. It reproduced state
transfer from the negative control to the candidate through shared namespaces, accepted candidate
identity changes through a different base or excluded path, and observed a non-idempotent repeated
verification of the same digest.

## Next action

Separate the two verifier phases at the operating-system boundary, freeze full candidate identity,
and return the stored verification result on a repeated digest. Continue the same external review
directly with the preserved reviewer.

## Guardrails

- Directory separation is experimental integrity, not a POC host-security claim.
- The verifier consumes exact objects, never producer conversation or mutable workspace paths.
- Integration reports semantic conflicts and does not resolve them centrally.

## Findings

- Shared Linux mount, PID, and network namespaces permit state transfer between verifier phases.
- Candidate identity does not yet reject a different base or an excluded-path modification.
- Repeating verification for one digest changes the event history and consumes the attempt budget.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
