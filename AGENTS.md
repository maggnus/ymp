# Development contract

The application and executable are named `ymp`. Product documents, code comments,
examples and user-facing strings are written in English.

Read `intent.md` before changing product behavior. `ymp-docs/domain.md` owns the
canonical vocabulary, `ymp-docs/architecture.md` owns subsystem boundaries, and
`ymp-docs/foundation.md` defines the current executable scope. Use their names in
code. Add a new domain term only with a corresponding definition and rationale.

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
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Read the diff and check affected consumers. Use independent review for changes to
public contracts, resource ownership, persistence or execution authority. Keep
parallel work read-only unless writers have disjoint, explicitly owned worktrees.

Record delivered outcomes in Git and keep `ymp-docs/roadmap.md` accurate. Do not
create a second task database or record speculative progress as completion.
