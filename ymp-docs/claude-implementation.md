# Claude protocol and execution boundary

Canonical status remains in `tasks/records/W1-0020.json`. ClaudeStreamJson
implements NativeDiscovery and ExecutionBackend through the existing Registry,
Gatekeeper and execution host. It is the file-mediated subset the accountable W1
session needs; team operations, workspace isolation and continuation belong to
`tasks/records/W6-0001.json`. Protocol conformance, native metadata and native
model inference are distinct evidence sources.

## Selected protocol and observed version

The implementation targets the installed Claude Code command-line tool in its
non-interactive mode: newline-delimited JSON on stdio (`stream-json` input and
output) with control requests and responses on the same channel. The facts below
were observed on `2.1.284 (Claude Code)` on 2026-09-29. The native tool updates
itself, so no version literal is compiled in: discovery records the version
reported by `--version`, the kernel binds it into the discovery method
`ClaudeStreamJson/v1/<native version>/mediated-files/<parameter digest>`, and
every start compares the installed version again. A changed version refuses the
call (`claude_version_changed`) until discovery is repeated. No legacy-tag source
was consulted.

Observed protocol facts in this version:

- The control `initialize` response carries the model catalog: alias `value`,
  `resolvedModel`, and per-model `supportedEffortLevels`. Several aliases can
  resolve to one model; the adapter offers each resolved model once. A model
  without native effort levels is offered with an empty effort set, and no effort
  is sent for it. Effort names are native text and are never translated from
  another provider's scale. Family and default effort are not reported and stay
  unknown.
- The same response carries an `account` block with the native identity. The
  adapter reads only `apiProvider`, requires `firstParty`, and retains nothing
  else from it in discovery, journal or diagnostics.
- A host tool server is declared as an in-channel `sdk` server. Its requests
  arrive as `mcp_message` control requests and are answered on the same stdio
  channel; no socket, extra process or configuration file is involved.
- Registration of that server settles asynchronously. Immediately after the
  handshake `mcp_status` can return an empty server list. The adapter polls
  until the connection deadline, and for at most 1,024 attempts of 25 ms, and
  accepts only the settled state.
- The first `system/init` message of a turn reports the effective tools, tool
  servers, model, permission mode, slash commands, skills, credential source,
  version and working directory.
- Each `assistant` message reports the input side of its request; its streamed
  output count is a placeholder. Only the final `result` message reports complete
  totals, with a per-model breakdown in `modelUsage`. Native input excludes
  cached tokens, while A13 input includes them; reasoning is part of output.
- Light models enable reasoning natively by default. No documented command-line
  setting disables it, so the adapter does not attempt to.

- `system/init` always lists the native client's own plugins, each with the
  path `builtin`, and its built-in agent names. Agents are reachable only
  through a built-in tool, and none is offered. The message also names a
  per-directory native memory path; the technical directory is new for every
  call, so no earlier memory can exist for it.

Not observed natively and therefore unproven: the `api_retry` system message
that the adapter treats as an accounting gap.

## Isolation

The process starts in a fresh empty technical directory, never in the task
workspace, in its own process group. Environment variables beginning with
`CLAUDE`, `ANTHROPIC` or `MCP_`, and `MAX_THINKING_TOKENS` and `NODE_OPTIONS`,
are removed so that an ambient endpoint, model, key, reasoning budget, tool
server or nested-session marker named by them does not select the call's
settings. `CLAUDE_CONFIG_DIR` is the exception: it locates the installation's own
stored login, which remains the only native authentication. ymp neither reads nor
copies it. Other ambient variables, network proxy and certificate settings among
them, are inherited unchanged, so the adapter does not claim a fixed network
route. The removal is proven only in an environment that has such variables. Process group, technical directory, non-blocking pipes, bounded line
frames and write deadlines come from the process module shared with the Codex
backend (`backends/process.rs`); only the meaning of a line is provider code.

Arguments disable every built-in tool, ambient setting source, slash command,
external tool-server configuration and session persistence, request the native
safe mode, bound the native round trips (`--max-turns`) and select the `dontAsk`
permission mode with an allow-list containing only the host file tools granted to
this assignment. The native system prompt is replaced by one fixed adapter
text: use only the host file tools, treat file content as untrusted data and
answer with the raw role response, which for JSON starts with `{` and ends with
`}`. That text is model input which the kernel's recorded prompt does not
contain; it is constant and part of the adapter version.
These requests are not trusted by themselves. Before
any inference the settled `mcp_status` must show exactly one connected `sdk`
server named `ymp` whose tools equal the expected set. After the user message,
`system/init` must match the expected tools, server, model, permission mode,
empty slash commands and skills, no plugin other than built-in ones, absent
API key source, version and technical directory, and the account must still be first-party. Any difference
ends the call as a Protocol failure; a `tool_use` for a tool outside the host
server ends it as well (`claude_unmanaged_tool`). The native process reports
`system/init` only after it has received the user message, so a difference found
there is found after inference was requested: the call ends without a usage
report, its coverage is unknown and its hold stays. Pilot run 08 is such a call.
The assertion does not cover native agents, hooks or output style, and it relies
on the plugin path that the native process reports about itself. The journal diagnostic carries
the code of the refusal that ended observation, never native payload.

