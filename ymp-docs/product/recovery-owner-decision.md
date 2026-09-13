# Owner decision: new work after unverified historical effects

Status: approved by the owner on 2026-09-14. The owner explicitly selected the
option allowing the action by an explicit owner decision. YMP-153 records this
product decision; implementation and verification remain part of YMP-146.
This approval does not authorize operating on a real session or conducting a
comparative model trial.

## Problem and approved behavior

Genuine old ACP sessions do not contain the new complete local-effect declaration.
A fresh independent read-only review can assess a saved proposal, but cannot
establish that all later writes are independent of unknown historical effects.
The task model does not contain a complete causal map of those effects.

The owner approved an explicit `Continue with current files` action after prior
local execution has verifiably ended and conflicting ownership is released. It
allows new work from the current workspace within existing constraints, retaining
unknown historical effects without claiming that they are safe or resolved.
This supersedes the unconditional write refusal in the earlier maintainer-authored
recovery contract only when this particular owner authorization is present.

## Command and authority

- Expose the action only through the trusted local owner API. Agents and providers
  cannot assert owner identity or issue this authorization.
- Present a reviewable context identifying the session, affected failed execution
  records, current saved stage/result and current workspace state. Explain the
  unresolved effects before the owner chooses the action.
- Bind the command to that context, the expected revision and a durable command
  ID. Validate it atomically; stale or foreign requests have no partial effect.
  Repeating an already handled command returns its recorded outcome rather than
  authorizing or starting another attempt. Persist the decision across restart.
- Require verified termination of prior local execution and release of conflicting
  access. Acquiring a lock alone is not termination evidence. Unknown or active
  prior execution still refuses continuation.
- Record a distinct owner authorization, not `manual_permit` for replay and not
  `effect_resolution`. Scope it to the acknowledged historical failures; a later
  unrelated uncertain failure does not inherit this permission.

## What continuation may do

The ordinary runtime may consume the saved review and task graph and admit new
work from the current files under this authorization. Retain the exact saved
proposal, accepted results, objections, usage and authorship. Do not repeat
completed planning or turn a negative verdict into approval.

Use fresh assignments and native contexts where an uncertain invocation is being
replaced. Do not blindly retry the old invocation or revive its native context
or grants. Preserve ordinary independent review, resource admission, team
constraints and serialization of conflicting or unbounded writes.

An authorization command must not silently clear an existing owner Wait/Pause or
global pause. Use a reachable explicit hold-release transition where required;
releasing a hold alone does not authorize uncertain replay. Keep authorization,
hold release and actual task admission distinguishable in the recorded history.

Historical uncertainty stays visible. The owner decision does not fill
`effect_resolution`, increase reputation, certify external effects, reset consumed
resources, enlarge budgets or permissions, or imply rollback. Existing evidence
inspection remains a separate operation with its original requirements.

## Acceptance of implementation

Exercise the public owner action and ordinary continuation on temporary data with
genuine legacy-shaped records and no `local_effect_scope`. Through a controlled
native-protocol fixture or a justified equivalent, show that new task execution
can finish in the same session with no duplicate planner, no blind replay and no
loss of history. A plan-only result or a new waiting label is insufficient.

Cover absence of owner authorization, active or unknown prior execution, stale or
foreign commands, replay after restart, explicit holds, exhausted resources,
independence, and a new uncertain failure outside the acknowledged scope. Assert
that historical uncertainty remains unresolved after successful continuation.
No actual native inference, live session change or comparative trial is authorized
by these engineering tests. Linux and real-provider behavior must be reported
separately if unverified.
