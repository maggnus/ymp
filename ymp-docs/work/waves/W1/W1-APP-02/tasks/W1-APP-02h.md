---
id: W1-APP-02h
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T09:42:00+08:00
updated_at: 2026-08-12T09:42:00+08:00
started_at:
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

Independent review found that the Codex and Claude resume operations return `Unsupported`.
Interruption recovery is therefore absent from the contracted managed-runtime behavior.

## Next action

Implement and test the common resume lifecycle without adding controller crash recovery.

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
