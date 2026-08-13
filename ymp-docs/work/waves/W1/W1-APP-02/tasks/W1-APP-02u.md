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

# W1-APP-02u — The draft refuses an answer it can check and never loses itself

## Outcome

Every answer the draft accepts is one the system verified it can use: a source directory that exists,
a verifier that is an executable file, a negative control that exists. An answer it cannot use is
refused on the spot, naming what is wrong, and the draft survives until it is authorized or
explicitly abandoned.

## Scope

### In

- Validation of each answer in the request-to-contract dialogue, in the interface and in the command
  that shares its scenario.
- The lifetime of a draft across answers, including what happens after the last question.

### Out

- Inventing an acceptance condition, which stays forbidden; this card refuses bad answers rather than
  supplying good ones.

## Acceptance

- [ ] A source directory that does not exist is refused with its path named; the negative half is the
      current build, which accepts it.
- [ ] A verifier answer that is not an existing executable file is refused, including a command line
      with arguments; the negative half is the current build, which accepted `mkdir ~/Code/test`.
- [ ] A negative control that does not exist, or that the named verifier accepts, is refused.
- [ ] After the last answer the draft is either an approved contract or an explicit refusal naming
      what is missing; it is never silently discarded. Establish first whether it is discarded today
      and report what you found.

## Current state

Ready. The owner drove the interface directly and it accepted a non-existent directory as the source
and a shell command line as the verifier program, then returned to the cold screen reporting no draft.
A contract assembled from answers nobody checked would start a run that cannot be judged, which is
the failure the acceptance condition exists to prevent.

## Next action

Validate each answer where it is entered, then establish what happens to the draft after the last
question.

## Guardrails

- Refusing an unusable answer is not the same as inventing a usable one; the second stays forbidden.

## Findings

- Measured by the owner on the built product: source `pong` and verifier `mkdir ~/Code/test` were both
  accepted as answers.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
