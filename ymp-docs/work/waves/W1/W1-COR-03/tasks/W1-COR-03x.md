---
id: W1-COR-03x
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03f]
blocks: []
created_at: 2026-08-15T16:45:00+08:00
updated_at: 2026-08-15T19:30:00+08:00
started_at: 2026-08-15T16:45:00+08:00
accepted_at: 2026-08-15T19:30:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/eb0fdd34f3564eaf05c6a2e6b02b6d9eb3cbfc65
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger: the kernel serialisation changed again without a mutation that fails the race test; or a second production caller of execute_commitment lands without a terminal guard independent of StopRun
deliberate_partial: false


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

- [x] A product-started run journals its commitment facts and shows them on `/commitments`; the
      negative half (current tree) leaves no records and refuses the page.
- [x] Concurrent commands from two participants are serialised without lost or reordered facts
      (race test), and restart equivalence still holds.
- [x] R2 and R3 closed with a test each.

## Current state

Accepted 2026-08-15 (candidate eb0fdd3, review ACCEPT WITH RESIDUE, merged at 9cd989f).

## Findings

- The run's kernel now journals through `Application::execute_commitment` under the run journal's
  own lock; product-path test proves records + run facts + non-empty `/commitments`; fails on
  baseline. Lock discipline verified: no inversion, no await, bounded shutdown intact.
- Behaviour change (a) measured: a cancelled run's ledger can still take a wake offer with an
  escrow draw until `StopRun` — bounded (no surface total moves, escrow settles back), not a
  regression versus the in-memory ledger.
- R1 (residue): the race test's stated falsifier does not fire at a microsecond window (0/30);
  it fires at 5 ms (27/30). Stronger shape without new API: contend N threads on one aggregate.
- R2 (residue): `execute_commitment` has no terminal guard above the ledger's `stopped` flag; the
  first non-kernel production caller (e.g. `submit_bundle` from 03d's child) must add one.
- Child: retire `CommitmentService` (no production caller) — sequence after the 03d merge since
  both touch `invocation_wakes.rs`.
