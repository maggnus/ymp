---
id: W1-APP-02e.3
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: blocked
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02e.2]
blocks: []
created_at: 2026-08-13T01:26:13+08:00
updated_at: 2026-08-13T01:26:13+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker: W1-APP-02e.2 has not yet accepted the replacement visual contract
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02e.3 — Ratatui implements the accepted chat-first contract

## Outcome

The production Ratatui path implements the independently accepted chat-first POC-1 contract while
representing future-only concepts as unavailable rather than fabricating unsupported domain state.

## Scope

### In

- Transcript-first application frame, input line, context and status rows, `:` data-page
  navigation, decision modals, size guard, and POC-1 state variants accepted in W1-APP-02e.2.
- Deterministic 80×24 and 120×40 buffers, keyboard transitions, monochrome distinctions, bounded
  high-volume content, and a structural negative control.
- Migration from the superseded dashboard buffers without changing domain transition ownership.

### Out

- Design-contract review, multi-participant POC-2 semantics, intent/contract domain expansion,
  real-provider profile correction, and uncontrolled visual polish.

## Acceptance

- [ ] Starting `ymp` renders the accepted transcript-first POC-1 path and reaches every supported
  page and decision surface with the declared keyboard commands at 80×24 and 120×40.
- [ ] Deterministic buffers preserve terminal reason, primary action, current selection, bounded
  content, and state distinctions without relying on colour or mouse input.
- [ ] Unsupported POC-2 concepts are absent or explicitly unavailable; they never appear as
  invented participants, budgets, events, or completed actions.
- [ ] Removing one required surface, state marker, or keyboard transition makes the same test
  harness fail before independent review.

## Current state

Blocked on independent acceptance of W1-APP-02e.2. The existing production buffers continue to
represent the superseded dashboard contract and remain usable, but they are not acceptance
evidence for the chat-first revision. An owner-authored chat-first implementation already exists
outside this fleet and is preserved as an inherited candidate; it has received no review and is
not acceptance evidence either.

## Next action

Resume after W1-APP-02e.2 is accepted, then judge the inherited candidate against the approved
POC-1 projection instead of starting the implementation from an empty write zone.

## Guardrails

- Widgets consume immutable application projections and never own domain transition rules.
- Missing domain concepts are not added as presentation-only state.
- Changes to application or domain boundaries require a separately approved scope expansion.

## Findings

- An owner-authored chat-first terminal implementation was completed outside this fleet and left
  uncommitted in the integration tree. Its finished state is preserved as
  [`4efd94f`](https://github.com/maggnus/ymp/commit/4efd94f03c77e8a03752805af5825c8eafd67378)
  on branch `paseo/w1-app-02e3-inherited-candidate-20260813` (local-only until push): 18 files,
  `+5052/-2199`, rewriting
  [`ymp-rust/crates/ymp-tui/src/lib.rs`](https://github.com/maggnus/ymp/blob/4efd94f03c77e8a03752805af5825c8eafd67378/ymp-rust/crates/ymp-tui/src/lib.rs)
  into thirteen modules with a preview example. The integration tree was returned to
  [`0ca9bac`](https://github.com/maggnus/ymp/commit/0ca9bacb0c4e014ddc48c83782227fe36578c2b9)
  so an accepted candidate can land.
- The inherited candidate carries no independent review, no acceptance evidence, and no declared
  correspondence to the visual contract still under review in W1-APP-02e.2. It is a starting point
  for this subtask, not a result of it.

## Closure

Not accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
