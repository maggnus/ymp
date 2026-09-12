# Controlled evaluation protocol

This protocol tests whether a team produces more independently verified outcomes than a strong solo agent under declared resource constraints. It also separates the effects of memory, assignment, effort, ceremony, and prior-outcome retrieval. It is a proposal for approval, not a record of model performance.

## Preconditions and units

The initial comparison uses one provider and one resolved model for both arms. A suitable candidate is the already available Claude Opus 5, with two separately named team agents and the same model in the solo arm. Availability, effective effort, native settings, SDK version, and the resolved model identifier must be recorded at authorization. A model substitution creates a new treatment. Codex can be a later replication; installed GLM last-request-only usage is insufficient for a claim about equal whole-run token expenditure.

An ymp invocation can contain several native model requests and tool calls. Equal invocation counts are therefore not equal compute. Capture raw input, cache read/write, output, reasoning-as-a-subset, coverage, model-request counts when exposed, wall time, native invocations, failed attempts, and any versioned price estimate. Missing fields remain missing. Native automatic memory, skills, project instructions, and background-agent behavior must be fixed or consistently disabled; otherwise a comparison of ymp memory also changes an uncontrolled memory channel.

For the first homogeneous-provider study, the proposed primary resource constraint is total reported raw input plus output, including cache input. This measures protocol token volume, not money. Report cache-adjusted shadow-price estimates separately using one frozen model-specific rate table and output rates. If the owner chooses estimated currency as the primary budget instead, pre-register that change before seeing held-out results. A provider-local equivalent-input token is not automatically comparable to another provider's equivalent token.

The existing turn and timeout limits are useful guards but do not enforce a hard token or cash ceiling. New-call admission, reservations for concurrent work, cancellation, and the treatment of unknown usage must be defined and verified before a budget-controlled model experiment. A last-request notification can arrive after a request has already spent its tokens; record and report any overshoot. Do not silently classify such a run as within budget.

## Approval envelope

No experimental provider quota was authorized or consumed by this research program. The delegated UI review is development work using the explicitly requested Claude model; it is not part of the experimental quota below.

| Stage | Proposed maximum | Purpose | Authorization |
| --- | --- | --- | --- |
| Calibration | 4 outcome attempts: 2 tasks × solo/team; 500,000 observed raw tokens per attempt; 2,000,000 total; one active attempt; 20 minutes per attempt | Verify instrumentation, native limits, and whether the selected ceiling permits a meaningful trial | Explicit owner approval required |
| Initial pilot | 20 held-out tasks × 2 arms × 1 run = 40 attempts; at most 500,000 observed raw tokens each and 20,000,000 total; one active attempt; 20 minutes each | Estimate failure classes, paired outcome differences, and resource distributions | Separate approval after calibration; no automatic escalation |
| Follow-up studies | No allocation yet | Memory/assignment, effort/ceremony, and outcome routing | Each receives its own pre-registered budget |

These are maximum proposed allocations, not expenditure estimates. The proposed secondary guards are 80 ymp invocations per outcome attempt, 180 seconds per invocation, and zero application-level retries; native retries remain inside the invocation's timeout and resource accounting. Invocation count is not used as a claim of equal compute. Before approval, verify a documented bound or stop rule for an in-flight token overshoot. If a provider cannot support the intended ceiling, report the limitation and do not call the experiment strictly budget-equal. Unused calibration quota does not authorize the pilot. Failed, invalid, and cancelled attempts still consume their measured resource allocation; replacements require a new allocation.

## Task catalog

Use fresh, explicitly selected fixture directories for each treatment. Work happens directly in those directories; do not introduce private candidates or worktree orchestration into the product. Keep the evaluator's expected answers outside the agents' accessible fixture contents. Both arms receive identical inputs, tools, instructions, and declared acceptance requirements. The same independent evaluator inspects their resulting artifacts.

The following is a task catalog to materialize and freeze before live execution, not a claim that twenty executable benchmark fixtures already exist. The current repository has one greeting scenario, which must pass its own independent acceptance commands before it is used.

| IDs | Class | Four case definitions | Independent evidence |
| --- | --- | --- | --- |
| SW01–04 | Software | Greeting edge cases; Unicode normalization bug; a small multi-file behavioral change; regression repair with an unrelated invariant | Maintainer-written tests and checks outside the generated plan |
| DA01–04 | Data artifacts | CSV deduplication; decimal totals; inconsistent timestamp cleanup; cross-table reconciliation | Exact row/value assertions, retained source records, and independent arithmetic |
| DO01–04 | Documents | Requirements coverage; inconsistent terminology; source-backed fact table; conflicting specification reconciliation | Frozen source facts, missing/incorrect-fact counts, and a blinded rubric where judgement remains subjective |
| PL01–04 | Planning | Resource-constrained schedule; dependency ordering; mutually inconsistent requirements; revised constraints after an initial plan | Executable constraint checks plus human review of requirements not reducible to a validator |
| CT01–04 | Controls | One-step file lookup; exact-format conversion; stale prior artifact; superficially similar request with a new requirement | Exact answers/artifact checks; these are negative controls for unnecessary coordination and unsafe reuse |

This preserves the product's universal scope. Software is one class, not the definition of a valid task. Independent verification is the candidate advantage: inspectable deliverables with multiple constraints, source disagreements, or expensive errors. It remains a candidate class until the results show an advantage over solo work. Trivial operations are deliberate controls; a team must not be credited for spending more on them.

## Solo and team comparison

