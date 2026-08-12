---
id: W1-EVL-04a
kind: task
wave: W1
card: W1-EVL-04
state: blocked
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01a, W1-EXP-01b, W1-COR-03b, W1-COR-03d]
blocks: [W1-EVL-04b, W1-EVL-04c]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-12T09:29:40+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker: Owner gate G3 and listed dependencies
pause_reason:
return_trigger:
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

## Acceptance

- [ ] Every admitted observation is traceable to a frozen task, seed, arm, runtime profile, budget,
  contract package, candidate digest, verifier result, and audit disposition.
- [ ] Arm-level reports include accepted-result rate, audited false acceptance, actual provider
  cost, wall time, verifier queries, infrastructure failure, and abstention with uncertainty by
  decomposability stratum.
- [ ] A run with budget mismatch, unapproved profile, unverifiable usage, oracle-integrity loss,
  missing seed assignment, or undeclared exclusion is rejected from the primary comparison by the
  compliance check.
- [ ] Any approved in-flight overshoot is charged to the arm that incurred it and remains within
  the tolerance fixed by `G3` and the preregistration.
- [ ] The selector commits its primary assessment before receiving arm identity, producer
  rationale, messages, reputation, or other assessments. A deliberately early disclosure makes
  the compliance check reject the observation from the primary comparison.

## Current state

No corpus, executable system, study harness, provider budget, or experimental record exists. Work
is blocked by `G3` and every preceding required checkpoint.

## Next action

After all dependencies close, execute a small compliance dry run before consuming the primary
comparison budget.

## Guardrails

- A passing candidate proves only the approved oracle observations for its exact digest.
- Infrastructure failures are reported separately from candidate failures.
- No paid expansion or new data disclosure occurs without a new owner decision and run record.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
