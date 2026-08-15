---
id: W1-COR-03y
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03d, W1-COR-03x]
blocks: []
created_at: 2026-08-15T19:35:00+08:00
updated_at: 2026-08-15T23:00:00+08:00
started_at: 2026-08-15T19:35:00+08:00
accepted_at: 2026-08-15T23:00:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/acdf9f82df02a65a7cf8d8612c17e43e5e91e804
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger: a caller of submitted() lands that carries parents forward without the pre-check extended to the effective change set
deliberate_partial: false


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

- [x] A product-started run's journal carries `bundle_recorded` and `candidate_formed` with the
      exact base and object digests; negative half: current tree carries only `submission_recorded`.
- [x] After the run's terminal, a creating command (advertise/wake offer) is refused before
      `StopRun` is committed; closing commands still land; the 03x probe sequence yields
      `offers after == before`.
- [x] `CommitmentService` is gone; the two suites pass on the durable path; the race test fails
      under its documented mutation.

## Current state

Accepted 2026-08-15 (candidate acdf9f8 after two residue passes; review ACCEPT WITH RESIDUE; merged at 1d2ae00).

## Findings

- Live path records object_recorded / bundle_recorded / candidate_formed with real digests; the
  bundle's publishability is decided before any object is recorded (no orphan facts).
- Terminal guard classifies all 24 commands (compiler-enforced); creating ones refused after the
  run journal is terminal; the 03x cancel window is closed and load-bearing (guard removal fails 3 tests).
- A construction the protocol cannot describe (>32 changes, unacceptable path) still reaches the
  verdict via the snapshot seal; the limitation is journaled as `candidate_provenance_unrecorded`
  (journal v5; v4 stores refused) and stated on `/commitments`. Child: multi-bundle or raised bound.
- Race falsifier reshaped: 26/30 with a scheduling point between decide and append, 0/30 back-to-back.
- CommitmentService retired; both protocol suites on the durable path; 03d identity-triple test added.
- Residue: pre-check exhaustive only while submitted() carries no parents; a cancel between
  submitted() and the provenance note yields a supervision-failed report (records still agree).
