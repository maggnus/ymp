# First homogeneous GPT pilot

This is experimental evidence, not a development-task status register. The
delivered W1-0014 Dispatcher supplies the fixed workflow; agent initiative and
dynamic cooperation are not claimed by this pilot.

## Procedure

The explicit example is
`crates/ymp-storage/examples/homogeneous_gpt.rs`. Its `discover CODEX` command
reads native metadata without model inference. Its
`run CODEX NEW_DIRECTORY MODEL EFFORT` command refuses an existing directory,
uses the real Dispatcher and retains a SQLite journal, exact inputs/settings,
policy parameters, snapshots, receipts and a final projection/summary.

Two agent identities, `gpt-a` and `gpt-b`, are pinned to the same discovered
`gpt-6-luna` model and `low` effort. Discovery observed Codex 0.156.1 and seven
available offerings. Comparative token efficiency is unknown. Requested/sent
settings are recorded; independent reported per-turn model/effort remain unknown
when the native provider does not supply them.

The task transforms `[3,2,1,2]` into exactly `[1,2,2,3]` followed by a newline.
Separate owner checks cover exact output bytes and unchanged input. Attempt
capacity is now two to admit the workflow's two distinct Plan stages; the selected
ladder permits no production retry. One concurrent invocation and two members
are allowed. The overall deadline is
480 seconds with 30 seconds of bounded cleanup. Base invocation timeout is
60 seconds and output bound 8,000 characters, scaled by difficulty through the
existing policy; each invocation permits one native turn. The report is
deterministic, with no paid narration call.

Accounting uses relative token weights, **not currency or provider billing**:
input 1, cache read 0.1, cache write 1.25, output 4. Overall limit is 250,000 units,
with a 60,000 verification reserve and 30,000 reporting reserve. The existing
per-call ceiling is 30,000. UnknownUsage::Stop preserves incomplete accounting.
Complete receipt coverage means complete provider-reported usage under the
adapter's documented boundary, not independent billing verification.

## Observations

| Run | Native calls | Observed outcome |
| --- | ---: | --- |
| [01](run-01.json) | 0 | Missing executable dependency observation excluded both agents before admission. |
| [02](run-02.json) | 0 | A 10 ms offer window elapsed before RuntimeProxy submission under the real clock. The host then stopped and reported. |
| [03](run-03.json) | 1 | Intake returned valid structured output, but actual weighted usage crossed its 12,000-unit allowance. The engine retained the receipt and stopped before production. |
| [04](run-04.json) | 1 | Intake completed and settled. Capacity one then refused the second distinct Plan stage, which shares its attempt scope under the existing admission rule. |
| [05](run-05.json) | 2 | Both paid Plan stages completed. Production received one valid offer, but Dispatcher attempted another after the deadline and returned an error. |
| [06](run-06.json) | 2 | On the repaired runtime, intake completed; native planning returned an error object because its code-mode host was disabled. The kernel refused that output as a plan. |

Preparation corrections supplied a real StaticDependencyProbe observation, pinned
the roster/model/effort (defaults alone do not restrict the registry), and used a
finite 1,000 ms offer window. Both zero-call journals were preserved.

Run 03 used 21,110 input tokens including 9,984 cache reads, plus 70 output tokens
and reported zero reasoning tokens. Its receipt coverage was Complete. The exact
weighted charge was 12,404.4 units, with zero remaining held amount. Requested
and sent settings were `gpt-6-luna / low`; reported model/effort were null.
The output was `{"criteria":[],"questions":[]}`. Crossing the allowance correctly
prevented consuming that output as successful paid planning.

Runs 04 and 05 used a revised initial forecast based on this observation:
20,000 input, 1,000 output, p90 factor 1.25. Model, effort, task, overall budget and
per-call ceiling stay unchanged. This is an explicit startup-cost calibration,
not evidence of team cooperation or a reason to raise reasoning effort.
Run 04 spent 12,454.4 units with no remaining hold. Its native receipt reported
21,144 input tokens (9,984 cache reads), 74 output and zero reasoning tokens.
The example's attempt capacity was then corrected from one to two to permit the
two required planning stages; the StopPreserving-only ladder remains unchanged.

Run 05 spent 26,509.8 units with no remaining hold. Both Plan invocations were
Completed with Complete receipts. Intake reported 21,129 input / 9,984 cache read /
81 output tokens; planning reported 22,260 input / 9,984 cache read / 192 output.
Both reported zero reasoning tokens. No producer invocation or independent review
occurred. There is still no completed native team result.

The late-offer failure is a Dispatcher ordering defect under P1: after a deadline,
the existing AwardPolicy must consider already received valid offers rather than
attempting to submit missing late offers. W1-0014 was reopened for this repair.
A short consumer scenario reproduced the old error in 0.14 seconds. The fixed
consumer and a no-offers case passed together in 0.10 seconds. Empty windows now
record Blocked(no_offers); a failed narrator solicitation can use the deterministic
fallback. No window is silently extended and no native call is duplicated.
Independent review accepted the repair (R1, 9/10). Mandatory combined verification
passed, including all nine Dispatcher scenarios in 140.16 seconds. The repair is
committed in `714e7046`, with completion recorded in `f4e63d8f`.

Run 06 used that repaired runtime and unchanged run05 parameters. It spent
18,729.4 units with zero hold. Intake reported input 21,127/cache 9,984/output 75;
planning reported input 22,232/cache 17,920/output 46; both receipts were Complete
and reasoning was reported as zero. Planning returned an error object instead
of PlanDefinition, and the kernel correctly stopped with no producer admission.

Read-only inspection used the known pilot thread ID and
[`thread/read`](https://learn.chatgpt.com/docs/app-server#read-a-stored-thread-without-resuming),
which does not resume the thread. Its own persisted tool items show a native
`exec` call attempting `tools.ymp_read({path:"input.json",limit:1000})`, followed
by `code-mode host is disabled`. The compact trace is retained in run-06.json;
no new inference was started during inspection. Native tool-mode compatibility
must be understood before another run. Enabling arbitrary execution or bypassing
the InvocationFiles mediator would not be a valid fix.

Full local journals and logs use `/tmp/ymp-gpt-elementary-20260929-NN` and the
corresponding `.log` file. Compact evidence in this directory preserves the
observed inputs, effective parameters, usage and outcomes. Never restart a native
run solely because its process or conversation disappeared; inspect the journal.
