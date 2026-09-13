# Required rework: additive backend integration API

This supplements the frozen backend API. It defines no UI, strategy settings,
provider-facing owner tools, or new dependency.

## Ready responsibility and stopped stages

`RecordLinks.board_release: Option<BoardCommitmentRelease>` records an accepted
owner departure's withdrawal of a Ready, unadmitted commitment. The original
proposal remains in history. `BoardTask.version` changes; old claims fail.
Admitted ownership still drains.

`RecoveryStage.wait_reason: Option<RecoveryWaitReason>` distinguishes owner
Wait/Pause, participant availability, policy stops, admission, cancellation,
unresolved effects, unbound legacy records and inspected effects. Missing legacy
metadata remains unknown. Membership edits invalidate selection without granting
manual retry. Only participant availability is reconsidered automatically through
the ordinary policy, with unchanged failure/attempt counters.

## Explicit independent effect inspection

```rust
let stage = engine.recovery_stages(&session_id)?
    .into_iter().find(|stage| stage.id == stage_id).unwrap();
let command = RecoveryInspectionCommand {
    session_id: session_id.clone(),
    stage_id: stage.id.clone(),
    expected_revision: stage.revision,
    command_id: new_id(), // Retain the exact command for idempotent retry.
};
let inspected = engine.inspect_recovery(&command).await?;
engine.control_recovery(&RecoveryControlCommand {
    session_id: session_id.clone(),
    stage_id: stage.id,
    expected_revision: inspected.resulting_revision,
    command_id: new_id(),
    action: RecoveryControl::Continue,
})?;
engine.run(&directory, "", Some(&session_id)).await?;
```

The async `Engine::inspect_recovery` is a trusted local command. It obtains the
session/project ownership locks, reads the captured limits, holds workspace access
through snapshots, independent inspection and commitment, and uses normal
allocation, invocation/grant/resource admission and message attribution. It does
not interrupt active invocations or infer termination from acquiring a lock.
The inspector is neither a failed actor nor a saved-result producer. Its actual
backend access must be read-only before admission.

`RecoveryControlKind::InspectEffects` describes this separate async API in
`manual_actions()`; it is not a new synchronous `RecoveryControl` variant. Availability
of the action family is not proof that effect scope or current files are sufficient.
The command reports the actual missing evidence if they are not.

An explicit inspection can examine a stage held by Wait/Pause while preserving
that hold and condition exactly. It does not resume the stage or set manual_permit.
A session-wide owner Wait/Pause still prohibits all native admission, including
inspection. An owner command racing the inspection changes the stage revision;
the stale inspection cannot commit its resolution. Its actual invocation/usage
remain recorded. After a successful inspection, a separate Continue is required.
Policy stops and counters are also preserved. Resolved review continuation requires
actual read-only backend access; new write capability cannot replace it silently.

Receipts use a separate `(inspection API, session_id, command_id)` namespace.
Identical replay returns the saved receipt without a new call. Changed command
content, stale revisions and foreign stage/result/inspector bindings fail. Historical
receipts do not assert that file contents remain current forever.

## Supported trusted evidence source

`ExecutionBackend::local_effect_scope(&TurnRequest) -> Option<LocalEffectScope>`
is a narrow compiled-backend enforcement contract, analogous to its actual
workspace-access declaration. A backend may return a complete set of relative
regular-file paths only when its implementation enforces that all possible effects
are limited to those files, with no network effects, detached execution or outside
writes. Request/model text is not a source of this guarantee. The runtime validates
paths and captures the declaration in the normally admitted
`WorkspaceAccessDecision.local_effect_scope`, binding it to backend identity,
assignment, invocation and directory. It rechecks the declaration after resolving
native continuation and before admission. No arbitrary owner or provider JSON
operation can install the declaration.

All existing native adapters, including the built-in mock adapter, inherit None.
WriteAll by itself remains insufficient. There is no special case for Mock and
no backfill of missing historical scope. The positive scripted backend executes
one fixed local file write using Rust, runs no external commands or remote work,
and supplies this real enforcement contract. Its public historical reconstruction
retains the genuine admitted scope record while omitting recovery stages; it never
inserts a fabricated resolution or safety flag into Store.

Inspection validates the original terminal assignment/invocation, explicit
BackendEnded evidence, backend/directory/access correspondence and complete scope.
It reuses `FileSnapshot` for bounded local observations (at most 64 declared paths
per invocation, 4 MiB per file and 16 MiB total). The independent review must approve
the exact obligation and observed effects. Its text must match the message from
the originating completed, read-only invocation. A model safe=true is insufficient.

`RecordLinks.recovery_inspection` retains the command, task/plan/result binding,
original failures, access-record IDs, terminal timestamps, observed files and actual
inspector response/assignment/invocation. `RecoveryStage.effect_resolution` points
to that inspection and its exact resolved failures. History is never cleared.
The narrow inspection evidence is confirmed; it does not confirm task deliverables
or increase reputation. Both Continue and atomic replay admission revalidate the
resolution against the stored obligation and current observed files. A changed
file requires new inspection, not an old permit. An unverified termination or
unknown external scope remains blocked.

## Failed execution waves

Selection excludes durable occupied participants as well as current-wave choices.
After unrelated ready work drains, known-ended read-only transport failures can
enter the existing independent interruption review in the same run. This transition
does not accept work or repeat execution. Normal review/rejection, attempt limits,
confirmation and fresh admission determine subsequent rework. Write-capable or
otherwise unverified failures retain responsibility and do not use this automatic
transition. Existing claim_busy and access checks remain unchanged.

No source isolation, rollback, remote-effect proof, arbitrary external filesystem
race protection, native provider certification, UI integration or installed release
is supplied by these changes. Native backends without an enforced local-only
effect boundary cannot obtain it from a user assertion or model answer.
