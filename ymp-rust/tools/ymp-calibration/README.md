# ymp calibration tool

This tool prepares isolated development-calibration cases and evaluates an exact candidate with
an oracle that is absent from the agent workspace. It is not the frozen POC study corpus and its
results must not be reported as evidence for the coordination hypothesis.

The case ladder increases one source of difficulty at a time:

1. `L1-line-endings` repairs one local UTF-8 boundary function.
2. `L2-size-parser` repairs a strict multi-file parser with overflow semantics.
3. `L3-command-ledger` repairs temporal idempotency across state changes and rejected commands.

Prepare a disposable Git repository:

```sh
cargo run -q -p ymp-calibration -- prepare L1-line-endings /tmp/ymp-l1
```

After an agent finishes, run the protected checks in a private copy and emit a JSON result:

```sh
cargo run -q -p ymp-calibration -- verify L1-line-endings /tmp/ymp-l1
```

Validate that every seeded defect is rejected before using the ladder:

```sh
cargo run -q -p ymp-calibration -- validate-cases
```

The `contracts` directory contains production managed-run contracts for the L2 and L3 cases. Build
the release verifier before loading either contract in the TUI:

```sh
cargo build --release -p ymp-cli -p ymp-calibration
target/release/ymp --contract tools/ymp-calibration/contracts/L2-size-parser.json
```

Provider execution remains outside this tool until the production runtime drivers enforce
profile isolation and bounded termination. Run records must pin provider, model, reasoning level,
agent mode, harness version, usage, wall time, candidate digest, and oracle result.
