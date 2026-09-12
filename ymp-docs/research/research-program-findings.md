# Research findings and delivery priorities

Current planning status: the owner recorded product goals and requested a [bounded dynamic team and effort policy](../architecture/team-and-effort-policy.md). Implementation remains paused during the broader architecture review in YMP-010; the delivery recommendations below are provisional. Current terminology is defined [separately](../product/entities.md). Historical measurements remain unchanged.

The final [intent.md](../../intent.md) defines the product: autonomous user-task solving by a self-organizing agent pool, with team-selected executors/models/effort, independent verification, a shared budget and incremental reusable knowledge and reputation. This report identifies implementation gaps and evaluates proposed mechanisms in support of that product. It does not make a benchmark victory or a statistical superiority claim a prerequisite for implementing the required capabilities.

The measurements do not establish that a team, adaptive assignment or accumulated memory outperforms a strong solo agent. They do establish specific implementation facts: not every provider failure blocks a run, native execution is separated from read-only work, the three repeat examples span two projects, and absent native usage fields are not recorded zeros.

Within the research scope, the most defensible improvements are specific: honor provider-declared retries, preserve an already verified result if final narration fails, make evaluations and budgets explicit, avoid content hashing when only names are needed, and use task terms for memory retrieval. Automatic outcome substitution, an effective-token-only interface, a universal effort ladder, cost-penalized assignment, and broad module extraction are not justified as defaults.

## Evidence and scope

The code baseline is **56e21dd**, ymp 0.3.0. Source references and line numbers in this note refer to that revision. Evidence consists of a consistent read-only snapshot of the existing journal, 32 offline protocol runs against the unchanged release binary, a sanity check of the existing scenario against Mock, release-profile calls into the actual workspace/storage crates, 81 offline effort-policy cases, twelve outcome-reuse guard cases with a wrong-intent negative control, four harmless check-gate examples, and a source/history review of the interface. No experimental model requests were made. The explicitly delegated UI review consumed development-agent quota and is distinct from the unapproved comparative experiments.

The final journal snapshot at **2026-09-12 04:56:11 UTC** contains five sessions, 79 started invocations, 78 completed invocations, and one cancelled invocation. There are 32 assignment decisions, eight positive observations and no negative observations, and five active memory entries. These are uncontrolled product traces, not a benchmark. A completed session is not an independent quality label, and a missing or interrupted outcome is not automatically a competence failure. [Journal evidence](evidence/journal-audit.json)

At the audited code baseline, the repository contained four commits from one day and one committed evaluation scenario, greeting. There are verification reports, but no task-level comparative results, no recorded treatment flags in the session record, and no review-latency or merge-conflict time series. Statements that the learning flags were never used cannot be independently established from this journal because their values are not recorded. [History evidence](evidence/history-audit.json), [existing methodology](evaluation.md), [CLI run output](../../ymp-rust/crates/ymp-cli/src/main.rs) (headless, lines 330–380)

The detailed live-study proposal is [Controlled evaluation protocol](experiment-protocol.md). Accepted recommendations and pending decisions are in the [project task register](../tasks/README.md). A recommendation is not an implemented feature or permission to consume experimental provider quota.

## Verdicts

| Hypothesis | Verdict | Decision |
| --- | --- | --- |
| H1: infrastructure failures | Required resilience; targeted fixes | Preserve verified work and support recovery/reassignment; fix confirmed errors without blind replay of ambiguous writes. |
| H2: ceremony and fast path | Required flexible coordination; test specific shortcuts | The team must choose its method. Current preparation cost is measured; the proposed fixed shortcut has no quality-equivalence result. |
| H3: effort and low-effort ensemble | Required autonomous controls; test particular policies | Agent/model/effort choice is core. A universal ladder, fixed funnel and superiority claim remain unproven. |
| H4: cost-aware assignment | Required resource-aware choices; park the proposed lambda rule | The team must use resources and a shared budget; this dataset does not validate the particular cost-penalty formula. |
| H5: context hygiene | Required usable knowledge/context; targeted fixes | Incremental shared knowledge and correction are core. The measured lookup fixes help; blanket purpose isolation remains unproven. |
| H6: check-command gate | Reject prefix containment; park mandatory confirmation | Preserve autonomy and provide factual recovery/check evidence. |
| H7: large-file decomposition | Reject now | The tiny history does not demonstrate a cost caused by concentration. |
| H8: outcome reuse | Optional dedicated router | The team may reuse verified work as part of its method. A separate pre-run substitution gate needs intent/state guards; hashes alone are insufficient. |
| H9: effective tokens | Required accounting/budget; optional estimates | Preserve observed counts and coverage. A shared budget is core; effective-only display and universal price equivalence are unsupported. |

## Product coverage at the audited baseline

This table separates requirements from observations. The linked task IDs are planned work, not claims of implemented behavior.

