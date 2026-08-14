---
id: W1-COR-03o
kind: task
wave: W1
card: W1-COR-03
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-COR-03n]
blocks: []
created_at: 2026-08-15T00:03:05+08:00
updated_at: 2026-08-15T00:34:25+08:00
started_at: 2026-08-15T00:03:21+08:00
accepted_at: 2026-08-15T00:34:25+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/3ee3942beb0c231f99a6d319372a400c5d760a90
closure_commit: https://github.com/maggnus/ymp/commit/d6ec0f7
evidence: cancel visible under the completion mutex decides the terminal regardless of the runtime's last report; reviewer reproduced both serialization mutations (exit 101 each) and the base divergence
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-COR-03o — Cancel during a running tool call terminates as cancelled, not as infrastructure error

## Outcome

A cancel that causes a tool-call failure yields Cancelled in both accountings; the negative half is the measured InfrastructureError divergence. A control-side serialization test that fails when take_terminal leaves cancel or verified (review finding 3).

## Scope

### In

- ymp-rust/crates/ymp-runtime-supervisor.

### Out

- The completion serialization accepted in W1-COR-03n.

## Acceptance

- [ ] A cancel that causes a tool-call failure yields Cancelled in both accountings; the negative half is the measured InfrastructureError divergence. A control-side serialization test that fails when take_terminal leaves cancel or verified (review finding 3).

## Current state

Ready. The worker classifies the terminal by the tool failure before reading the cancellation flag (lib.rs:1020-1027 at bcfddbc), so a cancel that makes a tool call fail records InfrastructureError in the kernel while the journal says Cancelled — measured 2/48 in the review sweep, identically on the base.

## Findings

None yet.

## Closure

Filled when the task is accepted.

### Accepted outcome

Not accepted.

### Residuals

None recorded.

### Evidence

None recorded.
