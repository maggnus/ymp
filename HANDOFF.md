# Resume handoff

Updated: 2026-09-29, after the native-pilot P1 deadline repair.

This is a recovery note, not another task register. Reconcile it with Git and the
canonical JSON task records before acting. Replace stale details when work moves
forward; do not accumulate a transcript here.

## Current continuation point

The P1 deadline repair is committed on main in `714e7046`, with the bounded
native example and evidence from runs 01-05. W1-0014 is again recorded complete
at revision 14; its completion record is being committed with this handoff.
Independent review accepted (R1, 9/10). Full `make verify` passed; all nine
Dispatcher scenarios passed in 140.16s. Log: `/tmp/ymp-pilot-offer-verify.log`.
Do not repeat those checks solely because the session resumed.

Next: build the actual example binary against the repaired runtime, then perform
one bounded run in NEW `/tmp/ymp-gpt-elementary-20260929-06`, retaining the same
model, effort and final run05 limits. Build command:

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true cargo build -p ymp-storage \
  --example homogeneous_gpt --offline
```

Run command (check directory/journal first after any interruption):

```sh
target/debug/examples/homogeneous_gpt run /Users/maggnus/.local/bin/codex \
  /tmp/ymp-gpt-elementary-20260929-06 gpt-6-luna low
```

No native process is active at this checkpoint. Never infer safe repetition from
an absent process: inspect the retained invocation and accounting records.
Full journals/logs use `/tmp/ymp-gpt-elementary-20260929-NN[.log]`. Committed
compact evidence and limitations are in
[the pilot directory](ymp-docs/experiments/homogeneous-gpt-elementary/README.md).

Finished runs:
- 01: zero calls, missing executable observation; example corrected.
- 02: zero calls, 10ms offer window expired; corrected to1000ms.
- 03: one native intake call; valid JSON but cost12404.4 exceeded allowance12000.
- 04: intake completed, cost12454.4; attempt capacity1 refused the second Plan
  stage. Example now permits2; its StopPreserving-only ladder selects no retry.
- 05: both Plan calls completed, cost26509.8, held0; a second late production offer
  exposed the repaired P1 defect. No native producer or independent review yet.

All four actual calls requested/sent `gpt-6-luna / low`; independently reported
model/effort remained unknown. Codex0.156.1 advertised seven offerings. Current
forecast: input20000/output1000/p90 factor1.25; unchanged ceiling30000 per call,
budget250000, verification reserve60000, reporting reserve30000 (relative weighted
token units, not currency). Two pinned identities, one concurrent invocation,
attempt capacity2 for two Plan stages, base timeout60s, overall480s+30s cleanup.
Comparison of token efficiency and useful self-organization remains unproved.

W1-0015 remains unclaimed (last observed revision8). After the bounded pilot,
continue the existing task plan through `manage.py next` then `show`; do not
silently replace the TUI task with extra architecture or broader experiments.
Root owns main; no child writer is active.

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
   python3 ymp-docs/tasks/manage.py show W1-0015
   ```

2. Read [intent.md](intent.md) and the authoritative
   [domain model](ymp-docs/self-organizing-team-domain-model.md) before changing
   behavior. Read implementation notes selectively; do not reload every task.

3. From the current continuation point above, continue the bounded homogeneous GPT pilot
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
   is ready again after the W1-0014 repair.
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
