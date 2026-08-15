---
id: W1-PRD-05f
kind: task
wave: W1
card: W1-PRD-05
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05d, W1-PRD-05e]
blocks: []
created_at: 2026-08-15T23:05:00+08:00
updated_at: 2026-08-15T23:05:00+08:00
started_at: 2026-08-15T23:05:00+08:00
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

# W1-PRD-05f — /pools surfaces and the reconciler wired to the enable transition (P4)

## Outcome

Migration unit P4: the AgentPool reconciler accepted in P3 runs on the product path — after every
provider observation (enable, refresh, disable) the pools are reconciled, so a fresh root gets its
`default` pool the moment the first provider is ready; the operator can see pools as a table
(`/pools`) and a properties view (declared list or tracking, capacity, resolved entries with
admissibility and reason, digest, tracking/explicit); editing `default` (explicit ordered list,
`maxAgents`, concurrency) is a key on the selected row with the consequence stated above it and no
confirmation; every key is a mirrored command.

## Scope

### In

- `Session` (ymp-tui) calls `Pools::reconcile` after `observe`; CLI `provider enable|refresh|disable`
  do the same; `/pools` table + properties view; `ymp show pools|pool --pool <name>`;
  `ymp pool set-models|set-capacity <name> …` mirrored commands; screen tests 80×24/120×40.
- The reply when a goal is typed with a ready provider but (impossibly) no pool: names the state
  plainly (should never occur once reconcile is wired — assert it in a test).

### Out

- Freeze (P5), any run start, recruitment. No typed identifiers: pool selection is a table row;
  the explicit model list is edited by toggling rows on the catalog table, not by typing names.

## Acceptance

- [ ] On a fresh root: `provider enable` → `pools/default.json` exists with resolved entries equal
      to the catalog; `provider disable` of the only provider → default degraded/empty, never deleted;
      negative half: without the wiring the root has no pool after enable (test fails on the current tree).
- [ ] `/pools` and the pool properties view render deterministically at both sizes; toggling an
      entry off in the properties view turns tracking into an explicit list and the record says so.
- [ ] Every key is a command (command_surface correspondence test passes).

## Current state

Active. Built on the P2 candidate branch (72b288b) since both touch the observation path.
