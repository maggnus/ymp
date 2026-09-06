---
id: W2-TUI-03b
kind: task
wave: W2
card: W2-TUI-03
state: ready
risk: significant
maturity: BUILD
relation: required
depends_on: [W2-TUI-03a, W2-TUI-03d]
blocks: [W2-TUI-03c]
created_at: 2026-09-06T03:49:08+08:00
updated_at: 2026-09-06T03:49:08+08:00
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

# W2-TUI-03b — Slash navigation preserves input and performs no action while browsing

## Outcome

A user can enter Cyrillic text, open/filter the slash palette, inspect a supported page and return with the draft preserved. Browsing has no domain effect and unavailable/unknown commands have one clear response.

## Scope

### In

Read AGENTS, accepted UX-02 and a's integrated code. Exclusive files under ymp-rust/crates/ymp-tui: src/state.rs, src/overlay.rs, src/pages.rs, src/app.rs; tests/command_prefix.rs, tests/surfaces.rs, tests/typed_request.rs, tests/authorization_weight.rs, tests/support/mod.rs. Nine files maximum.

### Out

No new authorization, provider-disclosure, cancellation, verification or apply semantics. No board backend wiring (c), model recruitment, knowledge, Russian check generation or redesign of a's frame.

## Acceptance

- [ ] All application-authored interface text in this slice is English. User input, filenames,
      source quotations and attributed external content remain verbatim; Unicode is not banned.

- [ ] The slash palette handles empty/filter/selected/unavailable/no-match states consistently. Selection/filtering alone returns no mutating Action.
- [ ] Esc removes only the top layer and restores focus and text; data-page return goes to the recorded source. Enter executes only the explicit supported choice.
- [ ] Unknown/unavailable inputs preserve the draft and show one concise useful reply. Existing supported commands remain reachable; future features do not receive a fake success path.
- [ ] A test traverses actual key handling through page opening and back, and observes no domain journal change while browsing. A removed Esc return or unintended action is detected.
- [ ] Narrow affected tests, format and package Clippy pass. One disposable consumer-path check covers the navigation change; full suite deferred to final integration.

## Current state

Awaiting a's integrated frame. The existing navigation is retained until this bounded replacement is reviewed.

## Next action

Implement the accepted navigation/focus contract on the integrated frame.

## Guardrails

No Application/domain/runtime/verifier/storage/dependency or protected-oracle change. Do not implement Russian requirement generation, recruitment, knowledge or new effects. Preserve user files and existing supported commands. Every product/runtime-boundary check uses a fresh disposable root with separate project, HOME, YMP_HOME, TMPDIR, build and exports; never launch there from the source checkout or use real ~/.ymp.

## Findings

UX-02 review minor: the /pool fixture says that complete limit enforcement is not yet promised. When this page is implemented, show available facts/actions without speaking in roadmap or implementation terms. This is an authorized bounded wording correction; do not change real limit semantics.

## Review rounds

Recorded by the CTO ledger.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
