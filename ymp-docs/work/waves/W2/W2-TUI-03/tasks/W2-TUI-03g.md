---
id: W2-TUI-03g
kind: task
wave: W2
card: W2-TUI-03
state: ready
risk: routine
maturity: BUILD
relation: required
depends_on: []
blocks: [W2-TUI-03b]
created_at: 2026-09-06T12:52:44+08:00
updated_at: 2026-09-06T12:52:44+08:00
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

# W2-TUI-03g — CLI output includes the conversation status row

## Outcome

A one-shot CLI response prints the conversation status provided by the shared SurfaceSpec, as well as the complete body and prompt/footer. The row moved to input_left in a; the CLI consumer must not silently omit it. This changes presentation only.

## Scope

### In

Independent finding from f review: print_surface_spec emits header, body and status_left only. For conversation, status_left is now the prompt while input_left contains the actual operation/status. The visible state is therefore absent from command output. Input is f candidate 3fa266e with independent ACCEPT8; the CTO supplies a combined source revision before dispatch. No accepted-node prerequisite is required because this corrects the candidate before integration.

Exclusive write zone: ymp-rust/crates/ymp-cli/src/surface.rs and ymp-rust/crates/ymp-cli/tests/generated_verifier.rs. At most two files. Batch with a R2 under the same author only after f has finished; each node retains its own result and review depth.

### Out

No new state derivation, authorization, provider readiness, application/domain/runtime/storage/verifier logic, command inventory, dependencies or other tests. No rewording of the pre-existing standing-hint expectation to conceal its failure. Do not revive old store identifiers in user copy just to satisfy that old assertion. No real model or full suite.

## Acceptance

- [ ] The actual one-shot command output includes the shared conversation state row in the intended order and retains complete body text and prompt/footer. Empty page operation rows do not add misleading content; existing page footer remains.
- [ ] One narrow consumer check distinguishes omission of the state row and passes with the fix; no facts or cancellation capabilities are invented by CLI formatting.
- [ ] A negative variant that omits the new row fails for that reason. Existing independent standing-hint failure remains reported if still present.
- [ ] Narrow checks, package formatting/Clippy and diff checks pass; executable checks use fresh short /tmp project, HOME, YMP_HOME, TMPDIR, build and exports. Preserve compact logs.

## Current state

Ready for the same retained Sol author as a R2. f has finished writing and has independent acceptance. a/g will be the sole writers of their explicit combined zones.

## Next action

Restore the omitted status row and demonstrate it through command output.

## Guardrails

Use the existing shared projection and sanitization. Keep English application text and verbatim attributed Unicode. No new domain effects. Keep the separate a convergence requirement: its two existing consumer tests must pass without edits.

## Findings

This was classified independent-defect minor by f's reviewer. It is a separately owned outcome, not an extra hidden return on a or f.

## Review rounds

Recorded by the CTO ledger.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
