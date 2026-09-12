# Public local MCP interface

`ymp -C /absolute/project mcp` starts the public stdio facade. It uses the same
`Engine`, captured policies, resource admission, workspace locks, assignment grants,
confirmation and storage as CLI/TUI. It has no background daemon or network listener.
The direct selected-directory policy applies to the 0.4.0 MVP only.

## Client configuration

This configuration works with MCP clients that accept the standard `mcpServers`
command/args configuration, including Claude Desktop. Replace both absolute paths;
`ymp init` can prepare configuration before starting the client.

```json
{
  "mcpServers": {
    "ymp-project": {
      "command": "/absolute/path/to/ymp",
      "args": ["-C", "/absolute/project", "mcp", "--client-id", "desktop"]
    }
  }
}
```

Reads are enabled by default. Add `--allow-execution` to the launch arguments to
permit explicit bounded starts, resumes and cancellation. `--home /absolute/metadata`
optionally selects a different metadata directory; the default remains `~/.ymp2`.
No token or provider credential belongs in this configuration. Native authentication
continues to use the installed providers. Launching the facade does not send a model
request or probe authentication.

The launcher fixes the canonical project and action scope. Tool arguments cannot
change project, directory, client identity or internal capability. `--client-id` is a
stable local audit/deduplication namespace, **not authentication** against another
process owned by the same OS user. A caller with access to launch arbitrary local
commands already has that user's filesystem authority. Session references are checked
against the launcher's project before reading or resuming. Knowledge includes that
project and intentionally shared entries; a shared source ID does not grant access to
its originating project's session. Relocation requires reopening the facade, and
resume rejects a captured directory that differs from its launch scope.

The existing `ymp mcp --socket /path/to/socket` internal bridge remains available for
native agent adapters. It requires its process-owned assignment capability. Public
clients cannot call its tools, borrow a live team capability, or establish assignment
authority through their external identity. Durable evidence can name grant/assignment
record IDs; these are audit references, not capability secrets.

## Protocol contract

The public server supports **MCP 2025-06-18**. Initialization negotiates that revision;
for an unsupported requested revision the response names 2025-06-18, allowing the
client to accept it or disconnect. The implementation follows the official
[lifecycle](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle),
[stdio transport](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports),
[tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools) and
[cancellation](https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/cancellation)
contracts. Newer revisions, MCP task extensions, HTTP and SSE are not advertised.

Messages are newline-delimited UTF-8 JSON-RPC 2.0 objects. Stdout contains only protocol
messages; diagnostics use stderr. Maximum request frame size is 65,536 bytes. An
oversized frame receives an invalid-request error and closes the transport. JSON-RPC
batches are not supported by the selected revision. An invalid JSON frame receives a
parse error; the connection can continue. Request IDs are strings or integers, unique
within a connection, with at most 10,000 remembered IDs before reopening is required.

Call `initialize`, then send `notifications/initialized` before tool discovery/calls.
`ping` works before and after initialization. `tools/list` returns all five tools in
one page and does not advertise list-change notifications. Unknown methods return
`-32601`; malformed requests return `-32600`, invalid discovery/tool names `-32602`,
pre-initialization operations `-32002`, and JSON parse failures `-32700`. Tool argument
or domain failures return `isError: true`, a stable `tool_error` code and an explanatory
message; important admission cases also name `execution_denied`, `execution_busy`,
`request_conflict`, `scope_denied`, `project_relocated`, or `not_owned` in that message.
A final 128 KiB response guard returns `-32603` if a response cannot fit.

All calls complete immediately with a read result or durable execution handle. There
is no outstanding long-running MCP call after `ymp_run_v1` returns. Consequently a
`notifications/cancelled` message for that completed RPC is ignored, as required for
completed/unknown requests. Use `ymp_cancel_v1` to cancel its separate durable operation.
Unknown notifications receive no reply. Progress is polled through `ymp_request_v1`;
this version does not emit `notifications/progress` or support subscriptions.

## Version 1 tools

The checked-in [tool schemas](public-mcp-tools-v1.json) are the schemas returned by
`tools/list`. Tool names and response `schema_version` version the public API.

| Tool | Purpose |
| --- | --- |
| `ymp_inspect_v1` | `scope`, `pool`, `sessions`, or a project's `session`, `tasks`, `results`, `evidence`, `history` |
| `ymp_knowledge_v1` | Search with `query`, or resolve with `id` and optional full-source `version`; `include_unconfirmed` defaults to false |
| `ymp_run_v1` | Explicit `start` or `resume`, returning an operation and stable session ID |
| `ymp_request_v1` | Read a `request_id`'s operation, observation of liveness, captured budget and usage |
| `ymp_cancel_v1` | Signal cancellation of a `request_id` owned by this process; poll until terminal |

Every read goes directly to discovery/storage projections and never starts an agent.
Pool and usage identities are agent IDs; providers/models are execution backends.
Results retain source session, immutable captured directory/artifact paths, result
ID/version and current confirmation grade. Accepted-but-unconfirmed results remain
unconfirmed, without reputation credit. Evidence inspection projects existing
runtime decisions and check captures; it does not run checks. Starting through this
v1 tool uses ordinary runtime acceptance and does not install new objective acceptance
contracts. Resuming preserves any objective contracts already captured by another
trusted client.

For example, after enabling execution:

