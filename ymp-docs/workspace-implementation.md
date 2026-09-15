# Workspace implementation notes

Canonical task status is in `tasks/records/W1-0005.json`. Direct capture and
retained snapshots are implemented. The ownership checkpoint adds path-lock state,
opaque access/cessation inputs and aggregate checks within one Journal. W1-0005
remains in progress: protected physical-root binding across independent stores,
a real mediated executor/provider and temporary capture read holds are still
required. There are no production access-evidence factories or native write calls.

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

## Ownership bookkeeping and aggregate transactions

`workspace_locks.rs` validates LockChanged acquisition, single-use invocation
authorization, revocation and evidence-backed release. A lock's effective paths
come from an opaque AccessEvidence bound to the Guard, session, workspace reference
and profile. The requested paths cannot narrow the effective lock footprint or
exceed the required task/backend capabilities. In particular, an exclusive Write
lock is not permission to perform an otherwise forbidden ReadFiles operation.
Registry constraints and profile eligibility are rechecked at authorization.

Direct.observe_paths records physical ancestry and the missing suffix of each
requested path. Comparison uses shared physical objects to detect aliases and
ancestor workspace roots; ASCII case is conflated conservatively, and ambiguous
Unicode components cannot establish disjointness. Traversal and unsupported
filesystem objects are refused. These observations alone do not establish confinement.
Actual broker operations will need to revalidate paths while enforcing each ticket.

WorkspaceGuard uses replayed Journal.read data, reconstructs effective claims and
checks all current owners before appending. SQLite repeats the kernel check under
BEGIN IMMEDIATE; MemoryJournal uses one common append mutex. Thus a second session
cannot commit a conflicting lock between validation and commit in the same store.
SQLite enumerates sessions from heads, events and content links, so a missing head
cannot hide a retained owner. Other sessions are replayed one at a time; only their
active ownership is retained in the bounded aggregate projection. Unrelated historical
events do not exhaust an aggregate-history quota or prevent release.

Active ownership is bounded to 4,096 assignments and 16 MiB of projected claims.
SQLite also protects the remaining authorize/revoke/release event slots and 64 KiB
per control event from ordinary appends. Basis lists are bounded to 32 references.
This reserves control capacity under the existing per-session journal limits; it
does not promise writes when the filesystem itself is full or unavailable.

Revocation keeps all holds. NeverAuthorized evidence is issued only for an unreleased
assignment without invocation authorization; state changes invalidate old evidence.
Terminated/AccessWithdrawn evidence is non-deserializable, scoped to the current
assignment/workspace/invocation state and requires a recorded basis. Production
factories remain unimplemented. Synthetic fixtures exercise those consumer inputs
without claiming actual execution, confinement or cessation. No cost receipt or
financial settlement releases workspace access.

## Evidence

`ymp-storage/tests/workspace.rs` exercises Direct through the real WorkspaceGuard and
SQLite content/journal adapters. It checks exact binary bytes, executable permissions,
empty directories, changed/deleted files and reopening both snapshots after restart.
Negative cases cover traversal, symlink/hard-link aliases, root replacement, corruption
of stored content, limits, a deterministic write between capture scans and a forged
adapter view trying to attribute another directory to the recorded workspace. Failed
capture leaves journal events and revision unchanged.

`workspace_guard_tests.rs` exercises the actual consumer with private synthetic
access and cessation inputs: broad scopes, compatible reads, disjoint writes,
revoked holds, state-bound release, foreign evidence and missing read capability.
`workspace_locks.rs` integration tests use Direct observations and trusted synthetic
LockChanged facts to check same-store cross-session/thread/process conflicts,
physical ancestor roots and corruption of an owner's head. The SQLite capacity
test uses a small test-only event limit to demonstrate that an ordinary snapshot
append is refused while authorize/revoke/release can still complete.

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
The remaining work includes a protected same-root binding across independent stores,
an alternate provider that enforces operations through the actual consumer, withdrawal
with operation draining, and a temporary root Read hold covering the entire capture.
Their integration/concurrency/restart tests are required before W1-0005 is complete.
