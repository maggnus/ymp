---
id: W1-PRD-05h
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-PRD-05e, W1-PRD-05f, W1-COR-03f]
blocks: []
created_at: 2026-08-15T22:15:00+08:00
updated_at: 2026-08-16T01:35:00+08:00
started_at: 2026-08-15T22:15:00+08:00
accepted_at: 2026-08-16T01:35:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/bde5d9aaf052c65b942c32f4f49ad2321aad11ba
closure_commit: https://github.com/maggnus/ymp/commit/54ccbeeb7d998aa387befcd010692edf7db3ee24
evidence: ["[bde5d9a](https://github.com/maggnus/ymp/commit/bde5d9aaf052c65b942c32f4f49ad2321aad11ba)"]
duration_minutes: 200
blocker:
pause_reason:
return_trigger:
deliberate_partial: true
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

## Current state

Accepted. Reviewer verdict ACCEPT (Critical depth, independent out-of-process falsifier on the
built binary; cross-family property lost — compensated by a different-shape falsifier). Candidate
range 23f5710..bde5d9a merged into main as 54ccbee; post-merge ymp-domain/ymp-application/ymp-testkit
22 result blocks green, 0 failed.

## Next action

Carry the two minor findings as residuals below; P9 (origin participant start) builds on the frozen
fact.

## Guardrails

- Kernel decisions read the frozen fact, never the live pool.
- The freeze is internal provenance; the operator is never asked to create or name it.

## Findings

None blocking. Review findings recorded as residuals.

## Closure

### Accepted outcome

PoolFrozen journal fact (schema v6) committed at run creation from the resolved pool; origin entry
per D2 recorded; run pages name the snapshot digest; second authorization freezes the updated pool;
emptied pool refuses run creation with plain words. Verified by an independently selected
out-of-process falsifier against digest/order/liveness oracles; mutation of the ignition rule
fails exactly one check.

### Residuals

1. (minor, outcome-defect) SCHEMA.md v6 claims records with a foreign origin are rejected at read,
   but the pool.rs L198 check is called only from unit tests; an injected record with recomputed
   digests opens and names a forged ignition. Write path never produces it. Return trigger: any
   work that makes journal records externally loadable (import/recovery) must close the read-side
   rejection first.
2. (minor, independent-defect) Intermittent ymp-agent-rpc::invocation_bound_yield_replays_one_authoritative_confirmation
   failure predates this change; crate byte-identical to main. Return trigger: its own card when
   reproduced on a clean tree.

### Evidence

- [bde5d9a](https://github.com/maggnus/ymp/commit/bde5d9aaf052c65b942c32f4f49ad2321aad11ba) — candidate head, reviewed range 23f5710..bde5d9a
[54ccbee](https://github.com/maggnus/ymp/commit/54ccbeeb7d998aa387befcd010692edf7db3ee24) — integration merge into main
- Post-merge composition check: `cargo test -p ymp-domain -p ymp-application -p ymp-testkit` — 22 blocks ok, 0 failed (run by CTO on the merged tree)
