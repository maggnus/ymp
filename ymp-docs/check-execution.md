# Visible check execution (W1-0009)

`AcceptanceAuthority` registers visible checks and records attributed check runs.
It does not create Evidence, Reviews, ResultVersions, confirmation grades, hidden
checks, mutants or discrimination decisions. A real before/after check in the
integration test demonstrates discrimination of its two artifacts only; it is
not a product confirmation grade or a native-agent experiment.

## Registration and authority

An owner registers through the `SessionControl` issued by the same `Intake`
instance. `Application::acceptance` and `Intake::acceptance` share that exact
in-process authority; an equal session ID from another application is insufficient.
An agent requires a real `GrantToken` with `CheckPropose`, matching assignment,
current commitment, funding, workspace holds, capabilities and criterion scope.
API preflight and replay share the grant scope predicate.

User checks receive `Trusted`; an agent cannot request that classification or
impersonate User. The kernel conservatively assigns `ProducerAuthored` when the
author's agent has a producer assignment for the criterion, otherwise
`IndependentVisible`. Later evidence applicability and producer/result exclusion
remain W1-0019 responsibilities. Only visible registration is implemented.

Each proposal retains its effective policy, parameters, rationale and basis.
`VerificationDesigner/ExplicitVisible/1` with empty parameters must be selected
at session opening. It records submission of an already authored visible check;
it does not design tests or implement the planned hidden VerificationDesigner.
Unknown, unselected or mismatched policy attribution is rejected.

A Check's digest commits to its complete content, criterion version, author,
independence, needs and verifier snapshot reference. Registered IDs are immutable;
a revised check uses a new ID and version. Registration appends its ID to a new
AcceptanceContract version. Intake refinement retains checks for unchanged
criterion versions and removes changed-criterion checks from the active contract.
Historical checks and runs remain readable with their original versions.

## Execution and retention

The owner selects a CheckRunner through `AcceptanceAuthority::run`. Both
`ProcessRunner` and the process-free `RetainedBytes` implementation use this
consumer. Current allowed capabilities and deadline are checked before calling
the adapter. Unknown check versions and target snapshots are rejected first.
Retained manifests and file contents are validated through WorkspaceGuard; the
changing live workspace is not an execution source.

`Command(program, args, inputs)` uses a normalized relative executable path and
explicit argument list. Program and declared input paths resolve in the check's
pinned verifier snapshot. The target snapshot is the process working directory;
`YMP_CHECK_INPUTS` names the separate pinned verifier directory. A shell command
string is not accepted in place of program and arguments. An executable script
may explicitly use its interpreter. Missing programs/inputs and nonexecutable
programs produce `Error(Environment)`.

The runner returns observations, never an independence label or a Pass verdict.
The kernel compares returned check, snapshot and environment bindings, computes
Pass/Fail from a normal command exit or an exact-byte observation, and validates
again during event replay. ExactBytes also cross-checks the observed digest with
the retained manifest. A normal nonzero command exit means Fail; it does not
identify whether the check itself is defective. Missing target files are an
ExactBytes Fail. Preparation/start failures are Error; timeout, excess output,
signal termination and observed integrity failures are Error. Exit is optional:
no synthetic process exit is assigned to errors or process-free byte checks.
Shell launcher statuses 126/127 are conservatively Error(Environment), including
an executable that deliberately returns one of those reserved statuses.

The journal retains exact check and target versions, role (Baseline, Candidate,
Control), observation and kernel outcome. Bounded stdout/stderr bytes and the
serialized environment are content-addressed in ContentStore and are required
storage links of CheckRunRecorded. The environment names runner implementation,
version, parameters, limits, OS/architecture, OS release-file digest, launcher
binary digests, filesystem access profile and cleared process variables. A run
cannot substitute another environment after preparation. This is attributable
execution metadata, not a hermetic image of every system library.

## ProcessRunner v1 boundary and limits