The solo baseline is a native agent with the same complete task, tool access, effective effort, and resource ceiling. It may plan, use tools, inspect its work, and self-correct inside its native loop. Do not artificially make solo a one-message completion while giving the team an agentic loop. In the team arm, independent task review remains mandatory and the executor cannot approve its own candidate. Record model family separately from agent identity.

Randomize treatment order within each task and retain the seed. Use fresh native handles and separate application homes. Keep cache warming and environment preparation symmetric and document residual differences. Run all scheduled cases, including failures; never select only the prompts on which a team already looks useful. Keep plan-supplied checks as product behavior, but score outcomes with the independent checks and rubrics specified before the run.

Primary endpoints are independently verified success and defects that escaped acceptance. Report both intention-to-treat results, where operational failures matter, and a diagnostic breakdown by content, infrastructure, cancellation, and budget exhaustion. Secondary endpoints are median and tail resource use, wall time, attempt count, and coordination phases. Distinguish file correctness from the application's session status: a failed synthesis can leave a verified artifact, while a superficially successful run can have repeated a side effect.

Twenty paired cases constitute a pilot, not proof of a small quality margin. Even 20 successes in 20 independent trials have a Wilson 95% lower bound of about 83.9%. Repeated runs of the same case are not new independent tasks. Use task-level paired intervals and disclose heterogeneity; pre-register any confirmatory sample size from the pilot variance and the smallest useful effect. Do not claim noninferiority from a non-significant difference or from a degenerate bootstrap of all-success pairs.

## Memory and adaptive assignment

Only after the first pilot identifies a plausible task class, run the four documented team treatments: neither mechanism, memory only, assignment only, and both. Adapt on disjoint task instances, freeze the resulting state, and evaluate on held-out instances. Start each treatment from the same eligible starting conditions; do not contaminate one treatment with another's memories or reputation.

Include three memory controls: relevant verified memory, irrelevant/stale memory, and no memory. Log which entries were retrieved, their versions and source sessions, the effective retrieval query, and the injected byte/token budget. The current first-twelve-words query can select generic instructions rather than task terms, so retrieval relevance must be validated first. A memory table with active rows is not evidence of a useful memory mechanism.

Charge adaptation and learning to the resource total. For K delivered outcomes, report amortized resource use as (adaptation cost + all evaluation and learning cost) / K. Compare success and escaped defects as well as average cost. Document when memory first repays its collection cost, or that it never did within the study. A memory-quality decision must not be based on agent consensus alone.

## Effort, ceremony, and ensemble studies

Run separate interventions, rather than changing several mechanisms and attributing the result to one:

1. Hold architecture and prompts fixed; compare fixed effort with the declared role policy. Disable adaptive assignment for this comparison. Store effective effort in the observation identity; an unsupported GLM tier is not silently substituted.
2. Hold model and effort fixed; compare the current full preparation with one proposal and no competitive bidding for pre-registered simple cases. Preserve independent plan/task review and the final acceptance checks. Difficulty declared by a planner is not an independent ground-truth label.
3. Compare one high-effort agent with low-effort generation plus a separately budgeted verifier, and with solo self-consistency under the same total resource ceiling. Charge the verifier and synthesis to the ensemble. Distinguish diversity of agents from diversity of models.

The existing planning filter hides plan messages from one context suffix but does not restrict team_read or erase native history. Therefore it does not establish a blind ensemble. A blind planning treatment needs a frozen pre-proposal information set or a verified restriction on access to peer proposals. Record information exposure before interpreting correlated outputs.

## Outcome routing study

Test a project-scoped prior-outcome router only when eligible history exists. Candidate retrieval, routing, local validation, and any re-verification are all charged. Run the twelve offline guard cases first; include changed intent, a translated paraphrase, another project, explicit redo, edited/deleted artifacts, missing verification state, failed checks, and uncovered dependencies.

Grounded file facts cannot rescue a falsely classified intent. Primary failure is an incorrect reuse answer when a new or modified task was requested. Such a failure rejects deployment regardless of average savings. A current file fingerprint does not cover changed external services, ignored dependencies, symlink targets outside the recorded scope, or time-sensitive facts. Restrict eligibility to artifacts with an explicit verification scope, and treat ambiguity as new work or a qualified historical answer, never as newly satisfied intent.

The three cited real requests include two projects. S04 had no prior completed outcome in its project, while S05 had the earlier P01 outcome. S03 and S05 have matching initial manifests and empty end change lists, which supports a narrower tracked-file reuse opportunity; it does not prove a semantic router would have been correct or cost only 3–10k input tokens. New or changed outcomes need an explicit post-verification anchor. Do not automatically replay stored shell checks while answering a read-only informational question.

A gate has a positive expected resource return only when p_valid × (C_full − C_revalidate) exceeds C_gate, before adding the expected loss of incorrect reuse. Measure the eligible-repeat rate and this inequality. If no eligible record exists, the gate should cost no native invocation. If the result is new work, preserve the original request and route to normal execution with the prior record as context.

## Decision rules

Approve no universal team advantage from this study plan. Expand the product's complexity only after an independently scored task class shows a useful quality/resource tradeoff. Reject a fast path or reuse router that misses changed requirements, reject a learning treatment that increases escaped defects, and park cost-aware allocation until outcomes and counterfactual evaluation support it. Native providers and their own authentication remain the execution boundary.

The project register records proposed experiments separately from implemented features. Its acceptance criteria, dependency links, and authorization fields determine what may start next.
