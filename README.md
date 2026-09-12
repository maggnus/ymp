# ymp

A terminal workspace for a self-organizing team of local AI agents. Written in Rust with Ratatui, with a small official-SDK bridge for Claude Code.

Give the team a task. Participants propose a plan, bid for work, implement in the current working directory, review each other's results, and verify the deliverable. Verified experience informs later assignments and supplies reusable project knowledge.

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

The interface is chat-first with a right sidebar that carries navigation, live team activity, and the current session's context. The sidebar appears from 80 columns; hide it with `Ctrl+B`.

Commands: `/chat`, `/help`, `/tasks`, `/usage`, `/sessions`, `/files`, `/diff`, `/checks`, `/assignments`, `/decisions`, `/providers`, `/agents`, `/agent`, `/team`, `/limits`, `/memory`, `/reputation`, `/theme`, `/sidebar`, `/details`, `/new`, `/resume`, `/pause`, `/stop`, and `/quit`. `Ctrl+P` opens the command palette, and `/help` lists every command and key.

Enter sends, Ctrl+J inserts a newline, and Tab completes a command name or moves the focus to the next region. Esc removes the topmost surface and eventually returns to the composer. Ctrl+C stops an active run; when idle, it exits. Messages entered during execution are delivered at the next turn boundary.

Your prompts and the team's final answers are the content of the transcript; routine plan, bid and review payloads are collapsed into one readable line each, and `Enter` on an entry shows the complete attributed message. `/details` switches to full messages. Scrolling up pauses auto-follow, which the status row reports along with the key that returns to the newest message.

Five colour themes ship with ymp, including a light one, a high-contrast one, and one that inherits your terminal's own palette. `/theme` or `Ctrl+T` opens a chooser that previews as you move; the choice is remembered. No state is shown by colour alone.

Every destination in the sidebar is read-only: opening one never starts an agent and never writes to your working directory. See the [interface guide](ymp-docs/guides/interface.md).

Files are created and modified directly in the directory where you start `ymp` (or the directory selected with `-C`). Only metadata lives under `~/.ymp2`; no source copies or hidden Git repositories are created. One task writes at a time, while planning and discussion can run in parallel. `/diff` lists files changed during the session, states how the directory is used and what that rules out, states that ymp recorded a hash rather than a copy and so cannot restore an earlier version of a file, and lists the session's accepted results with the directory each was recorded in. `/checks` lists the acceptance commands ymp ran itself, with their recorded outcome. `/assignments` shows the turns the run assigned, with the model and effort each one requested, what was sent to the installation and what it reported back; `/decisions` shows the plans, reviews, acceptances and competence credit the session recorded, and says whether an acceptance rests on evidence or on an independent review alone.

After a run finishes or stops, the next message continues that conversation. A question such as ‘Where is the file?’ receives the previous outcome and actual file paths without restarting execution. Use `/new` for an unrelated task.

## Repository

- `ymp-rust/crates/`: Rust workspace packages.
- `ymp-bridges/claude/`: TypeScript adapter around the official Claude Agent SDK.
- `ymp-docs/`: product requirements, architecture, protocols, research, and user guides.
- `ymp-evals/`: quality-evaluation scenarios and report specifications.

See [architecture](ymp-docs/architecture/system.md), [team protocol](ymp-docs/protocols/team.md), [provider integration](ymp-docs/architecture/providers.md), [storage](ymp-docs/architecture/storage.md), [recorded checks and recovery limits](ymp-docs/architecture/recorded-checks-and-recovery.md), [assignment, budget and confirmation views](ymp-docs/architecture/assignment-and-confirmation-views.md), [usage](ymp-docs/guides/usage.md), and the [interface guide](ymp-docs/guides/interface.md).

Follow the [approved intent](intent.md), [delivery plan](ymp-docs/tasks/plan.md), [project tasks and progress](ymp-docs/tasks/README.md), [research findings](ymp-docs/research/research-program-findings.md), and the [controlled evaluation protocol](ymp-docs/research/experiment-protocol.md). The plan describes pending delivery; the usage instructions above describe the current implementation.

## Development checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check --prefix ymp-bridges/claude
```

Real provider checks are separate from these tests. Evaluation modes `--no-memory` and `--no-adaptive` disable experience retrieval/creation and learned assignment respectively. See [evaluation methodology](ymp-docs/research/evaluation.md).
