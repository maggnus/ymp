# Contributing

Read `AGENTS.md`, `intent.md`, `ymp-docs/domain.md`,
`ymp-docs/architecture.md`, and `ymp-docs/foundation.md` before changing product
behavior. Keep product prose, code comments, examples, and user-facing strings in
English.

The workspace requires Rust 1.89.0 and uses only the standard library. Keep crate
dependencies aligned with `ymp-docs/architecture.md`, use relative workspace paths,
and do not introduce machine-specific configuration or undisclosed services.

Before submitting a change, run:

```sh
cargo build --workspace --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

Read the final diff and verify affected consumers. Update `ymp-docs/roadmap.md` only
for behavior that is implemented and checked.
