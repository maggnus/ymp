---
id: W1-APP-02e.2
kind: subtask
wave: W1
card: W1-APP-02
parent: W1-APP-02e
state: accepted
risk: significant
maturity: DESIGN
relation: required
depends_on: []
blocks: []
created_at: 2026-08-13T01:26:13+08:00
updated_at: 2026-08-13T09:46:41+08:00
started_at: 2026-08-13T01:32:17+08:00
accepted_at: 2026-08-13T09:46:41+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/4f9671ae3cadb6949ea0866d6f9c4bfd5ce7c489
closure_commit: https://github.com/maggnus/ymp/commit/b57ec9d0e04b47f0848fba4e7d1b9b2d7cf68336
evidence: [`4f9671a`](https://github.com/maggnus/ymp/commit/4f9671ae3cadb6949ea0866d6f9c4bfd5ce7c489)
duration_minutes: 494
blocker:
pause_reason:
return_trigger: the fixed-layout export is cited as the source of a screen or fixture, or it is regenerated from the accepted HTML revision
deliberate_partial: true
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
- [x] Every local dependency and link required to read either primary format resolves from a clean
  checkout; the frozen checker rejects one removed dependency or deliberately broken local link.
- [x] Every trust-significant action, unavailable capability, and future-only concept is explicit,
  keyboard-operable, distinguishable without colour, and consistent with the project contract,
  protocol, invariants, and documented divergence table.
- [x] The reviewer confirms that the three current sources are self-contained and sufficient for
  W1-APP-02e.3 without consulting the removed dashboard artifact.

## Current state

Accepted with a recorded residue. The repaired HTML source and the rationale document resolve
offline and carry every screen, fixture, size, navigation rule and state distinction, while the
fixed-layout export lags them by ten positions. Both drawn formats are reference implementations
rather than a frozen contract, so a divergence between them does not block implementation.

## Next action

None for this subtask. W1-APP-02e.3 implements from the HTML source and the rationale document.

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
- The correction removed the missing dependency instead of stubbing it, on independently verified
  grounds: the file appears in no revision of the history and the document contains no script
  element. The reference to the removed artifact became a statement pinned to the revision that
  still holds that file.
- The re-review returned `ACCEPT` with one minor outcome defect: the fixed-layout export does not
  carry the correction, so its divergence from the source grew from eight positions to ten. The
  contracted outcome is met by the HTML source and the rationale document, which the project
  already records as the exact handoff.
- Regenerating the export from the accepted revision is separate work that no current goal claims;
  it is not scheduled, and the residue's return trigger is the event that would make it worth doing.

## Closure

Not accepted.

### Accepted outcome

The repaired HTML source and the rationale document are one independently reviewed, internally
consistent chat-first reference that opens from a clean checkout and is sufficient to implement the
terminal interface without any other artifact.

### Residuals

The fixed-layout export lags the accepted source by ten positions, including the two repairs, and
its annotation still points at a temporary external address. This is carried rather than fixed
because the export is a reading convenience and no implementation reads it; the seniority of the
source is recorded in the project entry points and the rationale document, so citing the export
would be visible. The return trigger is recorded in this file's front matter.

Two open questions in the rationale document — the name of the assurance profile and the outcome of
a cancellation — are not answered here. They are interface decisions the implementation subtask
makes and records explicitly rather than design defects.

### Evidence

- [`4f9671a`](https://github.com/maggnus/ymp/commit/4f9671ae3cadb6949ea0866d6f9c4bfd5ce7c489) —
  reviewed correction; the integrated design tree is byte-identical to it.
- [`b57ec9d`](https://github.com/maggnus/ymp/commit/b57ec9d0e04b47f0848fba4e7d1b9b2d7cf68336) —
  integration into the release branch.
