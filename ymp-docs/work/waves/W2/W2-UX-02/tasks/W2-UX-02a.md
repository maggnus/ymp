---
id: W2-UX-02a
kind: task
wave: W2
card: W2-UX-02
state: accepted
risk: routine
maturity: DESIGN
relation: expansion
depends_on: [W2-UX-02]
blocks: []
created_at: 2026-09-06T09:13:50+08:00
updated_at: 2026-09-06T09:13:50+08:00
started_at: 2026-09-06T09:20:57+08:00
accepted_at: 2026-09-06T10:00:29+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/27112d85c1be4f48b6c5909882d92ea6d22a400d
closure_commit: https://github.com/maggnus/ymp/commit/27112d85c1be4f48b6c5909882d92ea6d22a400d
evidence:
  - [Candidate 27112d85c1be](https://github.com/maggnus/ymp/commit/27112d85c1be4f48b6c5909882d92ea6d22a400d)
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 1
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

- [x] All application-owned text in each terminal frame is English; remaining non-English text is explicitly attributable input, path, source or external content, not a missed UI string.
- [x] The same 20 required states and five variants remain. Layout, keys, action semantics, scope of consent and true/unavailable state distinctions are preserved.
- [x] Regenerated PDF and changed PNGs correspond to the HTML; translated text has no clipping or overlap at the three declared sizes. No network assets.
- [x] Diff/path checks pass. Report a bounded inventory of preserved non-English content instead of a naive no-Cyrillic claim.

## Current state

Accepted at27112d8 after a Routine non-author CTO glance of the return, three-file scope, language rule and cold/provider/consequence PNGs. Application copy is English; verbatim content is retained. No production behavior is claimed by this reference change.

## Next action

Translate the interface copy and regenerate its fixed-layout reading version.

## Guardrails

No fake team, cost, knowledge or completion. Do not translate evidence silently. Keep the existing local render pipeline and a separate browser profile; no real model/provider execution.

## Findings

None.

## Review rounds

Recorded by the CTO ledger.
- R1(9/10) ACCEPT 06/09 10:00 — CTO Routine glance:3 разрешённых файла,25 кадров, английский app-owned текст;3 ключевых PNG осмотрены, Unicode user content сохранён, геометрия/семантика не менялись.

## Closure

### Accepted outcome

English app-owned copy in all25 terminal frames; verbatim content retained;18-page reading PDF regenerated.

### Residuals

Pending.

### Evidence

- [Candidate 27112d85c1be](https://github.com/maggnus/ymp/commit/27112d85c1be4f48b6c5909882d92ea6d22a400d)
