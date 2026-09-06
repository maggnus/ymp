---
id: W2-TUI-03a
kind: task
wave: W2
card: W2-TUI-03
state: deferred
risk: significant
maturity: BUILD
relation: required
depends_on: [W2-UX-02]
blocks: [W2-TUI-03b]
created_at: 2026-09-06T03:49:08+08:00
updated_at: 2026-09-06T03:49:08+08:00
started_at: 2026-09-06T04:05:04+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/acaf81b92c2c333a7ba57e17d6f60d0b1f9a7b4d
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason: Owner rejected the current UI direction and closed this work
return_trigger: Only a new explicit owner instruction with revised direction; queued automation does not resume work
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

Exclusive files under ymp-rust/crates/ymp-tui: src/ui.rs, src/projection.rs, src/transcript.rs, src/text.rs; tests/surfaces.rs, tests/live_session.rs, tests/support/mod.rs, tests/typed_request.rs, tests/state_binding.rs; examples/preview.rs. Ten files maximum, only those actually needed. Preserve stable API fields where possible. Update tests only for intentional changed behavior and retain independent semantic assertions.

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

Stopped by the owner after rejection of the current design direction. Code and review evidence are preserved; this outcome is not claimed complete or integrated.

## Next action

None under the closed instruction. Only a new explicit owner request may authorize a revised direction.

## Guardrails

No Application/domain/runtime/verifier/storage/dependency or protected-oracle change. Do not implement Russian requirement generation, recruitment, knowledge or new effects. Preserve user files and existing supported commands. Every product/runtime-boundary check uses a fresh disposable root with separate project, HOME, YMP_HOME, TMPDIR, build and exports; never launch there from the source checkout or use real ~/.ymp.

## Findings

R2 outcome-defect: conversation_status reads working only when run is Some. Before any run, contract assembly loses the operation, advancing marker and Esc cancels; provider measurement falsely displays idle. Convergence: show working and its advancing marker independently of run, show the actual cancellation key exactly when working_ends_on_esc, and never show idle during working. The two existing consumer tests remain byte-unchanged. One separate validation migration is authorized: in typed_request, replace only the superseded cold-invitation copy expectation with the accepted English invitation while retaining every subsequent behavioral assertion. Its two other failures reproduce on the pre-frame baseline and are outside this correction. The never-written framework_inventory file is replaced in the ten-file zone by typed_request; no prior changed path is removed from accountability. The reviewer used the exact full-suite failures and byte-identical ui.rs, without rerunning tests.

## Review rounds

One record per substantive correction round, including its return and accepted delta. The original reports and pre-canonicalization record remain retained; ACCEPT deltas do not grant or consume an extra return.

- R1(9/10) ACCEPT 06/09 10:00 — Initial RETURN5 found hidden conversation, lost wide operation and invalid-width overflow. Corrected at cd5dbf6/0af2354; the unchanged independent example switched from failure to success and the three findings closed.
- R2(9/10) ACCEPT 06/09 13:41 — No-run work hid operation, animation and Esc. acaf81b passes both unchanged consumers, the negative variant and three rendered sizes. Two substantive returns spent; no further a correction.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
