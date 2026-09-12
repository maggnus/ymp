# Delivery plan for the approved intent

The owner approved [intent.md](../../intent.md) on 2026-09-12 and requested this work plan. Its SHA-256 is `4479e919c5b47934e69e81ac493d16e90a0dc420a9a370382e79bca3f3c754d9`. The approved file is preserved byte for byte. The current implementation baseline is `c443ad0`, ymp 0.3.0; the research measurements retain their original `56e21dd` baseline.

The canonical task definitions, dependencies, milestones and intent coverage are in [tasks.json](tasks.json). The generated [progress page](README.md) shows ready work and completion counts. Existing IDs and historical evidence are retained. This plan replaces the earlier product-definition pause; it does not claim that the implementation already satisfies the intent or authorize experimental provider quota.

## Delivery objective

Deliver a universal local-agent workspace in which the runtime commits assignments and final acceptance, authority lasts for one assignment, teams and native settings adapt within constraints, useful independent work can run concurrently, and evidence-backed experience survives across sessions.

Independent review and confirmation are separate requirements. A reviewed result may be accepted without confirmation, but it must remain explicitly unconfirmed and earn no reputation. Confirmation requires relevant deterministic checks or external data, not model agreement. This distinction applies to execution, planning, review, knowledge reuse and historical migration.

The five product goals remain measurable hypotheses: greater completion probability, lower resource use, faster completion where useful parallelism exists, less user intervention and improvement on later tasks. Delivery establishes the mechanisms and observable results; comparative studies assess whether particular policies improve those goals.

## Current implementation and concrete gaps

| Area | Existing basis | Missing behavior to deliver |
| --- | --- | --- |
| Identity and native access | Stable AgentProfile IDs, native authentication, session snapshots and provider adapters | Complete pool/membership distinction and per-assignment model/effort settings |
| Trusted state | Task states, durable events and per-agent/session team-tool credentials | Assignment-scoped grants, expiry/revocation and runtime validation of changing proposals |
| Review and experience | Candidate review excludes its executor; deterministic failures overrule approval | Separate acceptance from confirmation, exclude executors from final review, prevent unsupported reputation credit |
| Budget | Invocation, timeout, attempt and concurrency limits; usage snapshots | Explicit shared admission, bounded startup, protected verification allowance and honest native enforcement limits |
| Coordination | Fixed plan/bid/execute/review workflow; recorded task proposals | Apply proposals, revise the graph, change participants and renegotiate unfinished work |
| Concurrency | Parallel planning/bidding and a semaphore | The ready-task selection uses `take(1)`, so actual task execution and verification are serialized |
| Knowledge | Incremental events, FTS retrieval, observations and memory records | Evidence-qualified incremental general knowledge and implemented correction/supersession |
| User visibility | Chat TUI, session/per-agent usage and recovery views | Assignment permissions and settings, reviewed versus confirmed results, adaptation and provenance |

The [runtime](../../ymp-rust/crates/ymp-runtime/src/engine.rs), [team tool server](../../ymp-rust/crates/ymp-runtime/src/mcp.rs) and [core model](../../ymp-rust/crates/ymp-core/src/model.rs) were inspected for this plan. In particular, no-check approval can currently create a positive observation, reviewer agreement can create another observation, and the final reviewer can be an executor. These are implementation gaps, not hypothetical model-performance claims.

The batch error handling will need attention when execution becomes concurrent. At this baseline `take(1)` prevents multiple ready tasks from entering that batch, so a multi-sibling loss is a latent concurrency hazard rather than a reproduced current multi-task failure. YMP-115 must cover it before enabling overlap.

## Milestones and completion conditions

Milestones group outcomes; actual task dependencies determine when work may start. They are not calendar estimates or an instruction to keep otherwise independent work idle.

