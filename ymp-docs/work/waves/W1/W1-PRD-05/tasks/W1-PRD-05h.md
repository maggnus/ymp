---
id: W1-PRD-05h
kind: task
wave: W1
card: W1-PRD-05
state: active
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-PRD-05e, W1-PRD-05f, W1-COR-03f]
blocks: []
created_at: 2026-08-15T22:15:00+08:00
updated_at: 2026-08-15T22:15:00+08:00
started_at: 2026-08-15T22:15:00+08:00
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

# W1-PRD-05h — The run-scoped pool freeze: `PoolFrozen` committed at run creation (P5)

## Outcome

Migration unit P5: when a run is created its pool is frozen **by value** — a kernel fact
`PoolFrozen { entries, entry_model, digest }` is committed to the run's journal at creation, holding
every permitted entry (provider · engine · model, admissibility and measured reason as the resolver
stated it) in declared order, the digest of the ordered set, and the origin entry (decision D2: the
first admissible entry in declared order — recorded, never reconstructed). Every later kernel
decision that names an entry reads that fact and never the live pool: changing a provider or a
pool after the run started cannot change what the run may do; the next run gets the updated pool.
The operator never creates the snapshot and never sees it unless they open the evidence.

## Scope

### In

- Domain fact `PoolFrozen` (journal additive → version bump per schema policy), committed by the
  run creation path in `ymp-application` from the `default` pool's resolved half (or the pool the
  task names, when P4's declared list exists); refusal when no pool has an admissible entry (plain
  words: names `/providers`); the origin entry per D2 in the fact.
- The kernel's containment check reads the frozen fact: a later `request_participant` / entry
  naming outside the frozen set is refused with `EntryNotPermitted` — the check itself is P10's,
  but the fact and a domain-level containment predicate over it are this unit's.
- Evidence: the run's evidence bundle / `/commitments` (or the run's page) names the snapshot digest
  and origin entry.

### Out

- Registering/starting the origin participant (P9), recruitment (P10), the run launcher process side.

## Acceptance

- [ ] Creating a run on a root with a ready provider commits `PoolFrozen` with entries == the pool's
      resolved entries in order, digest == the pool's digest, origin == first admissible; the
      negative half: on the current tree run creation commits no such fact.
- [ ] After creation, disabling the provider / editing the pool changes the live pool's digest but
      NOT the run's frozen fact (re-read from the journal); a second run created afterwards freezes
      the new digest.
- [ ] A root whose pools have no admissible entry refuses run creation with the plain-words state.

## Current state

Active. Built on the P4 candidate branch (23f5710) since it reads the pool the P4 wiring creates.
