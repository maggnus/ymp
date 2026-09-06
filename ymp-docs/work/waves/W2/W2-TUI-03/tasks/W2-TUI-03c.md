---
id: W2-TUI-03c
kind: task
wave: W2
card: W2-TUI-03
state: deferred
risk: significant
maturity: BUILD
relation: required
depends_on: [W2-TUI-03b]
blocks: []
created_at: 2026-09-06T03:49:08+08:00
updated_at: 2026-09-06T03:49:08+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason: Owner rejected the current UI direction and closed this work
return_trigger: Only a new explicit owner instruction with revised direction; queued automation does not resume work
deliberate_partial: false
review_rounds: 0
escalation_decision:
---

# W2-TUI-03c — Operator reads attributed inert board messages through Application

## Outcome

The operator can open /board and inspect real attributed messages from Application::operator_board_projection, including audience and empty/error states, without mutation or interpretation as authority.

## Scope

### In

Read AGENTS, accepted UX-02 and b's integrated code. Session already owns Arc<Mutex<Application>>; the operator projection is public and ymp-tui already depends on ymp-application (independently checked in UX review).

Exclusive files under ymp-rust/crates/ymp-tui: src/app.rs, src/projection.rs, src/state.rs, src/pages.rs; tests/board_projection.rs (new if needed), tests/live_session.rs, tests/surfaces.rs, tests/state_binding.rs; examples/preview.rs. Nine files maximum. Use the typed Application projection, never private store parsing or test-only injected state in place of the product binding.

### Out

No human board publishing, automatic actions from messages, additional audience grants, new participant semantics, knowledge-memory objects or Application/domain changes.

## Acceptance

- [ ] All application-authored interface text in this slice is English. User input, filenames,
      source quotations and attributed external content remain verbatim; Unicode is not banned.

- [ ] /board reads the production Application projection and shows exact attributable content/audience, with readable long text and explicit empty/unavailable/error states.
- [ ] Opening, filtering, selecting and closing the page changes no domain state or permissions. Untrusted message bytes cannot create terminal controls, links that execute or commands.
- [ ] An Application-backed test creates genuine board evidence and reaches it through Session and the page; unauthorized/invalid or control-character payload cases remain bounded and inert.
- [ ] The full first-slice navigation is walked through the production rendering/session path with a disposable root and no real model traffic. Changed board PNGs are shown.
- [ ] Narrow tests, format and package Clippy pass. The closing integration of W2 runs the full workspace suite once in a disposable evaluation root and records any unmet overall-PoC requirements honestly.

## Current state

Stopped by the owner after rejection of the current design direction. Code and review evidence are preserved; this outcome is not claimed complete or integrated.

## Next action

None under the closed instruction. Only a new explicit owner request may authorize a revised direction.

## Guardrails

No Application/domain/runtime/verifier/storage/dependency or protected-oracle change. Do not implement Russian requirement generation, recruitment, knowledge or new effects. Preserve user files and existing supported commands. Every product/runtime-boundary check uses a fresh disposable root with separate project, HOME, YMP_HOME, TMPDIR, build and exports; never launch there from the source checkout or use real ~/.ymp.

## Findings

None yet. A newly necessary independent outcome is reported before scope grows.

## Review rounds

Recorded by the CTO ledger.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
