---
id: W1-APP-02u
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02m]
blocks: []
created_at: 2026-08-14T00:51:54+08:00
updated_at: 2026-08-14T00:51:54+08:00
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

# W1-APP-02u — A verifier answer is an executable that demonstrably discriminates

## Outcome

The dialogue accepts as a verifier only a program that exists, can be executed, and is shown to
reject the negative control before the contract is stored. A command name, a directory or a program
that accepts everything is refused where it is typed, with the reason named.

## Scope

### In

- Validation of the verifier answer and the negative-control answer at the point of entry.
- Running the negative control against the named verifier before a contract is stored, in the
  interface and in the command that shares its scenario.

### Out

- Inventing an acceptance condition, which stays forbidden.

## Acceptance

- [ ] A verifier answer that does not resolve to an existing executable file is refused where it is
      typed; the negative half is the current build, which accepted a command line with arguments.
- [ ] A verifier that accepts the negative control is refused with that reason, so a program which
      succeeds on anything cannot enter a contract; the negative half names such a program and shows
      today's behaviour.
- [ ] The source directory is validated where it is typed rather than at assembly. Today the refusal
      arrives correctly but only after the remaining questions have been answered, naming the path
      and the system error.

## Current state

Ready. Driven directly on the built product, the dialogue accepted a command line as the verifier and
a non-existent directory as the source, and refused only at assembly — correctly, naming the path and
the system error, recording nothing. What is missing is validation at the point of entry and an
executed negative control before the contract is stored, which is what separates a verifier from any
program that exits zero.

## Next action

Validate each answer where it is entered, then run the negative control before storing.

## Guardrails

- Refusing an unusable answer is not the same as inventing a usable one; the second stays forbidden.
- A program that accepts the negative control is not a verifier, whatever its path.

## Findings

- Measured on the built product: `mkdir ~/Code/test` was accepted as the verifier answer, and the
  refusal for a non-existent source arrived at assembly with an exact message and no record written.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
