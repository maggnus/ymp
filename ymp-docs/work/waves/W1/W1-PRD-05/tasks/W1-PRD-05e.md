---
id: W1-PRD-05e
kind: task
wave: W1
card: W1-PRD-05
state: active
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-PRD-05b, W1-PRD-05c]
blocks: []
created_at: 2026-08-15T18:55:00+08:00
updated_at: 2026-08-15T18:55:00+08:00
started_at: 2026-08-15T18:55:00+08:00
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

# W1-PRD-05e — The AgentPool resource, its reconciler and the automatic `default` (P3)

## Outcome

Migration unit P3: `AgentPool` exists as a stored resource under the product root with
`spec` (selector or explicit ordered list of catalog entries, `maxAgents`, concurrency limit,
resource limits — no `minAgents`, no `desired`, no roles: decisions D1, D11) and `status`
(resolved entries, digest, conditions), per
[COLLECTIVE-RESOURCES.md](../../../../design/COLLECTIVE-RESOURCES.md). A reconciler creates the
`default` pool automatically on the first ready provider (tracking the catalog) and keeps its
resolved entries and digest current; editing `default` turns tracking into an explicit list and
says so. Nothing here instantiates a participant.

## Scope

### In

- Pool record layout (SCHEMA additive), the reconciler as a mechanical controller (may never read
  a goal or rank entries), `default` creation and tracking, resolution against the P1 catalog,
  digest of resolved entries; read surface for `/pools` (the list/properties views themselves are
  P4 unless trivially included).

### Out

- The run-scoped freeze (P5), recruitment (P10), any UI beyond a read surface, any semantic choice.

## Acceptance

- [ ] A root with one enabled, ready provider holds a `default` pool whose resolved entries equal
      the catalog and whose digest changes when the catalog changes; a root with no ready provider
      holds no pool (negative half); no participant is created.
- [ ] `spec` fields are operator-owned and `status` fields controller-owned; a mutation that lets
      the reconciler write `spec` (other than creating `default`) or the operator write `status`
      fails a test; there is no `minAgents`, `desired`, or role field.
- [ ] Editing `default` records the explicit list and stops tracking, stated in `status`.

## Current state

Active.