| Intent capability | What already exists | What remains | Tasks |
| --- | --- | --- | --- |
| Individual agents from native providers | AgentProfile IDs, native adapters/authentication and per-agent usage | Complete pool/capability presentation and stable historical identity across all views | YMP-109 |
| Autonomous team, model and effort choices | Assignment sampling and a fixed coordination workflow | Team formation from the eligible pool; assignment-level model/effort choices and live adaptation | YMP-110, YMP-111 |
| Temporary roles, commitments and reassignment | Task states, assignments and proposal recording | Apply team proposals, change plans, add participants and reassign unfinished work within constraints | YMP-112 |
| Useful concurrent execution | Parallel planning/bidding and an invocation semaphore | Ready-task selection uses take(1); execute independent assignments concurrently while preserving dependencies, resource constraints and verification eligibility | YMP-115 |
| Shared board and explainable history | Team/addressed messages and persistent events | Complete recipient/decision presentation and link decisions to execution and verification | YMP-101, YMP-112 |
| Independent verification and resilient progress | Candidate review excludes the executor; checks and observations exist | Final-review selection still allows executors; also correct nonterminal errors and preserve/reassign work safely | YMP-103, YMP-104, YMP-112 |
| Shared budget | Turn/time/attempt limits and usage recording | Explicit common resource policy, admission and honest unknown/overshoot handling | YMP-102 |
| Incremental cross-session experience | Incremental events, reputation and some memory; global learning largely after success | Publish usable general knowledge during work and retrieve it in subsequent sessions | YMP-106, YMP-113 |
| Correctable knowledge with provenance | Memory source fields and an unused supersedes field | Verified correction/replacement with retained history, applicability and evidence | YMP-114 |

A specific verification gap deserves its own acceptance check: candidate review filters out the assignee (engine.rs, lines 989–1008), but final review selects from the entire session team (lines 679–686). The final checker can therefore be an executor of the result. Independent final acceptance must be enforced by eligibility rules, not merely by naming the phase final_review. YMP-112 records this requirement and the possibility of adding an independent participant within the budget.

The product implementation is guided by [English requirements](../product/requirements.md) and [ADR 0002](../adr/0002-agent-identity-and-reasoning.md). Comparative studies evaluate particular policies after working capabilities exist; they do not decide whether these intent requirements should exist.

The current execution loop admits one ready task at a time through take(1) (engine.rs, lines 600–618); its comment explicitly serializes execution and verification in the shared working directory. Increasing the configured parallel limit or pool size therefore does not enable concurrent task execution. YMP-115 records this implementation gap. A twenty-agent utilization or speedup result has not been measured, and more planning/bidding activity must not be presented as proof of useful parallel execution.

## H1 — Infrastructure failure and competence

The blanket failure premise is false. Planning collects valid proposals after a participant fails, and bidding can select remaining willing participants. Learning is already optional after verification. By contrast, failure of the selected plan reviewer, executor, task reviewer, final reviewer, or synthesizer can abort the main path. A failed execute invocation has already consumed an assignment attempt, but does not by itself create a negative reputation observation. [Engine](../../ymp-rust/crates/ymp-runtime/src/engine.rs) (plan 800–918, bid 921–955, perform 958–975, observe/verify 978–1063, optional learning 705–715)

The offline fixture distinguishes the important cases:

| Fault or policy | Application outcome | Independent artifact/effect evidence |
| --- | --- | --- |
| One planner or bidder exits | Completed | Remaining agents complete the artifact. |
| Selected plan reviewer receives a transient refusal | Blocked | No execution occurred. |
| Same refusal, bounded read-stage retry prototype | Completed | One task attempt; no failure observation caused by retry. |
| Persistent refusal with two prototype retries | Blocked | Bounded retry exhaustion; no fabricated success. |
| Executor dies before or after writing | Blocked in both cases | One case wrote nothing; the other wrote the correct artifact. The generic exit signal does not distinguish them. |
| Blind replay after writing | Completed | The side effect occurs twice; independent acceptance fails. |
| Malformed review or reviewer timeout | Blocked | An artifact exists, but its adjudication did not finish. |
| Bad artifact, approving model response | Blocked after two attempts | Failing shell checks overrule approval and create two negative execution observations. |
| Cancelled pending review | Paused | Cancellation is preserved. |
| Optional learning exits | Completed | The verified deliverable survives. |
| Final synthesis exits | Blocked | The artifact and final review already passed. |

The retry prototype is scripted inside the deterministic fixture; no application retry policy was implemented. It emulates bounded responses within one invocation and checks the resulting bookkeeping and failure boundaries. It does not prove a real network retry implementation correct or measure provider completion-rate improvement or retry cost. The unchanged product was also tested against a Codex-shaped `error` notification with `willRetry: true`, followed by a successful final notification. The adapter still blocks because it treats every `error` notification as terminal. The installed generated ErrorNotification schema explicitly includes `willRetry`; this should be handled before adding a second retry layer around a native agent that already retries. [Protocol results](evidence/protocol-experiments.json), [schema evidence](evidence/effort-capabilities.json), [provider adapter](../../ymp-rust/crates/ymp-providers/src/lib.rs) (Codex event loop)

Keep transport corruption, an invalid model decision, authentication failure, explicit refusal, and an ambiguous interrupted execution distinct. Preserve native retry notices within the same invocation, bounded by the existing timeout and cancellation. Never infer that a process exit means no side effect occurred. A later application retry policy should count its own attempts and usage without treating a successful retry as evidence of competence; it must not silently compound native and application retry budgets.

Two independent changes are recommended: **YMP-103**, honor nonterminal native retry notices; **YMP-104**, retain verified completion with a recorded reporting error if final synthesis fails. General automatic replay of write turns is rejected. The existing journal has one cancellation and no recorded infrastructure-failure cohort, so no empirical improvement percentage is claimed.

## H2 — Preparation cost and difficulty

For N agents and T tasks, the successful fixture without memory uses

**N + 3 + T × (N + 2) native ymp invocations.**

This counts N proposals, one plan review, N bids per task, execution and independent review per task, final review, and synthesis. It excludes revision/arbitration failures and native model requests inside an invocation. Enabling the fixture's useful learning adds two invocations. Changing the plan's declared difficulty does not change the number of phases. [Protocol results](evidence/protocol-experiments.json)

