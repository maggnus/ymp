---
id: W1-APP-02h
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T09:42:00+08:00
updated_at: 2026-08-12T10:26:09+08:00
started_at: 2026-08-12T09:44:30+08:00
accepted_at: 2026-08-12T10:26:09+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/8cc3e90b143c717f063d61ababbc1e0527d8bd4a
closure_commit: https://github.com/maggnus/ymp/commit/0d351f6015c25869bd4e45bb126482fd60945795
evidence: ["[8cc3e90](https://github.com/maggnus/ymp/commit/8cc3e90b143c717f063d61ababbc1e0527d8bd4a)", "[0d351f6](https://github.com/maggnus/ymp/commit/0d351f6015c25869bd4e45bb126482fd60945795)"]
duration_minutes: 41
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

- [x] Both installed runtime profiles implement the common resume contract for a known session.
- [x] A fake managed session proves that resume continues the same attempt and preserves committed
  progress through the production driver abstraction.
- [x] An unknown, mismatched, or already terminal session fails with a typed error and never starts
  a replacement participant implicitly.

## Current state

Accepted. Codex and Claude use their native resume commands for the same managed session and attempt;
unknown, mismatched, or already terminal sessions fail without an implicit replacement process.

## Next action

Proceed with the remaining required W1-APP-02 tasks.

## Guardrails

- Resume must preserve one attempt identity and the foreground controller's authority.
- Do not introduce a daemon, background controller, or transparent process crash recovery.

## Findings

None.

## Closure

### Accepted outcome

Both installed runtime profiles preserve session state and resume the same attempt after a
recoverable child-process failure. An external seven-case matrix confirmed native resume commands,
stable session and attempt identifiers, and terminal failure after exactly two launches for unknown
or mismatched resumed sessions.

### Residuals

None.

### Evidence

- [Reviewed correction](https://github.com/maggnus/ymp/commit/8cc3e90b143c717f063d61ababbc1e0527d8bd4a).
- [Integration commit](https://github.com/maggnus/ymp/commit/0d351f6015c25869bd4e45bb126482fd60945795).
