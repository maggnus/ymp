---
id: W0-UX-01b
kind: task
wave: W0
card: W0-UX-01
state: ready
risk: significant
maturity: DESIGN
relation: required
depends_on: [W0-UX-01a]
blocks: [W0-UX-01c]
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

# W0-UX-01b — Claude Design produces the POC screen and state system

## Outcome

Claude Design supplies a traceable terminal-native design package covering the complete POC-1 and
POC-2 operator journey, normal and material failure states, responsive terminal variants, and MVP
extension points without production code.

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

- [ ] Every brief requirement maps to a reviewable mockup, state variant, interaction annotation,
  explicit non-applicability statement, or documented open decision.
- [ ] POC-1 and POC-2 designs cover 80 × 24, 120 × 40, and a wide high-volume case with
  keyboard-first and color-independent operation.
- [ ] The design visibly distinguishes authoritative control, untrusted collaboration, independent
  verification, human intervention, enforced limits, observational values, and typed terminal
  outcomes.
- [ ] A deliberately misleading fixture containing a fluent but causally irrelevant transcript,
  a central graph node, and a failed verifier cannot be presented as collective reasoning,
  leadership, or acceptance under the supplied visual semantics.

## Current state

No Claude Design output, static screen image, responsive variant, or handoff index exists. The task
depends on acceptance of the requirements brief.

## Next action

Submit the accepted `ympus-docs/CLAUDE_REQUESTS.md` to Claude Design and collect the complete design
package without narrowing the requested state matrix during handoff.

## Guardrails

- Claude Design owns composition and visual language but does not own domain or protocol changes.
- A design omission becomes an explicit open decision or returned requirement; it is not hidden by
  a generic placeholder screen.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