| Team size | One task | Three tasks | Eight tasks |
| ---: | ---: | ---: | ---: |
| 2 | 9 | 17 | 37 |
| 3 | 11 | 21 | 46 |
| 5 | 15 | 29 | 64 |

Before the first execution, one-task runs use 2N + 1 preparation calls: 5, 7, or 11. The three recorded HTML outcomes used 14, 7, and 9 calls before their first execution, including plan revisions. Across the whole journal, proposals, plan reviews, and bids account for 54 of 79 invocations. These are exact call counts, but not a statement that all of this work was useless. [Journal evidence](evidence/journal-audit.json)

The fixture's per-call tokens are synthetic and its latency is local process overhead. It cannot establish a real token saving or quality equivalence. Current journal token figures also mix partial historical accounting with newer complete snapshots. Do not divide a known subtotal by another known subtotal and call the result a guaranteed fraction of true expenditure.

A one-proposal/no-bid prototype could reduce one-task preparation while preserving independent plan and candidate review. Its six-call floor, without learning or revisions, is an arithmetic possibility rather than a measured product improvement. Compare it at fixed model, effort, budget, and external acceptance requirements. A planner's own `simple` label is not an independent safety or difficulty classifier. **YMP-203** records the conditional comparison of that specific shortcut. Flexible planning and team choice of a suitable method are core work in YMP-110/YMP-112 and are not deferred by this experiment.

## H3 — Effort controls and ensembles

The implementation does not pass an effort field in TurnRequest or include effective per-invocation effort in the observation identity. Installed interfaces do offer controls, with different scopes and vocabularies:

| Backend | Native control | Verified scope and limitation |
| --- | --- | --- |
| Codex 0.153.4 | `turn/start.params.effort` | Applies to that and subsequent turns. The generated schema accepts a nonempty model-advertised string; it is not a universal fixed enum. `modelReasoningEffort` is not this request field. |
| Claude Code 2.1.269 / SDK 0.3.246 | `Options.effort`, separately `Options.thinking` | One query, created anew for each ymp invocation. The SDK exposes named tiers, but selected-model support and defaults must be verified. |
| Installed GLM ACP 1.3.0 | `session/set_config_option`, `configId: thought_level` | Session setting. GLM-5.2 advertises none/high/max; older supported families expose none/on. Low, medium and xhigh are invalid option names in this installation. |

The installed GLM capability functions were exercised without instantiating a model client. A known but unsupported option can resolve to the model default, so an acknowledgement must be checked rather than assuming the requested tier took effect. Current upstream GLM documentation has a broader GLM-5.3 ladder; it does not describe the cached local 1.3.0 capability set. [Local capabilities](evidence/effort-capabilities.json), [GLM function results](evidence/glm-effort-capabilities.json), [^9], [^10], [^13]

Eighty-one offline policy cases validate requested role tiers against declared capabilities and demonstrate unsupported GLM combinations without silent fallback. They also require a different experience key when effective effort changes. They do not simulate how reasoning quality or real token consumption changes. Splitting observations across effort adds sparsity: the existing 32 selections include 26 all-cold-prior choices, so more dimensions are not free statistical evidence. [Policy probe](evidence/effort-policy-probe.json), [journal](evidence/journal-audit.json)

The claimed blind ensemble is not enforced. The last-message context hides messages of kind `plan` during planning, but team_read still exposes session history, and native read-only threads can retain earlier roles. Blind comparison requires controlled information exposure. An independent candidate reviewer is also different from an independent ground-truth evaluator.

External findings are mixed and narrower than the product claim. Wunderlich et al. report compute-efficient gains for debate/mixtures on MMLU-Pro and BBH; Tran and Kiela find solo models competitive or better on matched-thinking-budget multi-hop reasoning. Kim et al. find strong task/architecture dependence and report limits to generalizing scaling relationships. These results justify controlled tests, not adopting a universal low-effort team rule. [^4], [^5], [^3]

**YMP-111** records native reasoning controls with effective-config versioning. Autonomous team selection of executors, models and effort is required by [intent.md](../../intent.md) and is covered by planned work YMP-110/YMP-112. **YMP-203** tests particular effort and coordination policies; its results do not gate the existence of autonomous choice. The specific fixed low/medium/xhigh funnel remains unvalidated.

## H4 — Retrospective assignment analysis

The journal has 32 choices, 26 with only Beta(1,1) candidates. The selected participant has a nonzero posterior in only three choices. The eight observations are all positive; they cover implementation/simple and planning/standard. There is no controlled task-class distribution or verified solo baseline. No assignment event records a task ID, candidate cost estimate, or a logging propensity. [Journal and parameter replay](evidence/journal-audit.json)

A sampled Beta score is not the probability of selecting an action. Under the current independent-sampling rule, the probability of selecting agent i is the integral of its Beta density multiplied by the other candidates' Beta CDFs. This can be reconstructed approximately from the logged parameters, provided the candidate set and rule are complete. A seeded 10,000-draw replay per choice recovers approximately 1/2 or 1/3 for equal-prior two/three-agent choices. Thus absence of an explicit propensity does not make reconstruction impossible.

It still does not supply counterfactual costs or outcomes. Costs vary with role, task, context, native tool-loop length, and cache state; the unchosen participant's cost and verified outcome are not observed. Ranking recorded successful choices with their realized costs would be a hindsight exercise, not an estimate of a new policy's value. Off-policy evaluation requires explicit assumptions about reward or logging models; the current sample cannot support a useful causal cost-penalty estimate. [^2]

