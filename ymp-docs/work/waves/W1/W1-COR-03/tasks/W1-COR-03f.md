---
id: W1-COR-03f
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03a]
blocks: []
created_at: 2026-08-13T16:02:47+08:00
updated_at: 2026-08-15T16:40:00+08:00
started_at: 2026-08-15T15:07:23+08:00
accepted_at: 2026-08-15T16:40:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/8f5b81551599c0ef61edf80e9379369dfd8b2120
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger: W1-COR-03 reaching closure without the child that moves the supervisor ledger onto Application::execute_commitment, or that child landing while a product-started run still shows no commitments page
deliberate_partial: false


# W1-COR-03f — Commitment facts are durable and visible, not only in memory

## Outcome

The facts a commitment produces — contract, escrow transfer, lease, obligation, return and terminal
outcome — survive a restart and appear on the operator's screen, so a run's commitments can be
audited from the record rather than from a live process.

## Scope

### In

- Journalling the commitment facts with their own durable records, and the schema version that
  requires.
- The terminal projection that shows commitments and obligations.

### Out

- The commitment rules themselves, which W1-COR-03a owns, and any change to who decides what.

## Acceptance

- [x] Every fact the commitment kernel produces is written to the journal before the command result
      returns; the negative half removes one write and a restart then loses that fact, with a
      captured non-zero exit.
- [x] A restart reconstructs the registry from the journal alone, with no live state carried over.
- [x] The operator sees commitments and obligations on a page whose every value derives from an
      application projection, and the existing framework and state-binding checks still hold.

## Current state

Accepted 2026-08-15 (candidate 8f5b815, review ACCEPT WITH RESIDUE, merged into main at 3f9e604).

## Findings

- R1 (residue): the durable path (`open_commitment_kernel`, `execute_commitment`) has no production
  caller — the run's kernel is still the in-memory `CommitmentService` in the supervisor, so a
  product-started run journals no commitment facts and `ymp show commitments` on a real store says
  the store has no such page. Child: move the supervisor's ledger onto the durable path (needs its
  own serialisation — `execute_commitment` takes `&mut self`).
- R2 (minor, ymp-tui journal.rs:250-262): after a replay refusal the next `commitment_facts_recorded`
  overwrites the recorded reason with "no kernel was opened".
- R3 (minor, ymp-application lib.rs:656): a second `commitment_kernel_opened` record is ignored in
  the fold instead of refused.
- Journal version 3 is an explicit refusal of version-2 stores (`IncompatibleStore`), no migration:
  an existing `~/.ymp` store needs a new data root.
- Pre-existing, environmental: `engine_registry::the_mirrored_command_disables_and_enables_an_engine_durably`
  fails on baseline (seeded codex disabled → 0 ready where the test expects 1).
