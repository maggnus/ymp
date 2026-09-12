# Initial implementation verification

Date: 2026-09-12. Platform: macOS arm64. This report describes the original 0.1.0 behavior; the later working-directory change supersedes its isolation details.

## Automated checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 12 tests passed.
- `npm run check --prefix ymp-bridges/claude`: passed.
- Interactive pseudo-terminal exercise: completed a mock team task, opened memory, and restored terminal state on exit.
- Supervisor exercise: closing the host pipe terminated an owned background child process.

The tests include negative controls for self-acceptance, cyclic dependencies, duplicate observations, an approving reviewer paired with a failing command, forged chat authorship, and interrupted execution recovery without incorrectly recording failure.

## Native providers

Versions inspected: Codex CLI 0.153.4, Claude Code 2.1.268, GLM ACP agent 1.3.0; Claude Agent SDK 0.3.246.

All three providers returned a real diagnostic response using existing local authentication. Separate `doctor --probe --team-tools` runs independently confirmed actual `team_post` delivery for each provider.

## Real team task

Session: `ec4a3ba8-4eff-4a3a-b52a-5b8d9f8dada1`. Outcome: `completed`. Provider turns: 17.

The team created `greeting.py` and `test_greeting.py` in one implementation task. GLM executed; Claude independently reviewed the candidate; the integrated result passed review and checks. The source fixture directory retained only its original README.

The application recorded 4 acceptance-check executions. An additional independent check after the run confirmed exact outputs for `Ada` and the empty string; Python unittest discovered and passed two tests.

Shared-chat records by participant in this run: [('codex', 1), ('glm', 2)]. Separate diagnostic runs cover every participant's chat transport.

Memory records in this run (global flag, status, count): [(0, 'active', 1), (1, 'active', 1)].

## Limits of this evidence

These runs confirm functionality, not a measured quality advantage over a single agent. The deterministic suite verifies storage, execution invariants, and recovery. A controlled comparison on held-out tasks remains necessary before claiming that accumulated experience improves outcomes.

Native CLI personalization can affect generated prose even when the application requests English. All shipped project documentation, interface strings, and comments are English. The final integration snapshot from this earlier real run predates the generated-cache exclusion added during final review; current snapshots exclude new Python caches from result patches.
