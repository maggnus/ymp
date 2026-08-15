---
id: W1-PRD-05h
kind: task
wave: W1
card: W1-PRD-05
state: review
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

- [x] Creating a run on a root with a ready provider commits `PoolFrozen` with entries == the pool's
      resolved entries in order, digest == the pool's digest, origin == first admissible; the
      negative half: on the current tree run creation commits no such fact.
- [x] After creation, disabling the provider / editing the pool changes the live pool's digest but
      NOT the run's frozen fact (re-read from the journal); a second run created afterwards freezes
      the new digest.
- [x] A root whose pools have no admissible entry refuses run creation with the plain-words state.
- [x] The same path driven on the built executable.

## Current state

Built; in review.

The fact is a **run-journal record** rather than a commitment-ledger fact, and the reason is the
zone of this unit. A ledger fact can only be committed into a ledger, and this build opens a run's
commitment kernel when its first attempt starts — opening one at creation means naming the root
participant and the root obligation, which is P9. The journal is the durable record that exists when
a run is created, and it is the record a ledger fact would have been written into in any case: a
commitment command reaches disk as one journal record. What P10 needs is therefore carried without
being decided twice — `FrozenPool` is a domain value with the containment predicate on it, so when
P9 moves the kernel genesis to run creation the ledger adopts that same value and the containment
check reads it there, with no second source and no re-decision.

The record spells the ignition entry `origin` rather than `entry_model`: it names a
provider · engine · model triple rather than a model, and `Run.declared.entryModel` of
`COLLECTIVE-RESOURCES.md` is a field of the Run resource rather than of this fact. A rename is one
line if the owner prefers the resource's word.
