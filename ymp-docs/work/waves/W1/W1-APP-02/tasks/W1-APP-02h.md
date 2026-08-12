---
id: W1-APP-02h
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
updated_at: 2026-08-12T10:05:00+08:00
started_at: 2026-08-12T09:44:30+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/10e2c92065ea4561a8d484ba7c62818f2784ae0f
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02h — Managed Codex and Claude sessions resume after interruption

## Outcome

A live foreground controller can resume an identified managed Codex or Claude participant session
after a recoverable driver interruption without creating a second attempt or losing committed work.

## Scope

### In

- Codex and Claude driver resume operations, session identifiers, lifecycle transitions, and fake
  runtime coverage for the common contract.

### Out

- Controller crash continuation, runtime event ordering, verification evidence, and TUI redesign.

## Acceptance

- [ ] Both installed runtime profiles implement the common resume contract for a known session.
- [ ] A fake managed session proves that resume continues the same attempt and preserves committed
  progress through the production driver abstraction.
- [ ] An unknown, mismatched, or already terminal session fails with a typed error and never starts
  a replacement participant implicitly.

## Current state

The candidate resumes Codex and Claude processes with the same session and attempt identifiers and
rejects unknown or mismatched sessions without launching a replacement. Builder-owned targeted
checks pass; independent review is pending on the exact candidate revision.

## Next action

Decide acceptance through an external product-path falsifier in an independent workspace.

## Guardrails

- Resume must preserve one attempt identity and the foreground controller's authority.
- Do not introduce a daemon, background controller, or transparent process crash recovery.

## Findings

- Codex and Claude drivers currently reject resume as unsupported.

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger. Empty when there are none.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
