---
id: W1-APP-02b
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-APP-02a, W1-EXP-01a]
blocks: [W1-APP-02e]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T19:02:23+08:00
started_at: 2026-08-12T16:14:11+08:00
accepted_at: 2026-08-12T19:02:23+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/1a88a1017bdf28347a2ce5c72c4c6be8afd04520
closure_commit: https://github.com/maggnus/ymp/commit/5571a07f5ca8f1ffcf515a73fc24401dce361ecd
evidence: ["[1a88a10](https://github.com/maggnus/ymp/commit/1a88a1017bdf28347a2ce5c72c4c6be8afd04520)", "[5571a07](https://github.com/maggnus/ymp/commit/5571a07f5ca8f1ffcf515a73fc24401dce361ecd)"]
duration_minutes: 168
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

- [x] A fake attempt modifies only its private workspace, submits an exact-base bundle, and produces
  a candidate whose digest and ancestry reproduce from clean inputs.
- [x] Re-running the approved verifier against the exact candidate, environment, contract, and
  oracle digests reproduces the recorded evidence within declared nondeterminism bounds.
- [x] A stale base, out-of-scope path, altered object, partial object write, integration conflict,
  or mismatched digest is rejected without changing an existing candidate.
- [x] If a protected negative control passes, verifier execution yields `infrastructure_error`
  rather than candidate acceptance.
- [x] The producing attempt and collaboration board cannot write verifier state or read protected
  oracle bytes.

## Current state

Accepted. Verification phases use separate root filesystems, immutable oracle copies, private
candidate data, and typed launcher failures. Repeated verification returns the stored result, while
stale ancestry, excluded-path changes, corrupted objects, and positive negative controls fail
without replacing the candidate.

## Next action

Proceed with exact-result recovery in W1-APP-02a.1 and the public foreground CLI boundary in
W1-APP-02a.2.

## Guardrails

- Directory separation is experimental integrity, not a POC host-security claim.
- The verifier consumes exact objects, never producer conversation or mutable workspace paths.
- Integration reports semantic conflicts and does not resolve them centrally.

## Findings

None. The prior shared-root and launcher-classification findings were corrected and independently
rechecked through the preserved external Linux scenarios. The handoff audit found no attempt to
negotiate the verdict, conceal findings, or weaken the independent check.

## Closure

### Accepted outcome

The production candidate path now binds the exact source base and content digest, rejects excluded
or conflicting changes without replacing an existing candidate, and records verification only from
separate immutable observations. An external product-path review confirmed that state cannot pass
between phases through the outer root, namespace-launch failure becomes `infrastructure_error`, a
genuine oracle rejection remains distinct, and exact replay does not change the journal or budget.

### Residuals

None.

### Evidence

- [Reviewed candidate](https://github.com/maggnus/ymp/commit/1a88a1017bdf28347a2ce5c72c4c6be8afd04520).
- [Byte-equivalent integration](https://github.com/maggnus/ymp/commit/5571a07f5ca8f1ffcf515a73fc24401dce361ecd).
