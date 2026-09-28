# Resume handoff

Updated: 2026-09-29, during preparation of the first native pilot.

This is a recovery note, not another task register. Reconcile it with Git and the
canonical JSON task records before acting. Replace stale details when work moves
forward; do not accumulate a transcript here.

## Active work after the checkpoint

Root is the only writer on main. W1-0014 was reopened through the task tool
(planned revision 11, then claimed by codex at revision 12) for a concrete P1
real-clock regression found during the native pilot. Prior delivery commits and
evidence remain valid history; the current record now owns repair status.

Uncommitted changes: `crates/ymp-storage/examples/homogeneous_gpt.rs`, compact
run evidence and README under `ymp-docs/experiments/homogeneous-gpt-elementary/`,
a narrow Dispatcher deadline fix, one short consumer regression, task records
and this handoff. Never reapply the old W1-0014 worktree over these changes.

Five sequential runs are finished; no native process is active:
- 01: zero calls, missing executable dependency observation; fixed in the example.
- 02: zero calls, 10 ms offer window expired; changed to 1000 ms.
- 03: one real intake call; correct JSON, but actual cost 12404.4 exceeded the
  12000 relative-unit allowance. Receipt Complete, held 0; output not consumed.
- 04: intake completed, cost 12454.4; capacity 1 refused the second distinct Plan
  stage. Driver now uses capacity 2, with StopPreserving only and no production retry.
- 05: both Plan calls completed, cost 26509.8, held 0; production solicitation
  received one valid offer, then a second late submission caused cancellation.
  No producer artifact or independent review has yet been exercised natively.

All actual calls requested/sent `gpt-6-luna / low`; reported model/effort remain
unknown. Do not change models or effort to hide integration failures. Current
forecast: input 20000/output 1000/p90 factor 1.25, within unchanged 30000 per-call
ceiling and 250000 session budget, 60000 verification reserve, 30000 reporting
reserve, base timeout60s, overall480s plus30s cleanup. Two identities and profile
are now strict Pins. Discovery observed Codex0.156.1 and seven offerings.

Immediate next steps:
1. The focused check passed both received-offer and empty-window cases in 0.10s
   (`/tmp/ymp-offer-deadline-green.log`); independent review accepted (R1, 9/10).
   Full `make verify` passed with exit 0: `/tmp/ymp-pilot-offer-verify.log`.
   All nine Dispatcher scenarios passed together in 140.16s. Ready to commit;
   do not repeat these checks without a new change or concern. The new
   short test failed on old code in0.14s (`/tmp/ymp-offer-deadline-red.log`).
   Fix: stop submitting RuntimeProxy offers at the deadline, then use the existing
   AwardPolicy on valid received offers. Source writer is root; review is read-only.
2. Verify the fix, preserve all pilot outcomes, commit the repaired W1-0014 and
   its canonical completion. No native inference while this boundary is unverified.
3. A further homogeneous pilot can then test the repaired production transition;
   use a NEW directory and retain the earlier costs/results. Do not repeat a run
   because the conversation or its process disappeared.

Current example build log: `/tmp/ymp-pilot-build.log`. Native logs/directories use
`/tmp/ymp-gpt-elementary-20260929-NN[.log]`; complete SQLite journals and view.json
remain there. Compact evidence run-01.json through run-05.json is in the repository
but not committed yet. Normal builds/tests never invoke a native model.

## Owner direction

- Follow the existing detailed plan and explicit dependencies. Continue useful
  work without repeatedly asking permission to proceed.
- Prioritize a working proof of concept and rapid development. Add code tests
  only for necessary behavioral evidence or a concrete regression, at the point
  the relevant boundary becomes usable. Required pre-commit checks still apply.
- Experiments are sequential: a homogeneous GPT team first, then different GPT
  models, then different model families. No single-agent prerequisite. Difficulty
  progresses from elementary to simple to medium, one dimension at a time.
