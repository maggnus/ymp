---
id: W0-UX-01b
kind: task
wave: W0
card: W0-UX-01
state: accepted
risk: significant
maturity: DESIGN
relation: required
depends_on: [W0-UX-01a]
blocks: [W0-UX-01c]
created_at: 2026-08-10T21:23:34+08:00
updated_at: 2026-08-12T13:30:46+08:00
started_at: 2026-08-12T12:05:36+08:00
accepted_at: 2026-08-12T13:30:46+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1
closure_commit: https://github.com/maggnus/ymp/commit/d01354c6046b8a7c1fcd4380087f0e43805b24ff
evidence: ["[35cc659](https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1)", "[d01354c](https://github.com/maggnus/ymp/commit/d01354c6046b8a7c1fcd4380087f0e43805b24ff)"]
duration_minutes: 85
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

- [x] Every project requirement maps to a reviewable mockup, state variant, interaction annotation,
  explicit non-applicability statement, or documented open decision. An open decision that affects
  trust, authorization, verification, or a terminal outcome blocks acceptance rather than becoming
  a warning.
- [x] POC-1 and POC-2 designs cover 80 × 24, 120 × 40, and a wide high-volume case with
  keyboard-first and color-independent operation.
- [x] The design visibly distinguishes authoritative control, untrusted collaboration, independent
  verification, human intervention, enforced limits, observational values, and typed terminal
  outcomes.
- [x] A deliberately misleading fixture containing a fluent but causally irrelevant transcript,
  a central graph node, and a failed verifier cannot be presented as collective reasoning,
  leadership, or acceptance under the supplied visual semantics.

## Current state

Accepted. The sole HTML artifact supplies mechanical state and volume fixtures for the nine reusable
templates, explicit projection envelopes, independent command lifecycles, lost-reply and unread-
message controls, assurance states, three terminal-size classes, and deterministic handoff data.
Independent re-review confirmed all 138 typed fixtures, 35 boundary-volume cases, twelve command
previews, the local-only runtime boundary, and the negative semantic control.

## Next action

Perform W0-UX-01c against the exact accepted artifact revision and freeze the implementable screen
contract only if its semantic, accessibility, and terminal-feasibility checks pass.

## Guardrails

- The artifact owns its existing composition and visual language but does not own domain or
  protocol changes.
- A design omission becomes an explicit open decision or returned requirement; it is not hidden by
  a generic placeholder screen.
- Lost MCP replies and wake/quiescence with unread inert messages each have an explicit state; an
  unresolved version of either state blocks authorization and task acceptance.

## Findings

None. The returned one-item, projection-envelope, view-state, compound-command, and archive-subject
defects were corrected and independently rechecked.

## Closure

### Accepted outcome

The artifact is a projection-backed terminal design system rather than a set of static screens.
Seven applicable template families now distinguish zero, one, full, overflow, and large data sets;
every test fixture has a mechanical projection and view-state mapping; every command preview has
one command identity and a complete lifecycle.

### Residuals

The final independent contract freeze remains assigned to W0-UX-01c and is not a residue of this
accepted task.

### Evidence

- [Reviewed candidate](https://github.com/maggnus/ymp/commit/35cc659981f73700296c9ed37b378e59eabff4a1).
- [Integration commit](https://github.com/maggnus/ymp/commit/d01354c6046b8a7c1fcd4380087f0e43805b24ff).
