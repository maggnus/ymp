---
id: W1-COR-03x
kind: task
wave: W1
card: W1-COR-03
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03f]
blocks: []
created_at: 2026-08-15T16:45:00+08:00
updated_at: 2026-08-15T16:45:00+08:00
started_at: 2026-08-15T16:45:00+08:00
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

# W1-COR-03x — The run's commitment kernel journals through the durable path

## Outcome

The supervisor's in-memory `CommitmentService` is replaced as the run's kernel by the durable
application path accepted in W1-COR-03f (`open_commitment_kernel` / `execute_commitment`), so a run
started through the product journals its commitment facts and `ymp show commitments` on that run's
store shows the ledger. Closes residue R1 of W1-COR-03f.

## Scope

### In

- `ymp-runtime-supervisor` kernel construction (kernel.rs:148 today) onto the application's durable
  ledger, with the serialisation the durable path lacks (`execute_commitment` takes `&mut self`;
  the current `CommitmentService` mutex needs a counterpart) — decided on a copy, journaled, then
  answered, under concurrent participants.
- One product-path test: a run started through the CLI/TUI product surface leaves
  `commitment_kernel_opened` and `commitment_facts_recorded` records in its journal and its
  `commitments` page renders non-empty; the negative half is the current tree, where the same run
  leaves no such records and the page refuses.
- Fixing R2 (ymp-tui journal.rs:250-262 — the replay-refusal reason is overwritten) and R3
  (ymp-application lib.rs:656 — a second `commitment_kernel_opened` is ignored, must be refused).

### Out

- Any change to who decides what in the commitment protocol; TUI redesign under brief v2 (W1-PRD-05c).

## Acceptance

- [ ] A product-started run journals its commitment facts and shows them on `/commitments`; the
      negative half (current tree) leaves no records and refuses the page.
- [ ] Concurrent commands from two participants are serialised without lost or reordered facts
      (race test), and restart equivalence still holds.
- [ ] R2 and R3 closed with a test each.

## Current state

Active.
