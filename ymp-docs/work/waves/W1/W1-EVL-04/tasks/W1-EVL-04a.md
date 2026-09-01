---
id: W1-EVL-04a
kind: task
wave: W1
card: W1-EVL-04
state: deferred
risk: critical
maturity: RESEARCH
relation: required
depends_on: [W1-EXP-01a, W1-EXP-01b, W1-COR-03b, W1-COR-03d]
blocks: [W1-EVL-04b, W1-EVL-04c]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-09-01T12:22:00+08:00
started_at: 2026-09-01T12:18:20+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason: weak-diagnostic-v1 is defined; executable comparison awaits the multi-participant collaboration path
return_trigger: W1-COR-03z and W1-COR-03e.2 accepted, with a two-participant run reachable through the product
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
      missing seed assignment, or undeclared exclusion is rejected from the primary comparison by
      the compliance check.
- [ ] Any in-flight excess is charged to the arm that incurred it and included in the declared
      accounting and statistical analysis. If an optional project-wide monetary budget is configured,
      admission of new work also respects its remaining balance.
- [ ] The selector commits its primary assessment before receiving arm identity, producer
      rationale, messages, reputation, or other assessments. A deliberately early disclosure makes
      the compliance check reject the observation from the primary comparison.

## Current state

The Sol max research result is recorded in
[WEAK_DIAGNOSTIC.md](../../../../../WEAK_DIAGNOSTIC.md). No primary seed, paid model call, or
experimental quota was consumed. The protocol is executable only after the product can run two
participants with collaboration tools. The frozen study manifest
([d12eff3](https://github.com/maggnus/ymp/commit/d12eff31940f8ad124f5f8f566a0dd21de3d2ae6))
binds arms, corpus, seeds, and the 0.125 minimum useful effect.

## Next action

After the return trigger, assign Sol to implement the diagnostic manifest, scheduler, blinded
best-of-2 selector, and compliance record. Prove them first with deterministic fake runtimes in a
fresh disposable evaluation root before any model call or primary seed is used.

## Guardrails

- A passing candidate proves only the approved oracle observations for its exact digest.
- Infrastructure failures are reported separately from candidate failures.
- Only openly distributable, nonsensitive external inputs may be disclosed to model providers;
  every paid expansion is recorded in the run and aggregate statistics.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
