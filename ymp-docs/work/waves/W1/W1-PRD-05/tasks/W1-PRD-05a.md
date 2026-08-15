---
id: W1-PRD-05a
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: critical
maturity: DESIGN
relation: required
depends_on: []
blocks: []
created_at: 2026-08-15T02:07:10+08:00
updated_at: 2026-08-15T07:47:03+08:00
started_at: 2026-08-15T02:07:10+08:00
accepted_at: 2026-08-15T07:47:03+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/0897ecf9c7392e29af359fdcfd5c38cfc86713d1
closure_commit: https://github.com/maggnus/ymp/commit/decd7ea26b4640467f6861d595a6f81a92f81100
evidence: ["[decd7ea](https://github.com/maggnus/ymp/commit/decd7ea26b4640467f6861d595a6f81a92f81100)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: implementing P4a, P5 or P8 returns the four recorded residuals (the unresolved-divergence terminal, the policy-table fields, the bootstrap step circularity wording, the divergent-set ambiguity)
deliberate_partial: true
---

# W1-PRD-05a — The collective design deliverable

## Outcome

A complete design document set in `ymp-docs/design/` answering all 24 deliverable items of
[PRODUCT-BRIEF-collective.md](https://github.com/maggnus/ymp/blob/7f3730f65171fe2290194f814bddeac0d7f9f3a8/ymp-docs/design/PRODUCT-BRIEF-collective.md): product mental
model, operator lifecycle, full TUI information architecture (the 30 surfaces, each with what the
operator sees, available actions and their effects), provider/catalog/pool/recruitment design,
internal ownership of contracts/oracles/verifiers with provenance classes A-E, exact
operator/collective/kernel/verifier/TUI boundaries, required domain/API/state and crate changes,
tests and acceptance scenarios, a migration path from the current design, and the honest list of
decisions only the owner can make. Grounded in the actual repository; sound constraints
preserved; ownership moved instead of mechanisms removed.

## Scope

### In

- `ymp-docs/design/**` (the deliverable documents); reading everything else.
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

## Next action

None recorded.

## Guardrails

None recorded.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

Four re-review findings, all passing the reversibility and detection tests, carried into
implementation: (1) major — an unresolved divergence has no deadline, default or terminal of its
own, so an unattended run drifts to exhausted with the wrong named cause; the design must gain an
abstained-with-published-divergence terminal or a default resolution (also raised to the owner as
an eleventh decision); (2) the policy table of item 7 misses the derivation-permission fields and
still says set membership with a contract-frozen pool; (3) bootstrap steps 2-3 are circular as
worded; (4) "no verification query against a diverged requirement set" reads two ways.

### Evidence

- one return round; the re-review walked the 24-step final test without operator machinery, verified
  the retraction of the nothing-spent claim in every place, the kernel purity via admission facts,
  the NVIDIA model-route reading against ARCHITECTURE.md and PROTOCOL.md, and adversarial
  answer-classification probes
