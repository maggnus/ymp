# ymp

A terminal workspace for a self-organizing team of local AI agents. Written in Rust with Ratatui, with a small official-SDK bridge for Claude Code.

Give the team a task. Participants propose a plan, bid for work, implement in isolated working copies, review each other's results, and verify the integrated deliverable. Verified experience informs later assignments and supplies reusable project knowledge.

**Status:** initial working implementation. Quality improvements from memory and adaptive assignment are hypotheses to evaluate, not a measured product claim.

## Requirements

- macOS, Git, and Rust 1.89 or later.
- At least two enabled agent profiles for independently reviewed team runs.
- Already authenticated local Codex and/or Claude Code installations.
- Node.js 22 or later for the Claude bridge; an existing GLM ACP installation for GLM.

The agent programs manage their own credentials. `ymp` does not extract access tokens or call model APIs with copied CLI credentials.

## Build and run

```sh
npm ci --prefix ymp-bridges/claude
npm run build --prefix ymp-bridges/claude
cargo build --release
./target/release/ymp init
./target/release/ymp doctor
./target/release/ymp
```

For a local command on your PATH, run `./ymp-scripts/install.sh`. The installation links to this checkout; keep it available for the Claude bridge.

Run in the project's source directory, or use `ymp -C /path/to/project`. The default application home is `~/.ymp2`; override it with `--home PATH` or `YMP_HOME`.

```sh
# Small real-provider authentication check (uses provider limits).
ymp doctor --probe
ymp doctor --probe --team-tools

# Autonomous team execution without the TUI.
ymp -C /path/to/project run 'Add validation and tests for the import command'

# Machine-readable result and a reproducible local demonstration.
ymp run 'Explain this project and produce an architecture note' --json
ymp demo

# Single-agent operation uses read-only mode unless --write is supplied.
ymp ask codex 'Summarize the entry points'
```

`ymp demo --tui` opens the interface with deterministic test agents. It makes no model requests.

## Working with a team

The TUI supports `/providers`, `/agents`, `/agent`, `/team`, `/new`, `/sessions`, `/resume`, `/tasks`, `/diff`, `/reputation`, `/memory`, `/limits`, `/pause`, `/stop`, and `/quit`. Use `/help` for arguments.

Enter sends, Ctrl+J inserts a newline, Tab completes command names, and PageUp/PageDown scroll. Ctrl+C stops an active run; when idle, it exits. Messages entered during execution are delivered at the next turn boundary.

Work is produced in a session's isolated integration directory. The final source snapshot includes the user's uncommitted files, while the original source directory, Git index, and branches remain unchanged. Each stopped run writes `artifacts/result.patch`. Review that patch before applying it to the source project.

## Repository

- `ymp-rust/crates/`: Rust workspace packages.
- `ymp-bridges/claude/`: TypeScript adapter around the official Claude Agent SDK.
- `ymp-docs/`: product requirements, architecture, protocols, research, and user guides.
- `ymp-evals/`: quality-evaluation scenarios and report specifications.

See [architecture](ymp-docs/architecture/system.md), [team protocol](ymp-docs/protocols/team.md), [provider integration](ymp-docs/architecture/providers.md), [storage](ymp-docs/architecture/storage.md), and [usage](ymp-docs/guides/usage.md).

## Development checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check --prefix ymp-bridges/claude
```

Real provider checks are separate from these tests. Evaluation modes `--no-memory` and `--no-adaptive` disable experience retrieval/creation and learned assignment respectively. See [evaluation methodology](ymp-docs/research/evaluation.md).
