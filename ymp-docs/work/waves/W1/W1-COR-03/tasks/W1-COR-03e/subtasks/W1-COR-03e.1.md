---
id: W1-COR-03e.1
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: rework
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-09-01T12:14:00+08:00
updated_at: 2026-09-01T12:50:00+08:00
started_at: 2026-09-01T12:15:03+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/ff0dae3d54f01b9c1ba3d0a244d217a714bfb0cf
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 2
escalation_decision: independent_review
---

# W1-COR-03e.1 — Application persists and resolves inert board payloads

## Outcome

The application atomically persists each inert collaboration payload before its board fact and
returns an owned operator projection whose visible messages resolve to the exact verified bytes,
without exposing storage, a mutable board, or authority to the TUI.

## Scope

### In

- The application publication seam for `Publish` plus exact payload bytes.
- Object-before-record ordering, digest and length validation, and owned resolved projections.
- Focused application tests for reopen, mutation isolation, and fail-closed mismatch or loss.
- The exact `ymp-testkit/src/lib.rs` call site that currently obtains `Application::object_store()`
  and a candidate path; it is replaced by a typed application verification operation.
- The exact `ymp-runtime-supervisor::start_candidate` and `ymp-tui::App::from_application` call
  sites that currently consume `Application::data_root()`; they may be replaced only by typed
  application operations or owned values that do not reveal the storage root.

### Out

- TUI rendering or visual behaviour, runtime scheduling semantics, agent-facing tools, new board
  semantics, automatic migration, and any causal interpretation of messages.

## Acceptance

- [ ] Exact payload bytes survive publication, reopen, and read through the typed application API.
- [ ] The returned message and bytes are owned copies; mutating them changes neither a later read
      nor the persisted board or object.
- [ ] A digest or length mismatch, and a missing or corrupt object, fail without changing the board
      audit or durable record position.
- [ ] Existing board-section and board-projection checks, strict application Clippy, formatting,
      and `git diff --check` pass.

## Current state

The clean range resolved R2's branch contamination. R3 found `Application::data_root()`; the author
then found two necessary consumers outside the former zone and stopped without changes. Those exact
runtime-supervisor and TUI call sites are now authorized for typed replacement.

## Next action

Replace all three `data_root()` callers, close the accessor, and return one bounded commit.

## Guardrails

- Payload bytes remain inert object data and are never parsed by the kernel or board transition.
- An object is durable before its board fact; a later refusal may leave an unreferenced object but
  never a board record pointing to missing or partial bytes.
- No path, object store, `BoardLedger`, or `BoardStore` crosses the application API.

## Findings

- R1 scope break was corrected: the exact `ymp-testkit` call now uses a typed application verifier.
- R2 break was branch topology, not a new product defect; the contaminated range was replaced by a
  clean candidate instead of ratifying unrelated paths.
- R3 major outcome defect: public `data_root()` still reveals storage paths; the independent
  compile-fail proof did not cover this equivalent accessor.
- R3 scope break: runtime launch and TUI construction also call `data_root()`; only those two exact
  call sites are added to the correction zone.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(7/10) RETURN 01/09 12:26 — public object-store and board access bypasses the typed seam → Sol
  correction assigned → candidate `ec45bce` returned
- R2(6/10) ESCALATE 01/09 12:44 — correction range also carried unrelated mainline documents → R1
  finding closed but range rejected → clean two-commit candidate required
- CTO independent_review 01/09 12:46 — replacement review inspects the clean range only
- R3(6/10) RETURN 01/09 12:48 — public `data_root()` reconstructs storage paths → original Sol
  author found two out-of-zone consumers and stopped → exact call sites added before correction

## Closure

Filled when the subtask is accepted.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
