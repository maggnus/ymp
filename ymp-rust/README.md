# ymp Rust workspace

This workspace is the production codebase for ymp. The POC is its first bounded feature stage,
not a disposable implementation. Every release is composed into one executable named `ymp`.

Package boundaries separate pure domain logic, authoritative control, persistence, artifacts,
runtime integrations, agent-facing bindings, independent verification, application use cases,
and presentation. Process boundaries and package boundaries remain explicit even though the
release artifact is a single executable.

## Workspace map

| Package | Responsibility |
|---|---|
| `ymp-domain` | Versioned commands, events, run state, digests, and pure transitions |
| `ymp-kernel` | Mechanical invariant checks and authoritative control rules |
| `ymp-storage` | Single-writer lock, size-bounded digest-linked journal, atomic metadata, and canonical object store |
| `ymp-artifacts` | Source snapshots, private workspace materialization, submissions, and candidates |
| `ymp-runtime-api` | Provider-neutral probe and invocation lifecycle |
| `ymp-runtime-fake` | Deterministic lifecycle and fault scenarios without provider access |
| `ymp-runtime-codex` | Pinned Codex process lifecycle, isolated configuration, structured events, interruption, and usage |
| `ymp-runtime-claude` | Pinned Claude Code lifecycle, isolated configuration, structured events, budget stop, interruption, and usage |
| `ymp-runtime-supervisor` | Contract loading, private attempt setup, runtime supervision, cancellation, candidate capture, and bounded runtime evidence |
| `ymp-agent-api` | Strict transport-neutral agent tool schema and controller binding |
| `ymp-agent-mcp` | JSON-RPC stdio MCP initialization, discovery, calls, and errors |
| `ymp-agent-rpc` | Private attempt-bound Unix RPC between an agent MCP process and the foreground controller |
| `ymp-verifier` | Independent exact-input checks and opaque, digest-bound verification evidence |
| `ymp-application` | Foreground use cases, persistence ordering, replay, cursor recovery, and ownership |
| `ymp-tui` | Ratatui presentation and terminal interaction |
| `ymp-cli` | Composition root and the only `ymp` executable |
| `ymp-testkit` | Cross-package deterministic fixtures |
| `tools/ymp-evals` | Executable protocol model and mutation controls |
| `tools/ymp-calibration` | Graduated agent calibration cases, private oracle injection, and candidate verification |

Server, web, and cluster adapters can be added later without moving these packages or changing the
one-executable boundary for the supported local product.

Durable event, object, compatibility, and migration rules are frozen in [SCHEMA.md](SCHEMA.md).

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo build --release -p ymp-cli
```

Run the provider-free vertical slice and protocol model:

```sh
cargo run -q -p ymp-cli -- demo
cargo run -q -p ymp-evals
cargo run -q -p ymp-calibration -- validate-cases
```

`ymp internal runtime-smoke` is a development-only entry point that drives the same compiled
runtime implementations used by the application. It is not a second product interface.
`ymp internal managed-runtime-smoke` proves the attempt-scoped MCP channel, while
`ymp internal managed-candidate-smoke` captures a private source snapshot, supervises one runtime,
and commits its quiescent workspace as an immutable candidate. Both commands leave user runtime
configuration unchanged. The MCP `submit` tool captures only the workspace and base snapshot bound
to that attempt; it accepts no caller-supplied path or digest. If the agent does not submit, the
supervisor captures the quiescent workspace after successful process completion.
`ymp internal verify-managed-candidate` materializes the controller-bound immutable candidate
itself and runs an exact approved command with a negative control, bounded time and output, then
records opaque evidence for the current candidate.

The TUI `e` action exports a submitted candidate, exact event journal, state projection, and all
content-addressed verifier and runtime evidence into a new atomic export directory. Existing
exports are never overwritten.

## Managed TUI contracts

Pass one or more versioned contracts to the default TUI. Use the arrow keys to select a runtime,
`[` and `]` to select a contract, and `Enter` to start the exact pairing. There is no silent runtime
fallback.

```sh
ymp --data-root /path/to/run-data --contract /path/to/contract.json
```

A version 1 contract has this bounded structure:

```json
{
  "schema_version": 1,
  "contract_id": "parser-repair-v1",
  "source": "./public-source",
  "prompt": "Read TASK.md, implement the public requirements, and run the visible checks.",
  "capture_exclusions": ["target"],
  "verifier": {
    "program": "./verify-candidate",
    "arguments": ["--case", "parser-repair"],
    "negative_control": "./known-invalid",
    "oracle_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "wall_time_ms": 60000,
    "output_limit_bytes": 1048576
  }
}
```

Relative paths resolve against the contract file. The raw contract bytes define its SHA-256
identifier. The TUI refuses missing sources, malformed exclusions, unavailable runtime profiles,
non-canonical oracle digests, and unusable verifier paths before spending a model request. After a
managed runtime completes, the controller uses its agent-submitted immutable candidate or captures
the quiescent workspace as a fallback. The `v` action runs the declared verifier against both its
negative control and the exact candidate, and `e` exports the candidate, control journal, verifier
object, and privacy-preserving runtime transcript.