The proposed utility can be interpreted as a Lagrangian heuristic for maximizing verified outcomes subject to a resource budget. It is not automatically an optimizer of success per dollar, nor is a single fixed lambda meaningful without a declared cost scale. Raw tokens, provider-local equivalent inputs, and currency are different resources. Establish a budget convention, complete measurement, and linked external outcomes first. **YMP-101** and **YMP-102** provide the decision/resource facts the team needs. Resource-aware executor, model and effort selection remains required in YMP-110/YMP-112. Only adoption or a superiority claim for this particular lambda-penalized policy is parked; no lambda is recommended.

## H5 — Context construction and retrieval

### File listing

`follow_up` serializes all paths returned by Workspace::files into the request. That method calls fingerprint, which reads and hashes every regular file before returning names. The hashing is useful for change detection but unnecessary for enumeration. A release-profile probe used the actual crate and a names-only comparison with matching path sets. [Workspace implementation](../../ymp-rust/crates/ymp-workspace/src/lib.rs) (files and fingerprint), [follow-up](../../ymp-rust/crates/ymp-runtime/src/engine.rs) (lines 125–150), [measurement](evidence/context-probe.json)

| Controlled fixture | Full absolute listing | First 200 paths | Hashing enumeration | Names-only enumeration |
| --- | ---: | ---: | ---: | ---: |
| 1,001 small files | 54,053 bytes | 10,801 bytes | 12.49 ms | 0.93 ms |
| 10,001 small files | 550,054 bytes | 11,001 bytes | 146.05 ms | 11.76 ms |
| One 64 MiB file plus a small target | 99 bytes | 99 bytes | 144.96 ms | 0.031 ms |

These are single warm-cache observations on this machine, not stable percentile or model-latency estimates. Bytes are measured; native model input tokens for these requests were not measured. The large-file case isolates unnecessary content work even when the eventual prompt is tiny. **YMP-105** is justified without claiming an AI quality gain.

Blind truncation is not an acceptable fix. In the 1,001- and 10,001-file controls, the relevant `zzz-required.md` disappears from the first 200 paths. A targeted shortlist retains it at essentially the same byte cost, but this tests information availability rather than a language model's answer quality. Use explicit artifact paths, task context, and a bounded relevant shortlist; do not equate a shorter prompt with preserved intent. A later bounded context change should preserve these controls.

### Message and memory context

The twelve-message suffix also truncates each message at 1,500 characters. A mid-run requirement followed by thirteen coordination messages is absent from that suffix in the offline control. However, candidate and final reviews separately receive the original request, task description, executor result, and actual check output; the claim that every review only sees twelve messages would be false. The unresolved risk is later steering or a critical finding that was never promoted into durable task context. Native history and team_read can mitigate the omission but do not guarantee retrieval. [Engine](../../ymp-rust/crates/ymp-runtime/src/engine.rs) (ask 356–386, verify 989–1008, final review 686), [control](evidence/context-probe.json)

A more immediate defect is the memory query. Store::memory takes the first twelve words of its query and combines them with OR. Engine::ask supplies the role instruction, not a separate task-focused retrieval query. The bidding prefix can contain no task terms at all. In an actual FTS5 call through Store, this prefix returned a generic workflow entry and missed a relevant invoice procedure; passing the task terms reversed that result. **YMP-106** addresses the query and records retrieval evidence before considering a different database. [Storage](../../ymp-rust/crates/ymp-storage/src/lib.rs) (memory, lines 323–346), [probe](evidence/context-probe.json)

### Native context partitioning

The native key is session × agent × directory × read/write mode. Execute already uses a different key from plan, bid, and review. Several read-only purposes do share a native thread. Historical input growth can motivate a test of context construction or partitioning, but it is not causal proof that adding purpose to every key will improve outcomes or cost: separate threads can lose useful history and prefix reuse. Record resume keys, effective effort, context size, and phase, then compare a selective alternative. Unconditional per-purpose isolation is parked.

## H6 — Check commands and recovery

A literal prefix allowlist is not a containment mechanism. Harmless offline stubs show that a command starting with `cargo test` can execute an additional command or a command substitution; even an argv-only invocation runs whatever behavior the permitted executable and project scripts implement. This is a demonstrated limit of the proposed rule, not an attack on the real project. [Gate controls](evidence/check-gate-probe.json)

There is a second scope issue: by the time acceptance commands run, the execution stage already had unrestricted access through the native agent. Gating only plan-supplied shell checks does not constrain all agent actions. Planning itself is read-only; the timing matters. A mandatory first-use confirmation also needs an engine-to-UI request/reply protocol and changes unattended behavior. Its friction has not been measured, so it is parked rather than presumed cheap. [Checks](../../ymp-rust/crates/ymp-runtime/src/engine.rs) (1066–1097), [providers](../../ymp-rust/crates/ymp-providers/src/lib.rs), [UI review](evidence/ui-architecture-review.md)

Retain a smaller, factual contract: show what checks actually ran, which files were recorded as changed, and what recovery data exists. ymp keeps metadata rather than previous file contents, so it must not promise rollback. A missing cwd/.git does not establish that the directory is outside a repository; a parent directory may be the repository root. Any repository awareness needs validated discovery and an explicit unknown state. **YMP-107** records this disclosure work, with all interface changes delegated to Claude Opus 5 max. This does not add isolation, automatic reversion, or mandatory ceremony.

## H7 — Concentration versus demonstrated maintenance cost

