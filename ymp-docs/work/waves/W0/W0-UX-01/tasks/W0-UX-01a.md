---
id: W0-UX-01a
kind: task
wave: W0
card: W0-UX-01
state: accepted
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: [W0-UX-01b]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T12:04:33+08:00
started_at: 2026-08-12T11:04:55+08:00
accepted_at: 2026-08-12T12:04:33+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/579eb08b6ceaba93cd44d3853d2e841fc966e2be
closure_commit: https://github.com/maggnus/ymp/commit/7642488df04471beb8dac505171ddaf91bfda498
evidence: ["[579eb08](https://github.com/maggnus/ymp/commit/579eb08b6ceaba93cd44d3853d2e841fc966e2be)", "[7642488](https://github.com/maggnus/ymp/commit/7642488df04471beb8dac505171ddaf91bfda498)"]
duration_minutes: 60
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

- [x] The matrix covers every predictable entity, operator input, displayed state, setting family,
  consequence-bearing action, POC-1 and POC-2 use case, terminal constraint, and material failure
  class found in the project documents.
- [x] The matrix binds Rust, Ratatui, and Crossterm as the implementation medium, defines
  deterministic terminal-test expectations, and identifies which layout, navigation, composition,
  and widget decisions already exist in the HTML artifact.
- [x] Each screen family defines reusable structure, projection-backed data slots, repeated-item
  rules, typed actions, and loading, empty, stale, failure, truncation, and size variants; the same
  template accepts different fixture data at 80 × 24, 120 × 40, and wide sizes.
- [x] The matrix explicitly prevents grades, central assignment, candidate ranking, leadership
  from centrality, causal claims from temporal order, hidden fallback, and POC containment claims.
- [x] A negative review using a deliberately central-dispatch or web-only interpretation rejects
  that interpretation under the brief rather than finding it permissible.

## Current state

Accepted. The sole artifact contains 172 traceable requirements, nine reusable structural
templates, named data regions, application projections, typed commands, deterministic fixture
rules, negative controls, and explicit W0-UX-01b blockers. Independent re-review confirmed the
template contract and readable layouts at 760, 1440, and 1920 pixel review widths.

## Next action

Resolve the seven explicit `BLK-*` fixture and state gaps in W0-UX-01b without changing the
accepted projection-backed template semantics.

## Guardrails

- Requirements may constrain product meaning and terminal feasibility but do not retroactively
  justify an artifact omission or turn composition preferences into protocol semantics.
- Open product assumptions remain labelled rather than being settled through design prose.

## Findings

None. The returned table overflow, clipped header mark, and two inaccurate source references were
corrected and independently rechecked.

## Closure

### Accepted outcome

The requirements matrix traces the project contract to reusable terminal structures rather than
static screens. Nine `TPL-*` templates consume immutable application projections through ten named
`REG-*` regions and twelve `PRJ-*` projection families; all fourteen existing fixtures declare
their template, projection, and data fixture. Negative central-dispatch and browser-client
interpretations are explicitly rejected.

### Residuals

Seven visible `BLK-*` rows intentionally remain assigned to W0-UX-01b: complete state fixtures,
three size classes, consequence previews, lost-reply behavior, unread-message quiescence,
assurance failures, and the final independent freeze review.

### Evidence

- [Reviewed candidate](https://github.com/maggnus/ymp/commit/579eb08b6ceaba93cd44d3853d2e841fc966e2be).
- [Integration commit](https://github.com/maggnus/ymp/commit/7642488df04471beb8dac505171ddaf91bfda498).
