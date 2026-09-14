# Fifteen-puzzle recovery and product trial

Status: preregistered plan; cause repair in progress; native repeat not started.

This study instantiates the [shared trial and acceptance protocol](../guides/session-trial-and-acceptance.md).
Its purpose is to determine whether ymp can finish an ordinary task autonomously
when its agents discover that a generated acceptance command is broken. It also
evaluates where the observed behavior supports the five goals in
[intent.md](../../intent.md), and where evidence is absent.

## Product question and primary endpoint

Can a Luna/Sonnet team deliver a working browser puzzle, correct its own failed
verification method through the runtime, and reach a truthful final outcome
without a person repairing the game or its session?

The experiment is unsuccessful until ALL required endpoint conditions hold:

1. The real native session finishes with `status=completed` and process exit 0.
2. Planned work reaches final acceptance; no blocked or unresolved task, pending
   relevant repair, open invocation or active execution remains.
3. The runtime executes its current required checks and an independent final
   review; no failed requirement is simply ignored to obtain completion.
4. Independent Playwright actions pass the predefined game criteria on the exact
   final artifact. An observer must not fix the game or inject its solved state.
5. The unchanged result is published to a distinct Cloudflare Workers URL; HTTP
   responses match the file hashes and private research files are excluded.
6. The completed report evaluates protocol and policy behavior, the five intent
   goals, competing explanations, product tradeoffs and prioritized proposals.

Runtime confirmation remains whatever the real contract supports. In the absence
of an owner-supplied trusted contract, `accepted/unconfirmed` is an honest native
label and must not be forged into `confirmed`. Independently verified artifact
success is recorded separately. False confirmation or reputation is a failure.

Bind native checks and final review to task/result and check-definition versions
and artifact hashes. Final acceptance must concern the same deliverable bytes
subsequently frozen for Playwright, publication and HTTP verification. Use the
runtime's existing artifact/evidence links and read-only observations; missing
bindings remain unknown, rather than inferred from exit 0. A relevant file change
requires affected checks and review through the runtime, not replay of unchanged
siblings. At terminal state establish that no writer remains before freezing.

The team produces the game and its own requested report. The external researcher
produces independent validation, publication and analysis. The external report
does not substitute for an unfinished native reporting task. Native delivery and
completion of this research assignment are separate: the latter also requires
the validated repository-scoped skill specified below.

## Baseline and mechanism

The unchanged first game passed 14 independent Playwright criteria and was
published, but session `acb9c4e3-6383-48f9-94c4-b99605de565b` ended blocked after
27m00.568s. That experiment remains **failed**. The exact source was `c18fe31`.
Evidence is retained in the [baseline report](/tmp/ymp-fifteen-research-20260914T064003Z-adqmbpka/report.md),
[trace](/tmp/ymp-fifteen-research-20260914T064003Z-adqmbpka/trace.json), and
[invocations](/tmp/ymp-fifteen-research-20260914T064003Z-adqmbpka/invocations.json).

| Phase | Calls | Sum of invocation seconds | Input | Output |
| --- | ---: | ---: | ---: | ---: |
| Planning and plan review | 4 | 259.790 | 274,546 | 13,929 |
| Page creation and review | 2 | 338.100 | 753,406 | 18,885 |
| Mechanics and its repair | 4 | 233.319 | 1,425,069 | 11,010 |
| README and review | 2 | 72.096 | 759,860 | 3,190 |
| Shuffle verification | 2 | 114.319 | 1,102,898 | 6,156 |
| Browser verification and repairs | 6 | 590.233 | 4,778,894 | 29,981 |

These are outer invocation measurements, not native request counts or bills.
The browser branch used about 52.5% of input and 36.7% of invocation time. That
does not mean all browser verification was waste: one review found a real test
defect. Mechanics also included necessary evidence repair despite no game change.

The [recomputed baseline](/tmp/ymp-fifteen-repeat-analysis/baseline-analysis.json)
uses unrounded timestamps; tiny duration differences from this table reflect
rounding. Its first-file correction matters: a hash-only observation saw a page
at 06:47:26.479Z, and the final game hash first appeared at 06:48:16.649Z. The
snapshot named `first-file.json` was retained later, at 06:48:54.281Z. Earlier
page hashes differ, so the first implementation did include corrections. Only
the first *retained* page snapshot equals the final game. Usability was not
independently observed until the external validation; do not backdate it.

All 20 knowledge retrievals were empty, with zero supplied characters. There
were eight proposed/unconfirmed knowledge records and no reputation observations.
This supports the integrity assessment, but supplies no evidence of reuse benefit.

Two source-level mechanisms need correction before another native run:

