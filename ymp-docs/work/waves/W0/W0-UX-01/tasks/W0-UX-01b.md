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
updated_at: 2026-08-12T13:01:00+08:00
started_at: 2026-08-12T12:05:36+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/55452e94c62e02d3aa18c63de9a36ee69c2ae346
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

The first candidate preserves the nine projection-backed templates and supplies the intended state,
size, command, lost-reply, unread-message, assurance, and handoff data in the sole HTML artifact.
Independent review confirmed the template structure, responsive layout, negative semantics, and
three of the blocking gaps, but returned three bounded fixture-contract defects; author rework is
active in the original workspace.

## Next action

Add explicit one-item volume cases, make every fixture's projection and view-state mapping
mechanical, split compound command previews into independent command lifecycles, correct the
archive subject, and repeat the preserved independent review on the corrected candidate.

## Guardrails

- The artifact owns its existing composition and visual language but does not own domain or
  protocol changes.
- A design omission becomes an explicit open decision or returned requirement; it is not hidden by
  a generic placeholder screen.
- Lost MCP replies and wake/quiescence with unread inert messages each have an explicit state; an
  unresolved version of either state blocks authorization and task acceptance.

## Findings

- `BLOCKER`: `TEST-007` requires zero, one, exactly-full, +1, and large volumes, but the candidate
  has no unambiguous one-item fixture.
- `MAJOR`: 21 fixture rows use view-state variants outside the declared `ScreenProjection` enum,
  and 17 rows cannot yield `FixtureEnvelope.projection_id` without interpreting prose or identifiers.
- `MAJOR`: pause/resume and amendment previews combine distinct commands under one `command_id`;
  the archive rejection also names a different run from the command subject.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
