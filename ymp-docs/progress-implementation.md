# Recorded progress and bounded recovery

Canonical status remains in `tasks/records/W1-0012.json`. Application exposes
Progress using the original SessionControl. ProgressMonitor, FailureDiagnoser and
EscalationPolicy produce separate recorded Decisions with effective parameters,
inputs, rationale and source references. Handlers validate and perform their own
bounded transitions; a strategy cannot issue execution authority.

## Progress and diagnosis

The meaningful work boundary combines actual work references, criterion versions
and the current ledger's belief/status/evidence. Its identity excludes poll time
and bookkeeping-only ledger rewrites. Reassessing the same boundary cannot add
another stall. The bounded projection retains 128 recent work facts and uses the
last 64 references consistently for assessment and freshness. A sequence watermark
distinguishes new acceptance from an older acceptance still in that window.
Older events remain in Journal; the projection does not impose a new rejection
limit on historical replay.

EvidenceDelta follows A9's weighted belief change, epsilon, looping and stall
limit. Changed criterion definitions get a neutral comparison baseline rather
than inheriting a different requirement's old probability. Consecutive identical
work tuples are separate from the repeated-rejection window. Resolving old
rejections does not erase a current consecutive loop. AcceptedOnlyProgress is an
experimental control that resets a stall only for a new accepted result; its
record still exposes the actual weighted belief change.

RuleBasedDiagnoser uses the exact first-match order: Environment, CheckDefect,
CapabilityMismatch, ArtifactDefect, CapabilityLimit, Ambiguity, PlanDefect,
BudgetExhausted, Unknown. Applicable run roles, check/criterion versions, snapshots
and expected environments determine its facts. Execution Error is not a failing
artifact's Evidence. A repaired error and a retired check remain in history while
leaving the current diagnosis. DirectFailuresOnly is a conservative experimental
control that recognizes direct environment/check failures and otherwise returns
Unknown.

The minimum verification cost uses actual selected CostModel estimates for each
unmet required criterion and eligible profile. Satisfied/optional criteria do not
inflate that requirement. Profile readiness, current constraints, independence,
team membership and delivered execution capabilities constrain those inputs. No
eligible estimate means insufficient verification capacity; all required criteria
already satisfied produce a zero minimum. Uncalibrated contribution forecasts do
not supply calibrated success. Interpretation objections, StatusChange notices
and calibrated probabilities have no delivered source here and remain explicit
limitations. Stale plan scope can supply PlanDefect without a current candidate;
that branch received code review and compilation, not a separate integration run.

## Bounded actions

DiagnosisFirstLadder chooses the model's next step; StopOnUncertainty is an
experimental control that stops for diagnoses outside its direct repair/retry
cases. Each plan records expected effect, maximum uses, cost, timeout and success
condition. The selected Method ladder must permit the step. Current limits and
method are checked again at application, so several prepared proposals cannot
each reuse an earlier unused allowance. Successful ReplaceCheck also consumes
its use. A failed environment repair is bound to its exact old/new run scope,
not to unrelated failures elsewhere in the session.

- **FixEnvironment:** rerun the exact check, snapshot and role through a corrected,
  compatible CheckRunner environment. The retained actual environment must obey
  the step timeout. A non-Error result establishes repaired execution even when
  the check now correctly returns Fail. A retained matching rerun can complete a
  missing action record without executing it again. This handler repairs check
  execution; it does not install tools or repair a native provider's failed
  invocation. Invocation-only failures do not become successful check repairs.
- **ReplaceCheck:** stage a full owner-authored User check with the old/new check,
  criterion, candidate, current contract and exact defect references. A completed,
  accounted Reviewer invocation must approve precisely that staged proposal in
  its recorded prompt/output. Its identity must be independent of the candidate
  producer and any agent author of the old check. A generic candidate approval
  or suggested CheckSpec is insufficient. The atomic replacement removes the old
  check from the active contract and activates the new one, retaining both
  definitions and all earlier runs. A current candidate and eligible Reviewer
  are required; arbitrary agent-authored replacements are not delivered here.