- `commit_board_proposals` is reached after execution/review changes task state.
  `board_task_version` hashes the entire task, including lifecycle fields, so the
  current executor's repair can become stale before being considered.
- `BoardChange::Revise` adds checks. Even if proposal `16b7e2dd` had committed,
  the broken literal Chromium command would remain required. Timing alone is
  therefore insufficient to fix the observed cause.

The distinction is visible in
[board.rs](../../ymp-rust/crates/ymp-runtime/src/engine/board.rs),
[task versioning](../../ymp-rust/crates/ymp-core/src/board.rs), and
[event 972](/tmp/ymp-fifteen-research-20260914T064003Z-adqmbpka/events/event-00972.json).
Sonnet recommended changing the check in review; Luna submitted the structured
proposal. Neither participant had authority to commit that change itself.

## Repair boundary before launch

Implement an explicit, version-bound replacement of an ordinary generated task
check, with an independent decision and preserved old/new checks and rationale.
Keep legacy additive revision behavior and owner trusted contracts unchanged.
Handle the repair at a safe completed-execution/review boundary before lifecycle
bookkeeping or attempt exhaustion makes the usable proposal unreachable.

The reviewer must evaluate whether the replacement preserves the original
behavioral requirement. The runtime must execute the new check and invalidate
superseded evidence. It must not give the proposing executor self-approval,
accept a trivial command as proof, replay accepted sibling work or erase spend.
This does not prove semantic equivalence of arbitrary shell scripts; retain
that limitation and the actual independent review basis.

One controlled reproduction must first fail on the old implementation and finish
on the correction: a broken executable, a proposed valid replacement, actual
re-execution, final acceptance and an already accepted sibling left intact.
One negative case must reject an unauthorized, genuinely stale or weakened
trusted criterion. Existing related checks cover the remaining unchanged
constraints. Run the required fmt/Clippy/workspace sequence once on final source;
repeat only if integration changes executable bytes or reveals a new issue.

Choose the negative case at the newly changed boundary: independent replacement
review must reject an ordinary generated check weakened to a trivial success.
Tests of immutable trusted contracts alone do not establish that property.

An independent review must accept this narrow correction. No unrelated pending
statistics, reviewer ranking or UI changes are included just to improve the trial.

## Fixed inputs and permitted differences

Use the exact [original prompt](/tmp/ymp-fifteen-research-20260914T064003Z-adqmbpka/task.txt),
with no supplied decomposition or reference solution. Preserve:

| Item | Binding |
| --- | --- |
| Luna ID and model | `native-8da25f68f3db71627485cedf`, `gpt-5.6-luna` |
| Sonnet ID and resolved model | `native-0e430d170a75b84ec2a93d8f`, `claude-sonnet-5` |
| Roster | Fixed to those two existing IDs, not new copied participants |
| Effort | No observer override; preserve native settings and record actual reported values |
| Application limits | 200 invocations, 900s each, parallel 3, attempts 3, other defaults preserved |
| Additional ceilings | No observer token ceiling or external overall deadline |
| Application history | Fresh metadata without imported memory or reputation, as in baseline |
| Authentication | Native locations; never copy credentials |
| Output | A new empty directory under Downloads for each run |

Capture baseline and repeat configurations with hashes. Preserve the original
global home, existing sessions, installed release and published baseline. Use a
separate executable from the accepted fixed commit, and do not change it during
the run. Source repair is the intended intervention; record every other observed
difference, including catalog, browser, provider or native-setting drift.

Fresh output/application data do not establish an empty native memory or identical
cache state. Report these confounders; do not turn this diagnostic repeat into a
claim about the advantage of one model or team size. The weak-agent comparison
program remains paused.

## Unified criteria

All rows use the common record fields. Required rows must pass for study success;
exploratory rows may be unknown or not exercised, with an explanation.

