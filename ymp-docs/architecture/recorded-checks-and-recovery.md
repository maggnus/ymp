# Recorded checks and recovery limits

Two things the interface discloses about a session, both reads of data that already exists.
Neither adds a guard, a question before an action, or a limit on what a run may do.

## Recorded checks

`/checks` lists the acceptance commands the runtime ran for the loaded session, read from the
session log through `Store::checks`. A row is one recorded run, with the command, the
directory it ran in, whether it exited zero, when it was recorded, and the output that was
captured. Two runs of the same command stay two rows, because the log records each one.

The page states how a check is executed: `Engine::checks` runs it with `/bin/sh` in the
working directory after the agent's own turn in that same directory, nothing limits what the
command may do, and nothing is asked before it runs. This is disclosure, not protection. By
the time a check runs, the execution turn has already had unrestricted access to the same
directory, so describing the check as contained, agreed to or gated would be false. A
command that exits non-zero keeps its task unaccepted whatever an agent reported; a command
that exits zero is evidence about that command only.

A record is presented as it was written. A missing result is `no recorded outcome`, never a
pass. A check that timed out or was stopped with the run is written nowhere, so it cannot
appear: commands the accepted plan declared and the log has no run for are listed separately
as `no recorded run`, which is not a result. The task page labels a task's own commands
`planned checks` for the same reason.

Only the first 20000 characters of a check's combined output are recorded by the runtime, and
the page shows the first 80 display lines of that; the rest is reachable in the full record
with `Enter`. Reading the log is a scan of the session's events, which carries no index.

## Recovery limits

`/diff` lists the change metadata recorded for the session and states, in both its empty and
its populated state, that ymp recorded a path, a status and a content hash for each file and
never a copy. No previous file content exists anywhere in the application's data, so ymp
cannot restore an earlier version of a file, and the interface says so rather than implying
otherwise by listing paths alone. The statement is unconditional: it does not depend on what
version control, if any, covers the directory.

The page also states what the record does not cover. The fingerprint walk skips `.git`,
`node_modules`, `target`, `__pycache__`, `.DS_Store`, `.ymp2` and the session's own metadata
directory. A listed change means the file differs from the fingerprint taken when the session
started; it does not identify which agent or which other process wrote it. The list is
written once, when a run finishes or stops, so a run still working or one whose process ended
first leaves nothing to read.

## Repository discovery

`ymp_workspace::repository::discover` answers one question: is there a Git marker at the
working directory or above it? It reads directory metadata and, for a `.git` file, its text.
It runs no command and writes nothing. The controller calls it when the window opens, when
the change page is opened, and after a run finishes, which is when the answer can have
changed; pages present the result and never look at the filesystem themselves.

Absence of `.git` in the working directory is not an answer and is never reported as one. The
result distinguishes a repository directory, a `.git` file naming another Git directory as a
linked worktree or a submodule does, an entry that exists but cannot be read, and nothing
found. The last case reports how many directories were inspected and where the walk stopped,
and states what that does not establish: only `.git` entries were inspected, Git may stop
earlier at a filesystem boundary, and a repository held below the working directory or named
by configuration outside it would not appear. `GIT_DIR`, `GIT_WORK_TREE` and `GIT_COMMON_DIR`
are reported when set, because they change which repository a command run there would use.

## Evidence

Unit tests cover discovery of a working directory nested below a repository root, a `.git`
file with and without a readable `gitdir:` line, a directory whose walk finds nothing, an
environment override, and a path that cannot be canonicalized. Storage tests read back
recorded checks in order and show that a missing result, directory or command stays missing.
Interface tests assert the recovery statement on the change page in both its empty and its
populated state, the command and outcome of each recorded check at 80x24, a declared command
without a run, and that no surface uses the words this contract forbids.

The interface was also walked in a pseudo-terminal against the deterministic demo agents,
with the working directory two levels below a repository marker: the run completed, `/checks`
showed both recorded runs with `passed`, their directory and their captured output, and
`/diff` named the discovered repository and stated that no earlier content was recorded. That
walk made no provider request.