- Finish the engine before claiming a product experiment. Distinguish actual
  model inference, protocol fixtures and Scripted execution.
- Explain overall progress plainly in professional Russian. Product documents,
  code and user-facing application text remain English. The owner recently asked
  for a clearer explanation after too many detailed test updates.
- Keep this file current after meaningful progress and before ending a session
  or handing work over. This was explicitly requested to survive context loss.

## Verified checkpoint

Branch: `main`. The last product implementation commit is
`b566b0d9fce8c3294e7c4db67f260eec19e72f97` (W1-0014 session integration).
`5b6cf82e` commits its canonical completion record. Both are on `main`.

At this checkpoint, [W1-0014](ymp-docs/tasks/records/W1-0014.json) is recorded
complete at revision 10. Do not reimplement or reapply that work. Its immediate
prerequisites, including native Codex integration W1-0018, are already delivered.

The real runtime now connects paid planning, production, captured ResultVersion,
verification, paid independent candidate review, A7 acceptance, P2 discharge,
final aggregate checks/review and audited reporting. It includes atomic owner
stop, retained accounting, recovery, Retry and AddVerifier. Evidence is in
[session-implementation.md](ymp-docs/session-implementation.md).

This is the fixed W1 workflow. Agent initiative and dynamic cooperation remain
later work. The CLI and TUI are still placeholders. There has been **no native
GPT-team experiment and no ymp model inference**. Earlier native probes inspected
metadata only. No pilot runner was committed at this checkpoint; preparation is
described above.

There were no pending product edits or running checks at that checkpoint; current
work is described above. The pre-existing untracked `.wrangler/` is unrelated;
do not stage, remove or modify it.

## Resume actions

1. Read [AGENTS.md](AGENTS.md), this file and the current Git state:

   ```sh
   git status --short
   git log -5 --oneline
   python3 ymp-docs/tasks/manage.py next
   python3 ymp-docs/tasks/manage.py show W1-0014
   ```

2. Read [intent.md](intent.md) and the authoritative
   [domain model](ymp-docs/self-organizing-team-domain-model.md) before changing
   behavior. Read implementation notes selectively; do not reload every task.

3. After the active repair above, continue the bounded homogeneous GPT pilot
   under [poc-experiments.md](ymp-docs/poc-experiments.md), now that W1-0014 exists:
   two distinct agents, the same actually available model and supported low
   reasoning setting, an elementary sorted-JSON task in a disposable workspace.
   Use the actual Dispatcher, admission, checks, review and accounting. A direct
   provider call cannot substitute for the team experiment. A minimal invocation
   entry point still needs preparing; do not assume there is a runnable CLI.
   Fix finite limits before inference. Record exact input, policies, settings,
   usage/unknown coverage, outcomes, journal references and elapsed time. Preserve
   negative or inconclusive findings; do not automatically increase effort or
   repeat the experiment without a stated reason.

4. Before the deadline repair was reopened, the task tool selected
   [W1-0015](ymp-docs/tasks/records/W1-0015.json), the interactive Ratatui interface:
   last observed planned, unowned, revision 8. It has **not been claimed** and
   now waits for the W1-0014 repair to be completed again.
   Re-read the revision, then claim through `manage.py` before implementation.
   Follow [tui-reference.md](ymp-docs/tui-reference.md) and the task's complete
   terminal journey. The experiment direction does not reorder development tasks
   or bypass dependencies. Do not add unrelated architecture or polish first.

## Checks already completed

Final W1-0014 verification passed with exit status 0:

```sh
CARGO_PROFILE_TEST_OPT_LEVEL=1 \
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true make verify
```

This ran the legacy scan and all four required Cargo commands, including tests.
No Cargo profile source was changed. Test optimization reduces expensive journal
replay while keeping debug assertions and overflow checks enabled. Do not rerun
the full suite merely because the conversation resumed.

