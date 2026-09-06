---
id: W2-TUI-03
kind: card
wave: W2
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W2-UX-02, W2-UX-02a]
blocks: []
created_at: 2026-09-06T02:26:51+08:00
updated_at: 2026-09-06T02:26:51+08:00
started_at: 2026-09-06T04:05:04+08:00
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

# W2-TUI-03 — The production TUI exposes the first coherent conversation and inspection path

## Outcome

Launching the product in a fresh disposable project shows the reviewed conversation shell, responsive Cyrillic input, one slash-command navigation model and truthful read-only inspection from real application projections. The owner sees rendered before/after evidence and can exercise the bounded slice.

## Invariants

No kernel, budget, authorization, verifier or provider-disclosure semantics change. No fake live participants, messages, knowledge or success. A board payload is inert; existing effects retain their own typed commands. Keep the existing one-executable distribution.

## Scope

Read the accepted W2-UX-02 handoff, USER_JOURNEY, ymp-tui state/projection/render/session modules and tests. Exclusive zone: ymp-rust/crates/ymp-tui/ and its production-preview example. No Application, domain, storage, runtime, verifier, Cargo lockfile, design or research changes. Validation-only child e owns one stale CLI test fixture. Child f owns the explicitly named CLI/shared-render boundary where the narrower transcript exposed silent clipping; it changes output composition only, not command semantics.

The accepted eight-production-file handoff is implemented in three observable atoms: frame/text, navigation/focus, and Application-backed board reading. Test and preview updates count in each explicit task zone (at most ten files). Shared source files make the sequence serial. Preserve supported commands; future capabilities stay expressly unavailable.

## Aggregate acceptance

- [ ] A production render and bounded session exercise show coherent cold start, text input, slash palette, entry into a real inspection page and Esc return with text preserved.
- [ ] Empty/loading/error and terminal status are readable at 80x24, 120x40 and 180x50; Cyrillic and wrapped text neither overlap nor disappear.
- [ ] Browsing/filtering changes no domain state. Unknown and unavailable commands return one concise reply with a next step and preserve the draft.
- [ ] Existing visible facts originate in Application projections. Untrusted board text cannot inject terminal controls or actions.
- [ ] Narrow affected tests and their meaningful negative cases pass; cargo fmt and package Clippy pass. The full workspace suite belongs only to final integration.
- [ ] One disposable product/session check isolates project, HOME, YMP_HOME, TMPDIR, build and exports. No real provider/model call or real ~/.ymp access. Production-rendered PNGs are opened for the owner.

## Current state

The English visual reference is integrated. Frame a and sanitation d have independent acceptance; fixture e has scoped CTO acceptance. Their combined integration check is running before serial navigation b and board c. This card cannot claim completion of the overall POC.

## Tasks

Six required outcomes: the original three implementation slices plus the bounded pre-existing field-sanitization defect discovered in a’s review. The new d outcome is batched with a’s correction and changes no new subsystem; a/d precede b, then c. Each retains its own acceptance. Child e restores the unchanged driver’s valid test input before combined integration. Child f removes the discovered CLI viewport loss before navigation starts. No extra architecture study is introduced.
- [W2-TUI-03a](tasks/W2-TUI-03a.md) — required
- [W2-TUI-03b](tasks/W2-TUI-03b.md) — required
- [W2-TUI-03c](tasks/W2-TUI-03c.md) — required
- [W2-TUI-03d](tasks/W2-TUI-03d.md) — required
- [W2-TUI-03e](tasks/W2-TUI-03e.md) — required
- [W2-TUI-03f](tasks/W2-TUI-03f.md) — required
- [W2-TUI-03f](tasks/W2-TUI-03f.md) — required

## Review rounds

Recorded by the CTO ledger.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