| Milestone | Scope | Completion condition |
| --- | --- | --- |
| M0 — Approved baseline | YMP-009, YMP-010, YMP-011, YMP-012 | Approved intent preserved; normative documents, task dependencies and all goal/principle mappings agree |
| M1 — Trusted state and evidence | YMP-116, YMP-109, YMP-101, YMP-120, YMP-117 | Runtime-owned assignments and expiring grants work; independently accepted unconfirmed results remain unconfirmed and earn no reputation |
| M2 — Bounded native execution | YMP-111, YMP-102, YMP-103, YMP-104 | Native model/effort choices are validated; shared limits include startup and verification; retries and narration failures preserve actual outcomes |
| M3 — Adaptive teams and concurrency | YMP-110, YMP-112, YMP-115 | Fixed and changing teams apply valid work proposals; independent work overlaps and conflicting work remains coordinated |
| M4 — Reusable experience | YMP-105, YMP-106, YMP-113, YMP-114 | Later sessions retrieve applicable confirmed knowledge; corrections preserve history; unsupported outcomes cannot become reputation evidence |
| M5 — Observation and recovery | YMP-107, YMP-118 | Actual agent identities, assignment scope, settings, usage, review basis, confirmation and recoverable work are inspectable |
| M6 — Universal acceptance and release | YMP-119, YMP-121 | Offline end-to-end scenarios pass; native compatibility and packaging are checked under explicitly authorized limits |

The first useful integrated checkpoint is M1–M2: a bounded native assignment, independent review with the correct confirmation grade, preserved output and usable history. This provides a reliable basis for adaptation instead of adding membership changes to an ambiguous acceptance model. M3 supplies actual cooperative execution; M4 adds the accumulating benefit required by the product.

### M1: protocol and trusted runtime

YMP-116 produces the executable contract before schema and permission changes: authoritative transitions, event links, assignment grants, review/confirmation states, compatibility rules, resource admission and workspace/result-version ownership. Roles and privileges are temporary; a planning participant cannot keep coordination authority in later assignments.

YMP-109 and YMP-101 then establish identity/history and decision provenance. YMP-120 enforces grant scope and revocation. YMP-117 implements evidence-qualified acceptance and observation updates. Existing histories must migrate without deletion or promotion of unsupported historical success into confirmed evidence.

The contract should use the existing package boundaries: core records and invariants in ymp-core, persistence in ymp-storage, native controls in ymp-providers and the bridge, coordination in ymp-runtime, workspace ownership in ymp-workspace. No new orchestration service or storage rewrite is needed merely to define these responsibilities.

### M2: resource control and native reliability

YMP-111 transmits actual supported native settings and records requested, applied and reported values. YMP-102 controls shared resources, simultaneous reservations and verification capacity; one outer invocation must not be mistaken for one native model request. Strict limits are offered only where the adapter can substantiate enforcement and accounting. Unknown consumption stays unknown.

YMP-103 handles nonterminal native retry notices without adding another replay loop. YMP-104 retains accepted results and their confirmation grade when final narration fails, providing a factual fallback from stored evidence. These changes must not turn incomplete work into accepted work or unconfirmed work into confirmed work.

### M3: collaboration under constraints

YMP-110 selects the initial team and assignment settings. YMP-112 turns agent proposals and commitments into runtime-validated changes, including revised dependencies, allowed membership changes and diagnosed reassignment. A fixed size, pinned roster and concurrency ceiling are separate constraints.

YMP-115 enables overlapping useful work with explicit occupancy, dependencies, resource ownership and result-version consistency. Removing `take(1)` alone is insufficient. Independent work may overlap only within enforceable scopes; conflicting writes or publication and checks against changing outputs require coordination under the governing workspace policy. Each completed result is processed independently so one failure does not discard another result's review.

### M4: accumulating useful experience

YMP-105 removes unnecessary file-content reads from names-only enumeration. YMP-106 constructs retrieval queries from the task and records the selected knowledge versions. These are concrete improvements whose prerequisites permit earlier delivery.

YMP-113 persists useful findings and confirmation-qualified experience during work, including partial progress that survives a later failure. YMP-114 corrects or supersedes knowledge using newer supporting evidence while retaining history. Proposed or unconfirmed knowledge may remain inspectable, but must not acquire the status of confirmed reusable knowledge or increase reputation merely through agent agreement.

### M5: interface integration

All application UI work is assigned to Claude Code `claude-opus-5` with thinking `max` through Paseo. Backend contracts, integration and independent validation remain the maintainer's responsibility. YMP-107 covers recorded checks and factual recovery limits; YMP-118 revises the earlier UI contract for the approved protocol and exposes actual state, including accepted-but-unconfirmed outcomes.

