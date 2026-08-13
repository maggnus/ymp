---
id: W1-APP-02e.3
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02e.2]
blocks: []
created_at: 2026-08-13T01:26:13+08:00
updated_at: 2026-08-13T09:47:08+08:00
started_at: 2026-08-13T09:50:00+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/3aed1ac9b58beb52cd06bc19a9988c1135cd8112
closure_commit:
evidence:
duration_minutes: 0
blocker:
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

- [ ] Starting `ymp` renders the transcript-first POC-1 path and reaches every supported page and
      decision surface with the declared keyboard commands at 80x24 and 120x40.
- [ ] The interface is one framework rather than a set of per-screen implementations: every page and
      modal is composed from the shared component and layout layer, and no screen owns a private
      drawing path. The check that must fail: a page that bypasses the shared layer is rejected by an
      inventory check with a captured non-zero exit.
- [ ] Every value on screen is derived from an application projection. The check that must fail: a
      rendered field whose value is fixed in the presentation layer instead of read from state is
      rejected with a captured non-zero exit.
- [ ] Deterministic buffers preserve terminal reason, primary action, current selection, bounded
      content, and state distinctions without relying on colour or mouse input.
- [ ] Unsupported POC-2 concepts are absent or explicitly unavailable; they never appear as invented
      participants, budgets, events, or completed actions.
- [ ] Removing one required surface, state marker, or keyboard transition makes the same test harness
      fail before independent review.

## Current state

Returned by independent review after one round. The implementation reaches every page and both
decision surfaces at 80x24 and 120x40 with values that match the journal, but a badge drawn over the
header removes the irreversibility marker as soon as a real run identifier is used, so an
irreversible decision reads as reversible. Bounded rework is authorized for that defect, its missing
negative half, one hardcoded enforcement class, and two blind spots in the new inventory checks.

## Next action

Rebase the inherited candidate on the accepted base, make it compile against the runtime event
variants the managed-runtime work added, then prove the framework property and the data binding
before independent review.

## Guardrails

- Widgets consume immutable application projections and never own domain transition rules.
- Missing domain concepts are not added as presentation-only state.
- Changes to application or domain boundaries require a separately approved scope expansion.
- The inherited implementation is functional and replaces a poor earlier interface, so a review of
  it aims at one question: does every value on screen come from real application state. Invented
  participants, budgets, events or completed actions are the defect to look for. A return is narrow,
  names the exact fabricated or unbound value, and never discards working behaviour over
  presentation preference.
- The integrated managed-runtime work added exhaustive runtime event variants that the terminal
  crate must handle, so the inherited implementation is rebased on the accepted base before it is
  judged.

## Findings

- Two interface decisions are open in the rationale document and belong to this subtask: the name
  of the assurance profile, and what the operator sees when a run is cancelled. Each is decided
  here, recorded in the rationale document, and evidenced by the screen that shows it.
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
- `blocker`, defect in the contracted outcome. The decision badge overlaps the header, and with a
  product-generated run identifier the irreversibility marker is lost on the transcript and absent
  from the authorization surface at both sizes. A trust-significant action that renders as its own
  opposite is the defect this card exists to prevent.
- `major`, defect in the contracted outcome. The badge assertion runs only against a short fixture
  and cannot fail on a real identifier, so the check could not have caught the defect above.
- `minor`, defect in the contracted outcome. The enforcement class is written in code rather than
  read from the projection, which is the same fabrication class the card forbids, in miniature.
- `minor`, defect in the contracted outcome. Both inventory checks read only the top level of the
  source tree and omit two drawing calls, so a screen could bypass the shared layer undetected.
- Additional work: the product has no command that starts a run or records a verification. Nothing
  the contract required was removed with the deleted fixtures.
- Refinement of the starting hypothesis: the terminal does not consume runtime events at all, and
  the eight domain events it does consume are handled exhaustively. The premise that new runtime
  event variants would force terminal changes did not hold.

## Closure

Not accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
