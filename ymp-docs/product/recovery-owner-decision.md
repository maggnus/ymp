# Owner decision: new work after unverified historical effects

Status: proposed, awaiting an explicit owner answer. This does not authorize a
live-session action or a comparative model trial.

## Why a decision is needed

The current maintainer-authored recovery contract refuses write continuation
while earlier effects remain unverified. Genuine old ACP sessions do not contain
the new complete local-effect declaration. A fresh independent read-only review
can assess the saved proposal, but does not establish that every later write is
independent of those historical effects. The current task model only records task
dependencies, not a complete machine-verifiable causal map of unknown effects.

Without an additional owner decision or sufficient recovered evidence, the safe
implementation can retain the new verdict and task graph but must stop before
write execution. That is useful partial progress, not completion of the owner's
session-recovery goal.

## Proposed explicit owner action

UI wording: `Continue with current files`.

The owner could authorize new work from the observed current workspace after
prior local execution has verifiably ended and conflicting ownership is released.
The action would be scoped to the particular session, failures and current state,
recorded with a version and command ID. It would not silently repeat the old
native invocation or reuse its failed context.

Keep historical effects unknown where they remain unknown. An owner authorization
is not independent effect confirmation, does not populate effect_resolution,
does not award reputation and does not claim rollback or universal safety.
Existing team, permission, resource and acceptance constraints continue to apply;
no consumed resources or user data are reset. Explain unresolved historical
effects in the reviewable action context before the owner chooses it.

This is a proposed change to the conservative continuation rule, not an already
approved interpretation of team replacement. Its API and execution scope must
follow the owner's decision; no generic unsafe-retry switch is proposed.

## Current authorized work

Implement the fresh read-only review, saved-verdict consumption and malformed
completed-response correction. Preserve the write refusal where dependencies on
unknown effects are not established. Ensure explicit owner holds can be released
through a typed path without accidentally permitting unsafe replay; a hold and an
effect refusal are different conditions.

Question: should the owner have the explicit `Continue with current files` action
described above, or should writes remain refused until the effects are verified?
