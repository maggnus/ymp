---
id: W1-APP-02v
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02u]
blocks: []
created_at: 2026-08-14T02:05:00+08:00
updated_at: 2026-08-14T02:05:00+08:00
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

# W1-APP-02v — The dialogue experience converges on the Claude Code interface

## Outcome

An operator who knows Claude Code can use ymp without relearning the surface: commands are typed
with a leading `/`, a request starts with at most one contract question, and repeated authorization
of an unchanged contract is one action. The contract model underneath is unchanged: ymp still never
invents an acceptance condition, and nothing runs unauthorized.

## Scope

### In

- The command prefix: every palette command is entered as `/name`; `:` is retired from the input
  surface and from every hint that names a command.
- Question reduction in the draft dialogue: the source directory keeps its default; the negative
  control defaults to a product-supplied control (a clean copy of the source, which a discriminating
  verifier must reject as containing no result); the verifier question offers a detected project
  test entry point for confirmation when one exists, instead of demanding a path from memory.
- Authorization weight: an unchanged, previously authorized contract re-authorizes with a single
  confirmation; the typed-identifier ceremony remains for first authorization and for anything that
  spends or is irreversible.

### Out

- Inventing an acceptance condition on the user's behalf, which stays forbidden.
- Export of an accepted candidate into the working directory, which belongs to W1-APP-02e.
- Multi-participant surfaces, which belong to W1-COR-03e.

## Acceptance

- [ ] Every command the interface offers is entered with `/`; the negative half is the current
      build, where `:` opens the palette and hints print `:authorize`.
- [ ] A request in a directory with a detectable test entry point reaches a drafted contract with
      one question answered by one confirmation; the negative half is the current three-question
      dialogue driven on the same directory.
- [ ] Re-authorizing an unchanged contract is one confirmation; first authorization still requires
      the typed identifier, proved on both paths.

## Current state

Ready. Recorded from two owner decisions of 2026-08-14: the user experience must stay close to the
Claude Code interface, with multi-agent collaboration as the distinguishing addition, and commands
must be entered through `/` rather than `:`. The owner's live session the same night measured the
cost of the current dialogue: a drafted request was lost to an empty answer, and the verifier
questions demanded paths the operator had no way to know.

## Next action

After W1-APP-02u lands (its entry-point validation reshapes the same dialogue), implement the
prefix change, the default negative control, and the verifier suggestion over the accepted
chat-first contract.

## Guardrails

- Familiarity is the goal only above the contract line: a surface that starts work without an
  acceptance condition has not simplified the product, it has removed it.

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
