---
id: W1-EVL-04a
kind: task
wave: W1
card: W1-EVL-04
state: deferred
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01a, W1-EXP-01b, W1-EXP-01e, W1-COR-03b, W1-COR-03d, W1-COR-03z, W1-PRD-05j.1, W1-EVL-04e, W1-EVL-04f]
blocks: [W1-EVL-04b, W1-EVL-04c]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-09-01T13:42:00+08:00
started_at: 2026-09-01T12:18:20+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason: the frozen primary remains untouched while held-out weak diagnostic and strong-profile transfer gates are built and decided
return_trigger: W1-EXP-01e, W1-EVL-04e, W1-EVL-04f and W1-PRD-05j.1 accepted; W1-COR-03z is already accepted
deliberate_partial: false
---

# W1-EVL-04a — Matched-budget arms produce comparable acceptance evidence

## Outcome

The strong-single-participant, independent best-of-`n`, and locally coordinated conditions run on
the frozen corpus under the same declared resource and protected-query budget and produce
comparable acceptance, false-acceptance, cost, latency, failure, and abstention records.

## Scope

### In

- Preregistered task and seed assignment, repetition, runtime-profile balancing, total model-route
  cost, wall time, resource vector, protected-query accounting, blinded selector, verifier, human
  audit sample, exclusion handling, and uncertainty by task stratum.
- Externally disposable study environments with curated inputs and experiment-specific provider
  accounts or quotas.

### Out

- Protocol tuning after primary results, ordinary-machine execution, production data, external
  delivery, unregistered exclusions, and substitution of a weaker single-agent baseline.
- L1–L3 calibration, weak-profile development diagnostics, strong-profile transfer gating, and any
  reuse of their observations as primary samples.

## Acceptance

- [ ] Every admitted observation is traceable to a frozen task, seed, arm, runtime profile, budget,
      contract package, candidate digest, verifier result, and audit disposition.
- [ ] Arm-level reports include accepted-result rate, audited false acceptance, actual provider
      cost, wall time, verifier queries, infrastructure failure, and abstention with uncertainty by
      decomposability stratum.
- [ ] A run with budget mismatch, unapproved profile, unverifiable usage, oracle-integrity loss,
      missing seed assignment, or undeclared exclusion is rejected from the primary comparison by
      the compliance check.
- [ ] Any in-flight excess is charged to the arm that incurred it and included in the declared
      accounting and statistical analysis. If an optional project-wide monetary budget is configured,
      admission of new work also respects its remaining balance.
- [ ] The selector commits its primary assessment before receiving arm identity, producer
      rationale, messages, reputation, or other assessments. A deliberately early disclosure makes
      the compliance check reject the observation from the primary comparison.

## Current state

The frozen primary study manifest
([d12eff3](https://github.com/maggnus/ymp/commit/d12eff31940f8ad124f5f8f566a0dd21de3d2ae6))
binds arms, corpus, seeds, and the 0.125 minimum useful effect. No primary seed, paid model call, or
experimental quota has been consumed. Development diagnostics and transfer now have separate tasks
and cannot close this primary outcome.

## Next action

After the return trigger, execute only the frozen primary arms and compliance analysis; do not
reimplement or import development diagnostic observations.

## Guardrails

- A passing candidate proves only the approved oracle observations for its exact digest.
- Infrastructure failures are reported separately from candidate failures.
- Only openly distributable, nonsensitive external inputs may be disclosed to model providers;
  every paid expansion is recorded in the run and aggregate statistics.

## Findings

- Scientific plan audit removed weak-diagnostic implementation and the TUI prerequisite from this
  task; W1-EVL-04e/f and W1-PRD-05j.1 now own the actual prerequisites.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
