# Development contract

The application and executable are named `ymp`. Product documents, code comments,
examples and user-facing strings are written in English.

Read `intent.md` and `ymp-docs/self-organizing-team-domain-model.md` before changing
product behavior. The latter is the owner-approved, single authoritative source
of architecture, domain names and product scope. `ymp-docs/domain.md` and
`ymp-docs/architecture.md` are subordinate vocabulary and implementation notes;
`ymp-docs/foundation.md` retains historical implementation material. This iteration
starts from scratch; do not count previous code, APIs or tests as delivered
functionality or assume they are a baseline to extend. These documents,
historical amendments and previous iterations cannot override the approved model.
Use the model's names in code. Add a domain term only with its definition and
rationale in the model; architectural changes require an explicit owner decision.

Keep this repository self-contained. Dependencies between workspace crates use
relative paths inside the repository. Do not introduce machine-specific source
paths, ambient configuration requirements or undisclosed external services.

Keep domain values and validation separate from I/O. Strategies propose; the kernel
validates and commits. Adapters implement explicit ports and cannot grant themselves
authority through a returned verdict. Never label planned behavior as implemented.

Preserve native agent identities and authentication. No inference, installation,
publication or real user-data mutation is implied by a local build or test.

Use existing dependencies for shared responsibilities. Add dependencies only for
an actual capability need. Avoid placeholder crates, generic service containers,
empty plugin systems and speculative configuration fields.

Verification should answer the changed behavior's actual questions. There is no
required test count. Test maintenance is a means to useful evidence, not a product
objective. Keep unsupported claims and unexecuted checks explicit.

Before committing code, run:

```sh
cargo build --workspace --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

Read the diff and check affected consumers. Use independent review for changes to
public contracts, resource ownership, persistence or execution authority. Keep
parallel work read-only unless writers have disjoint, explicitly owned worktrees.

Record delivered outcomes in Git. Development task records under
`ymp-docs/tasks/records/` are the single source of task status; follow
`ymp-docs/development-tasks.md`. Start with `manage.py next`, then `show ID` for the
selected task, instead of reading every record. Use the task tool for coordinated
updates and retain its expected-revision checks. The roadmap describes product
outcomes and links to records; do not maintain duplicate status labels there.
Task ownership is cooperative development coordination, not runtime authority.
Do not record speculative progress as completion or create another task database.

Use these exact development-task commands from the repository root:

```sh
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show W1-0004
python3 ymp-docs/tasks/manage.py check
```

Before committing changes to the task workflow itself, also run:

```sh
python3 -m unittest discover -s ymp-docs/tasks/tests -v
python3 ymp-docs/tasks/manage.py render --limit 20 >/dev/null
```
