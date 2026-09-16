# Workspace implementation notes

Canonical task status is in `tasks/records/W1-0005.json`. Direct capture and
retained snapshots are implemented. The ownership checkpoint adds path-lock state,
opaque access/cessation inputs and aggregate checks within one Journal. W1-0005
has persistent physical-root binding across stores; a real mediated executor/provider
that protects the binding metadata is still required. Capture read holds now cover synchronous capture and
publication. There are no production access-evidence factories or native write calls.

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
included. Hidden task files are retained. Once a root binding is attached, capture
excludes exactly its validated application-owned marker; it does not apply an
ambient exclusion list or silently omit a user-owned file with that name.

Explicit limits bound entries, depth, per-file size and total bytes. The adapter
checks content and directory metadata around each read, compares directory listings,
and requires two equal scans of the complete bounded tree. A changed physical root
is refused. Matching scans detect ordinary concurrent changes, but cannot establish
an atomic filesystem snapshot against arbitrary external mutation or an ABA change.
WorkspaceGuard holds a root Read claim against managed writers in the same journal
throughout capture and publication. The adapter itself makes no OS isolation claim;
external mutation and independently configured stores are not thereby confined.

WorkspaceGuard reconstructs workspace identity and provider selection from the journal,
checks the returned tree and every stored file digest, and retains the manifest before
committing SnapshotTaken. Event.contents explicitly links the manifest and file bytes;
SQLite verifies those links on commit and read. A failed capture may leave unreferenced immutable content and records a capture
abort after its synchronous I/O ends; it does not record a completed snapshot. Application storage should
be outside the source tree so writes to storage cannot alter the tree being captured.

`retained` loads and verifies the stored manifest and its files without consulting the
live workspace. `read_artifact` returns the exact bytes named by a recorded snapshot.
No method here applies those bytes to the user's directory or claims to have run a check.

## Persistent root binding

`RootBinding` in `ymp-runtime/src/workspace/binding.rs` installs or verifies an
immutable `.ymp-workspace-owner` marker. It records format, physical workspace root,
and JournalIdentity (stored random identity, canonical database path and physical
file identity). The journal records the same value in WorkspaceBound through
WorkspaceGuard.bind_workspace. A marker alone does not grant execution access.

Markers are installed through a private staging file and a no-replace rename,
with file and parent-directory synchronization. Existing foreign or malformed files
are refused without overwrite. A recorded binding whose marker disappeared is not
silently recreated. Replaced markers and roots are refused by existing handles.
A copied database cannot join the original root merely by retaining its UUID.
Only the journal contains mutable access holds; there is no second mutable ledger
in the marker. Relocation, rebinding and arbitrary external state replacement are
not silently repaired by this interface.

Binding operations lock physical ancestor directories from top to bottom, shared
on ancestors and exclusive on the chosen root. They retain those descriptors until
validation and installation finish, then explicitly unlock them. Existing ancestor
markers and bounded descendant scans forbid nested bound roots. Use relative scopes
within one workspace instead. Independent sibling roots can bind concurrently.
Unavailable topology, aliases, links, device crossings and scan limits cause refusal.

Unbound Direct preview obtains an exclusive root lock plus shared ancestor locks
for all its synchronous I/O, checks ancestors and scans descendants for existing
bindings. Thus it cannot read a bound descendant from another journal or race a new
child binding. Bound capture uses shared coordination locks and verifies its marker.
Capture receives the calling journal's identity as a read-only observation and
checks it under the provider's binding mutex. A bound provider reused by another
Guard cannot capture through that Guard's unrelated journal.
This is cooperative filesystem coordination, not a native sandbox. The mediated
provider must still prevent access to control metadata and expose only checked file
operations; native execution must independently establish its actual enforcement.

## Capture read lifecycle

Before capture, WorkspaceGuard records LockChanged::CaptureStarted with a root
PathObservation and a random per-attempt owner from OS entropy (`getrandom`). The
owner is an identity, not a serialized authority token. It binds the local completion
capability and prevents identical requests from different Guards from sharing one
hold, including when an adapter resolves an indeterminate append by matching bytes.
CaptureRead participates in the same aggregate conflict checks as assignment holds.

The WorkspaceProvider capture contract is synchronous: all its I/O must end before
it returns, including on failure. A local pending completion retains the exact start
reference, owner, workspace, time and either a retained manifest digest or a bounded
failure. The Guard keeps at most 64 pending entries. Once I/O has ended, resolve_capture
retries only publication or abort at a fresh revision; it never recaptures the tree.
A confirmed foreign/absent failed begin is removed locally without aborting another
owner, even when the journal first became unavailable and is resolved later.

SnapshotTaken version 2 requires the matching live Read hold and atomically publishes
the snapshot and ends the hold. Version 1 remains replayable but cannot be newly
appended. Commit resolution checks a valid preceding prefix before recognizing an
exact historical packet, preserving the atomic intake-refinement boundary.

If publication fails, local completion evidence remains available for retry or explicit
abandon_capture. Abandonment is refused during I/O; after completion it records Abort
without publishing a snapshot. A lost final acknowledgement resolves idempotently.
A fresh Guard cannot recreate completion authority for an unfinished capture from
stored data, so unresolved holds survive restart. With a readable, matching owner,
a lost begin acknowledgement can resolve only that Guard's own never-started attempt.
When storage is unavailable the hold and any local completion remain unresolved.
The journal reserves one control event and 64 KiB per active capture for abort; this
is a logical capacity guarantee, not a promise under disk exhaustion or I/O failure.

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
validation before acquiring a hold leaves the journal unchanged; failures after
acquisition retain their Begin/Abort history without publishing a snapshot.

`workspace_guard_tests.rs` exercises the actual consumer with private synthetic
access and cessation inputs: broad scopes, compatible reads, disjoint writes,
revoked holds, state-bound release, foreign evidence and missing read capability.
`workspace_locks.rs` integration tests use Direct observations and trusted synthetic
LockChanged facts to check same-store cross-session/thread/process conflicts,
physical ancestor roots and corruption of an owner's head. The SQLite capacity
test uses a small test-only event limit to demonstrate that an ordinary pool-refresh
append is refused while authorize/revoke/release can still complete.

`ymp-storage/tests/capture.rs` checks capture/writer exclusion, failure and CAS retry,
lost begin/final acknowledgements, immutable completion without repeated I/O,
abandonment, unresolved holds after restart, historical v1 replay, invalid partial
refinement resolution and races between two Guards with identical requests. Repeated
foreign begin losses, including temporary read failures, do not exhaust local capacity
or abort the foreign holds.

`ymp-storage/tests/identity.rs` checks v1 migration without changing retained content,
concurrent identity creation, replacement/hard-link refusal and malformed identity.
`ymp-storage/tests/binding.rs` exercises WorkspaceGuard binding, marker exclusion,
missing/foreign metadata, copied databases, nested roots in both orders, thread and
process races, sibling roots and unbound-preview exclusion of bound descendants.

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
The remaining work is the alternate provider that enforces operations through the
actual consumer, protects binding metadata, and withdraws access only after admitted
operations drain. Integrate that enforcement with the existing binding and capture
read lifecycle.
Their integration/concurrency/restart tests are required before W1-0005 is complete.