On macOS, each command gets a fresh private directory containing its retained
subject, declared verifier files and scratch directory. Subject file and directory
permission bits are preserved. Snapshot and verifier bytes are checked when
materialized; verifier bytes are checked again after execution. OS confinement,
not permission-bit rewriting, protects these files.

Seatbelt (`sandbox-exec`) starts with deny-default. File data reads are limited
to subject, verifier, scratch, `/bin`, `/usr/bin`, `/usr/lib`, `/usr/share`,
`/System`, `/dev/null` and the root directory itself. Global filesystem metadata
and read-only sysctl queries are allowed. Root-directory data access is required
by dyld on the tested macOS 27; it does not permit reading arbitrary descendant
files. Writes are limited to scratch and `/dev/null`. Network, process forks and
other undeclared operations remain denied. There is no unrestricted fallback.

The no-fork restriction is deliberate: executable replacement preserves the
sandbox, and a check cannot leave children or detached descendants after exit.
Commands requiring subprocesses are outside this adapter's current execution
profile. Their own reported nonzero exits remain observations, not inferred
infrastructure diagnoses. ContainerRunner and broader execution profiles are
still future work. Native agent execution is unrelated to this adapter.

Processes use a cleared environment with fixed PATH and locale, private HOME and
TMPDIR, null stdin, nonblocking bounded output pipes and a separate process group.
A launcher marker distinguishes sandbox startup failure from command execution.
The marker is treated as command-controlled data: it is opened without following
symlinks and without blocking, must be a regular file of the exact expected size,
and is read into a fixed-size buffer. FIFO, symlink and oversized replacements
produce Error while retaining the already collected output streams.
Timeout is at most 120 seconds; each retained output stream is at most 4 MiB.
The process is killed and reaped on timeout/output failure. The output limit
counts retained bytes; excess output creates Error rather than a truncated Pass.
These limits do not constitute CPU or memory quotas. Per-run paths are disposable
locations, not part of the stable logical environment identity.

On other platforms, ProcessRunner Command returns Error(Environment); Linux
command confinement has not been implemented or tested. ExactBytes execution is
process-free, but this change was built and tested only on macOS. No Linux
support claim follows from macOS results. OS administrator actions, hostile
same-user host processes and compromised adapters are outside this process
boundary; adapter observations are not cryptographic attestations of execution.

## Executed evidence

Independent review accepted W1-0009 with R2(9/10) after the marker correction.
The reviewed `make verify` completed with exit 0 on 2026-09-28, covering the
legacy scan, offline workspace build, formatting, Clippy with warnings denied
and workspace tests. Task-register validation also completed with exit 0.

On macOS arm64, `cargo test -p ymp-storage --test checks --offline -- --nocapture`
exercised real temporary workspace capture, Command Fail/exit 1 on incorrect bytes
and Pass/exit 0 on corrected bytes, pinned verifier/input substitution resistance,
retained output/environment retrieval, both runners through AcceptanceAuthority,
SQLite reopening, and foreign check/snapshot/environment and outcome rejection.
The same integration suite covers host reads/writes, subject/verifier writes,
fork and exec-then-fork denial, output/time bounds, missing executable, actual
agent grant attribution, expired/revoked grants, owner impersonation, unknown
registration policy, current capability removal and deadline checks.

A narrow fault injection disabled only the kernel comparison of the runner's
check/snapshot/environment bindings. The existing foreign-binding integration
test failed (exit 101); restoring the comparison made it pass. No injected
failure remains in the implementation. Broad pre-commit validation is recorded
with the task completion evidence, not inferred from this targeted run.

Independent review reproduced an unbounded marker read after a command replaced
the marker with a FIFO. The actual consumer also incorrectly returned Pass for a
symlink to the expected marker bytes; its regression failed with exit 101 before
the fix. After bounded nonblocking marker validation, the existing process-boundary
scenario passes for FIFO, symlink and oversized replacements with retained output.
