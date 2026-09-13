# Second-round local owner API

This API implements the parent-approved recovery addendum and the owner's explicit
Continue with current files decision. It is not exposed through provider/team MCP.
No UI, real-session action, scope backfill, release or dependency change is included.

## Three distinct actions

1. `Engine::review_saved_plan_fresh(&FreshPlanReviewCommand).await` performs one
   new independent, actually read-only review of the exact immutable proposal.
   It excludes the author and failed actor, requires known-ended prior local
   execution and released access, and uses ordinary allocation/admission. It
   never resumes native context, executes production, certifies historical
   effects or sets manual_permit/effect_resolution. Positive and negative
   verdicts use the ordinary versioned `plan_review` record and original response
   attribution. Existing verdicts cannot be replaced by another reviewer.
2. `RecoveryControl::Continue` can consume an already recorded fresh verdict
   through `FreshPlanReviewState.next_action`, without a retry permit. Without
   owner authorization, unknown effect dependencies still stop production or
   revision. Retry remains a separate operation and still rejects uncertainty.
3. `Engine::continue_with_current_files(&ContinueWithCurrentFilesCommand)` records
   the owner's distinct authorization for new work from current files. It does
   not itself invoke a backend. A subsequent ordinary run may consume the saved
   verdict/graph and execute new tasks, with the old uncertainty retained.

The strict `inspect_recovery` operation remains separate. Its effect-resolution
requirements have not been waived. It now recognizes a completed invocation only
when an exact recorded MalformedResponse and no valid verdict justify that case.

## Reachable hold release

Fresh review and current-files authorization refuse an explicit stage OwnerWait
or OwnerPause. `RecoveryControl::ReleaseHold` restores the preceding condition,
retaining failures, counters and uncertainty, with zero calls and no retry permit.
Nested Wait/Pause edits retain the original underlying condition. Older holds
without saved origin return to an explicit unknown owner-action condition rather
than inventing a prior permission. The existing owner-team Continue releases a
session-wide hold. Hold release alone is not current-files authorization.

## Typical positive flow

All IDs and revisions below come from actual local read APIs. The application
presents the context and unresolved effects before the owner issues the action.

```rust
let stage = engine.recovery_stages(&session_id)?
    .into_iter().find(|stage| stage.id == stage_id).unwrap();
let review = engine.review_saved_plan_fresh(&FreshPlanReviewCommand {
    session_id: session_id.clone(),
    stage_id: stage.id.clone(),
    expected_revision: stage.revision,
    command_id: new_id(),
    proposal: stage.plan.unwrap(),
    reviewer_id: selected_native_agent_id,
}).await?;
// No production or graph commitment was performed by the review action.
let context = engine.current_files_context(&session_id, &stage_id)?;
// Present context, historical uncertainty and the owner's choice here.
let command = ContinueWithCurrentFilesCommand { command_id: new_id(), context };
let authorization = engine.continue_with_current_files(&command)?;
// The application separately starts ordinary continuation.
let outcome = engine.run(&directory, "", Some(&session_id)).await?;
```

Keep exact commands for retries. Each API has its own `(session_id, command_id)`
receipt namespace. An identical completed command returns the durable receipt
even after restart or later workspace changes. Reused IDs with changed content
fail. A fresh-review command that already admitted a call cannot issue a second
call under the same ID, even if that attempt failed or returned malformed output.
Its recorded invocation must be examined before another explicit action.

## Current-files context and admission

`CurrentFilesContext` contains the exact session/stage revision and saved
task/plan/result, team revision, current board digest, actual budget/usage,
acknowledged historical failures, canonical directory and file metadata manifest.
The manifest reuses `Workspace::files` and `FileSnapshot`: normal workspace
listing exclusions apply, regular files retain SHA-256/length only, symlinks retain
their link target. No source bytes or hidden source copies are persisted. Bounds
are 4096 listing entries and the existing 4 MiB per-file snapshot limit.

The runtime obtains project/session ownership and rechecks that context before
the storage transaction. Storage revalidates exact revisions, failures, budget,
termination, released prior access and owner holds. A lock alone cannot establish
termination. The distinct `owner_current_files_authorized` decision and receipt
record the scoped owner authorization; their outcome is not effect confirmation
or acceptance. `Engine::current_files_authorizations` exposes these records.

The authorization may mark an already saved fresh verdict for consumption, but
does not set manual_permit or effect_resolution. It does not clear Wait/Pause,
reset usage, change membership, enlarge budgets or issue grants. Every new
invocation retains the usual allocation, independent review, resource, access
and authority checks. An acknowledged uncertain actor always starts a fresh
native context for subsequent work; old native grants are never revived.

Only the exact acknowledged failures are covered. Later uncertain failures remain
uncovered. Both the ordinary runtime boundary and transactional native admission
refuse subsequent new work for that uncertainty. Independent fresh review and
strict inspection retain their own narrow admission rules. Historical failures
and any unresolved effects remain visible after successful new task execution.

The manifest binds the owner's decision to the presented workspace state; it is
not an ongoing filesystem freeze. Once authorization is accepted, legitimate new
work can change those files. No rollback, global external-effect certification,
source isolation or protection from arbitrary external filesystem changes is
claimed. Real provider behavior, Linux and the actual owner session are unverified.
