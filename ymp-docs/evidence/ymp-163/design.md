# YMP-163 ordinary check replacement contract

Status: implementation design recorded before code changes on 2026-09-14.

## Observed cause

An executor can submit a board proposal while its task is running. The proposal is
currently bound to the complete board task version. Candidate submission and review
change lifecycle fields before the next board-consumption boundary, so the proposal
becomes stale even when the task definition is unchanged. On the final permitted
attempt, review also moves the task to `blocked`, while the legacy board consumer
requires `ready`; relaxing the version comparison alone would therefore still fail.
The existing `revise` change is intentionally additive; using it to replace a broken
command would retain the broken command and keep the task blocked.

The reproduced source is proposal
`16b7e2dd-2675-444e-a130-714583339034` in event 972 of the retained native-run
report. It requested an environment-compatible browser command while preserving the
browser-interaction command and the task objective. The proposal remained pending
during review and was rejected only after the third attempt became blocked.

## Contract

- Add a distinct `replace_checks` board change. It contains one or more exact
  `old` to `new` command pairs and a task reference bound to a digest of the task
  definition, not its running/review lifecycle fields.
- Keep legacy `revise` additive. `replace_checks` may alter only ordinary task
  commands. Captured owner-supplied acceptance contracts remain immutable and run
  through their existing confirmation path.
- Reject an empty replacement set, blank or known trivial commands, unchanged
  pairs, duplicate sources, missing source commands, duplicate resulting commands,
  foreign or unauthorized proposal origins, and an actually changed task
  definition. Preserve title, objective, competence, difficulty, dependencies,
  access, assignment history, attempts, budget and owner controls.
- At the safe boundary after the executor invocation ends, run the retained current
  checks and the proposed resulting checks and admit one independent reviewer through
  the ordinary review workflow. The reviewer receives the exact old and proposed
  command sources, both outputs and the executor rationale. A rejection is final for
  that proposal; the runtime does not seek a different reviewer for approval.
- Only an independently approved replacement is committed. The original result and
  check evidence remain historical but cannot accept the revised definition. The
  runtime creates a distinct typed result binding with the same producer, artifacts,
  trusted contract and `TaskAttemptRef`, but a new result ID and the reviewed ordinary
  checks. It reruns those checks and ordinary candidate review without another
  production assignment. The saved attempt counter does not change, and accepted
  sibling tasks and their evidence remain unchanged.
- A replacement is considered before the normal candidate review can consume the
  last attempt. This avoids permanently blocking the session solely because a valid
  current-definition proposal was waiting behind lifecycle bookkeeping.

## Trade-off

The runtime can prove exact replacement, origin, definition freshness and independent
review, but it cannot mechanically prove semantic equivalence of arbitrary shell
commands. Independent review therefore remains necessary and receives both command
outputs. This costs one bounded replacement-review invocation and a fresh ordinary
candidate-review invocation, but no additional production attempt. The
design deliberately does not ignore a failed command, reset or extend attempts,
reset spend, auto-confirm a result, replay production, or allow agent proposals to
weaken trusted acceptance authority.
