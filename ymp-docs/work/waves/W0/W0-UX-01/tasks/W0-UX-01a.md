---
id: W0-UX-01a
kind: task
wave: W0
card: W0-UX-01
state: ready
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W0-UX-01b]
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

# W0-UX-01a — Claude Design brief defines the terminal product contract

## Outcome

One self-contained request gives Claude Design the stable product vocabulary, inputs, displayed
information, settings, actions, failure states, terminal constraints, forbidden semantics, and
expected deliverables needed to design the POC interface without prescribing a layout.

## Scope

### In

- POC-1, POC-2, and predictable MVP interface requirements derived from the current documents.
- Rust, Ratatui, Crossterm, deterministic screen testing, accessibility, terminal sizes, realistic
  fixtures, and design acceptance criteria.
- Explicit separation of stable requirements, delivery phase, open assumptions, and designer-owned
  composition decisions.

### Out

- Screen mockups, production code, a crate-version decision, protocol changes, and optional
  server, web, database, or cluster interface design.

## Acceptance

- [ ] `ympus-docs/CLAUDE_REQUESTS.md` covers every predictable entity, operator input, displayed
  state, setting family, consequence-bearing action, POC-1 and POC-2 use case, terminal constraint,
  and material failure class found in the project documents.
- [ ] The brief names Rust, Ratatui, and Crossterm as the implementation medium, defines
  deterministic terminal-test expectations, and leaves layout, navigation, composition, and exact
  widgets to Claude Design.
- [ ] The brief explicitly prevents grades, central assignment, candidate ranking, leadership from
  centrality, causal claims from temporal order, hidden fallback, and POC containment claims.
- [ ] A negative review using a deliberately central-dispatch or web-only interpretation rejects
  that interpretation under the brief rather than finding it permissible.

## Current state

The requirements brief exists and includes a full entity, action, setting, state, fixture, and
handoff contract. It has not received non-author review, commit-pinned evidence, or acceptance.

## Next action

Review the brief against the current project documents and record any missed or contradictory
requirement before submitting it to Claude Design.

## Guardrails

- The brief may constrain product meaning and terminal feasibility but must not dictate page count,
  panel arrangement, navigation hierarchy, color palette, or component placement.
- Open product assumptions remain labelled rather than being settled through design prose.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