- All seven Dispatcher scenarios passed together in 165.26 s: full session and
  reopening on Memory/SQLite, stop, failing check despite approval, actual
  verification-capacity shortage, missing final reviewer, Retry and AddVerifier.
- Final reporting passed in 40.44 s; results in 33.13 s; atomic session controls
  with concurrent writes in 0.75 s. Independent review accepted the stable code
  and documentation: R1, 9/10.
- A real negative control removed the A7 missing-evidence guard and produced
  false acceptance; restoring the guard restored the expected failure rejection.
- The first combined run hit Retry's external 120 s test watchdog. The isolated
  scenario had taken 104.66 s. Only that watchdog was raised to 240 s, with state
  diagnostics on timeout; the next complete run passed. Runtime limits were not
  relaxed. Contention was a plausible explanation, not a measured root cause.

Local logs, if still available:

- `/tmp/ymp-w1-0014-final-restored-verify.log`: final successful full run.
- `/tmp/ymp-w1-0014-final-verify.log`: earlier watchdog failure.
- `/tmp/ymp-w1-0014-a7-red.log` and `...-a7-restored.log`: negative control.

These temporary files may disappear after restart. Committed implementation
notes and task evidence preserve the conclusions; missing logs alone do not
justify rebuilding completed work.

## Important boundaries and unverified cases

- A7 acceptance and A8 criterion satisfaction are distinct. New candidate
  acceptance uses version 2; version 1 is replay-only. Already recorded exact
  conclusive check runs need canonical Evidence before acceptance.
- Paid ordinary review consumes the original completed, settled, closed call
  after grant revocation. Recovery restores local owner/result capabilities,
  never old grants or duplicate native starts.
- RecoveryIntent distinguishes Observe, Continue and deterministic-only Report.
  Continue cannot reopen reporting or sticky finalization stop. RecoveryConsumed
  is bound to one exact completion and cannot repeatedly clear a later block.
- Timely retained completion is reconciled before expiry of remaining work;
  already recorded Expired is not reversed. The precise interruption cut across
  this ordering and the last narrow narrator-unavailability/failure cuts were
  code-reviewed, but not independently reproduced as full scenarios.
- Memory recovery retains the same in-memory journal object; its real identity
  marker does not provide durable RAM recovery. SQLite supplies persistence.
  Losing a live executor does not authorize starting the same invocation again.
- FixEnvironment and ReplaceCheck need the explicit inputs/approval required by
  their services. Later board/consequence wakeups and automatic team growth are
  not implemented by this session integration.
- See [codex-implementation.md](ymp-docs/codex-implementation.md): the adapter was
  checked against `codex-cli 0.156.1`. Discover actual version/models/settings
  again for the pilot. It supplies mediated file access, not process/browser
  capability. Preserve native authentication and distinguish requested, sent
  and reported settings; missing usage is not zero. Standing experiment
  authorization and its limits are in AGENTS.md; no per-run approval is needed
  within that scope.

## Retained worktrees and ownership

- `/private/tmp/ymp-w1-0014-session`, branch `codex/w1-0014`, is the old task-owned
  integration worktree based on `92e061d3`. Its patch has already been integrated.
  Main also contains the later watchdog adjustment. No child writer is active.
  Do not treat its dirty files as missing work or apply them over main. It may be
  removed after confirming that it contains only this delivered task's artifacts.
- `/Users/maggnus/Code/ymp-wt-execfix` and
  `/Users/maggnus/Code/ymp-wt-execution` predate this task. Leave them untouched.
- No legacy-tag code was opened during W1-0014 or the handoff preparation.
  Every commit needs the `Legacy-Consulted:` trailer. Prefer committed reference
  notes over legacy access; any access must meet the task-specific AGENTS rule.

When updating this file, retain only the current checkpoint, next actions,
unresolved questions, ownership and useful evidence. Task JSON records remain
the sole authority for status and must be updated through the revision-checked
task tool. Commit delivered code before recording a task complete on `main`.
