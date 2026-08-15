---
id: W1-APP-02z.3
kind: subtask
wave: W1
card: W1-APP-02
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02z]
blocks: []
created_at: 2026-08-15T01:25:33+08:00
updated_at: 2026-08-15T11:26:50+08:00
started_at: 2026-08-15T11:26:50+08:00
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

# W1-APP-02z.3 — A testless project receives a generated verifier proposal for approval

## Outcome

A request in a project with no test entry point yields a draft whose verifier is generated from the
request itself — a self-contained program shown in the draft, demonstrated against the fresh
negative control, digest-pinned like any oracle — and it gains force only through the operator's
explicit approval. The refusal remains only for requests the product cannot honestly derive a
check from, and it says so.

## Scope

### In

- The assembled-draft path for projects where detect_test_entry_point finds nothing.
- Showing the generated program's text in the draft statement, so approval is informed.

### Out

- Silently applying a generated check without approval, which stays forbidden.
- Delegating verifiers and their pinning, accepted in W1-APP-02z.

## Acceptance

- [ ] The owner's measured scenario (an empty directory, "generate empty html file") reaches a
      one-statement draft with a generated verifier that rejects the fresh copy and accepts a
      candidate carrying the file; the negative half is the current refusal demanding a
      hand-written verifier.
- [ ] The generated program is digest-pinned in the contract and shown in the draft before
      authorization.
- [ ] A request whose checkable part is a produced artifact (e.g. "create an image") yields a
      check of the mechanical claim — the file exists and decodes as a valid image with non-zero
      dimensions — and the draft states plainly that the semantic part stays with the operator;
      the check never pretends to judge meaning.

## Current state

Ready. Recorded from the owner's comparison with Claude Code (2026-08-15): generation is the default
source of the proposal everywhere — detected project tests are merely the strongest generator —
and the operator's single action remains approval, never authorship. The checkable mechanical
claim is derived from the request; the semantic remainder is named as the operator's own.

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
