---
id: W0-UX-01c
kind: task
wave: W0
card: W0-UX-01
state: active
risk: critical
maturity: DESIGN
relation: required
depends_on: [W0-UX-01b]
blocks: [W1-APP-02e, W1-COR-03e]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T13:30:46+08:00
started_at: 2026-08-12T13:30:46+08:00
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

- Semantic review against the project documents and the sole HTML artifact.
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
  to a design artifact. No open assumption affecting trust, authorization, verification, or a
  terminal outcome may remain; lower-risk assumptions require an owner and implementation effect.
- [ ] The reviewer confirms that the screens do not assign work, rank agents or candidates, infer
  leadership, conflate trust planes, overstate causation, hide assurance weakness, or confuse any
  root terminal outcome.
- [ ] The 80 × 24, 120 × 40, and wide variants are feasible with terminal cells, keyboard input,
  bounded text, and deterministic Ratatui state tests; no required interaction depends on hover,
  animation, pixel geometry, a browser, or a network service.
- [ ] A negative-control design that labels message delivery as causal reasoning or process
  completion as acceptance is returned rather than accepted.
- [ ] Negative controls for a lost MCP reply and quiescence with unread inert messages keep
  authorization disabled until the corresponding state is resolved.

## Current state

The complete screen and state package is accepted at
[35cc659](https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1)
and integrated by [d01354c](https://github.com/maggnus/ymp/commit/d01354c6046b8a7c1fcd4380087f0e43805b24ff).
The final independent semantic, accessibility, and terminal-feasibility review is active; the `W1`
screen tasks remain blocked until it freezes an exact contract revision.

## Next action

Review the exact accepted HTML artifact against POC-1, POC-2, the forbidden semantics, keyboard and
monochrome operation, 80 × 24, 120 × 40 and wide terminal feasibility, lost MCP reply, unread inert
message quiescence, and the two required negative interpretations; then record `ACCEPT` or exact
return findings.

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
