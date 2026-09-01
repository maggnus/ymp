---
id: W1-COR-03e.1
kind: subtask
wave: W1
card: W1-COR-03
parent: W1-COR-03e
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-09-01T12:14:00+08:00
updated_at: 2026-09-01T13:00:00+08:00
started_at: 2026-09-01T12:15:03+08:00
accepted_at: 2026-09-01T13:00:00+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/183417da84ce7c9ee6cb53b1ece2a390539d54df
closure_commit: https://github.com/maggnus/ymp/commit/40315e299729ecdd4b3683c473fd42c5e93e57b7
evidence: ["[40315e2](https://github.com/maggnus/ymp/commit/40315e299729ecdd4b3683c473fd42c5e93e57b7)"]
duration_minutes: 45
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 4
escalation_decision: independent_review
---

# W1-COR-03e.1 — Application persists and resolves inert board payloads

## Outcome

The application atomically persists each inert collaboration payload before its board fact and
returns an owned operator projection whose visible messages resolve to the exact verified bytes,
without exposing object/board storage, mutable state, or model authority through product-facing APIs.

## Scope

### In

- The application publication seam for `Publish` plus exact payload bytes.
- Object-before-record ordering, digest and length validation, and owned resolved projections.
- Focused application tests for reopen, mutation isolation, and fail-closed mismatch or loss.
- The exact `ymp-testkit/src/lib.rs` call site that currently obtains `Application::object_store()`
  and a candidate path; it is replaced by a typed application verification operation.
- The exact `ymp-runtime-supervisor::start_candidate` and `ymp-tui::App::from_application` call
  sites that currently consume `Application::data_root()`.
- Removal of the generic root accessor. Trusted controller/runtime code may receive only the
  specific workspace or evidence path it already needs; TUI construction may receive the configured
  root from foreground composition, outside the board/operator projection.

### Out

- TUI rendering or visual behaviour, runtime scheduling semantics, agent-facing tools, new board
  semantics, automatic migration, and any causal interpretation of messages.
- Replacing accepted `InvocationRequest.workspace: PathBuf` or
  `WorkspaceSubmission.workspace: PathBuf` with a new cross-crate capability system.

## Acceptance

- [x] Exact payload bytes survive publication, reopen, and read through the typed application API.
- [x] The returned message and bytes are owned copies; mutating them changes neither a later read
      nor the persisted board or object.
- [x] A digest or length mismatch, and a missing or corrupt object, fail without changing the board
      audit or durable record position.
- [x] No generic `Application::data_root()`, object-store accessor, board accessor, or board/store
      path appears in the model-facing tool/RPC schema or owned operator projection. Managed runtime
      launch still receives its exact private workspace path and TUI startup receives the configured
      root through trusted foreground composition.
- [x] Existing board-section and board-projection checks, strict application Clippy, formatting,
      and `git diff --check` pass.

## Current state

Accepted at integrated main
[40315e2](https://github.com/maggnus/ymp/commit/40315e299729ecdd4b3683c473fd42c5e93e57b7).
Exact payloads are durable and resolved through an owned
projection; generic root/store/board access is closed, while trusted runtime composition receives
only the specific paths it already requires.

## Next action

Start W1-COR-03z on the accepted typed publication and projection seam.

## Guardrails

- Payload bytes remain inert object data and are never parsed by the kernel or board transition.
- An object is durable before its board fact; a later refusal may leave an unreferenced object but
  never a board record pointing to missing or partial bytes.
- No generic root, object/board path, `ObjectStore`, `BoardLedger`, or `BoardStore` crosses an
  operator projection, agent tool, RPC/MCP schema, or untrusted process boundary.
- A specific private workspace/evidence path may cross trusted application/runtime interfaces that
  already own filesystem authority; it does not grant board/object-store access to a participant.

## Findings

- R1 scope break was corrected: the exact `ymp-testkit` call now uses a typed application verifier.
- R2 break was branch topology, not a new product defect; the contaminated range was replaced by a
  clean candidate instead of ratifying unrelated paths.
- R3 major outcome defect: public `data_root()` still reveals storage paths; the independent
  compile-fail proof did not cover this equivalent accessor.
- R3 scope break: runtime launch and TUI construction also call `data_root()`; only those two exact
  call sites are added to the correction zone.
- R3 contract correction: an absolute no-path rule contradicts accepted runtime request types and
  the documented trusted foreground process; the external/model-facing boundary remains closed.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(7/10) RETURN 01/09 12:26 — public object-store and board access bypasses the typed seam → Sol
  correction assigned → candidate
  [ec45bce](https://github.com/maggnus/ymp/commit/ec45bce39c63ee25d8201dc5f741538e2e750d70)
  returned
- R2(6/10) ESCALATE 01/09 12:44 — correction range also carried unrelated mainline documents → R1
  finding closed but range rejected → clean two-commit candidate required
- CTO independent_review 01/09 12:46 — replacement review inspects the clean range only
- R3(6/10) RETURN 01/09 12:48 — public `data_root()` reconstructs storage paths → original Sol
  author found two out-of-zone consumers and stopped → exact call sites added before correction
- R4(9/10) ACCEPT 01/09 13:00 — generic root and equivalent store paths closed → trusted
  workspace/evidence paths retained by corrected contract → clean candidate integrated unchanged

## Closure

### Accepted outcome

Payload bytes are written before the corresponding board fact, validated against their digest and
length, recovered after reopen, and returned only as owned resolved messages. Missing or corrupt
objects fail closed. Public object-store, mutable-board, and generic-root accessors are absent;
trusted runtime and foreground TUI composition receive only their specific configured paths.

### Residuals

None.

### Evidence

- [40315e2](https://github.com/maggnus/ymp/commit/40315e299729ecdd4b3683c473fd42c5e93e57b7)
  — integrated code; its `ymp-rust` tree is byte-identical to reviewed candidate
  [183417d](https://github.com/maggnus/ymp/commit/183417da84ce7c9ee6cb53b1ece2a390539d54df).
