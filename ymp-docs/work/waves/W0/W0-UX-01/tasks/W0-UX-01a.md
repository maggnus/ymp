---
id: W0-UX-01a
kind: task
wave: W0
card: W0-UX-01
state: active
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W0-UX-01b]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T11:47:00+08:00
started_at: 2026-08-12T11:04:55+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/e8ff248fad346071e9adec2b94f07e6c027f17ed
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

One reviewable requirement matrix derives the stable product vocabulary, reusable screen
templates, data slots, inputs, displayed information, settings, actions, failure states, terminal
constraints, and forbidden semantics from the documents of record and maps them to the sole HTML
design artifact.

## Scope

### In

- POC-1, POC-2, and predictable MVP interface requirements derived from the current documents.
- Rust, Ratatui, Crossterm, deterministic screen testing, accessibility, terminal sizes, realistic
  fixtures, and design acceptance criteria.
- Reusable structural templates whose named data slots are populated from application projections;
  fixtures instantiate those templates but do not define separate screen semantics.
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
- [ ] Each screen family defines reusable structure, projection-backed data slots, repeated-item
  rules, typed actions, and loading, empty, stale, failure, truncation, and size variants; the same
  template accepts different fixture data at 80 × 24, 120 × 40, and wide sizes.
- [ ] The matrix explicitly prevents grades, central assignment, candidate ranking, leadership
  from centrality, causal claims from temporal order, hidden fallback, and POC containment claims.
- [ ] A negative review using a deliberately central-dispatch or web-only interpretation rejects
  that interpretation under the brief rather than finding it permissible.

## Current state

The first candidate adds 172 traceable requirements, nine reusable structural templates, named
data regions, application projections, typed commands, deterministic fixture rules, negative
controls, and explicit blockers. Independent review confirmed that structure but returned one
table-readability defect and three bounded source/layout corrections; author rework is active.

## Next action

Correct the four bounded review findings, then repeat structural and visual review at 760, 1440,
and 1920 pixel widths on the exact corrected candidate.

## Guardrails

- Requirements may constrain product meaning and terminal feasibility but do not retroactively
  justify an artifact omission or turn composition preferences into protocol semantics.
- Open product assumptions remain labelled rather than being settled through design prose.

## Findings

- `MAJOR`: long projection names in the `REG-*` source column cross the fixed table cell boundary
  and obscure the adjacent column at normal and wide review widths.
- `MINOR`: `NEG-CENTRAL` cites a nonexistent `INV-13` and must cite the exact central-assignment
  prohibition headed by `INV-1`.
- `MINOR`: `NEG-WEB` cites a nonexistent `PROJECT-CONTRACT.md` section.
- `MINOR`: the `1a` header mark is almost fully clipped in the normal fixture.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
