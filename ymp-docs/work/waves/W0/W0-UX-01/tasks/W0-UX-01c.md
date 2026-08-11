---
id: W0-UX-01c
kind: task
wave: W0
card: W0-UX-01
state: ready
risk: critical
maturity: DESIGN
relation: required
depends_on: [W0-UX-01b]
blocks: [W1-APP-02e, W1-COR-03e]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-10T21:23:34+08:00
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

# W0-UX-01c — Independent review freezes an implementable screen contract

## Outcome

An independent review accepts a versioned screen contract whose domain semantics match the project,
whose required states are complete and traceable, and whose interactions are implementable with
Rust, Ratatui, and Crossterm at the declared terminal sizes.

## Scope

### In

- Semantic review against the project documents and accepted Claude Design brief.
- Coverage review of the POC-1 and POC-2 journeys, material negative states, terminal and
  accessibility constraints, and requirement-to-mockup handoff index.
- Feasibility review against terminal cells, Ratatui rendering and deterministic tests, Crossterm
  input events, high event volume, and one-executable packaging.
- A frozen identifier and revision for every accepted screen and state contract consumed by `W1`.

### Out

- Production implementation, subjective redesign without a falsifiable defect, and acceptance of
  any layout that changes kernel semantics or requires a web or service runtime.

## Acceptance

- [ ] A non-author reviewer returns `ACCEPT` only after every POC-1 and POC-2 requirement is traced
  to a design artifact and every accepted open assumption has an owner and implementation effect.
- [ ] The reviewer confirms that the screens do not assign work, rank agents or candidates, infer
  leadership, conflate trust planes, overstate causation, hide assurance weakness, or confuse any
  root terminal outcome.
- [ ] The 80 × 24, 120 × 40, and wide variants are feasible with terminal cells, keyboard input,
  bounded text, and deterministic Ratatui state tests; no required interaction depends on hover,
  animation, pixel geometry, a browser, or a network service.
- [ ] A negative-control design that labels message delivery as causal reasoning or process
  completion as acceptance is returned rather than accepted.

## Current state

No Claude Design package or independent review exists, so no screen contract is frozen and both
`W1` screen implementation tasks remain blocked on this outcome.

## Next action

After Claude Design returns its package, commission an independent semantic and terminal-feasibility
review and record the accepted artifact revision or exact return findings.

## Guardrails

- The reviewer attacks correctness, coverage, accessibility, and feasibility rather than replacing
  the designer's layout preferences with another unvalidated preference.
- Any required semantic change returns to the project documents and receives its own decision; it
  is not smuggled into the screen contract.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