UI contracts may be prepared as their backend contracts stabilize. A milestone is not complete until the implemented views reflect real session state and historical snapshots. The earlier UI review is baseline evidence, not permission to retain contradictory labels or a separate provider-based team model.

### M6: acceptance and local release

YMP-119 provides deterministic universal workflows and failure controls. YMP-121 packages the release and prepares a bounded native-compatibility procedure. No provider inference is triggered by creating the plan, checking the task register, opening the TUI or running unattended acceptance scenarios.

Real compatibility checks are distinct from offline tests and comparative experiments. Prepare exact provider versions, commands, settings, expected results and enforceable resource limits before requesting quota; do not guess or reuse an experimental budget as authorization. Release notes distinguish verified native behavior, fixture coverage and limitations.

## Representative acceptance scenarios

| Scenario | Required evidence |
| --- | --- |
| Create a document, then ask where it is | Required file exists at the chosen destination; the follow-up reuses session/result context without rerunning production; works without Git |
| Transform a supplied dataset | Output agrees with deterministic expectations; an irrelevant passing command cannot establish correctness |
| Produce a grounded analysis | Claimed confirmation links to the supplied external records and relevant acceptance criteria; invented or unrelated citations do not qualify |
| Produce a qualitative recommendation | Independent review may accept it with a recorded basis; absent confirmation it remains unconfirmed and adds no reputation |
| Coordinate independent and conflicting work | Independent assignments overlap; conflicting side effects are coordinated; duplicate claims and expired permissions are rejected |
| Recover after interruption or budget exhaustion | Preserve outputs, review/confirmation grades, spend and history; inspect uncertain side effects before any replay |
| Reuse and correct prior knowledge | A later session retrieves applicable supported knowledge; changed requirements and evidence-backed corrections supersede stale entries without erasing provenance |

Each scenario needs positive and negative cases that distinguish the intended property from a superficially successful response. Offline synthetic tokens and fixture answers establish protocol behavior; they do not establish real model quality or savings.

## First work to start

1. **YMP-116:** specify the runtime/assignment/acceptance contract. This unlocks the main dependency chain.
2. **YMP-103:** fix the confirmed nonterminal-retry handling defect using the existing offline protocol fixture.
3. **YMP-105:** implement names-only enumeration with path-set and no-content-read checks.

These three have completed prerequisites. Work on them can be scheduled independently; this plan does not start agents or parallel workers. After YMP-116, identity and provenance work can begin, followed by assignment authority and evidence-qualified acceptance. The progress page lists the remaining topologically valid delivery order.

## Validation and completion discipline

Update task status when work starts, record evidence when an acceptance condition is met, and mark a task done only after its required checks pass. Do not infer feature completion from a written design or a completed research task. The registry validates dependencies, milestone membership, the approved-intent digest and complete goal/principle coverage.

For changes, run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Bridge changes also require its type checks, build and relevant offline protocol tests. Use controls appropriate to the defect: expired grants, unsupported effort, accepted without confirmation, stale knowledge and interrupted side effects are important negative cases. Repeating unrelated checks does not substitute for these acceptance conditions.

Representative noncoding outputs, consistent destination handling, history preservation and resource accounting are release conditions. Agent count and message volume are not progress measures. Compare quality, confirmation coverage, elapsed time, complete resource use and human intervention separately.

## Deferred optimization and comparison

YMP-201 remains parked until universal fixtures, release prerequisites and explicit quota authorization exist. Revise its comparison around a strong native solo agent using the full allowance, several independent attempts with the same aggregate allowance, and a cooperating team; charge output selection and verification to every relevant arm. Report accepted-but-unconfirmed outcomes separately from confirmed acceptance and preserve failures and cancellations.

YMP-202 evaluates experience on held-out tasks; YMP-203 evaluates specific effort and coordination policies; YMP-204 evaluates a dedicated outcome-reuse router. Optional currency estimates (YMP-108), external-user positioning (YMP-301), an Oracle, automatic numerical effort ladders and a universal optimal team size do not block core delivery. Their value is not established by this plan.

No dates or improvement percentages are promised without delivery and evaluation evidence. If the approved intent changes, reconcile affected tasks and coverage explicitly before accepting a new digest. Historical research datasets and the owner-authored source are not silently rewritten.
