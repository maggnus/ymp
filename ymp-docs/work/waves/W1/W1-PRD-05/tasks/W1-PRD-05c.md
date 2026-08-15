---
id: W1-PRD-05c
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: critical
maturity: DESIGN
relation: required
depends_on: [W1-PRD-05a]
blocks: []
created_at: 2026-08-15T16:12:00+08:00
updated_at: 2026-08-15T18:55:00+08:00
started_at: 2026-08-15T16:12:00+08:00
accepted_at: 2026-08-15T18:55:00+08:00
candidate_commit: 6dec28e
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-PRD-05c — Design correction under product brief v2 (TUI ownership + CRD resource model)

## Outcome

The collective design deliverable ([COLLECTIVE-GAP-ANALYSIS](../../../../design/COLLECTIVE-GAP-ANALYSIS.md),
[COLLECTIVE-DESIGN](../../../../design/COLLECTIVE-DESIGN.md), [COLLECTIVE-TUI](../../../../design/COLLECTIVE-TUI.md),
[COLLECTIVE-MIGRATION](../../../../design/COLLECTIVE-MIGRATION.md),
[COLLECTIVE-OWNER-DECISIONS](../../../../design/COLLECTIVE-OWNER-DECISIONS.md)) is corrected against
[PRODUCT-BRIEF-collective-v2.md](../../../../design/PRODUCT-BRIEF-collective-v2.md), delivering the
sixteen items of Part A §24 and the per-resource `spec`/`status`/ownership/lifecycle/reconciliation
definitions of Part B — without removing contract, oracle, verification, budget, isolation or any
other protective mechanism, and without introducing a hidden central semantic orchestrator.

## Scope

### In

- Repository inspection for ownership leaks (Part A §19) with, per leak: what stays, who owns it,
  the API/state transition, the TUI change, the tests.
- Target product mental model, operator lifecycle from startup to Result, TUI designed from the
  lifecycle (Part A §21) in k9s style (list → select → properties → action).
- Resource model: Provider, Model, AgentPool, Task, Run, Collective, Agent, Candidate,
  Verification, Result — each with spec/status, references, mutable vs immutable fields,
  lifecycle and terminal states, reconciliation responsibility, events and conditions (Part B).
- Dynamic recruitment, automatic `default` AgentPool, run-scoped pool snapshot/freeze,
  internalised contract/oracle/verification ownership, domain/state/API and Rust crate changes,
  migration path (rebased P1–P15), end-to-end tests, and the list of decisions that genuinely
  need a human — carried into COLLECTIVE-OWNER-DECISIONS as open blocks (D2–D11 revisited under v2).

### Out

- Any implementation. Any removal of safety mechanisms. Any TUI text not derivable from the brief.

## Acceptance

- [x] Every item of Part A §24 (1–16) is answered in the design set, and every resource of Part B
      has its spec/status/ownership/lifecycle/reconciliation stated or explicitly marked open.
- [x] The acceptance scenario of Part A §22 is walked screen by screen with no operator-created
      contract/oracle/verifier/team/agent/role/model assignment/decomposition; the negative half is
      the current design set, which still asks the operator for the permitted set at request time.
- [x] No hidden manager/planner: every semantic decision in the design is attributed to the
      collective, every mechanical check to the kernel.

## Current state

Accepted 2026-08-15 (candidate 6dec28e after a residue pass, review ACCEPT WITH RESIDUE → residues
closed on the branch; merged into main at 2a3531c). D11 decided in the same pass (no `minAgents`
floor). Design set: COLLECTIVE-RESOURCES.md (new), GAP-ANALYSIS (16 leaks), DESIGN, TUI (37
surfaces), MIGRATION (P1–P17), OWNER-DECISIONS (D1–D11 decided).

## Findings

- Review walked §22 surface by surface: operator acts are launch, Enable on provider rows, one
  goal sentence, one option key on a genuine clarification; no identifiers typed, no confirmations.
- file:line citations into sources were sampled (35), one wrong location corrected
  (`protocol.rs:627-633`); the remaining citations were not re-verified line by line — the next
  edit of those sources should re-check them.
