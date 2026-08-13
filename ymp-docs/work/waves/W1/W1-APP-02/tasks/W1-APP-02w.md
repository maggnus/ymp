---
id: W1-APP-02w
kind: task
wave: W1
card: W1-APP-02
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-14T02:25:00+08:00
updated_at: 2026-08-14T02:25:00+08:00
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

# W1-APP-02w — Product state lives under one .ymp root that supports many projects

## Outcome

Everything the product writes on a host lives under a single `.ymp` root, and that root holds many
projects and many runs without collision: a store is addressed as a child of the root, not as an
ad-hoc sibling directory the operator must invent per run. Existing accepted behaviour is
preserved: one run per store, immutable objects, a journal that recovers.

## Scope

### In

- The default data root: `.ymp` replaces `.ymp-data`, and the layout below it names the project and
  the run so a second project or a second run needs no operator-invented directory name.
- Every path the product writes today (store, workspaces, candidates, runtime sockets, admitted
  executables) accounted for under the root or explicitly justified outside it.
- Migration or refusal for an existing `.ymp-data`: an old store is either read where it stands or
  refused with the reason named — never silently duplicated.

### Out

- Multi-participant semantics (W1-COR-03) and export of accepted candidates (W1-APP-02e).
- A shared cross-host or user-home store; the root stays project-local unless a later decision
  moves it.

## Acceptance

- [ ] Two runs started from one project directory land in two stores under one `.ymp` root without
      operator-invented names; the negative half is the current build, where the second run refuses
      until the operator supplies a fresh `--data-root`.
- [ ] Two projects on one host keep disjoint state under their respective roots, proved by driving
      both and diffing the trees.
- [ ] No product path writes outside `.ymp` in the launch directory; the check enumerates written
      paths, and its negative half shows today's `.ymp-data` sibling.

## Current state

Ready. Recorded from the owner decision of 2026-08-14: keep everything under `.ymp` where possible,
and make the structure support multiple projects. The owner's live session the same night required
inventing `--data-root` names (`.ymp-data`, `.ymp-smoke`) by hand for each run.

## Next action

Design the layout under `.ymp` (project, run, shared objects), then move the default root and the
run-addressing over it.

## Guardrails

- A layout change must not weaken the accepted store invariants: one run per store, immutable
  objects, journal recovery, refusal of an unreadable store.

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
