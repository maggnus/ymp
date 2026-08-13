---
id: W1-APP-02e.2
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: rework
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: []
created_at: 2026-08-13T01:26:13+08:00
updated_at: 2026-08-13T09:25:00+08:00
started_at: 2026-08-13T01:32:17+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/b3005e29efc695cf29de4d006b5d8a489c4794ac
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

Returned by independent review. The three sources are consistent in composition, protocol
correspondence, keyboard operability and colour-independent distinction, but neither primary format
is self-contained: the HTML depends on a support file that does not exist in the repository, and
both formats still lead the reader to the removed dashboard artifact. The reviewer froze a
resolution checker with an observed negative half before inspecting content.

## Next action

Repair the two blockers and the minor record defects inside the design sources, then re-review with
the same frozen checker. The divergence between the two primary formats needs an owner decision on
which format carries authority before it can be closed.

## Guardrails

- This is a read-only design review. The reviewer must not edit the artifacts, production code,
  accepted historical work records, or negotiate a verdict with an implementer.
- No real model request, credential access, or production runtime execution is required.

## Findings

- `blocker`, defect in the contracted outcome. The HTML source declares a local dependency
  `./support.js` that exists neither in the working tree nor in the index, so the primary format
  does not open from a clean checkout.
- `blocker`, defect in the contracted outcome. Both primary formats route the reader to the removed
  dashboard artifact: the HTML link does not resolve, and the PDF annotation points at a temporary
  external address that carries no durable guarantee.
- `major`, defect in the contracted outcome. The PDF is missing five headings and three paragraphs
  that the HTML contains, including the 120x40 reference size and the distinction between a page and
  a modal. The two primary formats therefore describe different contracts, and an implementer
  reading only the PDF would miss required states.
- `major`, additional work. Open questions two and three in the rationale document — the name of the
  assurance profile and the outcome of a cancellation — leave the implementation subtask unable to
  choose two interface lines. Each needs a decision recorded in the contract, not in the design.
- `minor`, defect in the contracted outcome. Fixture `vr-19` appears on page one but not in the
  fixture table on page twenty-two; the rationale document cites a bare commit identifier instead of
  a source link; the divergence table names one provider profile and omits another.
- The reviewer's observation that the local reference folder is absent from the repository is
  answered by an existing decision: those files are non-normative local references, deliberately
  untracked, and now ignored by
  [`6d73501`](https://github.com/maggnus/ymp/commit/6d7350179fde85e3778e73d8638d660ce24a9c70).

## Closure

Not accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
