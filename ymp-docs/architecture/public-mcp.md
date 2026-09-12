# Public local MCP interface

Direction selected on 2026-09-12: use stdio for the first public MCP interface. This is YMP-123 delivery work, not an already available external server.

## Product boundary

An external client starts a YMP MCP process and exchanges protocol messages through its standard streams. The facade calls the same domain services as CLI and TUI: it does not implement a second scheduler, budget system or acceptance policy.

The public interface provides project-scoped knowledge search and inspection of sessions, tasks, results and confirmation evidence. Explicit execution requests can start or resume bounded team work, inspect its progress and cancel it. Tool responses carry stable session/result/request IDs and truthful states, including pending and accepted-but-unconfirmed outcomes.

An external client has its own request identity and declared project/action scope. It does not impersonate a team member or receive an internal assignment capability. The runtime continues to own native authentication, captured settings, admission, assignment grants, verification and persisted transitions. Provider credentials are neither exposed by the facade nor copied into metadata.

## Transport and lifecycle

Use the lifecycle, framing, tool discovery/call, cancellation and error requirements of explicitly supported MCP revisions. Reserve stdout for protocol messages and stderr for diagnostics. Declare and test supported revisions rather than treating a custom JSON stream as an interoperable MCP server.

The [MCP transport specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports) defines stdio and Streamable HTTP as standard bindings and permits custom transports such as Unix domain sockets. YMP already uses a Unix socket behind its internal team-tool bridge. That remains an implementation detail; external clients use standard stdio.

The first version does not require a shared background daemon. Specify what happens to active work when a request is cancelled, the client disconnects or the process exits. Preserve observed progress and usage; a replacement process inspects interrupted work before resuming it. A future shared server may sit behind the same stdio facade without changing tool semantics.

## Acceptance

Publish versioned tool schemas, a runnable local client configuration example, and a clear lifecycle contract. Exercise the real stdio process with an MCP client using temporary metadata and scripted execution: discovery, reads, scoped mutations, progress, cancellation, malformed requests, foreign-project references, disconnect and reopen. Read-only calls must not start agents. Execution still passes through the common runtime controls and confirmation rules.

YMP-123 delivers the facade after the relevant core interfaces are ready. YMP-121 checks interoperability and actual runtime behavior in the integrated release. Network serving, a remote authentication service and a separate plugin manager are outside this initial local interface.
