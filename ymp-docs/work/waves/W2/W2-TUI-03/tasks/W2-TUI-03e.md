---
id: W2-TUI-03e
kind: task
wave: W2
card: W2-TUI-03
state: ready
risk: routine
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-09-06T10:00:29+08:00
updated_at: 2026-09-06T10:00:29+08:00
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

# W2-TUI-03e — Claude integration fixture advertises the currently admitted tool catalogue

## Outcome

The zero-model Claude product-path fixture describes the actual coordination tool catalogue required by the unchanged strict driver. Its positive managed-path tests pass while the stale catalogue remains rejected. This corrects test input, not production admission policy.

## Scope

### In

Origin: integration of a/d reached five claude_product_path failures. The exact generated_claude_environment_completes_a_candidate_through_product_mcp failure also occurs on pre-TUI baseline7b2d5e9 in a fresh short-root environment. runtime-claude's coordination_tools always includes read_board and publish, whereas COORDINATED_INIT in the CLI test file omits both. request_participant remains capability-dependent; do not blindly add every tool.

Exclusive write zone: ymp-rust/crates/ymp-cli/tests/claude_product_path.rs. Read its fixture and runtime-claude's tool catalogue/init validation. Existing source evidence is in the integration logs named by the CTO dispatch.

### Out

No production Rust changes, gate weakening, skipped assertions, real provider/model, full workspace suite, other test files, dependencies, work records or unrelated cleanup. If updating the fixture does not explain the failure, report the actual remaining cause rather than widening scope.

## Acceptance

- [ ] The fixture's initialized tool list matches the current generated coordinated profile, including its capability-dependent exclusions.
- [ ] All seven tests in claude_product_path pass in a fresh short disposable project/home/YMP_HOME/tmp/build/export environment, without any real model or operator ~/.ymp.
- [ ] A controlled stale-catalogue variant still fails while the corresponding valid fixture succeeds; production driver bytes and strict matching remain unchanged.
- [ ] Package formatting/lint and diff checks pass. Do not run the full suite: the CTO owns the combined integration check.

## Current state

Ready. This validation-only child unblocks the combined integration; it adds no product subsystem to the TUI implementation.

## Next action

Correct only the stale initialized-tool fixture and prove the positive/negative pair.

## Guardrails

Use /tmp with a short unique root: the first integration harness's long macOS TMPDIR exceeded Unix socket limits. Preserve failed observations and source identities. No real Claude runtime is authorized by this task.

## Findings

Source mismatch is a diagnosis to verify, not a permission to force tests green.

## Review rounds

Recorded by the CTO ledger.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
