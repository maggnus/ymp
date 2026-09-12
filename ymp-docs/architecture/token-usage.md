# Token accounting

An agent is the working unit. A session captures a team of agents; providers and models are execution details. Two agents using the same provider have distinct usage records and distinct totals.

Every invocation receives a stable (session_id, turn) identity before the provider starts. Provider notifications replace that invocation's latest normalized snapshot. The session total is the sum of distinct invocation snapshots, and each agent total includes only its invocations. Final values replace streaming values; they are never added on top.

Input includes ordinary input, cache reads, and cache writes. Output includes reasoning when the provider reports it. Cache and reasoning counts are breakdowns, not additional tokens. These totals measure processing volume across requests, not context-window occupancy. Reusing cached context in another request legitimately contributes input again.

No count is estimated from text length. An invocation without reported counts is unknown, not zero. An uninvoked agent has a known zero. Partial or unfinished totals are marked as such. A failure or cancellation does not erase previously reported usage.

## Provider normalization

- Codex supplies thread/tokenUsage/updated with cumulative and last-request counters. The adapter uses restored baseline notifications where available and recognizes a fresh process when the first total equals the last request. It never blindly subtracts totals from an older process: current CLI versions can reset accounting when resuming a thread. An unavailable or reset baseline that cannot be established is marked partial.
- Claude stream usage is keyed by API response ID so repeated content blocks do not count twice. Input comes from message-start/assistant usage; output comes from message_delta, not the assistant block's placeholder. Final modelUsage supersedes partial data and includes query-pipeline subagents and auxiliary calls. A zeroed crash result cannot erase observed usage.
- The installed GLM ACP agent 1.3.0 exposes only the last model request at turn completion. Its known contribution is explicitly partial; there is no invented intermediate count or claim of complete loop accounting.

Counters are the values providers report, not a billing statement. A provider may omit cache or reasoning detail even when its input and output totals are available.

## Persistence and recovery

The SQLite token_usage table stores the agent ID, invocation state, and replacement snapshot. The runtime drains queued final usage notifications before closing an invocation, including when the result and final notification arrive together. Reading and publishing a session snapshot is serialized across parallel agents, so an older read cannot be delivered after a newer final total. Persisted team-tool diagnostic sessions also retain reported usage.

On schema upgrade, old completed-event usage is recovered conservatively. Historical Codex traces lack reliable per-invocation native baselines, so only last-request values are recovered. Historical Claude traces contain main-loop totals rather than whole-query totals. Those rows remain visibly partial. Existing sessions and messages are preserved.

A recorded open invocation means its final state was not saved; after a crash it does not prove that a process is still running.

## Verification

Rust tests cover cache/reasoning inclusion, unknown versus zero, repeated snapshots, separate agents sharing a provider, historical migration, native resume/reset behavior, live updates before completion, final-message draining, and retained usage after cancellation. The Claude bridge has independent synthetic-stream tests:

~~~sh
npm test --prefix ymp-bridges/claude
~~~

Real probes are opt-in and consume provider limits:

~~~sh
cargo run -p ymp-providers --example usage_probe -- codex
cargo run -p ymp-providers --example usage_probe -- claude
cargo run -p ymp-providers --example usage_probe -- glm
~~~

## Sources

- [Codex App Server notifications](https://learn.chatgpt.com/docs/app-server)
- [Claude SDK cost and usage tracking](https://code.claude.com/docs/en/agent-sdk/cost-tracking)
- The installed GLM agent's runPromptLoop implementation retains lastUsage and returns it in the ACP prompt response.
