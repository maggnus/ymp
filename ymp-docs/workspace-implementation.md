# Workspace implementation notes

Canonical task status is in `tasks/records/W1-0005.json`. This checkpoint implements
Direct capture and retained snapshots through WorkspaceGuard. It does not implement
assignment path locks, execution access, cessation evidence or cross-session ownership;
W1-0005 remains in progress. Opening a workspace grants no permission to an executor.

## Capture and retained content

`ymp-domain/src/workspace.rs` validates portable relative WorkspacePath values,
workspace identity and a SnapshotTree containing file digests, byte counts, ordinary
Unix modes and directories (including empty ones). Root is `.`; absolute paths,
traversal, empty components, control characters and ambiguous backslash/colon syntax
are refused. File/directory collisions and missing parent directories are invalid.
Device/inode and canonical root identify a workspace, not a confinement mechanism.

`ymp-runtime/src/workspace/direct.rs` implements WorkspaceProvider over an opened
directory. rustix supplies safe descriptor-relative filesystem operations; this is
the new dependency's concrete purpose. Each descendant is opened with NOFOLLOW,
regular-file opens use NONBLOCK, and the opened object's type, device, inode and
link count are checked. Symlinks, hard-linked files, device boundaries, special
files, non-UTF-8 names and special Unix permission bits are refused. Capture preserves
ordinary permissions; ownership, timestamps, ACLs and extended attributes are not
included. It does not omit hidden files or apply an ambient exclusion list.

Explicit limits bound entries, depth, per-file size and total bytes. The adapter
checks content and directory metadata around each read, compares directory listings,
and requires two equal scans of the complete bounded tree. A changed physical root
is refused. Matching scans detect ordinary concurrent changes, but cannot establish
an atomic filesystem snapshot against arbitrary external mutation or an ABA change.
A coherent check/result snapshot still requires quiescent access under the remaining
WorkspaceGuard ownership implementation. The adapter makes no isolation claim.

WorkspaceGuard reconstructs workspace identity and provider selection from the journal,
checks the returned tree and every stored file digest, and retains the manifest before
committing SnapshotTaken. Event.contents explicitly links the manifest and file bytes;
SQLite verifies those links on commit and read. A failed capture may leave unreferenced
immutable content, but does not record a completed snapshot. Application storage should
be outside the source tree so writes to storage cannot alter the tree being captured.

`retained` loads and verifies the stored manifest and its files without consulting the
live workspace. `read_artifact` returns the exact bytes named by a recorded snapshot.
No method here applies those bytes to the user's directory or claims to have run a check.

## Evidence

`ymp-storage/tests/workspace.rs` exercises Direct through the real WorkspaceGuard and
SQLite content/journal adapters. It checks exact binary bytes, executable permissions,
empty directories, changed/deleted files and reopening both snapshots after restart.
Negative cases cover traversal, symlink/hard-link aliases, root replacement, corruption
of stored content, limits, a deterministic write between capture scans and a forged
adapter view trying to attribute another directory to the recorded workspace. Failed
capture leaves journal events and revision unchanged.

## Remaining W1-0005 work

PathLock admission must account for effective access, not just requested paths.
A native process's cwd is not confinement. Unknown access must be denied; a confirmed
root-confined broad writer must hold that root, while independently enforced disjoint
scopes may proceed concurrently. Case/Unicode aliases, overlapping roots and changes
between validation and use must not create artificial independence.

Ownership checks and commits must have one shared atomic boundary across sessions
and processes for the physical tree. A per-session journal CAS or one Guard's mutex
is insufficient. Independent stores must not concurrently claim the same tree.
Persisted holds survive a coordinator exit; an OS lock alone does not establish that
an external process stopped writing.

The alternate constrained provider must mediate actual access rather than return a
boolean claim of confinement. Withdrawal must stop new operations and wait for admitted
ones before issuing scoped cessation evidence. Native evidence production belongs to
W1-0017; W1-0006 binds locks to admitted assignments. Revocation, stream loss, elapsed
time and parent exit alone cannot release conflicts. Financial settlement is independent.
These admission/release operations, an alternate provider through the same consumer,
and their concurrency/restart tests are still required before W1-0005 can be completed.
