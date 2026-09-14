# Contributing

## Development environment

The foundation targets Rust 1.89.0, edition 2024. Use Cargo, rustfmt and Clippy.
The initial workspace uses the Rust standard library and its own crates.

Build and check from the repository root. The executable foundation will provide:

```sh
cargo build --workspace --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

## Making a change

Define one outcome and its acceptance before editing. Locate the responsible
subsystem in the architecture and use the domain vocabulary. Keep changes small
enough to review and make related code, documentation and evidence agree.

Choose checks that can distinguish the intended result from a plausible failure.
For a new invariant, include an adverse case through the real consumer. A check
with no applicable input is not evidence of behavior. Do not preserve a test solely
to maintain a count; retain or change it when it helps evaluate the product.

Record the actual commands and outcomes in the change description. Separate local
verification from untested operating systems, native providers and external effects.

## Public boundaries

The kernel owns state transitions. Journal implementations must honor atomicity
and revision checks. Domain values must not require filesystem access or a native
provider. The command-line package assembles the application through its runtime
interface rather than modifying domain state directly.

Expose only the minimum useful API. Preserve typed causes when returning errors.
Document an incomplete capability plainly; avoid successful placeholder actions.

## Repository hygiene

Commit source, documents and the workspace lockfile. Keep build outputs and local
editor state out of Git. Do not commit credentials, machine-specific configuration,
absolute dependency paths or generated agent transcripts.
