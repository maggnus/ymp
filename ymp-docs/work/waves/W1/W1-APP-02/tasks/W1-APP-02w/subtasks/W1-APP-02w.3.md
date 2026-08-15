---
id: W1-APP-02w.3
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02w
state: accepted
risk: routine
maturity: BUILD
relation: supporting
depends_on: [W1-APP-02w.1]
blocks: []
created_at: 2026-08-15T08:55:35+08:00
updated_at: 2026-08-15T13:00:12+08:00
started_at: 2026-08-15T12:18:33+08:00
accepted_at: 2026-08-15T13:00:12+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/bf28efed51a3b265c8e5dbae7bf0e5b58f33a7ad
closure_commit: https://github.com/maggnus/ymp/commit/5d899c592a26f4cbb1d68b816368abbcd6753d6f
evidence: ["[5d899c5](https://github.com/maggnus/ymp/commit/5d899c592a26f4cbb1d68b816368abbcd6753d6f)"]
duration_minutes: 0
blocker:
pause_reason:
return_trigger: the check-then-rename window (a process with write access to the project directory could swap an inspected subdirectory for a symlink during materialization) closes only with descriptor-relative operations (openat/renameat with O_NOFOLLOW); return when the product stops declaring poc_process_isolation or in-place apply is offered where another process writes beside the operator
deliberate_partial: true
---

# W1-APP-02w.3 — Export can apply the accepted candidate in place

## Outcome

The owner's original expectation — "the file simply appears in the directory" — becomes an export
mode: applying the accepted candidate's files directly into the project directory (with the
evidence bundle remaining the default or a companion), decided against the delivery-bundle form
pinned in W1-APP-02w.1.

## Scope

### In

- The export surface and its mirrored command.

### Out

- The store layout and the untouched-launch-directory rule.

## Acceptance

- [ ] An in-place export puts the candidate files into the project directory and nothing else;
      the negative half is the current bundle-only delivery.

## Current state

Ready. From the W1-APP-02w.1 review finding and the owner's recorded expectation.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- one return round; the re-review reproduced its own partial-apply scenario as a clean ApplyBlocked
  with no file moved, all three path obstacles refuse without writing and ignore --overwrite,
  ApplyInterrupted lists exactly what moved; the ancestor walk now stops at the first obstacle by a
  disclosed CTO fix
