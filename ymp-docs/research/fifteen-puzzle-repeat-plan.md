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

If another product failure prevents completion, retain the attempt as failed,
diagnose the concrete cause and apply the smallest justified correction. Every
code/configuration change starts a separately identified subsequent attempt in a
fresh directory. Do not repeat an unchanged failure hoping for a favorable answer.
Do not call the experiment complete while it is blocked or merely pending review.
Broader model comparisons and unrelated new features remain outside this repair.

After native completion, use a fixed-seed external solver and legal UI clicks to
reach victory; never call game helper APIs or inject state. Adapt the observer
only to the delivered selectors, without weakening criteria. Preserve validator
source, trace, output, hashes and screenshots. Publish the exact final artifact
with Wrangler using existing account authorization and a distinct run name.

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