At the baseline, engine.rs has 1,391 lines and tui/state.rs has 1,907. After the initial import, two of three commits touch the engine, and the two substantive cross-cutting changes both do so. That is an observation about a tiny selected history, not a long-run change rate. There are no merges, no remote PR history, and no measured review-latency distribution. File length alone cannot identify the cause of a regression. [History audit](evidence/history-audit.json)

The UI already separates drawing, pages, sidebar, usage presentation, themes, and state. Its two documented token-statistics regressions arose from mutation ordering and stale live facts; both now have regression tests. Moving functions into more files would not by itself repair those causes. The optional idea of making selection an explicit page-building result is a possible API cleanup, not a demonstrated current defect or a reason to prioritize a refactor. [UI architecture review](evidence/ui-architecture-review.md), [committed verification](../../ymp-evals/reports/token-usage-verification.md)

The engine's inline prompts do make targeted context work touch orchestration code, but this history cannot show that a wholesale prompt, selection, and learning extraction would repay its review cost. Reject the three-step decomposition as a project now. A small helper needed by an accepted behavior change can be introduced within that change. Revisit after a meaningful set of incremental changes provides evidence of recurring conflicts, excessive editing scope, or review delay. This is a rejection of the current justification, not a permanent prohibition on modularization.

## H8 — Verification-grounded prior outcomes

### Reconciliation of the three examples

The cited requests are similar, but they are not three repeats within one registered project:

| Case | Project | Request | Invocations | First-start to last-completion time | Current status |
| --- | --- | --- | ---: | ---: | --- |
| S03 | P01 | create a simple index.html file | 20 | 19.43 min | completed |
| S04 | P02 | Create a simple html file | 13 | 6.73 min | completed |
| S05 | P01 | Create simple html file | 15 | 15.48 min | completed |

S04 had no earlier completed outcome in P02. A project-scoped router must not reuse P01's result for it by default. S05 did have a previous completed outcome in P01. This leaves one directly relevant repeat opportunity, rather than two established avoided runs. The data supports investigation, not a measured reuse success rate. [Outcome audit](evidence/outcome-economics.json)

Workspace metadata contains an initial fingerprint and an end change list, not an explicit post-verification manifest. There is a useful nuance: S03 and S05 have matching initial manifests and empty recorded end change lists. That supports equivalence of their recorded tracked-file states. It does not generalize to a run that changes files: S04 begins with zero tracked files and records one creation, so its initial map cannot represent its verified result. An explicit verification checkpoint is preferable when reusing changed outcomes.

### The remaining correctness problem

The offline guard exercise has twelve cases. Project mismatch, redo, changed/unknown intent, stale or deleted artifacts, incomplete sessions, failed checks, missing verification anchors, and uncovered dependencies route away from a successful reuse answer. Exact and translated repeats are accepted only with a declared same-contract label. This validates conditional guards; it does not test semantic matching or translation accuracy. [Reuse controls](evidence/outcome-reuse-probe.json)

A negative control deliberately labels a new dark-theme request as the same intent. The existing completed record and unchanged fingerprint still pass, and the facts-only gate returns the wrong answer. Consequently, the statement that evidence alone makes router mistakes harmless is rejected. Fingerprints establish file facts, not satisfaction of a changed request. They also do not cover external services, time-sensitive facts, excluded dependencies, or a symlink target outside the recorded scope.

The viable investigation is narrower: project-scoped retrieval of prior verified artifacts, explicit attribution, conservative handling of changed intent, and a clear verification scope. Informational follow-ups already have a one-turn answer/task fork in Engine::follow_up. An entry router for a new session could reuse that contract, but must not treat a plausible match as permission to ignore requested changes. It must record its own invocation cost and reference the source outcome without confusing the two session identities.

An exact prompt hash could help retrieve an exact candidate, but cannot handle paraphrases or authorize reuse. Lack of direct model-API authentication also does not rule out local semantic retrieval. The reason to start with existing project records is their small number and the need to validate reuse, not a fundamental impossibility of other retrieval methods.

The gate also has a cost on genuinely new work. Its expected resource saving, before error loss, is positive only when

**p_valid × (C_full − C_revalidate) > C_gate.**

The three examples do not establish p_valid, gate cost, or false-reuse risk. A 3–10k input prediction is unverified; native system and tool context can be substantial before candidate records are added. Start with bounded last-N project records or existing local retrieval; neither embeddings nor a new storage system are prerequisites. If no eligible candidate exists, avoid an extra model invocation. **YMP-204** records the conditional paired study; automatic production substitution is not accepted.

### The running-status claim

At the consistent audit snapshot, S05 is completed with 15 starts and 15 completed invocations. Its last completed invocation is at 04:37:57 UTC. A persistent zombie state is not reproduced by the current database. A host exit between the last invocation and final session persistence can still leave an unfinished status; capture that timing with a focused reproduction before fixing a specific cause. Do not infer completion from invocation count alone or add automatic crash continuation. This lifecycle issue remains distinct from eligibility to reuse a verified outcome.

## H9 — Raw volume, weighted input, and money

### One canonical accounting path already exists

The token_usage table is the normalized per-invocation accounting source used by Store::session_usage and the TUI. The event journal's turn_completed.usage retains raw provider payloads for diagnosis. Codex uses total/last objects, old Claude records are snake-case main-loop usage, new Claude records include a query model map, and ACP is flat last-request usage. They are not two interchangeable totals.

Across S03–S05, a naive read of a top-level inputTokens field finds it absent in 33 of 48 invocations and explicitly zero in none. Defaulting absence to zero produces the alleged zero stream. Native cumulative totals and historical last-request recovery also have different scopes. Do not add them together or select whichever is larger. Keep token_usage canonical and identify raw payload schemas; deleting diagnostic evidence is unnecessary. [Per-row reconciliation](evidence/outcome-economics.json), [accounting contract](../architecture/token-usage.md)

