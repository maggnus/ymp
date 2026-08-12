# POC work rules

This tree is the execution record for the POC described in
[`ROADMAP.md`](../ROADMAP.md). Product semantics remain in the design documents; current work and
closure evidence live only in this tree.

## Product clock

**Nearest shippable outcome.** A controlled, matched-budget experiment can decide whether local
self-organization improves independently accepted results and whether any observed communication
is causally useful.

**Scientific critical path.** `W1-EXP-01` → `W1-APP-02` → `W1-COR-03` →
`W1-EVL-04`.

**Interface-design path.** `W0-UX-01` → `W1-APP-02e` → `W1-COR-03e`. `W0` may proceed in
parallel with the falsification package but must be accepted before either POC screen task starts.

Only the current critical-path head is eligible for execution. Parallel work is admitted only when
it has disjoint write zones, independent acceptance, and available non-author review capacity.

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
repository-local `work.py` copy. Use version `9.13.0` installed from
`https://github.com/maggnus/claude-plugins`.

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
