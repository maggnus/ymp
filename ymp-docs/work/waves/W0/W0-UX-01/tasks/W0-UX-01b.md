---
id: W0-UX-01b
kind: task
wave: W0
card: W0-UX-01
state: active
risk: significant
maturity: DESIGN
relation: required
depends_on: [W0-UX-01a]
blocks: [W0-UX-01c]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T12:05:36+08:00
started_at: 2026-08-12T12:05:36+08:00
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

# W0-UX-01b — HTML artifact supplies the POC screen and state system

## Outcome

The sole HTML artifact supplies a traceable terminal-native design package covering the complete
POC-1 and POC-2 operator journey, normal and material failure states, responsive terminal variants,
and MVP extension points without becoming production code.

## Scope

### In

- Experience map, information architecture, trust legend, screen set, state variants, responsive
  variants, interaction annotations, component inventory, accessibility rules, content examples,
  decision log, and handoff index required by the accepted brief.
- Realistic fixtures for first launch, mixed runtime readiness, one-participant outcomes, local
  negotiation, competing candidates, communication study results, intervention, races, high volume,
  and a constrained terminal.

### Out

- Rust implementation, changes to protocol semantics, product claims based on the mockups, and
  designs that require a browser, service, pixel canvas, or optional post-Alpha modes.

## Acceptance

- [ ] Every project requirement maps to a reviewable mockup, state variant, interaction annotation,
  explicit non-applicability statement, or documented open decision. An open decision that affects
  trust, authorization, verification, or a terminal outcome blocks acceptance rather than becoming
  a warning.
- [ ] POC-1 and POC-2 designs cover 80 × 24, 120 × 40, and a wide high-volume case with
  keyboard-first and color-independent operation.
- [ ] The design visibly distinguishes authoritative control, untrusted collaboration, independent
  verification, human intervention, enforced limits, observational values, and typed terminal
  outcomes.
- [ ] A deliberately misleading fixture containing a fluent but causally irrelevant transcript,
  a central graph node, and a failed verifier cannot be presented as collective reasoning,
  leadership, or acceptance under the supplied visual semantics.

## Current state

The accepted W0-UX-01a matrix defines nine projection-backed templates and seven explicit `BLK-*`
gaps. Work is active in the sole HTML artifact on complete state fixtures, three terminal-size
classes, typed consequence previews, lost-reply and unread-message controls, assurance failures,
and deterministic handoff evidence.

## Next action

Resolve every `BLK-*` row in the same reusable template system, validate representative fixtures
locally at 80 × 24, 120 × 40, and 180 × 50, and submit the exact artifact to non-author review.

## Guardrails

- The artifact owns its existing composition and visual language but does not own domain or
  protocol changes.
- A design omission becomes an explicit open decision or returned requirement; it is not hidden by
  a generic placeholder screen.
- Lost MCP replies and wake/quiescence with unread inert messages each have an explicit state; an
  unresolved version of either state blocks authorization and task acceptance.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
