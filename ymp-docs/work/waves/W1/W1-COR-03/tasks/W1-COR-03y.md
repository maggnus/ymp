---
id: W1-COR-03y
kind: task
wave: W1
card: W1-COR-03
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03d, W1-COR-03x]
blocks: []
created_at: 2026-08-15T19:35:00+08:00
updated_at: 2026-08-15T19:35:00+08:00
started_at: 2026-08-15T19:35:00+08:00
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

# W1-COR-03y — The live controller submits bundles; the durable path guards its terminal

## Outcome

Closes the children proposed by W1-COR-03d and W1-COR-03x and residue R2 of 03x: the supervisor's
live path (`ymp-runtime-supervisor::submitted`) moves from opaque `SubmitResult` onto
`submit_bundle`, so a product-started run journals candidate provenance (base, objects, bundle,
contributors) and not only a seal; `execute_commitment` refuses creating commands on a run whose
journal has ended, independently of whether `StopRun` was committed; `CommitmentService` (no
production caller) is retired and its two protocol suites (`tests/commitments.rs`,
`tests/invocation_wakes.rs`) run through `Application::execute_commitment`.

## Scope

### In

- Supervisor `submitted` → `record_object` + `submit_bundle` with the run's real base and objects;
  the product-path test of 03x extended to assert `bundle_recorded` / `candidate_formed` facts.
- Terminal guard in `execute_commitment` (creating commands refused on a terminal run; closing
  commands still admitted — the cancel window measured in 03x's review must close for offers).
- Retiring `CommitmentService`; the two suites moved.
- Race falsifier of 03x's R1: reshape the race test to contend N threads on one aggregate so its
  stated mutation actually fails (or state precisely why not).

### Out

- Any change to who decides what; TUI; the pool/collective work (P3+).

## Acceptance

- [ ] A product-started run's journal carries `bundle_recorded` and `candidate_formed` with the
      exact base and object digests; negative half: current tree carries only `submission_recorded`.
- [ ] After the run's terminal, a creating command (advertise/wake offer) is refused before
      `StopRun` is committed; closing commands still land; the 03x probe sequence yields
      `offers after == before`.
- [ ] `CommitmentService` is gone; the two suites pass on the durable path; the race test fails
      under its documented mutation.

## Current state

Active.