| ID | Requirement and hypothesis | Evidence and competing explanation | Role |
| --- | --- | --- | --- |
| A1 | Correct offline 4x4 puzzle and required files | Playwright UI moves, valid permutation, solvable unsolved shuffle, counter, reset and victory; a screenshot or code review alone is insufficient | Required |
| A2 | No horizontal clipping at 390x844; usable desktop layout | Explicit viewport and root/body/board/control rectangles plus screenshots; distinguish capture-size errors from page layout | Required |
| N1 | Native completion and final independent acceptance | Trace, current task states, final decisions, process exit and unchanged artifact hashes; independent observer success cannot replace native completion | Required |
| N2 | No repaired acceptance requirement is dropped | Old/new check links, independent revision decision and actual subsequent output; inspect whether a passing replacement became vacuous | Required if exercised |
| P1 | Same authorized agents and settings | Captured IDs, effective assignments, native reported identity and effort; defaults and aliases are not additional participants | Required |
| P2 | Valid authority and durable decisions | Source grant, task/definition version, active access, decisions and provenance; reject actual conflicts while distinguishing lifecycle transitions | Required |
| P3 | Failure changes the approach when diagnosis justifies it | Rejection -> diagnosis -> proposal -> decision -> actual check; a repeated unchanged action is not recovery | Mechanism required in reproduction; observe in native repeat |
| P4 | Independent review adds or evaluates evidence | Identify what each rejection discovered and whether it changed code/checks/plan; useful review differs from merely repeating executor claims | Exploratory |
| C1 | Communication changes subsequent work | Trace each substantive critique/proposal to receipt, decision and effect; zero addressed messages does not mean no shared communication | Exploratory |
| T1 | Work overlaps only when useful and admissible | Task readiness, reserved reviewer, actual access, invocation overlap and wait reasons; a two-person team may have only one producer | Exploratory |
| R1 | Resource reporting is attributable and complete where supported | Final unique invocation snapshots; cache and reasoning remain subsets; observer work separately; incomplete data is not zero | Required for reported metrics |
| H1 | No human repair is needed for native completion | Intervention ledger including hints, file edits, restarts and task changes; observer validation/publication is separate delivery work | Required |
| K1 | No unsupported learning or reputation | Knowledge origin/status, supplied references and reputation observations; fresh history means reuse benefit is not exercised | Required integrity; benefit exploratory |
| U1 | Progress and failure are understandable | Saved console plus optional read-only TUI views; identify what a user learns without raw logs; headless evidence alone cannot establish live TUI correctness | Exploratory |
| D1 | Published exact result and useful final explanation | Unique Workers URL, HTTP bytes/hashes, excluded private paths, final causal/product report | Required |

The table is the single criterion index. Each row inherits the common record
fields: its requirement/hypothesis, expected behavior, measurement/evidence and
rival explanation are defined above; initial verdict is `unknown`, and analysis,
proposal, tradeoff and next check remain explicitly pending observation. Intent
links are G1 for A1/A2/N1/N2, G2 for R1/P4, G3 for T1, G4 for H1/U1/C1, and G5
for K1; P1/P2/P3 assess the corresponding identity, runtime authority, revision
and recovery principles. Material findings fill the pending fields; an observed
pass without an improvement need can state `no change justified`. Do not invent
findings merely to fill every field. Applicability is separate from verdict.

For N2/P3, an absent native revision trigger means `not exercised` in that run;
the controlled cause-repair reproduction still has to exercise the mechanism.
Capture all applicable implementations, including retrieval, knowledge projection
and correction. Baseline IDs are `ymp.bounded-allocation@2`,
`ymp.bounded-resources@1`, `ymp.ordered-board@1`, `ymp.direct-mvp@1`,
`ymp.fts5@1`, `ymp.evidence-projection@1`, `ymp.bound-correction@1`, and execution
backend `ymp.native@0.4.6`. Read the repeat's versions from its records. For
embedded behavior without a recorded policy ID, bind to source/executable and
state the limitation instead of inventing an identity.

## Predeclared external game validation

Freeze these semantics before launch; choose selectors from the delivered DOM
afterward. The old validator's optional Shuffle button, exact word `Victory` and
post-win input locking are not new product requirements.

1. Open the final `index.html` offline and verify the README against the game.
   Read the visible board, with one blank and the numbers 1 through 15, zero
   moves and a solvable unsolved initial state.
2. Exercise legal/illegal mouse moves and keyboard moves. Verify exact adjacent
   swaps and counter deltas. Determine an illegal direction from the actual blank
   position after legal moves to a real boundary; never guess that an arbitrary
   arrow is illegal.
3. Perform 20 New Game actions, retaining each board and checking permutation,
   independent parity, non-solved state, counter reset and cleared victory. Check
   a separate shuffle control only if it exists. This finite sample cannot prove
   "always solvable"; inspect the algorithm's invariant independently as well.
4. Solve the last recorded board with an external solver using deterministic
   tie-breaking. Do not resample for an easier board, seed the game, inject state
   or call helper APIs. Independently validate each planned step, perform legal
   UI moves and require ordered tiles and clear victory feedback. Failure of the
   external solver is a validation limitation, not proof of a game defect.
5. Use viewports 390x844 and 1280x800. Retain root/body widths, board and essential
   control rectangles and screenshots. Require no horizontal overflow and usable
   unobscured controls; normal vertical scrolling is allowed. Rectangle rounding
   tolerance is 0.5 CSS pixels and cannot excuse root overflow.
6. Record network requests and console/page errors. External responses must not
   be required. Retain validation source, observations and failures. Corrections
   may fix selectors or a demonstrated observer error, never weaken a failed
   requirement; preserve the original result and corrected interpretation.