- **Retry:** retain the original local PreparedAttempt, wait for actual lease
  expiry, expire the previous responsibility and abandon the failed attempt
  without removing its candidate. Rejection is not discharge. The new work uses
  the same producer/profile and exact failing evidence in its recorded prompt.
  Contribution creation and the recovery action are one journal packet. Normal
  admission, a new grant/reservation, a new Attempt and host execution follow;
  they recheck the recorded bounds, profile, context and attempt limit.
- **AddVerifier:** create bounded Research work for an existing independent team
  member in the Researcher role, with exact diagnostic references. Its subject is
  absent; the references and prompt carry the scope. Actual completion and receipt
  are recorded, but the researcher's text does not itself establish a new kernel
  diagnosis or criterion satisfaction.
- **StopPreserving:** use the original Treasury and BudgetControl of this journal
  and session to enter deterministic reporting mode. Further production and
  coordination funding is denied while accepted versions, spent cost and unknown
  holds remain. This transition does not create a report or certify cessation.

Results, PreparedAttempt, AcceptanceAuthority and budget controls are checked
against the same journal/session before effects. Freshness permits only the exact
effect belonging to the current recovery step, such as its rerun or paid review;
it does not authorize unrelated work or changed evidence.

Adding or replacing checks no longer prevents production/retry when the original
Task and criterion references are unchanged. The original Plan and Contribution
contracts remain recorded. Admission, contribution selection and acceptance-to-P2
resolution use that same narrow scope rule.

For sessions selecting the ProgressMonitor port, changing an agent's previously
admitted profile requires a recorded diagnosis. Growth handlers remain unavailable
in W1, so diagnosis alone does not grant a new profile. Initial assignments to the
original members remain possible. Switching monitor implementation cannot disable
this kernel rule. Previously shipped sessions without the port retain their
original replay behavior.

Reassign, Decompose, AlternativeAttempts, StrongerProfile, Clarify and Replan retain
their representation and history but return an explicit owning-task limitation.
Recovery does not reconstruct lost local capabilities after restart; the complete
session dispatcher and recovered owner controls remain separate work.
Ordinary candidate-scoped Verify/Diagnose contributions selected by W1-0011 still
lack their non-artifact P2 completion connection. Their current host completion
returns commitment_basis rather than discharging responsibility. W1-0014 owns
that integration; the Research(None) handler above uses an already connected path.

## Evidence and remaining coverage

`crates/ymp-storage/tests/progress.rs` runs the actual consumers through repeated
check errors, stall, corrected execution, a completed Researcher, exact paid
replacement review, new contradiction, expired-lease retry with a new invocation,
acceptance and stopping with accepted history retained. This uses Scripted agent
output/accounting, actual SQLite and retained-byte checks; it is not native-model
inference or an empirical self-organization experiment. Its first successful run
took 586.63 seconds, before the final narrow fixes.

A short real-consumer scenario rejects offered profile growth without diagnosis
and a foreign session's BudgetControl. It also proves that StopPreserving rejects
a valid Plan reservation with reporting_only while Production funds still remain.
Removing the R14 guard caused the expected failure (exit 101, 1.87 seconds);
restoration passed in 1.26 seconds. Targeted checks preserve literal A9 precedence
and separate consecutive work from resolved rejections. Final targeted Clippy,
formatting and diff checks passed. Final `make verify` exited 0: legacy scan,
offline workspace build, formatting, Clippy with denied warnings and all workspace
tests. Both storage progress scenarios passed on that final code (536.77 seconds);
the existing result/acceptance integration also passed. Independent review of code
and documentation returned R1(9/10) ACCEPT. The task register and staged diff
checks passed.

Native provider repair, calibrated inference, interpretation disputes, automatic
team/profile growth, general replanning, hidden/mutation checks and restart
reconstruction of PreparedAttempt are not established by these tests.
