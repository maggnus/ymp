# Minimal session recovery UI integration

Scope: the remaining UI consumer of YMP-146, using its typed owner APIs after
backend acceptance and integration. Claude Code claude-opus-5 high owns UI source.
The backend candidate 743543f is independently accepted and integrated in main
as c28f501. Combined-source checks are pending. This contract
is preparation, not implemented functionality. YMP-148/YMP-150 strategy selection
and the later longer slash-command list YMP-154 remain separate.

## Owner-visible behavior

Use /team for current-session membership and recovery controls. Display effective
participants, requested replacements/departures, current responsibilities, failed
work and the concrete remaining condition. Loaded-session add/remove/replace must
call the real owner API rather than edit Config.team. With no session selected,
retain YMP-149's explicitly labeled starting preferences. The /agents catalog and
/settings are not redesigned in this slice.

Offer only backend-supported actions for the current stage. Keep Retry, ordinary
Continue, fresh saved-plan review, effect inspection and hold release distinct.
A fresh review uses an eligible independent actually read-only participant and
retains the saved proposal. No new planning or automatic production follows from
selecting that review action. An unavailable action has a short concrete reason.

`Continue with current files` presents the selected session/directory, affected
failure and stage, and the fact that historical effects remain unverified. Bind
confirmation to the exact backend context. Confirmation explicitly requests both
the scoped owner authorization and subsequent ordinary session continuation;
record these as separate operations. The authorization API itself still makes no
provider call. A successful authorization followed by a failed run start must be
shown honestly and can reuse the same recorded receipt. Do not require a second
redundant confirmation for the continuation already named by this action.

Stage ReleaseHold and session-wide hold release preserve the underlying recovery
condition and do not issue replay permission. Do not silently release holds as
part of membership changes, fresh review or current-files confirmation. Current
constraints and resource limits remain in force. Use concise professional English;
internal identifiers such as manual_permit, effect_resolution and CAS belong only
in diagnostic details if needed.

## Integration boundary

Use Engine::team_control, owner_team_command, recovery_stages, control_recovery,
review_saved_plan_fresh, current_files_context and continue_with_current_files.
The second-round API documentation takes precedence over obsolete descriptions
in the initial backend API. Do not fabricate Store records or add provider-facing
owner commands. Stable IDs, revisions and exact command payloads remain bound to
their originating session even if the displayed page changes.

Execute asynchronous review and potentially expensive context construction away
from the input/render loop. Preserve responsive input, error delivery and
cancellation. Admit at most one ordinary Engine.run; coordinate with existing Git
checkout and project/session ownership. A membership command may apply during an
active run through the supported control boundary without replacing that run.
Refresh after stale reads/commands, discard obsolete asynchronous display results,
and never silently reinterpret a stale confirmation against a new context.

An interrupted command retains its exact ID and payload for an idempotent retry.
A new context or another requested review is a new explicit owner action, not an
automatic retry of an ambiguous paid call. Display pending and completed outcomes
from recorded state rather than optimistic local membership edits.

## Acceptance scenarios

Use the actual public consumers with scripted/native-protocol fixtures and
isolated temporary application state. Show live replacement of a busy member,
preserved existing work, no new admission to a departing participant and no
mutation of starting preferences for a loaded session. Show the complete saved
plan recovery through fresh review, current-files confirmation and ordinary
continuation; verify that unknown historical effects remain recorded.

Cover explicit holds and release, stale confirmation, switching displayed
sessions while work is pending, idempotent commands, cancellation, no admissible
reviewer, exhausted budget and prevention of a second run/Git conflict. Terminal
checks cover normal/small sizes and Unicode/ASCII with the accepted popup layout.
Reuse existing tests and shared components. Required Rust checks and independent
UI/terminal acceptance follow implementation. No real owner session, native model
trial, installation or strategy-selector completion is implied.
