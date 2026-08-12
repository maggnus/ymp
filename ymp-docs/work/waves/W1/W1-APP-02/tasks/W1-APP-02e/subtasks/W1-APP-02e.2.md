---
id: W1-APP-02e.2
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: ready
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: []
created_at: 2026-08-13T01:26:13+08:00
updated_at: 2026-08-13T01:29:56+08:00
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

# W1-APP-02e.2 — Chat-first visual contract is independently accepted

## Outcome

The HTML source, fixed-layout PDF, and visual-concept rationale form one independently reviewed,
internally consistent chat-first terminal-interface contract that can replace the accepted
dashboard revision without weakening protocol or trust constraints.

## Scope

### In

- Semantic and visual review of `ymp-docs/VISUAL_CONCEPT.md`,
  `ymp-docs/design/ymp_chat_tui.dc.html`, and `ymp-docs/design/ymp_chat_tui.pdf`.
- Cross-format correspondence of named screens, fixtures, sizes, state variants, navigation,
  accessibility markers, and reusable structures.
- Traceability to `PROJECT-CONTRACT.md`, `PROTOCOL.md`, `INVARIANTS.md`, and the honest divergence
  table for concepts absent from the current domain.
- A frozen independent falsifier for missing required states, dangling local resources, format
  divergence, and trust-significant semantic contradictions.

### Out

- Ratatui implementation, production domain expansion, runtime-profile correction, and rewriting
  the accepted historical closure of `W0-UX-01` or `W1-APP-02e.1`.
- The 17 owner-provided Paseo screenshots under `ymp-docs/design/paseo/`. They are non-normative
  visual references for interface ideas, not contract sources or acceptance evidence.

## Acceptance

- [ ] Independent review confirms that the HTML and all 23 PDF pages describe the same named
  surfaces, deterministic fixture identities, terminal sizes, navigation model, and state
  distinctions, or returns an exact divergence to the author.
- [ ] Every local dependency and link required to read either primary format resolves from a clean
  checkout; the frozen checker rejects one removed dependency or deliberately broken local link.
- [ ] Every trust-significant action, unavailable capability, and future-only concept is explicit,
  keyboard-operable, distinguishable without colour, and consistent with the project contract,
  protocol, invariants, and documented divergence table.
- [ ] The reviewer confirms that the three current sources are self-contained and sufficient for
  W1-APP-02e.3 without consulting the removed dashboard artifact.

## Current state

Ready. The owner designated the chat-first HTML/PDF pair as the primary visual documents and
`VISUAL_CONCEPT.md` as their rationale. Initial CTO inspection confirmed that the PDF has 23
legible pages and exposed local-resource and historical-link questions for independent review.

## Next action

Prepare a frozen design falsifier before inspecting implementation changes, then review the exact
three-source revision.

## Guardrails

- This is a read-only design review. The reviewer must not edit the artifacts, production code,
  accepted historical work records, or negotiate a verdict with an implementer.
- No real model request, credential access, or production runtime execution is required.

## Findings

None until independent review.

## Closure

Not accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
