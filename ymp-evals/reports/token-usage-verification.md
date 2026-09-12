# Token usage verification

Version: 0.3.0. Verified on macOS on 2026-09-12.

The interface was implemented and corrected by Claude Code using claude-opus-5 with thinking level max, through the Paseo CLI. The parent implemented provider normalization, persistence and runtime publication, then independently reviewed the integration and exercised the release binary.

## Automated checks

- cargo fmt --all --check passed.
- cargo clippy --workspace --all-targets -- -D warnings passed.
- cargo test --workspace passed all 90 tests, including 65 interface tests.
- The Claude bridge TypeScript check and all four Node stream-accounting tests passed.
- The release build completed, and the installed ymp command reports version 0.3.0.

Tests cover repeated snapshots, native counter resets and resume baselines, cache/reasoning inclusion, missing counters, cancelled invocations, final-notification draining, concurrent publication, historical migration, and separate agent identities sharing one provider. UI checks cover initial detail selection, live tokens/turns/status, captured teams, session switching, incomplete attribution, and narrow viewports.

## Release terminal check

An actual pseudo-terminal ran the installed release with a temporary application home and working directory. Atlas and Boreal used the same deterministic Mock provider, with explicit synthetic usage enabled only in the fixture.

- A task created greeting.txt directly in the working directory.
- Intermediate frames showed growing token totals, a growing invocation count, and running status together. One observed frame showed 100+ tokens and 1 / 80 turns.
- After the task, the session showed 1,320 tokens across 11 invocations, split into separate agent totals.
- Opening /usage immediately displayed the session's input/output breakdown. Cache and reasoning details absent from the fixture remained unknown.
- Asking where the file was located continued the same session, added one invocation, and produced a total of 1,440 tokens. It did not repeat the task graph.
- Restarting, opening the stored session, and returning to /usage preserved those totals without invoking an agent.
- Both 120x36 and 80x24 viewports retained the session total and agent rows. Exit restored the terminal screen.

The terminal check first rejected the earlier build that displayed growing tokens alongside a stale zero turn count. The corrected build passed the same check. A separate Mock diagnostic check retained 120 reported tokens for each of two agents even when team-tool verification failed.

Screenshots use deterministic synthetic token counts for UI validation:

- [Session breakdown at 120x36](../../ymp-docs/guides/images/token-usage.png)
- [Restored session at 80x24](../../ymp-docs/guides/images/token-usage-narrow.png)

## Existing-data migration

A SQLite backup of the existing application database was migrated from schema 1 to schema 2 in a temporary directory. It preserved all existing rows: 2 projects, 4 sessions, 102 messages, 307 events, 4 tasks, 6 observations, and 3 memory entries. It recovered 64 usage rows from historical events. Reopening did not add those rows again. The original database was not modified by this check.

Historical data remains partial where complete invocation usage or attribution cannot be recovered. Unknown usage is never presented as free usage.

## Separate real-provider checks

Opt-in probes exercised two read-only, tool-free requests with native session reuse for each installed provider. These were separate from unattended tests and used native authentication.

| Provider | First invocation: input / output | Resumed invocation: input / output | Usage events per invocation |
| --- | --- | --- | --- |
| Codex | 18,795 / 40 | 19,262 / 5 | 2 |
| Claude | 9,965 / 15 | 9,110 / 4 | 4 |
| GLM ACP | 1,818 / 47 | 1,859 / 37 | 1 |

The Codex probe confirmed that a new app-server process can reset native cumulative counters even when resuming an existing thread. The adapter recognizes a confirmed fresh baseline; it does not blindly subtract the previous process's values. One intermediate retry was rejected by upstream capacity before the subsequent successful probe.

The installed GLM ACP agent reports only its last model request at native turn completion. Those values remain explicitly partial, and the interface does not invent intermediate updates. These probes verify reported counters and native resume behavior; multi-request and parallel Claude stream accounting are covered by synthetic tests.

See [token accounting](../../ymp-docs/architecture/token-usage.md) for normalization and persistence rules, and [the interface guide](../../ymp-docs/guides/interface.md#token-usage) for display semantics.
