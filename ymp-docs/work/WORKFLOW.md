# POC work rules

This tree is the execution record for the POC described in
[`ROADMAP.md`](../ROADMAP.md). Product semantics remain in the design documents; current work and
closure evidence live only in this tree.

## Product clock

**Nearest shippable outcome.** W2 delivers a coherent first terminal interaction slice and a
feasible cumulative-knowledge POC direction under
[the current owner instruction](backlog/OWNER-DIRECTION-20260906.md).

**Current critical path.** W2-DEF-01 → W2-UX-02 → W2-TUI-03. Read-only research may run
alongside preparation. Canonical-contract writers run alone; independent acceptance and review
capacity decide concurrency. The previous W1 path is retained as implementation/evidence history
and supplies components to the revised POC; it is not an automatic same-budget experimental gate.

The complete goal remains a coherent end-to-end TUI and a valid empirical POC conclusion. W2 is an
intermediate visible increment, and its closure does not claim that real transfer has been tested.

## Sources of truth

- One work unit is one permanent file under `waves/`; acceptance changes its state and closure
  fields without moving it.
- [`STATUS.md`](STATUS.md) and [`WAVES.md`](WAVES.md) are generated indexes and are never edited by
  hand.
- [`INVARIANTS.md`](../INVARIANTS.md) names the contracts whose violation can invalidate the POC.
- [`VISUAL_CONCEPT.md`](../VISUAL_CONCEPT.md),
  [`ymp_chat_tui.dc.html`](../design/ymp_chat_tui.dc.html), and
  [`ymp_chat_tui.pdf`](../design/ymp_chat_tui.pdf) are the current visual-design sources. Their
  semantic, coverage, accessibility, cross-format, and implementation-feasibility review is owned
  by `W1-APP-02e.2`. The accepted `W0-UX-01` record remains historical evidence for the superseded
  dashboard revision and is not rewritten as if it had reviewed the chat-first concept.
- The canonical source repository is `https://github.com/maggnus/ymp`, and the integration branch
  is `main`. Production Rust source lives under `ymp-rust/`. Commits use English
  `Conventional Commits` messages with scopes where useful.
- Repository commits and files are durable evidence only when links are pinned to full immutable
  commits. Accepted changes may be pushed directly to `origin/main`.

## Authority and review

- The CTO owns identifiers, dependencies, state changes, and closure records. A worker reads its
  assigned file and does not edit the work tree.
- Repository writers commit locally and never push. Push, publication, deployment, paid budget
  expansion, use of real credentials, and irreversible actions require an explicit owner decision.
- Every delegated outcome receives a non-author review before integration. The decompositions of
  `W0` and `W1` must each receive an independent `ACCEPT` before any card in the corresponding wave
  becomes active.
- The current `pending` plan-review state is a real stop condition, not an administrative label.

## Validation

The initial implementation validation ladder is:

1. the narrow test named in the active task;
2. `cargo test --workspace --all-targets` from `ymp-rust/`;
3. `cargo fmt --all -- --check` and
   `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
4. `cargo run -q -p ymp-evals` for the executable protocol model and its negative controls;
5. `cargo build --release -p ymp-cli` at a card boundary; and
6. the complete controlled POC procedure only at the `W1-EVL-04` and wave acceptance boundaries.

The work-tree generator and validator are supplied by the pinned `paseo-cto` plugin rather than a
repository-local `work.py` copy. Use version `11.0.1` installed from
`https://github.com/maggnus/agentic-plugins`.

Every acceptance check includes a negative control. A check whose deliberately invalid case has
not been observed to fail is not acceptance evidence.

## POC boundary

- The product interface is the foreground ratatui application in one installed Rust executable.
- Codex and Claude Code are the two required real runtime profiles; the fake deterministic runtime
  precedes both.
- SQLite, OpenCode, NVIDIA Nemotron, strict host containment, a daemon, server/client mode, web
  access, PostgreSQL, and Kubernetes are outside this wave.
- The POC runs only in an externally disposable environment and does not claim safe execution on
  an ordinary developer machine or an untrusted repository.
- No implementation may add a global semantic scheduler, permanent role assignment, grade-based
  task eligibility, prescribed escalation, or candidate ranking.

## Findings and closure

A finding remains in its task only when it shares the same outcome, scope, risk, and acceptance
evidence. Independently assignable work receives a new stable identifier before execution.
`Current state` is rewritten and bounded; history belongs to Git and the evidence package.

A task is accepted only after its checklist, negative control, non-author review, closure commit,
and durable evidence are recorded. After the second return, the work is split, accepted with an
explicitly bounded limitation, or stopped at a named gate.
