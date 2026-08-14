---
id: W1-APP-02e
kind: task
wave: W1
card: W1-APP-02
state: active
risk: significant
maturity: BUILD
relation: required
depends_on: [W0-UX-01c, W1-APP-02b, W1-APP-02c, W1-APP-02d]
blocks: [W1-COR-03a]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-15T00:06:07+08:00
started_at: 2026-08-15T00:06:07+08:00
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 59
blocker: W1-APP-02c and W1-APP-02d are incomplete, and the new chat-first contract requires W1-APP-02e.2 review before implementation
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02e — TUI completes and exports a single-participant run

## Outcome

The ratatui interface exposes runtime readiness, run start, participant and obligation state,
candidate and verifier events, budgets, cancellation, terminal reason, and evidence export for one
complete participant attempt.

## Scope

### In

- Foreground startup, profile readiness, contract selection, run initiation, event-driven views,
  cancellation, terminal states, bounded logs, and evidence export.
- Views consume application projections and issue typed commands through the in-process adapter.
- The current chat-first visual contract after independent acceptance in `W1-APP-02e.2`, limited
  to the POC-1 semantics the domain actually supports.
- Automated TUI-state tests against the fake runtime plus one controlled run for each real profile.

### Out

- Multi-participant negotiation views, communication observatory analysis, interface polish,
  background attachment, web access, and remote control.

## Acceptance

- [ ] A user starting only `ymp` can select an approved contract and ready Codex or Claude Code
  profile, observe the attempt, and export exact candidate and verifier evidence.
- [ ] The same TUI path exposes an unavailable or unauthenticated profile before start and refuses
  to launch it rather than falling back to another route.
- [ ] Cancellation stops the managed runtime tree, records `cancelled`, and leaves no live attempt
  endpoint that can commit a later command.
- [ ] Corrupt evidence, runtime failure, verifier failure, or foreground-controller loss is shown
  as its typed terminal condition and never as candidate rejection or acceptance.
- [ ] Deterministic Ratatui state tests cover the accepted 80 × 24 and 120 × 40 contracts for first
  launch, mixed runtime readiness, running, cancellation, acceptance, candidate failure, budget
  exhaustion, and infrastructure failure without relying on color or mouse input.

## Current state

Every dependency is accepted: both managed profiles (02c, 02d), the chat-first contract and its
ratatui implementation (e.2, e.3), the assembled contract draft and slash surface (02v), entry
validation (02u, 02x, 02z). What remains is the end-to-end wiring the owner's live session
measured missing: after authorization the public surface stops at the recorded run, and the
attempt, verification and export had to be driven by internal commands.

## Absorbed findings

- The W1-COR-03n review: the interface cancel writes Command::Cancel into the journal directly
  (ymp-rust/crates/ymp-tui/src/app.rs:224) while the kernel lives in the supervising process, so
  the node must give the interface reachability of the kernel record, not merely forbid the
  bypass.
- The W1-APP-02w closure trigger: the interface refusal of a second run must address a fresh store
  under the single .ymp root instead of refusing outright.

## Next action

Wire the managed attempt, verification, terminal states and evidence export into the TUI and the
mirrored commands over the accepted application and supervisor surfaces.

## Guardrails

- Widgets do not own domain transition rules.
- The first TUI remains usable without a daemon, database service, or network listener.
- Human messages or interventions are explicitly attributed and exclude a run from autonomous
  experimental comparisons.

## Findings

None.

## Closure

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

- None until acceptance.
