# System architecture

The root Cargo workspace contains seven packages. `ymp-core` defines profiles, tasks, plans, memory, and reputation without dependencies on terminal rendering or providers. `ymp-storage` owns SQLite and migrations. `ymp-providers` implements native process protocols. `ymp-workspace` tracks the current working directory and file changes. `ymp-runtime` coordinates these components. `ymp-tui` presents events and user actions; `ymp-cli` composes the application and exposes headless commands.

```mermaid
flowchart TD
  User[User] --> CLI[CLI / Ratatui TUI]
  CLI --> Runtime[Team runtime]
  Runtime --> Store[SQLite event journal and memory]
  Runtime --> Workspace[Working directory and change metadata]
  Runtime --> Providers[Native provider adapters]
  Providers --> Codex[Codex App Server]
  Providers --> Bridge[Claude SDK bridge]
  Bridge --> Claude[Installed Claude Code]
  Providers --> GLM[GLM ACP agent]
  Codex & Claude & GLM --> MCP[ymp team MCP tools]
  MCP --> Runtime
```

Provider output is normalized before it reaches the UI. The UI can be absent during headless execution. Shared chat is durable; incremental text is transient until the corresponding final response is saved. A message does not implicitly approve a task or schedule every participant.

Profiles and tasks are different entities. A profile may execute, plan, or review different tasks. A session captures its team profile snapshot. Native provider session identifiers are stored separately from application session and task identifiers.

## Workspaces

For the **0.4.0 MVP only**, a run works directly in the user's selected directory. Files are immediately visible there. The application does not initialize Git, create worktrees, or copy source trees into its home. It records file hashes, changes and bounded evidence snapshots as metadata. Post-MVP isolation and recoverable publication remain separate work under the [workspace policy](workspace-policy.md).

A project lock prevents overlapping runs for the same registered project. Within a run, the scheduler selects bounded work waves and permits independent assignments to overlap when their actual backend access does not conflict. Unbounded writers reserve the whole directory, conflicting access is serialized, and verification takes exclusive access through checks and acceptance. Board changes commit at work boundaries. The default allocator selects from metadata without collecting bids. Failed checks leave the actual files available for inspection; they do not cause an automatic rollback. See [concurrency](concurrency-implementation.md) and [board coordination](board-coordination.md).

The next idle chat message retains the current session's history, outcome, and file paths. A read-only conversational turn answers questions directly. Only an explicit request for more implementation starts a new task run, linked to its parent conversation and using the same working directory. `/new` starts an unrelated conversation.

Captured outcome locations remain historical facts after a metadata-only project relocation. Deterministic location questions use the stored original paths; missing legacy capture remains explicitly unknown. Execution and resume still validate the captured directory against the selected workspace.

## Stopping and recovery

Each native process is owned by a short-lived supervisor in a separate process group. Closing the host's stdin, including after abrupt host termination, causes the supervisor to close the native process and terminate remaining members of its group. The supervisor is not a service and cannot accept new work after the host exits.

A persisted session can exist without a running provider process. Native sessions are resumed using their provider identifiers; the current implementation releases the provider process after each turn. A cancelled or interrupted execution is inspected before resuming. Unknown outcomes are never treated as successes solely because no error was recorded.

Session locks prevent two application instances from running the same session. Process/protocol failures do not count as failures of an agent's task competence.
