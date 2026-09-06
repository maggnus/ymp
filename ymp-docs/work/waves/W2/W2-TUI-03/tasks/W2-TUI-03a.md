---
id: W2-TUI-03a
kind: task
wave: W2
card: W2-TUI-03
state: review
risk: significant
maturity: BUILD
relation: required
depends_on: [W2-UX-02]
blocks: [W2-TUI-03b]
created_at: 2026-09-06T03:49:08+08:00
updated_at: 2026-09-06T03:49:08+08:00
started_at: 2026-09-06T04:05:04+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/0af2354b15c91abc728c82e1cf315be3c9dd2a24
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 2
escalation_decision:
---

# W2-TUI-03a — Conversation frame and Cyrillic text remain readable at three terminal sizes

## Outcome

The production conversation frame becomes the first visible implementation of the accepted design. Cold, active, empty and terminal views keep Cyrillic input and material facts readable at 80x24,120x40,180x50. This atom changes presentation, not application behavior.

## Scope

### In

Read AGENTS, accepted W2-UX-02 source `e1d6afa71190221e2d52fa2d129d05b865466d37` (integrated at `5d29f0d1d8242b14d1ecd6905f4c6f673661993f`), VISUAL_CONCEPT and the current TUI test helpers. The geometry/contrast already accepted by the design review is the reference.

Exclusive files under ymp-rust/crates/ymp-tui: src/ui.rs, src/projection.rs, src/transcript.rs, src/text.rs; tests/surfaces.rs, tests/live_session.rs, tests/support/mod.rs, tests/framework_inventory.rs, tests/state_binding.rs; examples/preview.rs. Ten files maximum, only those actually needed. Preserve stable API fields where possible. Update tests only for intentional changed behavior and retain independent semantic assertions.

### Out

Slash-command state changes and a new board page belong to b/c. Application-generated body text and future goal interpretation are not fabricated by the renderer. Existing supported pages stay usable. No new design composition or generic presentation framework.

## Acceptance

- [ ] All application-authored interface text in this slice is English. User input, filenames,
      source quotations and attributed external content remain verbatim; Unicode is not banned.

- [ ] Production rendering gives conversation/input prominence, a bounded context header and compact truthful status; no technical maturity commentary or invented work appears.
- [ ] Empty, working and terminal views are readable at all3 sizes; Cyrillic, paths and long words neither overlap nor disappear, and input remains visible.
- [ ] Dynamic content stays derived from the existing projection. Unknown cost/status is not displayed as zero or success. Control characters cannot turn content into terminal instructions.
- [ ] Narrow affected TUI tests pass, including meaningful no-overlap/state-honesty cases. Cargo fmt --check and package Clippy pass; no full workspace suite here.
- [ ] One fresh disposable preview/session exercise uses the production render path. Show PNGs of changed cold/active/terminal views; mark fixtures as such, not live model evidence.

## Current state

UX-02 is accepted; Opus independently accepted this first code contract in the same handoff review. Start only this atom. The whole TUI card remains open until b/c.

## Next action

Implement and show the bounded conversation frame from the accepted design.

## Guardrails

No Application/domain/runtime/verifier/storage/dependency or protected-oracle change. Do not implement Russian requirement generation, recruitment, knowledge or new effects. Preserve user files and existing supported commands. Every product/runtime-boundary check uses a fresh disposable root with separate project, HOME, YMP_HOME, TMPDIR, build and exports; never launch there from the source checkout or use real ~/.ymp.

## Findings

None yet. A newly necessary independent outcome is reported before scope grows.

## Review rounds

Recorded by the CTO ledger.
- R1(5/10) RETURN 06/09 04:47 — Стартовое приглашение скрывает реальные реплики; на широком экране теряется текущая операция; длинная причина invalid выходит за границу. Независимый исполняемый пример ещё не запущен.
- R2(9/10) ACCEPT 06/09 10:00 — [delta] Независимый неизменённый пример перешёл от exit1 на8348652 к exit0 на0af2354; диалог и wide busy сохранены. Интеграция отдельно ожидает проверки.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