### Recomputed table

Let I include cache reads R and writes W, as the normalized schema already does. For the requested illustrative input weights,

**E_input = (I − R − W) + 0.1R + 1.25W.**

This is weighted input, not total tokens and not a bill. Output remains separate; reasoning is already part of output. Missing cache-write fields create a range for the **recorded subtotal**, not a bounded estimate of the complete run. Historical partial invocations may omit further requests, so the true whole-run upper cost is not known.

| Case | Reported input | Reported output | Reported cache read | Reported cache write | Illustrative weighted input subtotal | Partial invocations |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| S03 | 838,568 | 14,093 | 791,908 | 26,676 | 132,520–133,500 | 20 / 20 |
| S04 | 259,994 | 4,736 | 240,585 | 9,112 | 45,746–46,986 | 13 / 13 |
| S05 | 1,268,960 | 29,722 | 1,139,743 | 30,030 | 250,699–251,673 | 3 / 15 |

The ranges account for absent ACP write-cache fields within the known input values. They are conditional on the illustrative coefficients. Canonical recomputation differs from the earlier table: S05 is a completed larger record, and S03's cache read is about 792k, not 240k. In S05, cache read is about 89.8% of reported input. The Codex-named agent contributes about 84.2% of known raw input-plus-output volume; this does not prove a share of cash cost or a 25-fold price difference.

The ranking is **S04 < S03 < S05** in raw input and in every one of twelve coefficient scenarios: read weight 0, 0.1, 0.5, or 1 crossed with write weight 1, 1.25, or 2. Therefore the proposed cheap test does not produce a ranking reversal. Even a reversal would demonstrate sensitivity to assumptions, not automatically validate a new default metric. The ranking of complete actual charges remains unidentified where coverage or prices are missing. [Sensitivity data](evidence/outcome-economics.json)

### A defensible cost convention

A model-specific estimate needs all relevant rates:

**C_est = [p_in(I − R − W) + p_read R + p_write W + p_out O] / 1,000,000 + separately priced tool fees.**

Rates need a model identifier, currency, effective date, source, cache-write convention, and uncertainty policy. Provider-local equivalent-input tokens divide each provider's estimate by a different input rate and therefore are not automatically a common denominator. If a common reference-token unit is desired, first estimate currency consistently and divide by one explicitly fixed reference rate. This is a reporting convention, not an intrinsic token count. [^12]

Money-shaped fields also need provenance. Claude's total_cost_usd and costUSD use a client price table and remain estimates. Only documented billing records establish charges; native subscription or coding-plan usage need not match API list prices. [^11]

### Display and experimental decisions

