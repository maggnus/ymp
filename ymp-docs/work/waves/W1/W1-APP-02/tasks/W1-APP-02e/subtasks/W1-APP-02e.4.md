---
id: W1-APP-02e.4
kind: subtask
wave: W1
card: W1-APP-02
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-15T01:30:15+08:00
updated_at: 2026-08-15T01:30:15+08:00
started_at: 2026-08-15T01:30:15+08:00
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

# W1-APP-02e.4 — The pinned runtime profiles admit the owner's host

## Outcome

Both managed profiles start on the machine the product is actually used on: the Claude Code pin
follows the installed release instead of one frozen build, and the Codex route names a model the
owner's account can call. Admission stays honest — what runs is still attested and recorded — but
a newer installed runtime is admitted by measured compatibility, not refused by string equality.

## Scope

### In

- The Claude Code version pin in ymp-rust/crates/ymp-runtime-claude (2.1.227 vs installed
  2.1.232) and what admission actually needs to trust a runtime.
- The Codex model route in ymp-rust/crates/ymp-runtime-codex (gpt-5.6-sol rejected with HTTP 400
  for a ChatGPT account); probe which routes the account accepts and pin one that works.

### Out

- The launch-chain attestation itself, accepted in W1-APP-02p/02q.

## Acceptance

- [ ] ymp probe reports both profiles ready on this host, and one managed attempt per profile
      completes a real run; the negative halves are the two measured failures (HTTP 400; the
      version refusal).

## Current state

Active. Recorded from the W1-APP-02e live runs: every end-to-end test is green while the owner's
machine cannot run either profile — the acceptance criterion of the whole surface is the owner's
scenario, so this mismatch outranks every remaining hardening node.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

None recorded.
