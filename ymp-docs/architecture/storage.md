# Storage and directory ownership

```text
~/.ymp2/
  config.toml
  state.sqlite
  projects/<project-id>/
    workspace.lock
    sessions/<session-id>/
      session.lock
      workspace/workspace.json
      workspace/changes.json
  run/<instance-id>.sock
```

Project and session directories use UUIDs. Display names and canonical source paths live in SQLite. `ymp relocate PROJECT_ID PATH` changes the registered source path without discarding project history. The workspace metadata contains the real directory path and initial file hashes. Changes contain paths and statuses, not source copies.

SQLite stores projects, sessions, task state, messages, domain events, native-session handles, per-invocation token usage, observations, and memory. WAL allows readers during writes. Schema versioning uses `PRAGMA user_version`; a newer unsupported database is rejected. Task snapshots and their transition events commit together.

Observations have unique attempt identifiers, so replaying an outcome cannot increase reputation twice. Reputation reads the last 100 resolved outcomes for the profile version, competence, and difficulty. The complete observations remain available for inspection.

Memory has project/global scope, provenance, author, reviewer, status, and optional predecessor. FTS5 searches only active records visible in the current scope. Retirement removes a record from retrieval without erasing its history. Global activation requires a reviewer other than its author.

`YMP_HOME` or `--home` overrides only ymp storage. `HOME`, `CODEX_HOME`, and native provider credential locations are not repurposed. Environment references in configuration name variables rather than storing their values.

Token snapshots are keyed by session and invocation, with the assigned agent ID. Updating a snapshot replaces its previous value. Statistics are summed across distinct invocations, so repeated provider notifications and reopening a session cannot add the same usage again. Schema version 2 backfills available historical events as partial data.
