---
id: W1-PRD-05b
kind: task
wave: W1
card: W1-PRD-05
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-PRD-05a, W1-APP-02e.6, W1-APP-02w.1]
blocks: []
created_at: 2026-08-15T15:07:23+08:00
updated_at: 2026-08-15T15:07:23+08:00
started_at: 2026-08-15T15:07:23+08:00
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

# W1-PRD-05b — The product root carries provider and catalog records (P1)

## Outcome

Migration unit P1 of [COLLECTIVE-MIGRATION.md](../../../../design/COLLECTIVE-MIGRATION.md): the
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

- [ ] Providers and their models are readable from the root as stored, measured records with the
      distinction provider / model / engine / driver preserved; the negative half is the current
      state where only engine records exist and provider is implicit.
- [ ] Nothing here instantiates a participant or changes admission.

## Current state

Active. Dispatched under the design's stated assumptions while D1–D11 are walked one at a time.

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
