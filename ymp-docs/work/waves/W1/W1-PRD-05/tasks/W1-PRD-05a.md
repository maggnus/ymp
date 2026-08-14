---
id: W1-PRD-05a
kind: task
wave: W1
card: W1-PRD-05
state: active
risk: critical
maturity: DESIGN
relation: required
depends_on: []
blocks: []
created_at: 2026-08-15T02:07:10+08:00
updated_at: 2026-08-15T02:07:10+08:00
started_at: 2026-08-15T02:07:10+08:00
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

# W1-PRD-05a — The collective design deliverable

## Outcome

A complete design document set in ymp-docs/design/ answering all 24 deliverable items of
[PRODUCT-BRIEF-collective.md](../../../../design/PRODUCT-BRIEF-collective.md): product mental
model, operator lifecycle, full TUI information architecture (the 30 surfaces, each with what the
operator sees, available actions and their effects), provider/catalog/pool/recruitment design,
internal ownership of contracts/oracles/verifiers with provenance classes A-E, exact
operator/collective/kernel/verifier/TUI boundaries, required domain/API/state and crate changes,
tests and acceptance scenarios, a migration path from the current design, and the honest list of
decisions only the owner can make. Grounded in the actual repository; sound constraints
preserved; ownership moved instead of mechanisms removed.

## Scope

### In

- ymp-docs/design/** (the deliverable documents); reading everything else.
- Mapping every accepted W1 node that the redesign preserves, re-scopes or supersedes
  (explicitly: the contract dialogue of 02v/02u/02z becomes internal derivation; the engine
  registry 02e.6 becomes the provider/catalog level; the generated verifier 02z.3 becomes the
  internal acceptance-plan derivation; ~/.ymp of 02w.1 is the state home).

### Out

- Any code change; any weakening of kernel or verification principles.

## Acceptance

- [ ] Every brief deliverable item 1-24 has a section; every acceptance criterion 17.1-17.16 is
      satisfied by the design or named as an owner decision; the final test scenario (18) is
      walked step by step through the designed surfaces with no operator-authored machinery.
- [ ] The migration path names which existing nodes/cards survive, change owner, or close.

## Current state

Active. The brief is recorded verbatim at design/PRODUCT-BRIEF-collective.md (23f76f9).

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
