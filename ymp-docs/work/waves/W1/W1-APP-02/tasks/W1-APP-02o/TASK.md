---
id: W1-APP-02o
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02m]
blocks: []
created_at: 2026-08-13T14:47:29+08:00
updated_at: 2026-08-15T14:21:36+08:00
started_at: 2026-08-15T13:24:11+08:00
accepted_at: 2026-08-15T14:21:36+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/29c86c5be8442094f3361accd0c5e137feead2dd
closure_commit: https://github.com/maggnus/ymp/commit/96e041b
evidence: one return round; the re-review drove twenty-two mutations — every start form, cfg shape and alias route it could construct was refused, only the stated macro/include boundaries pass; the old one_start_path guard stays for crates outside the shipped closure and its retirement is a separate node
duration_minutes: 200
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02o — The shipped binary carries no path that starts a run without a contract

## Outcome

The executable a user runs contains no route that begins a run outside the single approved-contract
scenario, and the check that enforces this covers the whole shipped dependency set rather than the
crates that happened to be listed.

## Scope

### In

- The test kit's place in the dependency graph of the shipped binary, and any other crate that can
  begin a run.
- The structural check that forbids starting a run outside the scenario, including what it currently
  excludes by declaration.

### Out

- The scenario itself and the contract record, which W1-APP-02m owns.

## Acceptance

- [ ] No crate reachable from the shipped binary can start a run without an approved contract. The
      negative half reintroduces such a path and the check rejects it with a captured non-zero exit.
- [ ] The check names no exclusion by declaration; an excluded crate is either unreachable from the
      binary, proved from the dependency graph, or it is covered.
- [ ] The check parses source elements instead of truncating a file at its first test marker, and it
      recognises a call made through an alias. The negative half is the reviewer's pair of mutations:
      a start placed after the first test marker and an aliased call; both must be rejected with a
      captured non-zero exit.

## Current state

Ready. The typed-request card is accepted, and its independent review measured that the structural
guard is weaker than its name: it truncates a file at the first test marker and ignores aliased
calls, so two mutations it claims to catch passed unnoticed. The test kit remains a dependency of
the command crate, verified unreachable in the built product but excluded from the check by
declaration.

## Next action

Establish from the dependency graph whether the test kit is reachable in the shipped binary, then
either remove it from that graph or bring it under the check.

## Guardrails

- An exclusion that is declared rather than proved is the defect this card closes, not a shortcut it
  may reuse.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
