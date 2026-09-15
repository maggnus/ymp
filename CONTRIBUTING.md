# Contributing

Read `AGENTS.md`, `intent.md`, and the approved
`ymp-docs/self-organizing-team-domain-model.md` before changing product behavior.
The model is the single architectural authority; `ymp-docs/domain.md`,
`ymp-docs/architecture.md`, and `ymp-docs/foundation.md` explain terminology,
implementation boundaries and the delivered subset. Keep product prose, code
comments, examples, and user-facing strings in English.

The workspace specifies its Rust toolchain and dependencies in the checked-in
toolchain and Cargo manifests. Keep crate dependencies aligned with the approved
model and its implementation notes, use relative workspace paths,
and do not introduce machine-specific configuration or undisclosed services.

Repository development tasks use Python 3.11 or newer and the standard library.
Their canonical records are individual JSON files under `ymp-docs/tasks/records/`.
Follow `ymp-docs/development-tasks.md` and `ymp-docs/tasks/README.md`; do not create a
combined backlog or generated status index. Start with a bounded query and inspect
only the selected task:

```sh
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show W1-0004
```

Use `claim`, `update`, and `status` with `--expect-revision` for coordinated writes.
Owner labels are cooperative coordination rather than authentication or product
execution authority. Separate clones and worktrees do not share the writer lock.

Before submitting a change, run:

```sh
python3 -m unittest discover -s ymp-docs/tasks/tests -v
python3 ymp-docs/tasks/manage.py check
python3 ymp-docs/tasks/manage.py render --limit 20 >/dev/null
cargo build --workspace --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

Read the final diff and verify affected consumers. Update `ymp-docs/roadmap.md` only
for product outcomes, and link their canonical task records instead of copying task
status into the roadmap.
