---
id: W2-TUI-03d
kind: task
wave: W2
card: W2-TUI-03
state: review
risk: significant
maturity: BUILD
relation: required
depends_on: [W2-UX-02]
blocks: [W2-TUI-03b]
created_at: 2026-09-06T04:47:56+08:00
updated_at: 2026-09-06T04:47:56+08:00
started_at: 2026-09-06T04:51:54+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/0af2354b15c91abc728c82e1cf315be3c9dd2a24
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 1
escalation_decision:
---

# W2-TUI-03d — Structured field values remain inert in both column and stacked layouts

## Outcome

Untrusted field label, class, value, unit, note and invalid-reason text is sanitized consistently before either column or stacked transcript layout. A layout choice cannot bypass the existing terminal-control filter.

## Scope

### In

Origin: independent-defect from Opus review of 8348652, transcript.rs column branch. This pre-existing defect did not cause a's RETURN; it is separately owned here and batched with a's correction because they change the same renderer and use the same proof context.

Exclusive files under ymp-rust/crates/ymp-tui: src/transcript.rs, tests/surfaces.rs, tests/state_binding.rs. These are already within a's zone; no additional source subsystem. The same Sol author may implement a+d together, but each acceptance is assessed separately. Read the current AGENTS and a's task. Do not modify the independent reviewer's external proof.

### Out

No Application, domain, provider, storage, runtime or authority change; no other security hardening or terminal-emulator behavior claim.

## Acceptance

- [ ] Both layout branches pass all externally supplied field strings through the existing sanitizer, and width/wrapping decisions use the displayed sanitized content.
- [ ] A bounded test exercises the column branch with a control-sequence-bearing field and rejects raw controls in the rendered representation; the stacked branch retains its protection and readable content.
- [ ] Before/after evidence distinguishes the old unfiltered branch from the fixed one without executing a harmful terminal sequence on the host.
- [ ] Narrow affected tests, package format/lint and diff checks pass. Share a's validation where identical; no second full suite or model run.

## Current state

Ready as a same-author batch with a's R1 correction. It must not be silently absorbed into a's original review verdict.

## Next action

Correct the field sanitation boundary and provide its separately attributable regression proof.

## Guardrails

Source Git worktree remains isolated. Any executable/runtime-boundary check uses a fresh disposable project, HOME, YMP_HOME, TMPDIR, build and export root. No real provider or real ~/.ymp.

## Findings

The source-level observation remains separate from any claim about physical terminal effects; the test checks the representation the renderer emits.

## Review rounds

Recorded by the CTO ledger.
- R1(9/10) ACCEPT 06/09 10:00 — Sanitized fields используются до измерения и выбора обеих ветвей; regression proof достаточен.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
