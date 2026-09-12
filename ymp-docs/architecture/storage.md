# Storage and directory ownership

```text
~/.ymp2/
  config.toml
  configuration.lock
  provider-catalog.json
  catalog-scan.lock
  state.sqlite
  projects/<project-id>/
    workspace.lock
    sessions/<session-id>/
      session.lock
      workspace/workspace.json
      workspace/changes.json
  run/<instance-id>.sock
```

Project and session directories use UUIDs. Captured names and canonical source paths live in SQLite; the native catalog cache holds observed offerings separately from editable configuration claims. `ymp relocate PROJECT_ID PATH` changes only the registered association: it moves no files and cannot rewrite historical output locations. Workspace metadata records the original directory and file hashes; result evidence may include bounded captured bytes. The [direct-directory policy](workspace-policy.md) applies only to the 0.4.0 MVP and provides no source-tree copy or rollback guarantee.

SQLite stores projects, sessions, task state, messages, domain events, native-session handles, per-invocation token usage, observations, and memory. WAL allows readers during writes. Schema versioning uses `PRAGMA user_version`; a newer unsupported database is rejected. Task snapshots and their transition events commit together.

Qualified observations are keyed by result/version/producing agent, so replay cannot add credit. The runtime requires current confirmed independent acceptance and a completed producing invocation; accepted-but-unconfirmed outcomes, planning agreement and provider failures earn no competence credit. Reputation uses at most the latest 100 qualified observations for the effective execution version, competence and difficulty. Historical observation records remain stored separately from this selection window; the ordinary inspection method returns at most 500.

Memory records project/global applicability, source/result versions, evidence, authorship, lifecycle and replacement links. Supported retrieval resolves current confirmation and source freshness; an active historical label or another agent's approval is insufficient. Free-text lessons remain unconfirmed candidates. Confirmed shared procedures come from runtime-projected passing check evidence. Scoped correction can atomically supersede an exact predecessor without deleting its history. Explicit candidate/history inspection stays distinct from supported automatic retrieval; see [incremental knowledge](incremental-knowledge.md) and [correction](knowledge-correction.md).

`YMP_HOME` or `--home` overrides only ymp storage. `HOME`, `CODEX_HOME`, and native provider credential locations are not repurposed. Environment references in configuration name variables rather than storing their values.

Internal team socket files stay in the metadata home's `run` directory. If its
absolute path exceeds the Unix socket address limit, a private `0700` temporary
directory under `/tmp` provides a short symlink address; it contains no task data.
The socket remains `0600` and stays in metadata. Normal shutdown removes the owned
socket and alias while preserving a replacement listener. Abrupt termination can
leave stale transport entries; no crash-cleanup sweep is claimed. The public MCP
interface continues to use stdio.

Token snapshots are keyed by session and invocation, with the assigned agent ID. Updating a snapshot replaces its previous value. Statistics are summed across distinct invocations, so repeated provider notifications and reopening a session cannot add the same usage again. Schema version 2 backfills available historical events as partial data.
