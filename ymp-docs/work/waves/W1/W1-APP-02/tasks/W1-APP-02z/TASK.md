---
id: W1-APP-02z
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-APP-02v]
blocks: []
created_at: 2026-08-14T03:44:51+08:00
updated_at: 2026-08-15T00:05:22+08:00
started_at: 2026-08-14T05:07:51+08:00
accepted_at: 2026-08-15T00:05:22+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/2e9570ab0526802bac8a269cf39eddfc354a9769
closure_commit: https://github.com/maggnus/ymp/commit/c364e3014ee3bb6248d31d80d367f6cb7b466391
evidence: ["[c364e30](https://github.com/maggnus/ymp/commit/c364e3014ee3bb6248d31d80d367f6cb7b466391)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: the internal verify-managed-candidate command still checks the program against a digest computed at verification time, not the contract oracle_digest; wiring verification through the public surface (W1-APP-02e) or reviving the internal path returns this residual
deliberate_partial: true
---

# W1-APP-02z — A verifier cannot be rewritten by the candidate it judges

## Outcome

The acceptance condition of a contract is immutable relative to the candidate: a verifier that
delegates to a file inside the candidate (the project's own test entry point) is pinned to the
bytes that existed when the contract was approved, so a candidate that replaces its own test
script with "exit 0" is rejected, not accepted.

## Scope

### In

- The generated verifier wrapper of the assembled contract draft (W1-APP-02v), which currently
  executes the candidate's own scripts/test entry.
- The oracle digest semantics for delegating verifiers.

### Out

- Verifiers the operator supplies as self-contained programs, already digest-pinned at approval.

## Acceptance

- [ ] A candidate that replaces its own test entry point with an always-accepting program is
      rejected by the pinned oracle; the negative half is the current build, where such a
      candidate was measured accepted (exit 0).

## Current state

Ready. Recorded from the W1-APP-02v review: the proposed verifier ran the candidate's own
./scripts/test.sh, so an empty candidate carrying "exit 0" as its test script was accepted.
Threatens INV-5 (only the verifier creates acceptance evidence).

## Next action

None recorded.

## Guardrails

None recorded.

## Findings

### Rounds

2

### Convergence

Split. The script and make entry points are proved by two independent shields and land; the
package.json entry point is removed from the proposal and becomes its own node, because the
candidate chooses the program that runs the pinned file (.npmrc script-shell — a measured silent
false accept) and because under the executor environment the generated npm verifier accepts
nothing. A silent false accept fails the detection test, so residue was not available.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- two independent shields (explicit -f invocation and read-before names refusal) measured
  separately, on a case-insensitive and a case-sensitive volume; the hostile GNUmakefile and .npmrc
  script-shell falsifiers drove two return rounds; the npm branch left the proposal as a typed
  distinction and the split children carry it