These are local user-validation actions, not model-call quotas or new product
test matrices. Record the exact browser/version in the manifest. Unexpected
browser-version changes are visible confounders.

## Measurements and interpretation

Use the final consistent trace as the main source; retain interim sequence bounds.
Record time to accepted plan, first retained artifact, last game change, native
completion and independent verification. A first file is not automatically a
verified usable game. Sum and union invocation intervals separately; count waits
as events unless start/end pairing supports durations.

Group time and input/output by actual agent ID, phase and corrective reason.
Separate game work, necessary verification, test repair, environment repair and
unchanged execution. Classification needs messages, diffs/hashes and check output;
`execute` by itself is not a productive-work measure. Estimate no counterfactual
time savings by simply deleting all unchanged-file calls.

Record native continuation IDs, supplied prompt/profile characters, context
references and reported usage per invocation. References may overlap the complete
prompt, so do not add both as supplied context. Unknown native-loaded history and
internal request counts stay unknown. Baseline transmitted prompt text totalled
207,252 characters; that is a different unit from 9,094,673 reported input tokens.
Sum unique final invocation snapshots, not cumulative stream updates.

Separate native repair interventions from observer setup, diagnosis, validation,
publication and analysis. Hints, manual edits, restarts and changed controls count
as interventions; passive observation does not. Observer labor is still real
delivery overhead and must be reported separately. Unknown active-human time
remains unknown. Zero repair interventions with blocked status is not autonomous
delivery. An unreached time to accepted result is not the failed run's duration.

For every useful review, record the defect found, evidence demanded and next
action. For every rejected board proposal, record age, target version, source
assignment and whether lifecycle or definition actually changed. Determine which
policy was compliant but operationally ineffective. Record policy ID/version
from the session for allocation, resources, board handling, access and recovery.

Quality comparisons against a strong solo model, causal speedup, learning benefit
and provider billing are outside this single repeat. They remain explicit gaps,
not inferred successes. A later matched study may follow a demonstrated mechanism;
this plan does not authorize it.

## Execution, stopping and failed attempts

Launch only after repair acceptance and freeze the manifest before the first
native call. Run with the normal product command. Do not install Chrome aliases,
copy the previous game, inject test answers, repair game files or change task
states from outside the session. If a broken command does not recur naturally,
the native run tests complete delivery; the deterministic reproduction supplies
the repair-mechanism evidence.

Use milestone updates, including new evidence and implications, rather than a
stream of routine tool results. Retain the native phase through its real terminal
state. Do not interrupt solely for elapsed time or token volume. Distinguish
silence from an active call, dependency wait or actual non-progressing cycle.

The current execution phase is one fresh native repeat on the accepted repair.
Internal retries under its captured attempt policy belong to that session and
are all counted. If it fails, retain the failure, all resources and artifacts,
and return a cause-specific diagnosis to the parent. Do not autonomously launch
another experiment, resume the failed session or change the running treatment.
A later native launch needs an explicit continuation for its exact accepted
source and is a new ledger entry, never a replacement for a failed outcome.
Do not call the experiment successful while blocked or pending; preservation of
all attempts is not evidence of a high first-attempt success rate.

After any native terminal outcome, assess the artifacts if present using the
predeclared Playwright checks above. A failed native run still needs an artifact
verdict or explicit unknown, avoiding evaluation only of completed sessions.
Diagnostic artifact success cannot rescue failed native acceptance. Publish the
exact final artifact with Wrangler using existing account authorization and a
distinct run name; any publication after native failure is labelled diagnostic.

## Product assessment and final deliverables

The report must answer whether this small task is practical for an ordinary user,
which coordination earned its cost, which policy decisions obstructed progress,
and what the product should change next. It must compare the failed baseline and
repeat without claiming a randomized causal estimate from two model runs.

Each material recommendation must contain a fact, mechanism, competing
explanation, expected user benefit, responsible subsystem, downside and smallest
discriminating check. Initial priorities to evaluate are check-repair liveness,
durable reviewer-visible test artifacts, unmet-evidence-aware task scheduling,
and a final status that distinguishes artifact availability from verification
failure. These are hypotheses and proposals, not new authorized implementations.

Deliver the completed unified criterion table, all-attempt run ledger, phase and
agent breakdown, explicit outcome/confirmation distinctions, ranked product
recommendations and links to the published game and evidence. Integrate the
validated common procedure into a repository-scoped trial skill after use;
retain the prior report and hashes without edits.

Completion of the project research task also requires skill validation. Use the
retained failed baseline for a dry check: the skill must reject "published but
blocked = success", retain accepted/unconfirmed, and respect the actual launch
authorization. No additional native run is needed to test those decisions.