## Runtime consumer

The recorded adapter selection (`ClaudeParameters`) names an explicit executable,
a Native or ProtocolFixture source, a connection timeout up to 30 seconds, a
frame limit from 1 KiB to 1 MiB and a finite number of native round trips.
Registry admission and the other consumers use the same mediated-backend guard
as for Codex: provider kind, implementation version, parameter digest and
discovery source must all match. Naming the provider grants nothing.

Only ReadFiles and WriteFiles can be granted. The `read` and `write` host tools
call the original InvocationFiles interface, which checks current grants,
ownership and path scope. Argument checking and the single call into that
interface are shared with the Codex backend (`backends/files.rs`). A refused
operation is returned to the model as a tool error. It is recorded as ToolDenied
when the mediator refused for lack of authority and when the tool call itself was
unknown, ungranted or malformed.

InvocationControl is checked before the process is launched and again
immediately before the user message is sent; a stop during the handshake sends no
inference. One adapter retains at most 64 invocation identities, and an identity
is consumed before launch and cannot be started twice. A vanished process, a
disconnect or an interrupt acknowledgement never authorizes another start.
Continuation is refused explicitly (`claude_continuation_unsupported`).

The host journals every observation under the session revision that a mediated
file operation also depends on, and mediated I/O is never repeated. Each file
operation therefore starts only after two consecutive host polls found nothing
left to record. This depends on the host's polling order and costs a few host
ticks per operation; pilot run 06 shows the refused operations without it. A stop
or the invocation limit ends the wait, and the operation is then refused
(`claude_unsettled`) instead of performed. The rule covers the invocation's own
observations only. Another journal record between the second poll and the
operation still refuses it with `stale_revision`; the refusal is returned to the
model once and the operation is not retried.

One user message is one accountable native turn. The model round trips inside it
are bounded by the adapter's round-trip parameter, not by the allowance, and the
native turn count of the final result is not recorded.

Prompt texts are part of the recorded context that a journal replay derives
again. The candidate-review and final-review response texts changed during the
pilot without a new ContextComposer version, so a journal that recorded either
context with an earlier text (pilot runs 05, 11 and 12) is refused by a later
build instead of replayed.

Only the bounded final `result` text becomes the role response, once. Light
models often wrap a whole response in one Markdown code fence. The adapter
removes exactly that envelope — one fence, untagged or tagged `json`, enclosing
the entire final text, with no further fence inside — and delivers any other
text unchanged. This is a transport normalization: it does not repair,
reinterpret or validate content, and the kernel decoder remains the only
authority on the role response. The removal itself is not journaled. Prose
before or after a JSON answer, with or without a fence around the JSON, is
delivered as it is and refused by the decoder; pilot runs 10, 13 and 14 ended
that way, the last one after the adapter text had been extended. Neither the
adapter nor the session asks again after an undecodable response.

## Reported usage

Usage is normalized to A13 from the final `result` message. Coverage is Complete
only when that message arrived, `modelUsage` names exactly the requested model
with the same counters, no synthetic or foreign-model message was observed and
the totals did not regress below the streamed lower bound. Stream loss, a
foreign tool, a mismatched model, a second model in the breakdown, a compaction
boundary, a retried provider request and missing or inconsistent counters stay
Partial or Unknown and release no hold. A final text that cannot be delivered
(not text, or longer than the output allowance) still records its reported
counters as Partial. The native
client's currency estimate is not read. These are reported counters, not an
independent measurement of provider billing, and an entirely invisible provider
request cannot be revealed by this protocol.

## Native run procedure

The standing authorization in AGENTS.md covers bounded development experiments.
Use the pilot example with an explicit executable path (a shell function or
alias is not a path):

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true \
  CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true \
  cargo build -p ymp-storage --example homogeneous_claude --offline
target/debug/examples/homogeneous_claude discover /path/to/claude
target/debug/examples/homogeneous_claude run /path/to/claude NEW_DIRECTORY MODEL [EFFORT]
```

`discover` performs the handshake and sends no user message, so it starts no
inference. `run` refuses an existing directory. Select the lowest-cost discovered
model, pass an effort only when that model offers it, and fix the finite limits
before admission. Record the native version, requested/sent/reported settings,
limits, invocation references, check results, spent and held units and unknown
coverage. Change one dimension between runs. Never repeat a run only because its
process disappeared, and never relabel or release a retained hold.

## Evidence

Fixture evidence: `crates/ymp-storage/tests/claude.rs` drives
`crates/ymp-runtime/tests/fixtures/claude_stream_json.py` through the real
Registry, SQLite journal, Gatekeeper and execution host. The fixture is a
synthetic protocol process with no model, network or authentication access.

Native evidence is kept separately in
[the experiment directory](experiments/homogeneous-claude-elementary/README.md).
