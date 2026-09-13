# Standalone editor research probe

Research only. The probe builds a separate executable and edits in-memory text. It does not
modify ymp source or any user document and does not access the clipboard. The active product
remains the previously installed 0.4.5.

## Reproduction

Copy `probe-Cargo.toml` to `Cargo.toml`, `probe-Cargo.lock` to `Cargo.lock` and `probe.rs` to
`src/main.rs` in a temporary directory. With Rust 1.89.0 installed, run:

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.89.0 run --locked
```

The dependency lockfile retains the compatible tree-sitter-language 0.1.7. Cargo originally
selected it automatically because 0.1.8 requires Rust 1.90. `probe.log` records the original
compilation and experiments. `probe-expanded.log` and `probe-observations.json` include the
additional light-theme, Markdown, raw-patch and tab observations.

The probe asserts text round-trip and undo/redo fidelity, and reports observable deficiencies
without interpreting them as successful integration. The timing sample uses a development
build and is not a comparative benchmark. API source identity and metadata are in
`verification.json`; the published source matches the pinned upstream revision.

The parent independently ran these experiments. Claude performed a separate source-only review
and did not claim these executions as its own. No application-wide fmt/Clippy/test rerun was
needed for research documentation; no product source or package dependency changed.
