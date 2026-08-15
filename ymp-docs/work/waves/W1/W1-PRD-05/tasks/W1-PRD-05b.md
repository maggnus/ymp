---
id: W1-PRD-05b
kind: task
wave: W1
card: W1-PRD-05
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05a, W1-APP-02e.6, W1-APP-02w.1]
blocks: []
created_at: 2026-08-15T15:07:23+08:00
updated_at: 2026-08-15T16:12:00+08:00
started_at: 2026-08-15T15:07:23+08:00
accepted_at: 2026-08-15T16:12:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/09fc9c455ced9b6572b4bbbeca778b762ba6bf83
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-PRD-05b — The product root carries provider and catalog records (P1)

## Outcome

Migration unit P1 of [COLLECTIVE-MIGRATION.md](https://github.com/maggnus/ymp/blob/ffde816625bc6a98536e486c2f5ccc0f6445b6d5/ymp-docs/design/COLLECTIVE-MIGRATION.md): the
product root gains provider records and a model catalog as first-class stored objects, and engines
become managed entities beneath providers — the registry accepted in W1-APP-02e.6 is the seed of
this level, not a parallel one. No participant is created from a catalog entry by this unit; it
only makes providers, engines and models addressable and measured. Independent of the open owner
decisions D1–D11: it stores what exists, not what may be used.

## Scope

### In

- Provider records (connected/authenticated state, family, credential origin) under the product
  root beside the engine registry; the model catalog as the union of measured engine catalogs with
  provider attribution; the read surface the later /providers and /models views will consume.
- Design item 6 (model catalog and availability) and item 17 (provider/model/runtime/participant
  distinctions) of COLLECTIVE-DESIGN.md.

### Out

- Pool freezing (P3, waits on D1), derivation (P4), the run launcher (P7), any TUI surface (P2).

## Acceptance

- [x] Providers and their models are readable from the root as stored, measured records with the
      distinction provider / model / engine / driver preserved; the negative half is the current
      state where only engine records exist and provider is implicit.
- [x] Nothing here instantiates a participant or changes admission.

## Current state

Accepted 2026-08-15 (candidate 09fc9c4, review ACCEPT WITH RESIDUE, merged fast-forward into main).

## Next action

None recorded.

## Guardrails

None recorded.

## Findings

- Review residue 1: `Catalog::read` re-reads engine records but takes admissibility from the stored
  provider state, which carries no observation time. Return trigger: P2 must re-observe before
  reading or state the age of the observation.
- Review residue 2: a deleted engine record is answered by the seeded record, so its models leave
  the catalog silently; an emptied `routes` array reads as a provider serving nothing. Return
  trigger: P2 — a route with no models must read as such, not as a shortened catalog.
- Pre-existing, not this task's: `engine_registry::the_mirrored_command_disables_and_enables_an_engine_durably`
  asserts `1 ready · 2 unusable`, unreachable since the fixture profile left the page (74cf451 →
  later); fails on baseline. `lib.rs:309` links a removed `Registry::addressing` (doc warning).
  Both left for the next editor of those files.

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

None recorded.
