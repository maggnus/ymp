---
id: W2-UX-02
kind: card
wave: W2
state: accepted
risk: significant
maturity: DESIGN
relation: required
depends_on: [W2-DEF-01]
blocks: [W2-TUI-03]
created_at: 2026-09-06T02:26:51+08:00
updated_at: 2026-09-06T02:26:51+08:00
started_at: 2026-09-06T03:11:50+08:00
accepted_at: 2026-09-06T04:00:27+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/e1d6afa71190221e2d52fa2d129d05b865466d37
closure_commit: https://github.com/maggnus/ymp/commit/5d29f0d1d8242b14d1ecd6905f4c6f673661993f
evidence:
  - [Candidate e1d6afa71190](https://github.com/maggnus/ymp/commit/e1d6afa71190221e2d52fa2d129d05b865466d37)
duration_minutes: 0
blocker:
pause_reason:
return_trigger: Implement W2-TUI-03b.
deliberate_partial: true
review_rounds: 2
escalation_decision:
---

# W2-UX-02 — The complete user journey has one coherent terminal interaction design

## Outcome

The owner can inspect the entire terminal journey as one designed product: Russian goal, clarification, preparation, real work, participants, knowledge/evidence, external result, revision and explicit application. Reusable interaction rules make the first implementation slice unambiguous.

## Invariants

The main surface is a conversation and compact observed progress. No mandatory internal identifiers, role assignment, model ranking, fabricated team, executable board text or false acceptance. Exact effects retain the application's authority; informational navigation has no side effect. Do not send Claude Design requests or recover removed design material.

## Scope

Read USER_JOURNEY, DESIGN_CONTEXT, revised CONCEPT (definition accepted at `6cc81764989887e3ec7ffd0ec8580ea6db389c45`), current VISUAL_CONCEPT and HTML/PDF, existing TUI state/projection/navigation, and Application::operator_board_projection. The rejected 893a7d1 is not a starting template.

Exclusive write zone: ymp-docs/VISUAL_CONCEPT.md, ymp-docs/design/ymp_chat_tui.dc.html, ymp-docs/design/ymp_chat_tui.pdf. Temporary local render evidence may be outside the repository. No product code, research decisions, work records or new runtime authority.

Design a complete sequence and explicitly mark implemented, first-slice and later behavior. Specify input/focus, / palette, Esc stack, information and consequence modals, wrapping, empty/loading/error/ended states, and on-demand providers/pool/agents/board/result/knowledge inspection. Match the latest user path; retain the restrained visual character only where useful. The knowledge view is explicitly unavailable until a domain projection exists. No technology choice is smuggled through fixtures.

## Aggregate acceptance

- [x] A complete readable journey from the exact Russian goal to a result and revision exists; technical settings are on demand.
- [x] 80x24, 120x40 and 180x50 have a deliberate layout with readable Cyrillic, bounded chrome, no overlap and consistent navigation/focus rules.
- [x] Empty, active, failure, cancellation and terminal result states show truthful next actions; data fixtures map to real or expressly unavailable projections.
- [x] HTML is the exact screen/state/fixture authority; PDF is generated locally without network assets and visually inspected. VISUAL_CONCEPT states the same semantics.
- [x] First implementation slice is named precisely: conversation shell, slash navigation, compact status and existing read-only inspection; unavailable intent/knowledge/multi-participant behavior is labelled.
- [x] Read-only independent review walks the screens and a contrary fixture (fabricated active team or selection causing a mutation) is rejected by the documented interaction contract. Link/path and diff checks pass.

## Current state

Accepted after Opus delta review9/10 at e1d6afa, integrated at5d29f0d. The design is an18-sheet reference with20 required states and5 variants; production code and empirical POC are not accepted by this card.

## Tasks

One design atom. No full workspace suite or model experiment. Return changed screen images and a precise production-slice handoff.
- [W2-UX-02a](tasks/W2-UX-02a.md) — expansion

## Review rounds

Recorded by the CTO ledger.
- R1(6/10) RETURN 06/09 03:44 — Внутри терминальных кадров остались комментарии о зрелости реализации и калька «материальный». Первый срез в8 файлах исполним; геометрия принята.
- R2(9/10) ACCEPT 06/09 03:58 — [delta] Служебный текст и кальки удалены; геометрия и точные PNG сохранены. Первый договор03a стартуем; остаток страницы пула передан03b.

## Closure

### Accepted outcome

Independent Opus delta ACCEPT9/10:20 states plus5 variants on18 sheets, coherent user journey and8-file handoff. Internal implementation text and calques removed. DESIGN only; no implemented TUI claim.

### Residuals

- Minor /pool wording is carried by W2-TUI-03b; production implementation and empirical POC remain unverified.

### Evidence

- [Candidate e1d6afa71190](https://github.com/maggnus/ymp/commit/e1d6afa71190221e2d52fa2d129d05b865466d37)