```json
{
  "name": "ymp_run_v1",
  "arguments": {
    "request_id": "report-20260913",
    "action": "start",
    "prompt": "Write a project overview in overview.md and independently review it.",
    "max_turns": 40,
    "turn_timeout_secs": 120,
    "max_seconds": 600
  }
}
```

The configured runtime ceilings still apply: `max_turns` and `turn_timeout_secs` must
be positive and no greater than configuration. Start captures these lower limits,
the eligible pool, settings/pins and resource allowances through `Engine`. Native
usage may be partial; an invocation/token allowance does not prove a strict whole-run
token or currency bound. `max_seconds` is an execution lease from 1 to 3,600 seconds;
on expiry the facade cancels and permits up to five seconds for runtime cleanup.

Resume requires `request_id`, `action: "resume"`, `session_id`, and `max_seconds`.
Omit prompt/turn budget overrides: resume uses the captured prompt and remaining
session budget, never fresh spend. Completed sessions cannot be resumed. One execution
is owned per facade process; the common runtime's project/session locks also serialize
conflicting work across processes. An already-active foreign process can cause a
runtime lock denial, which is recorded under the new operation without starting agents.

## IDs, recovery and cancellation

JSON-RPC request IDs last for one connection. The separate tool `request_id` lasts
across process restarts. Its key hashes a structured tuple of project ID, client ID
and external request ID. Its fingerprint hashes the normalized typed execution
arguments. Claiming that key is atomic and happens before execution effects.

Retrying the same request and arguments returns its original operation/session;
changing the payload under the same key is `request_conflict`. For new work use a new
request ID. The server generates the UUID session ID before starting Engine. Engine
creates it with an insert-only transaction: a duplicate ID cannot overwrite session
history, captured budgets or spend. Results and confirmation evidence keep the
runtime's existing durable IDs and versions.

Graceful stdin EOF, SIGINT, SIGTERM, write failure or lease expiry cancels owned work.
The runtime preserves recorded progress and observed usage, revokes capabilities and
usually saves a paused session. Shutdown allows five seconds for cleanup, then drops
unfinished futures; cancellation never rolls back writes in the selected directory.
SIGKILL, machine failure or grace expiry may leave a pending operation or saved
`running` state. Such records are not treated as live authority.

A replacement process reports `interrupted_or_running_elsewhere` for a nonterminal
operation that it does not own. This explicitly leaves another process's liveness
unknown. It does not cancel a different process's run. Inspect session/task/results,
then explicitly resume under a **new** request ID. The shared runtime obtains its
locks, interrupts old invocation records and inspects uncertain work before replay.
A crash between request claim and session creation can leave a handle without a
session: retrying never starts it; inspect that handle and submit new work with a new
request ID if needed. A terminal failed operation need not mean a session was created.

## Response bounds

Collections accept `limit` (1–25, default 10) and an optional returned `next_cursor`.
Pages contain `items: [{"value": ..., "truncated": false}]`. Cursors page the scoped
result set, not a transactionally frozen snapshot. Stable records preserve ordering;
concurrent insertion or lifecycle changes can require restarting pagination. History
uses persisted message sequence IDs so growing chat can be read incrementally.

Each metadata row caps strings at 1,024 Unicode characters, collections at 32 values,
and nesting depth at 10. Snapshot `bytes` and expected check bytes are omitted. A row
larger than 8 KiB is reduced to bounded identity/status/version fields. `truncated`
explicitly marks lossy rows. At most 48 KiB of projected rows are included per page;
`next_cursor` also continues a page shortened by this byte budget. The final MCP reply,
including duplicated text/structured content, is capped at 128 KiB.

Knowledge rows include the entry ID and digest of the **complete** stored entry, plus
its bounded projection, provenance/source IDs and result versions. Resolving a stale
version or foreign reference returns unavailable. This metadata API intentionally
does not export complete truncated text or captured file bytes. Original artifacts
remain in their recorded locations. Queries use the existing domain retrieval services;
transport bounds do not claim a bounded SQL scan or database-memory footprint.

The internal assignment `memory_search` uses the **same** projected page envelope and
supports `cursor`/`limit` alongside `query`/`include_unconfirmed`. This is distinct from
the runtime's separate 8,000-character automatic prompt-memory allowance, which does
not limit tool responses.

## Verification

The executable is walked using the [official Python SDK](https://github.com/modelcontextprotocol/python-sdk/tree/v1.x),
pinned to 1.28.1 in `ymp-evals/mcp/requirements.txt`:

```sh
cargo build -p ymp-cli
python3 -m venv /tmp/ymp-mcp-client
/tmp/ymp-mcp-client/bin/pip install -r ymp-evals/mcp/requirements.txt
/tmp/ymp-mcp-client/bin/python ymp-evals/mcp/stdio_client.py target/debug/ymp
```

The test isolates HOME/metadata/projects, configures only mock providers with explicit
`mock` model and `low` effort, and never invokes a real provider. It covers discovery,
reads without invocation growth, scoped mutations, durable IDs, conflict denial,
progress/usage, qualitative acceptance/evidence, cancellation/resume, foreign project
and client references, oversized knowledge with continuation/version checks,
disconnect/reopen, SIGKILL recovery, malformed wire requests and stdout framing. Internal socket/grant
compatibility and the shared memory projection are covered by runtime MCP tests.
See the [delivery evidence](../../ymp-evals/reports/ymp-123-stdio-verification.md) for
commands, failing controls and limitations. YMP-121 owns integrated release verification.
