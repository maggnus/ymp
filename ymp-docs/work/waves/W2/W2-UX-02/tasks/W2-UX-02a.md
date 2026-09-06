---
id: W2-UX-02a
kind: task
wave: W2
card: W2-UX-02
state: ready
risk: routine
maturity: DESIGN
relation: expansion
depends_on: [W2-UX-02]
blocks: []
created_at: 2026-09-06T09:13:50+08:00
updated_at: 2026-09-06T09:13:50+08:00
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
review_rounds: 0
escalation_decision:
---

# W2-UX-02a — English interface copy carries the accepted terminal screen contract

## Outcome

The authoritative HTML/PDF reference and visual handoff match the owner's English-only interface decision. Existing geometry, interaction and permission semantics remain unchanged; quoted input and other verbatim content retain Unicode and their source language.

## Scope

### In

Owner instruction: AGENTS Product language and OWNER-DIRECTION-20260906 interface-language clarification, recorded at 88c7539f44703e5a316c5f7e8c53f25549994d39. Read the accepted visual source e1d6afa and current three design files.

Exclusive zone: ymp-docs/VISUAL_CONCEPT.md, ymp-docs/design/ymp_chat_tui.dc.html, ymp-docs/design/ymp_chat_tui.pdf. Translate application-owned text inside terminal frames: labels, headings, menus, prompts, help, statuses, errors, confirmations and application replies. Narration outside the frames may remain Russian. Add a concise language rule to VISUAL_CONCEPT. Keep the original Russian user goal and real example path/content data verbatim. Do not replace Unicode tests with an ASCII-only rule.

### Out

No new screen design, interaction, authorization, participant or memory behavior. No Rust or test code, other canonical documents, work records, model experiments or Claude Design request. The code author independently applies the same owner instruction; write zones are disjoint and neither writer consumes the other's unreviewed edits.

## Acceptance

- [ ] All application-owned text in each terminal frame is English; remaining non-English text is explicitly attributable input, path, source or external content, not a missed UI string.
- [ ] The same 20 required states and five variants remain. Layout, keys, action semantics, scope of consent and true/unavailable state distinctions are preserved.
- [ ] Regenerated PDF and changed PNGs correspond to the HTML; translated text has no clipping or overlap at the three declared sizes. No network assets.
- [ ] Diff/path checks pass. Report a bounded inventory of preserved non-English content instead of a naive no-Cyrillic claim.

## Current state

Ready under the direct owner instruction. This is a new copy revision; the earlier accepted design evidence remains historical and is not rewritten as if English copy had already been inspected.

## Next action

Translate the interface copy and regenerate its fixed-layout reading version.

## Guardrails

No fake team, cost, knowledge or completion. Do not translate evidence silently. Keep the existing local render pipeline and a separate browser profile; no real model/provider execution.

## Findings

None.

## Review rounds

Recorded by the CTO ledger.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
