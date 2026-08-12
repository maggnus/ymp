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
updated_at: 2026-08-12T02:47:53+08:00
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

# W0-UX-01a — Project requirements define the terminal product contract

## Outcome

One reviewable requirement matrix derives the stable product vocabulary, inputs, displayed
information, settings, actions, failure states, terminal constraints, and forbidden semantics from
the documents of record and maps them to the sole HTML design artifact.

## Scope

### In

- POC-1, POC-2, and predictable MVP interface requirements derived from the current documents.
- Rust, Ratatui, Crossterm, deterministic screen testing, accessibility, terminal sizes, realistic
  fixtures, and design acceptance criteria.
- Explicit separation of stable requirements, delivery phase, open assumptions, and artifact-owned
  composition decisions.

### Out

- Production code, a crate-version decision, protocol changes, alternative design artifacts, and
  optional server, web, database, or cluster interface design.

## Acceptance

- [ ] The matrix covers every predictable entity, operator input, displayed state, setting family,
  consequence-bearing action, POC-1 and POC-2 use case, terminal constraint, and material failure
  class found in the project documents.
- [ ] The matrix binds Rust, Ratatui, and Crossterm as the implementation medium, defines
  deterministic terminal-test expectations, and identifies which layout, navigation, composition,
  and widget decisions already exist in the HTML artifact.
- [ ] The matrix explicitly prevents grades, central assignment, candidate ranking, leadership
  from centrality, causal claims from temporal order, hidden fallback, and POC containment claims.
- [ ] A negative review using a deliberately central-dispatch or web-only interpretation rejects
  that interpretation under the brief rather than finding it permissible.

## Current state

The project requirements and `ymp-docs/design/ymp_k9s_tui.dc.html` exist. A complete
requirement-to-screen matrix, non-author review, and commit-pinned acceptance evidence do not yet
exist.

## Next action

Build the requirement-to-screen matrix from the current documents and record every omission,
contradiction, or intentionally deferred state in the existing artifact.

## Guardrails

- Requirements may constrain product meaning and terminal feasibility but do not retroactively
  justify an artifact omission or turn composition preferences into protocol semantics.
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
