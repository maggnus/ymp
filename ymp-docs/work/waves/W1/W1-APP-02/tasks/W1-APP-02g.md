---
id: W1-APP-02g
kind: task
wave: W1
card: W1-APP-02
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T09:42:00+08:00
updated_at: 2026-08-12T10:48:00+08:00
started_at: 2026-08-12T10:28:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/da8632f56800930e8842231fe25a6e9a5168c8e9
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02g — Verification evidence binds the exact runtime environment

## Outcome

Every accepted verification record identifies the exact environment object used by the verifier,
and replay refuses evidence whose environment digest is absent or mismatched.

## Scope

### In

- Durable verification-record fields, environment object binding, compatibility handling, and
  verifier/application checks that enforce the binding.

### Out

- Runtime event ordering, participant-session resume, TUI layout, and unrelated schema revisions.

## Acceptance

- [ ] A newly accepted record carries the exact environment digest consumed by verification and
  reproduces from clean immutable inputs.
- [ ] Missing, substituted, or mismatched environment data is rejected as a typed infrastructure
  failure rather than accepted evidence.
- [ ] Compatibility fixtures cover the deliberate schema decision for existing version-1 data.

## Current state

The candidate introduces verification evidence schema version 2, binds the exact environment
object through Application and Verifier, and fails closed on altered, absent, mismatched, or
ambiguous version-1 environment data. Independent review is pending.

## Next action

Decide acceptance through an independent product-path falsifier on the exact candidate.

## Guardrails

- Preserve content-addressed immutable evidence and fail closed on ambiguous legacy data.
- Any durable schema change must update its compatibility fixtures and `ymp-rust/SCHEMA.md`.

## Findings

None pending from the author; independent review is in progress.

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger. Empty when there are none.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
