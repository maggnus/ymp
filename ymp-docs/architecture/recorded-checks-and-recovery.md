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
`planned checks` for the same reason. With no session open the page reports that nothing was
read, which is not the same statement as a session having recorded nothing.

A record names its command and not the task that declared it, because the runtime writes the
command, the directory, the exit result and the output and nothing else. Declared commands are
therefore matched against recorded runs by their text alone, and the page says so: a command
that two tasks declare and one run reaches appears as that one run, and not also as a command
still waiting for the other task. Matching on the pair of task and command would require the
runtime to record a task identity with each check, which this work did not change.

Only the first 20000 characters of a check's combined output are recorded by the runtime, and
the page shows the first 80 display lines of that. The remainder is not reachable from the
interface at all: `Enter` opens the lines the row was already built with, so it shows the same
cut, and the note under the output says the rest stays in the session log. Reading the log is
a scan of the session's events, which carries no index.

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
without a run, and that no surface uses the words this contract forbids. They also assert what
`Enter` opens for a record longer than the page shows, that the hint describes exactly that,
that an unopened session is not reported as a session without checks, and that the page states
the text-alone matching of declared commands.

The interface was also walked in a pseudo-terminal against the deterministic demo agents,
with the working directory two levels below a repository marker: the run completed, `/checks`
showed both recorded runs with `passed`, their directory and their captured output, and
`/diff` named the discovered repository and stated that no earlier content was recorded. That
walk made no provider request.

## Rounds

- Round 1, commit `9562f8b`: `/checks`, the recovery statement on `/diff` and repository
  discovery in the workspace layer, with 118 workspace tests, formatting and lint clean, and a
  pseudo-terminal walk on the deterministic demo agents.
- Round 2, correction after independent review returned R1 8 of 10: the claim that `Enter`
  reaches output beyond the first 80 display lines was false and is corrected here and in the
  page hint, the `/checks` empty state no longer reports an unopened session as a session
  without checks, and the text-alone matching of declared commands is now stated on the page.
  No viewer for the remaining output was added, and no logic changed.

The exact limitation, stated once: of a check's recorded output, the interface shows the first
80 display lines of the first 20000 characters the runtime kept, and the rest can be read only
from the session log in `~/.ymp2/state.sqlite`. Clipping of long detail text by the shared
frame, which affects every page and not only these, is a separate follow-up tracked with
YMP-118 and is not corrected here.