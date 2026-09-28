# Resume handoff

Updated: 2026-09-29 04:05 Asia/Hong_Kong. Reconcile this note with Git and task
JSON records before acting; it is not another task register.

## Immediate continuation

Root owns all current writes on `main`. Active task: W1-0018, in_progress,
owner codex, revision 12. W1-0014 is planned, revision 15, waiting for its native
adapter prerequisite to be repaired. W1-0015 is unclaimed, revision 8.

**No native model run is active.** A candidate CodexAppServer v2 fix is uncommitted.
The full mandatory check passed with exit 0:
`/tmp/ymp-code-mode-final-verify.log` (exec session 6753 in the current conversation).
All nine Dispatcher scenarios passed in 139.51s. Independent read-only review by
acceptance_boundary_read accepted the final helper-readiness addition (R1, 9/10).
Ready to commit; do not repeat checks without a new change or concern.

Next actions:

1. Inspect `git status`, the verification log and the final review. Fix only actual
   findings; do not repeat completed broad checks without a reason.
2. Record final evidence in W1-0018 with expected revision 12, commit its code,
   verify main reachability, then record and commit completion. Do not mark it
   done just because the review accepted it.
3. Reclaim W1-0014 using its latest revision and reconfirm session integration
   against the repaired adapter. Its P1 code repair is already committed; do not
   rewrite the session. Then close its canonical record with actual evidence.
4. Build the actual homogeneous_gpt example binary against the new runtime before
   any further pilot. Use a NEW directory (next is run07), with the same selected
   model/effort and finite limits. Never restart an old native invocation merely
   because its process or this conversation disappeared.
5. Continue the existing task plan with `manage.py next` then `show`; W1-0015 is
   the simple Ratatui interface. Do not replace it with unrelated architecture.

```sh
git status --short
git log -5 --oneline
python3 ymp-docs/tasks/manage.py next
python3 ymp-docs/tasks/manage.py show W1-0018
python3 ymp-docs/tasks/manage.py check
```

## Current uncommitted repair

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
- Final full verification/review are pending as described above. Temporary logs
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
