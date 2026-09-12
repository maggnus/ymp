# ymp

The executable is named `ymp`. Application-owned data lives in `~/.ymp2`.
All project documentation, code comments, UI strings, and examples are in English.

The root Cargo workspace contains packages in `ymp-rust/crates`. Documentation lives in `ymp-docs`, SDK bridges in `ymp-bridges`, and evaluation scenarios in `ymp-evals`.

Use the installed agents' native authentication. Never log credentials or copy tokens into application storage. Team runs work directly in the user's working directory. Serialize writes; keep only metadata under ~/.ymp2. Do not create hidden source copies or Git repositories.

After changes, run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Real provider checks are separate from unattended tests.
