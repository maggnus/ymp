# ymp

ymp explores and enables useful self-organization among AI agents. Participants
can propose work, take and transfer commitments, exchange context, raise
objections and adapt their team and plan within kernel-enforced authority and
resource limits. User tasks make the benefits, failures and costs observable.
An evolving terminal interface exposes this behavior.

## Starting point

This iteration starts with a new implementation. The journal foundation supplies
validated values, atomic in-memory events and replayable method decisions; see
[the implementation boundary and evidence](ymp-docs/journal-implementation.md).
The [Application intake](ymp-docs/intake-implementation.md) records explicit goals,
constraints and versioned criteria, including attributable user refinements.
The [Registry](ymp-docs/registry-implementation.md) records profile readiness,
native metadata and typed exclusions without model calls.
The [SQLite journal and content store](ymp-docs/storage-implementation.md) retain
those events and immutable bytes across restart, with explicit commit resolution.
The [Treasury](ymp-docs/resources-implementation.md) records budget reservations,
receipt observations and policy-attributed charges while protecting verification
and reporting capacity. Native execution and its evidence remain separate work.
The [Direct workspace capture](ymp-docs/workspace-implementation.md) retains file
content, ordinary permissions and empty directories in immutable snapshots.
Assignment path ownership and cessation checks remain under development.
The remaining modules are placeholders under
[the proposed layout](ymp-docs/project-worktree.md). The executable still has no
interactive or task-execution behavior.
Earlier source material and implementation notes are historical references, not a
baseline to extend or a reason to omit planned work. The previous iteration's code
was removed from the working tree and is reachable only at git tag `legacy-foundation`;
the facts worth keeping from it are collected in [legacy lessons](ymp-docs/legacy-lessons.md).

## Product definition

- [Intent](intent.md) explains the product purpose and user expectations.
- [Self-organizing team domain model](ymp-docs/self-organizing-team-domain-model.md) is the owner-approved, single authoritative architecture and product scope.
- [Product waves](ymp-docs/waves_ideas.md) describe the proposed product increments.
- [Domain notes](ymp-docs/domain.md) explain terminology and historical API names.
- [Proposed crate and module layout](ymp-docs/project-worktree.md) maps model sections to target crates, modules and owning tasks.
- [Historical implementation notes](ymp-docs/architecture.md) and [foundation contract](ymp-docs/foundation.md) do not establish current functionality.
- [Historical amendments](ymp-docs/domain-model-amendments.md) have no normative authority.

## Development tasks

Read [task progress in Markdown](ymp-docs/tasks/PROGRESS.md): per-wave tables with
`[ ]`, `[x]`, `[~]`, `[!]` and `[=]` marks, Hong Kong timestamps and links to the
evidence commits on GitHub.
Task write commands and `make all-checks` regenerate it; `make tasks-progress`
regenerates it on demand after the canonical task records change any other way.

Use the [development workflow](ymp-docs/development-tasks.md) and
[task guide](ymp-docs/tasks/README.md). Status lives in individual records under
`ymp-docs/tasks/records/`; planning does not start implementation.

```sh
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show W1-0001
python3 ymp-docs/tasks/manage.py check
```

`make help` lists Makefile shortcuts for these commands and for the Cargo checks.

Before implementing product behavior, read `AGENTS.md` and the approved model.
Use the repository's required checks when committing implementation work. Builds,
offline checks and task records never imply authorization for native inference,
installation, publication or mutation of real user data.
