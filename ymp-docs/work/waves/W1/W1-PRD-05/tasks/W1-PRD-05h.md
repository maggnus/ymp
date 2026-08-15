---
id: W1-PRD-05h
kind: task
wave: W1
card: W1-PRD-05
state: review
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-PRD-05f]
blocks: []
created_at: 2026-08-15T23:55:00+08:00
updated_at: 2026-08-15T23:55:00+08:00
started_at: 2026-08-15T23:55:00+08:00
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

# W1-PRD-05h — The run-scoped pool freeze

## Outcome

Creating a run fixes what it may create participants from. The pool the root holds is copied into
the run's own journal by value — every permitted entry in the pool's declared order, the entry the
run ignites on, and the pool's own digest — and every later decision about recruitment reads that
copy. A pool edited, a provider disabled or a model discovered after a run was created belongs to
the next run and changes nothing about the one already standing. A root that can offer a run nothing
creates none, and what the operator reads is the held state naming `/providers` rather than a
technical refusal.

## Scope

### In

- Domain: the `pool_frozen` journal record and the `FrozenPool` value it carries; the ignition rule
  of decision D2 decided in the domain from position and measured readiness alone; the containment
  predicate `FrozenPool::permits` that P10 is held to; the refusal a pool with nothing live
  produces. Journal schema version 6, its `SCHEMA.md` section, and the refusal of every superseded
  version.
- Application: `create_with_contract` takes the frozen pool as an argument, so no path creates a run
  without one, and commits the freeze before it returns; `freeze_under` reads one product root's
  pools and produces the value or the plain-words refusal.
- Surfaces: the interface and the internal commands freeze before they create; the run's events page
  and its commitment page state the digest and the ignition entry in one line.

### Out

- Participant registration and the start of the origin participant (P9).
- Recruitment and the containment check that reads this fact (P10). The predicate is built and
  tested here; nothing calls it yet.
- Any change to what a pool is, how it is resolved or how it is edited (P3, P4).

## Acceptance

- [x] A run created on a root with one ready provider carries a `pool_frozen` record whose entries
      are the pool's resolved entries in order, whose digest is the pool's own, and whose origin is
      the first entry admission found live.
- [x] `provider disable` and a pool edit after the creation move the live pool's digest and leave
      the record re-read from the run's journal exactly as it was; a run created afterwards freezes
      the new digest.
- [x] A root whose pools offer nothing creates no run and states the held state naming
      `/providers`; no journal is written.
- [x] The same path through the built executable.
- [x] The negative half of the record: with the freeze removed from run creation, the journal of a
      run created through the product carries no such record.

## Current state

Built; in review. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` are green on the merge of this branch with the accepted P4 fix.
