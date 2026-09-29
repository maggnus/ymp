# Resume handoff

Updated: 2026-09-29 20:40 Asia/Hong_Kong. Reconcile this note with Git and task
JSON records before acting; it is not another task register.

## Immediate continuation

Root owns main. **W1-0015 (interactive Ratatui interface and the `ymp`
executable) is done at revision 11, delivered in `4a0b903f`**; `make verify`
passed on that tree. Read [tui-implementation.md](ymp-docs/tui-implementation.md) for the composition,
the refusals, the adaptations against the reference, the evidence and the
limits. The terminal journey is reproduced by
`ymp-docs/tui-journey/journey.sh target/debug/ymp NEW_DIRECTORY TRANSCRIPT`
(scripted team, no model, about six minutes, needs tmux); the committed
`transcript.txt` is one such run. No `legacy-*` tag was opened for the task.

Owner direction 2026-09-29: run native experiments on light Anthropic models,
parallelize independent work, watch the disk (about 13 GiB free, 98 % used;
delete nothing without the owner's word). Scratch directories of the interface
work are under `/tmp/ymp-tui-check/` and are not authoritative.

Not verified for W1-0015: terminal restoration after a panic of the interface
loop or a terminal error was not provoked; `SIGTERM`/`SIGHUP` are not handled;
an interrupt, a recovery and a question to the user were exercised on scripted
sessions only. One native session was driven through the interface (pilot run
15 below).

Questions for the owner from W1-0015:
- The task context says headless subcommands must not link the terminal UI.
  `ymp` is one executable, so `sessions` and `report` are linked with the
  terminal library although they never enter it. Removing that needs a second
  binary target; this was recorded as an adaptation, not decided.
- `Dispatcher::recover` records the owner's intent before its own checks, and
  `Dispatcher::run` returns the first `stale_revision` of a step. The interface
  works around both (`host.recover` compares strategies first; the session
  thread repeats a refused report step of a stopped session only). An
  independent reading found Dispatcher steps with more than one commit that
  cannot be repeated after a refusal (`blocked`, captures, recovery retry,
  `launch` after a refused dispatch). Whether the runtime itself changes is
  open.

Backlog observations from W1-0015, not work: a Dispatcher tick takes 400 to
650 ms on a journal of more than a hundred events; the runtime accepts an
interrupt after a delivered report; an `Unknown` producer coverage waits for
the call deadline (240 s) before the session blocks; `LiveSession::publish`
ignores a failed projection read; outcomes of commands still queued at close
are not read; the sidebar overlay of a short terminal does not scroll; at 130
columns the sidebar and the footer cut words without an ellipsis; the blocking
and report lines of the conversation print the latest recorded time, not their
own; the report lists criteria as unmet although their candidate checks passed.

Native pilot, `claude-haiku-4-5-20251001`, native version 2.1.284, no effort
(the model offers none). Runs live in `/tmp/ymp-claude-pilot/runNN` with
`runNN.log`; never open an original `journal.sqlite`, read the copies under
`/tmp/ymp-claude-pilot/scratch/runNN/`. Units are relative token weights, NOT
currency.

| Run | Calls | Spent | Held | Session phase | Changed dimension |
| --- | ---: | ---: | ---: | --- | --- |
| 01 | 0 | 0 | 0 | `no_offers` | first run, unoptimized build |
| 02 | 2 | 25,663.25 | 0 | `work_failed` (`cost_limit`) | optimized build |
| 03 | 3 | 32,717 | 0 | `no_offers` | forecast 10000/1000/2.0 |
| 04 | 1 | 8,700 | 0 | `decoding` (Markdown fence) | offer window 5000 ms |
| 05 | 5 | 69,253 | 0 | `decoding` (reviewer) | adapter strips one fence |
| 06 | 5 | 59,361 | 0 | `candidate_rejected` | reviewer response contract |
| 07 | 5 | 59,098 | 0 | `commitment_expired` | adapter waits for host records |
| 08 | 1 | 0 | 28,000 | `claude_environment` | review fixes; wrong empty-plugins check |
| 09 | 5 | 51,212 | 0 | `commitment_expired` | built-in plugins accepted |
| 10 | 1 | 14,028 | 0 | `decoding` (prose before JSON) | invocation timeout 240 s |
| 11 | 6 | 79,245.75 | 0 | `decoding` (final reviewer) | repeat of run 10 parameters |
| 12 | 6 | 78,274 | 0 | `Cancelled` (`final_review_basis`) | final-review contract states JSON shape |
| 13 | 6 | 70,628.75 | 0 | `decoding` (prose before final verdict JSON) | final-review contract lists evidence ids; adapter review fixes |
| 14 | 5 | 70,835.5 | 0 | `decoding` (reviewer: prose and fence before JSON) | adapter guidance states the JSON first and last character |
| 15 | 5 | 65,406.25 | 0 | `decoding` (reviewer: prose and fence before JSON) | consumer: the interactive interface of `ymp` at `258b348d` |

Runs 01 to 11 and 13 to 15 delivered `Blocked(final_acceptance_unavailable)`; run
12 ended `Cancelled`. No native run reached final acceptance. Runs 07
and 09 produced the expected `sorted.json`, reviewer `Approve` and acceptance
`Accepted`/`Confirmed(TrustedCheck)`; the producer's 60 s lease had expired during
verification, because the lease is bounded by the invocation timeout and is
discharged only by acceptance. With 240 s (run 11) the lease held, the session
reached `Finalizing` and a sixth call, the final review, answered
`FinalVerdict: ...` as one text line instead of JSON. The contract text in
`crates/ymp-kernel/src/finalization/context.rs` now names the aggregate
reference as `subject` and states the JSON shape. In run 12 the verdict decoded
and named the right aggregate, but its basis held check ids instead of evidence
ids (the prompt is a positional tuple without labels); the kernel refused it
with `final_review_basis` and the example cancelled the session. The purpose now
also carries the evidence ids under `evidence`. In run 13 the final verdict
named the right aggregate and the two supplied evidence ids with `Approve`, but
prose preceded the JSON object and the decoder refused it. The adapter guidance
was then extended (a JSON response starts with `{` and ends with `}`); in run 14
the candidate reviewer still answered prose plus a fenced JSON object. Run 15
repeated the task through the interactive interface and ended the same way as
run 14, with the expected artifact and passing candidate checks. Four of the
fifteen runs (10, 13, 14, 15) ended on prose around an otherwise usable JSON
answer. The pilot stops here: no further repeats without a product decision on
a retry after an undecodable response.

Independent review of the tree (read-only) found no authority or data-leak
defect. Applied from it: the mediated file operation is shared by both process
backends (`backends/files.rs`) instead of duplicated; an unsettled wait refuses
the file operation (`claude_unsettled`); `CLAUDE_CONFIG_DIR` is kept so that the
stored native login is preserved; document claims were corrected. Recorded as
limits in `claude-implementation.md`, not repaired: isolation is asserted by
`system/init` only after the user message; native turn count is not recorded;
prompt text changes are not versioned, so journals of pilot runs 05, 11 and 12
cannot be replayed by a later build. Run 08's 28,000 held units and unknown usage are
retained evidence: never release, relabel or repeat them.

Native evidence for runs 01 to 15 is in
`ymp-docs/experiments/homogeneous-claude-elementary/` (`README.md`, `NOTES.md`,
`derive.py`, `run-NN.json`; run 15 has no run file, its evidence is
`run-15-terminal.txt` and `run-15-calls.txt`); the privacy scan found no
address, home path or key. Totals: 56 native calls, 684,422.5 units spent,
28,000 held (run 08). The working copies remain under `/tmp/ymp-claude-pilot/`
and are not authoritative.

Immediate next actions:
1. `manage.py next` returns no task: all twenty W1 tasks are done and the tasks
   of W2 to W6 are `new`. Scheduling them is the owner's decision. Read
   `intent.md` and the model before changing product behavior.
2. Do not repeat the elementary pilot as it is. A further native run needs one
   named changed dimension, a fresh directory, and no build or test running
   beside it (the run is time-sensitive).
3. Later experiment stages (different models, different families) follow
   `ymp-docs/poc-experiments.md` and need a native run that reaches final
   acceptance first; whether the session asks again after an undecodable
   response is a product decision for the owner.

Reversible decisions of W1-0020 to report to the owner: the adapter strips
exactly one whole-response Markdown fence (untagged or `json`) without a journal
record; the reviewer prompts in `paid_review.rs` and `finalization/context.rs` state the
exact JSON shape; the
adapter waits for two quiet host polls before each mediated file operation; the
shared process channel and file operation changed the delivered Codex
transport; the pilot's invocation timeout is 240 s; `CLAUDE_CONFIG_DIR` is kept
in the child environment; the adapter's fixed system text names the first and
last character of a JSON response (run 14 shows no benefit from it).

Known limits left in place: 64 calls per adapter instance; the technical
directory is removed only when empty; `claude_receipt_pending` is not recognized
by the host; an unknown tool is labelled with the wrong authority; a read
response may exceed the frame. Environment-variable removal is proven only where
such variables exist; `api_retry` was never observed natively.

Backlog observations, not work (the owner assigns priority): slow Dispatcher tick
(full projection rebuild); Haiku reasoning is 50-95 % of output; `lease_renewal`
diagnostic on every call with `renewals: 0`; no retry after a rejected candidate
or an undecodable intake answer; the delivered report lists criteria as unmet
while the ledger records `Satisfied`; `stale_revision` refusals are not
journaled; Codex accounting grouping (below); a refused final verdict
(`final_review_basis`) leaves no journal record and the example cancels the
session (run 12); the request context is a positional tuple without labels; a
producer's lease is bounded by the invocation timeout; Haiku put prose around a
JSON answer in three of fourteen runs.

## Retained Codex state

CodexAppServer v2 is committed in `aad168d6`, with W1-0018 completion in
`9f453715` (done revision14). W1-0014 is reconfirmed done revision17.
Codex native run07 is finished: mediated ymp_read succeeded, coverage was marked
Partial because commentary plus a file callback preceded one shared usage
update; held30000 remains, spent0. Do not relabel, release or repeat it. The
accounting grouping question (`codex/mod.rs::drive`, `pending_usage` /
`accounting_gap`) is open and unchanged; no accounting source change was made.

```sh
git status --short
git log -5 --oneline
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show W1-0015
python3 ymp-docs/tasks/manage.py check
```

## Delivered native compatibility repair

Native pilot run06 showed a real `exec` call trying
`tools.ymp_read({path:"input.json",limit:1000})`; the native response was
`code-mode host is disabled`. The model returned that error instead of a plan,
which the kernel rejected. Inspection used thread/read without resume and only
that known thread's stored tool items; no extra inference was started.

Exact upstream rust-v0.156.1 explains that model tool_mode overrides
features.code_mode=false. CodeModeOnly cannot fall back to Direct. The selected
fix enables the local V8 Code Mode host with in-process fallback disabled, while
retaining code_mode=false, empty environments and all other restrictions.
The V8 host has no Node/Deno/fs/process/fetch API, rejects imports and routes
registered tools through the existing broker. Shell/ApplyPatch/ViewImage require
an environment and remain absent. Pure native clock/plan helpers may exist;
ALL_TOOLS is not claimed to contain only two entries.

Changed files are under:
- `crates/ymp-runtime/src/backends/codex/{mod,protocol}.rs`;
- kernel policy schemas, `registry::mediated_backend`, `ports::execution`;
- the Codex process fixture and its existing storage consumer tests;
- model/implementation notes and canonical task records.

Version 2 has a distinct PolicyRef/discovery method. Version 1's parameter schema
and exact method bytes remain readable, and v1 discovery cannot satisfy v2 checks.
Both effective host.enabled=true and disable_in_process_fallback=true are checked,
including the native feature flag. Existing before-inference stop, exact
thread/turn, duplicate-call, InvocationFiles and accounting rules are unchanged.

A new Native discovery/start guard checks the selected helper before model work.
It follows recognized package bin/resources/CodexCLI.app layouts, preferring the
resource helper over bin, then the native parent fallback. A non-executable
preferred file is refused, not silently replaced by a sibling. Ambiguous
non-package legacy resource layouts return explicit unavailable rather than
infer ambient CODEX_HOME/package-manager settings. ProtocolFixture stays distinct.
On this installation the package manifest exists, the preferred resource helper
is absent, and the selected bin/codex-code-mode-host is a regular executable0755.

Code Mode host has its own process group. Exact source confirms stdin EOF cancels
cells, shuts sessions down with bounded waits and terminates V8 execution. This
is not proof of financial cessation; scoped withdrawal and unknown-cost rules
remain unchanged. Cells can outlive turns, so stale callbacks must stay denied.

Read [codex-implementation.md](ymp-docs/codex-implementation.md) for exact source
links, limits and version2 notes. No native file-tool execution under v2 has yet
been observed; that is the next pilot's evidence, not a completed claim.

## Evidence already available for this repair

- Focused actual protocol consumers: three passed in6.25s, metadata test ignored;
  `/tmp/ymp-code-mode-consumer-final.log`. This includes v1/v2 attribution and
  denial of an effectively disabled host or enabled in-process fallback.
- Selected-helper filesystem regression passed in0.00s:
  `/tmp/ymp-code-mode-layout.log`.
- Real metadata-only consumer after the helper guard passed in1.83s, seven
  offerings, zero model turns: `/tmp/ymp-code-mode-native-final.log`.
- Native effective-config probe confirmed required host flags, zero environments,
  roots/instruction sources and network=false: `/tmp/ymp-code-mode-metadata.log`.
- Static helper observation: `/tmp/ymp-code-mode-host-availability.json`.
- Final full verification/review passed as described above. Temporary logs
  may disappear; committed notes preserve completed conclusions.

Required verification command (all required Cargo commands still run):

```sh
CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true make verify
```

Actual example build (normal builds/tests never run models):

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true cargo build -p ymp-storage \
  --example homogeneous_gpt --offline
```

## Committed project and experiment checkpoint

- `b566b0d9` / `5b6cf82e`: original W1-0014 fixed accountable session and completion.
- `714e7046` / `f4e63d8f`: P1 deadline fix, pilot example/evidence, completion.
  After a deadline, use received offers rather than submit late ones; no_offers
  follows existing Blocked/deterministic reporting. Independent ACCEPT9/10;
  full verify passed, nine Dispatcher scenarios in140.16s. Do not redo this fix.
- `2ddc1bde`: run06 evidence and investigation handoff.

Six sequential pilot preparations/runs are finished. Full records are in
[the experiment directory](ymp-docs/experiments/homogeneous-gpt-elementary/README.md).
Runs01/02 made zero model calls (missing executable observation; too-small offer
window). Run03 intake crossed its initial cost forecast. Run04 intake completed,
but capacity1 refused the second distinct Plan stage. Run05 completed both Plan
stages, then exposed the repaired deadline defect. Run06 returned the disabled
Code Mode error. There have been six actual model calls, all planning. No native
producer artifact or independent review has succeeded yet; useful self-organization
has not been established. Scripted evidence remains separate.

Requested/sent profile: gpt-6-luna/low for two pinned identities; reported per-turn
model/effort remain unknown. Codex0.156.1. Latest limits: input forecast20000,
output1000, p90factor1.25; per-call ceiling30000, budget250000, verification60000,
reporting30000 in relative token-weighted units, NOT currency. Attempt capacity2
admits the two Plan stages; ladder only StopPreserving, no production retry.
One concurrent invocation, base timeout60s, global480s plus30s cleanup, offer
window1000ms. Read/write only through the mediator. Do not raise effort/budget or
silently change profiles to conceal integration failures.

Local runs: `/tmp/ymp-gpt-elementary-20260929-NN`, corresponding `.log` files.
Run06 known-thread metadata: `/tmp/ymp-gpt-elementary-20260929-06/native-thread.json`.
Next native run must use a fresh path and record its actual source commit.

## Owner direction and workspace hygiene

Follow the detailed plan/dependencies. Prioritize rapid POC work and necessary
behavioral evidence, not test counts or visual polish. Experiments are sequential:
homogeneous GPT, different GPT models, different families; elementary, simple,
medium tasks. No preliminary single-agent experiment. Standing native experiment
authorization in AGENTS.md persists; keep native authentication and finite limits.
Do not claim provider billing, complete tool isolation or recovery beyond evidence.

Communicate overall progress clearly in professional Russian. Product documents,
code/comments and application text stay English. Update this handoff after
meaningful progress and before ending/handover; keep it a current recovery note.
Read AGENTS.md, intent.md and the authoritative model before changing behavior.
Task JSON is the only status authority; use revision-checked commands and commit
code on main before canonical completion. Every commit needs Legacy-Consulted.
No ymp legacy-tag code was consulted for these fixes; native upstream source is
not an ymp legacy tag.

Leave pre-existing `.wrangler/` untouched and unstaged. Also leave
`/Users/maggnus/Code/ymp-wt-execfix` and `ymp-wt-execution` untouched.
`/private/tmp/ymp-w1-0014-session` is an old task-owned worktree based on92e061d3;
its original patch is already integrated and main has later fixes. No writer is
active there. Never reapply its dirty files over main; cleanup is optional after
confirming ownership. Root is the sole current writer.