Reject the effective-only proposal. Raw input-plus-output is a truthful volume measure, and /usage already exposes the cache and reasoning breakdown. A configurable estimate should coexist with raw counts only when its basis and missing-data handling are visible. The current plus marker means incomplete reporting; price-model uncertainty is a different property. The UI review recommends retaining raw primary figures and, conditionally, qualifying a second estimate in detailed usage. **YMP-108** is parked until a price/budget convention is chosen. [UI analysis](evidence/ui-architecture-review.md#addendum--h9-ui-portion-what-the-interface-labels-today)

Every live experiment must pre-register whether its budget is raw token volume, a versioned currency estimate, or a common reference unit. Report the other available measures rather than silently changing the denominator to improve a conclusion. Homogeneous-model calibration can start with raw volume plus cache breakdowns; cross-provider economic claims need the additional price basis and complete usage coverage.

## Implementation gaps and unproven claims

### 1. Evidence for comparative performance

Comparative performance remains unmeasured. Building the autonomous team described in intent.md is required independently of whether it beats a solo benchmark. A functional implementation, several completed HTML requests, or successful Mock runs do not prove a team-plus-memory advantage at equal resources. A sanity check of the existing greeting scenario makes the distinction concrete: Mock completes its canned greeting.txt workflow, while the scenario's independent greeting.py checks fail. This is a fixture/oracle control, not a zero score for an actual model. [Scenario check](evidence/scenario-smoke.json)

**YMP-201** defines calibration and a twenty-case paired solo/team pilot after measurement and budget prerequisites. **YMP-202** separately tests the four memory/assignment treatments. All model-run allocations remain proposed. Require a frozen evaluator and task-level uncertainty before accepting a quality or efficiency claim. The existing Beta sampler follows a familiar exploration/exploitation idea, but dependent reviews, changing models, selective observation, and sparse categories do not inherit an idealized guarantee. [^1]

### 2. Universal scope and evaluation segments

Retain a falsifiable candidate: inspectable deliverables with multiple constraints or conflicting evidence, where independent verification can discover consequential mistakes. This includes software, data work, document consistency, and planning constraints. It excludes a presumption that trivial file lookup or one-step conversion deserves a full team protocol. The proposed catalog deliberately includes both promising classes and negative controls.

This is a possible evaluation segment, not a restriction of the universal product intent. The user supplies tasks and constraints; the team chooses an appropriate method. YMP-301 is parked as a later positioning/distribution decision and must not block the core local workflow.

### 3. Reliability in sustained use

An infrastructure-failure cohort is missing: the only recorded turn failure is cancellation, while blocked sessions can arise after valid model responses, failed checks, or malformed decisions. Separate user-facing reliability from competence, and record the stage and terminal/nonterminal nature of failures. The MAST study is useful as a reminder to distinguish system design, coordination, and verification failure; its results are not an estimate of ymp's reliability. [^7]

Dogfooding can produce valuable longitudinal data, but it is not the only possible evidence source and it does not replace controlled acceptance. After explicit budget approval, retain a sequence of independently inspected real tasks, with failures and cancellations included. Do not create a scheduled fleet or spend quota merely to fill the journal. YMP-103 and YMP-104 fix specific reproducible failure paths before a broader retry policy is attempted.

### 4. Recovery of the working directory

The direct-directory contract stays. Existing metadata identifies changes but stores no prior content backup. Previously dirty files, untracked files, deleted files, and external side effects have different recovery prospects; a generic promise to restore them would be false. A dirty-repository notice is a possible owner-selected policy, not a substitute for recovery or a universally authorized confirmation prompt.

**YMP-107** retains factual change/check disclosure and accurate recovery limits. It does not add snapshots, private workspaces, automatic reversion, or command-prefix containment. A host-exit lifecycle status should be diagnosed separately and never used to fabricate successful verification.

### 5. Useful memory

Five active entries and eight positive observations are too little to demonstrate accumulation benefits. The current query construction has a measured retrieval defect, so replacing FTS5 with an embedding database would address the wrong first problem. Log retrieved entry identities, test relevance and stale-memory controls, and count the cost of collecting and reviewing memory.

Procedural-memory research provides a reason to test retrieval and update jointly; it does not prove that any stored procedure improves an arbitrary local-agent workflow. Memp's evaluation uses benchmark rewards and its own limitations include uncertainty about deployment without those reward signals. [^6] Implement task-focused retrieval in YMP-106, incremental general knowledge in YMP-113, and evidence-backed correction/supersession in YMP-114. These are intent requirements. YMP-202 measures the benefit of particular policies later. Automatic time-decay heuristics, hierarchical statistical priors and storage rewrites remain unvalidated; this does not defer correction of wrong or outdated knowledge.

### 6. Economics and resource constraints

A single monetary or equivalent-token number cannot repair incomplete accounting or unidentified prices. The immediate requirement is an explicit unit and an enforceable, honestly described stopping policy. Raw token volume, a cache-adjusted input subtotal, a modeled currency estimate, and billing are separate values. Do not use an input-only weighted number as the whole experiment denominator while omitting output and tool charges.

YMP-102 supplies the required shared budget, and YMP-110/YMP-112 use resource facts when selecting and adapting work. YMP-108 is an optional presentation choice with raw fields retained and price provenance visible. The unvalidated lambda formula and cross-provider economic superiority claims remain deferred; resource-aware autonomous decisions do not.

### 7. Other users and installation

The implementation is macOS-tested and depends on Unix process/socket behavior; Unix sockets and libc alone do not establish a macOS-only design. Linux support has not been verified in this investigation, and no Windows support claim is made. The default Claude bridge path is compiled from the checkout location, though YMP_CLAUDE_BRIDGE already provides an override. Packaging, supported operating systems, and native CLI authentication setup require an explicit audience decision. [Engine construction](../../ymp-rust/crates/ymp-runtime/src/engine.rs) (new), [CI](../../.github/workflows/check.yml)

The final intent already defines a local, universal agent-team utility. YMP-301 may later refine external distribution commitments, but it is not a prerequisite for that product. Implement the core workflow and evaluate alternative mechanisms as evidence accumulates; general engineering guidance does not replace the owner-defined direction. [^8]

## Reproduction and limitations

The evidence directory contains derived, content-free measurements rather than a copy of private native credentials or message bodies. Each tool is research instrumentation, outside the application crates. The protocol fixture runs local Python processes, writes harmless artifacts in temporary working directories, and uses synthetic token counters. The context probe builds an offline temporary Rust program against unchanged local crates. The exact private journal is not published; its aggregate records and calculation inputs are included.

~~~sh
python3 ymp-docs/research/tools/run_protocol_experiments.py --binary target/release/ymp --output ymp-docs/research/evidence/protocol-experiments.json
python3 ymp-docs/research/tools/run_context_probe.py
python3 ymp-docs/research/tools/check_gate_probe.py
python3 ymp-docs/research/tools/outcome_reuse_probe.py
python3 ymp-docs/research/tools/effort_policy_probe.py
python3 ymp-docs/research/tools/scenario_smoke_probe.py
python3 ymp-docs/tasks/manage.py check
~~~

Journal and economic audits take an explicitly selected database path and do not migrate or write it. Their snapshot timestamps and maximum event sequence identify the observations. Re-running them against later activity will produce a new dataset. Working-tree enumeration includes any current untracked files; the controlled synthetic directory cases are the stable comparison.

No local fixture establishes semantic routing quality, reasoning-tier quality, real infrastructure failure frequency, actual billed cost, or long-run memory benefit. The numerical ranking sensitivity concerns reported subtotals; incomplete historical requests can still change the unknown full-run ranking. The UI review rejects file-size causation from a four-commit history. These limits are part of the result, not work silently marked complete.

[^1]: [Thompson sampling tutorial](https://arxiv.org/abs/1707.02038v3). Full citation in Sources.
[^2]: [Doubly robust policy evaluation](https://arxiv.org/abs/1103.4601). Full citation in Sources.
[^3]: [Agent systems scaling study](https://arxiv.org/html/2512.08296v3). Full citation in Sources.
[^4]: [Compute-efficient multi-agent reasoning](https://aclanthology.org/2026.acl-srw.1/). Full citation in Sources.
[^5]: [Equal-budget multi-hop comparison](https://arxiv.org/abs/2604.02460v2). Full citation in Sources.
[^6]: [Memp procedural memory](https://arxiv.org/html/2508.06433v4). Full citation in Sources.
[^7]: [MAST failure taxonomy](https://arxiv.org/html/2503.13657v3). Full citation in Sources.
[^8]: [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents). Full citation in Sources.
[^9]: [Codex App Server](https://learn.chatgpt.com/docs/app-server). Full citation in Sources.
[^10]: [Claude agent loop controls](https://code.claude.com/docs/en/agent-sdk/agent-loop). Full citation in Sources.
[^11]: [Claude cost and usage tracking](https://code.claude.com/docs/en/agent-sdk/cost-tracking). Full citation in Sources.
[^12]: [OpenAI API pricing categories](https://developers.openai.com/api/docs/pricing). Full citation in Sources.
[^13]: [GLM ACP upstream documentation](https://github.com/stefandevo/glm-acp-agent). Full citation in Sources.

## Sources

The linked local evidence and source files are the primary basis for the findings. External research supplies context and methodological constraints; none is treated as direct evidence of ymp's superiority. Mutable provider documentation was checked on 2026-09-12, with installed versions recorded separately in effort-capabilities.json.

1. Russo, Van Roy, Kazerouni, Osband, and Wen. [A Tutorial on Thompson Sampling](https://arxiv.org/abs/1707.02038v3). Foundations and Trends in Machine Learning, 2018; arXiv v3, 2020. Bernoulli posterior sampling and the limits of importing idealized assumptions.
2. Dudík, Langford, and Li. [Doubly Robust Policy Evaluation and Learning](https://arxiv.org/abs/1103.4601). ICML, 2011. Policy evaluation from selectively observed rewards; cited for the need to specify reward/logging assumptions.
3. Kim et al. [Towards a Science of Scaling Agent Systems](https://arxiv.org/html/2512.08296v3). arXiv v3, 2026. Task-dependent architecture tradeoffs and generalization limitations.
4. Wunderlich, Kaesberg, Wahle, Ruas, and Gipp. [Multi-Agent Reasoning Improves Compute Efficiency: Pareto-Optimal Test-Time Scaling](https://aclanthology.org/2026.acl-srw.1/). ACL Student Research Workshop, July 2026, pp. 1–14. Positive results on specific reasoning benchmarks under declared compute comparisons.
5. Tran and Kiela. [Single-Agent LLMs Outperform Multi-Agent Systems on Multi-Hop Reasoning Under Equal Thinking Token Budgets](https://arxiv.org/abs/2604.02460v2). arXiv v2, April 2026. Contrasting evidence and sensitivity to budget/context definitions.
6. Fang et al. [Memp: Exploring Agent Procedural Memory](https://arxiv.org/html/2508.06433v4). arXiv v4 / ACL Findings, 2026. Retrieval/update mechanisms and benchmark-reward limitations.
7. Cemri et al. [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/html/2503.13657v3). arXiv v3, October 2025. Failure categories across system design, coordination, and verification.
8. Anthropic. [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents). Engineering guidance, accessed 2026-09-12. Simplicity and measured reasons to add coordination.
9. OpenAI. [Codex App Server](https://learn.chatgpt.com/docs/app-server). Native turn configuration and schema generation. The installed 0.153.4 generated schemas, rather than a presumed generic enum, ground the local effort and error-notification audit.
10. Anthropic. [How the agent loop works](https://code.claude.com/docs/en/agent-sdk/agent-loop). Effort, thinking, turn and budget controls; installed SDK types are checked separately.
11. Anthropic. [Track cost and usage](https://code.claude.com/docs/en/agent-sdk/cost-tracking). Whole-query accounting and the explicit distinction between SDK price estimates and authoritative billing.
12. OpenAI. [API pricing](https://developers.openai.com/api/docs/pricing). Input, cached-input, output and additional charge categories. No listed rate was silently adopted as the price of a native subscription.
13. stefandevo. [glm-acp-agent](https://github.com/stefandevo/glm-acp-agent), upstream README. Current advertised controls differ from the cached local 1.3.0 installation; local source hashes and pure-function results are retained.

## Research-derived delivery list

The current product queue is defined by intent.md and maintained in the [task register](../tasks/README.md). It covers the available pool (YMP-109), native execution choices (YMP-111), decision history and shared budget (YMP-101/YMP-102), autonomous formation and adaptation (YMP-110/YMP-112), and incremental correctable experience (YMP-113/YMP-114). All are implementation work, not claims of completed features.

The six improvements below retain their measured engineering rationale and support that core delivery. This is not a replacement product roadmap or a requirement to prove benchmark superiority first. Experimental provider quota remains separately authorized.

1. **YMP-103 — Honor Codex native retry notices.** Preserve the current invocation when the native provider announces a retry; retain timeout, cancellation, and terminal-failure behavior.
2. **YMP-104 — Preserve a verified result if synthesis fails.** Return a factual fallback with evidence and record the reporting failure without misclassifying the artifact.
3. **YMP-101 — Preserve explainable session decisions and execution provenance.** Link team choices, assignments, settings, resources and independent verification; expose history and structured records.
4. **YMP-102 — Enforce the shared resource budget.** Define units, admission, in-flight work, unknown usage and overshoot for normal autonomous execution as well as later studies.
5. **YMP-105 — Enumerate names without hashing contents.** Preserve path semantics while removing measured unnecessary work; leave verification hashing intact.
6. **YMP-106 — Retrieve memory using task terms.** Fix the demonstrated boilerplate-query failure and record retrieved identities before claiming a learning advantage.
