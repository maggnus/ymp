# Product brief: ymp as an autonomous agent collective

Recorded verbatim from the owner, 2026-08-15. This brief is the product-level source of truth for
the collective redesign; the design deliverable lives beside it and the work tree tracks the
implementation. Where this brief and older documents disagree, this brief wins at the product
level; kernel trust boundaries and verification principles are explicitly preserved by it.

## Central idea

    operator starts ymp
        -> configures/enables available providers
        -> enters a natural-language goal
        -> an autonomous, self-organizing collective of agents works on it
        -> the collective determines how to decompose, delegate, collaborate,
           implement, challenge, test, verify, revise, and finish the work
        -> operator receives the result and evidence

The operator is NOT required to understand or manually construct the internal machinery.
ymp must not become a conventional orchestrator where the user assembles a team of roles.

    The collective is the intelligence.
    The kernel provides rules, authority, resources, isolation, accounting, reproducibility.
    Verification establishes whether the candidate satisfies the intended outcome.
    The TUI is the operator interface.

## 1. Product principle

The operator provides intent, not an execution contract. The operator must not be required to
prepare: PROJECT-CONTRACT.md, task contracts, acceptance criteria, oracle bundles, verifier
implementations, agent roles, decomposition plans, team membership, model assignments,
coordination plans. If such machinery is required for correctness, ymp creates, derives,
validates and manages it internally. Do NOT remove the mechanisms to simplify UX — move their
responsibility behind the product boundary.

Ask the operator only when the intended outcome genuinely cannot be determined from: the request,
repository state, project documentation, visible tests/checks, approved policies, and what the
collective discovers.

    ambiguity in user intent    -> ask operator
    missing internal machinery  -> ymp/collective creates it

Appropriate: "Should this use Authorization Code or Client Credentials?"
Not appropriate: "Please create an acceptance oracle / agent contract / verifier."

## 2. Collective intelligence

The fundamental unit of intelligence is the collective, not one orchestrating agent. A collective
may inspect, investigate independently, discuss, challenge, identify ambiguity, decompose, create
local tasks, recruit, choose model capabilities, delegate, compare competing approaches,
implement, review, request independent verification, react to failures, revise, abandon,
synthesize, and stop when the result is demonstrated. No fixed role hierarchy; no predefined
manager/architect/QA/reviewer roles. Temporary roles may emerge from decomposition.

    Collective decides what should happen.
    Kernel decides whether it is permitted.
    Oracle/verifier determines whether the result satisfies the contract.

## 3. Agent != model != provider

Preserve and make explicit: Provider / Model / Runtime / Runtime Driver / Participant / Attempt /
Invocation. A provider exposes models; a model is a capability source; a participant is an actual
autonomous member of the collective. Never instantiate every available model as an agent (100
models must NOT become 100 agents). Availability/pool model:

    providers -> model catalog -> project policy / allowed pool
              -> collective recruitment -> actual participants

Recruitment requests go through the kernel, which checks mechanical constraints only (provider,
model and runtime availability; policy; budget; concurrency; participant-start limits; capability
restrictions; data-disclosure policy; assurance profile). The kernel must NOT decide "model X is
best for this task" — that is the collective's semantic decision.

## 4. Provider / model / project configuration

Three levels. PROVIDER LEVEL: the operator connects/authenticates providers (/providers with
ready / not configured states). MODEL CATALOG: providers expose models; no manual agent per
model. PROJECT/TASK POLICY: optional restriction of providers, models, concurrency, total
participants, budget, wall-clock, disclosure classes, assurance profiles — boundaries, not
semantic routing. Defaults must be useful without configuration: "these providers are available"
and the collective decides how to use them.

## 5. Dynamic recruitment

Controlled flow: participant -> recruitment request -> kernel checks policy/capacity/budget ->
approved runtime/profile -> new participant -> joins scoped task/collective. Recruitment consumes
finite resources; starting an agent to ask whether it wants work is not free. Preserve the
existing budget distinctions (proposal/recruitment, execution escrow, communication, task,
runtime-start, concurrency). Participants may decline, stop, yield, expire, be cancelled,
replaced, or recruit. The kernel records transitions without interpreting semantic reasons.

## 6. TUI designed from zero

The current TUI concept is not sufficient. Design the complete operator journey from process
start to final result. Similar in feel to Claude Code and Paseo (one local command, interactive,
configure providers, observe agents, inspect activity, intervene, inspect results, continue) —
but do not copy their architecture or lifecycle semantics. Define at minimum the 30 enumerated
surfaces (startup; first run; provider discovery/auth; model catalog; workspace selection; task
creation and clarification; collective startup; live collective view; /agents and participant
details; task/obligation view; communication/activity view; candidate view; verification state;
failures; pause; resume; cancellation; intervention; result; evidence; export; history; archive;
provider/model configuration; budgets; runtime readiness; recovery from invalid configuration) —
with what the operator sees, what actions exist, and what each action does. Hide internal
terminology (obligation, lease, fencing token, oracle bundle, capability namespace, runtime
driver) behind advanced diagnostics; lead with task/agents/activity/progress/result/verification/
budget/provider/model/workspace/evidence.

## 7. Complete operator lifecycle

    $ ymp -> startup checks -> provider/runtime readiness -> operator enters task
          -> repository discovery -> collective bootstrap -> analysis -> self-organization
          -> optional recruitment -> implementation/research/review -> candidate -> verification
          -> failure/revision loop -> accepted result -> inspection -> explicit export/delivery

The operator never manually coordinates this. The TUI informs without becoming a per-participant
control panel.

## 8. Internal contracts and verification

Preserve the contract/oracle architecture where technically necessary; reinterpret its product
boundary. Internally derive: intent -> observable requirements -> internal task contract ->
acceptance plan -> verification strategy. Distinguish provenance classes: (A) operator-supplied,
(B) repository-discovered, (C) collective-inferred assumptions, (D) internally generated
verification mechanisms, (E) explicit operator clarifications. If assumptions materially affect
the result and cannot be safely resolved — ask; never silently invent requirements. Protected
verification keeps the existing principles (no secret requirements; protected tests instantiate
public requirements; negative controls discriminate; infrastructure failure is not rejection;
exact digests recorded; bounded adaptive query budgets; independence from producer conversation).

## 9. No central semantic orchestrator

Do not hide a "manager agent" behind the TUI. The kernel enforces authority, accounting,
lifecycle, isolation, concurrency, leases, capabilities, provenance, candidate immutability,
verification authorization, terminal states. The kernel must not choose agents or models, assign
roles, decompose, rank bids by intelligence, synthesize answers, interpret natural language as
commands, or judge semantic quality.

## 10. Operator control

The operator is the principal, not the team manager: provides the goal, configures providers,
sets optional boundaries, observes, clarifies genuine ambiguity, pauses/cancels, optionally
intervenes, reviews, explicitly exports. Intuitive views: /providers /models /agents /tasks
/activity /budget /verify /result /help (exact structure per existing TUI conventions).
Operator intervention is distinguishable from autonomous execution and may mark the run as
intervened per the observation policy.

## 11. Agent observability

The operator sees the collective forming and changing (/agents: active and history, per
participant: identity, runtime/model profile, lifecycle state, scoped task, resource usage,
progress events, findings, submissions, verification requests/results, recruitment, failures,
yields, cancellations). No private chain-of-thought. Conversation content remains untrusted
collaboration data per the observation policy.

## 12. Result experience

Not "the agents stopped" but "here is the result": completed summary, verification checklist,
participants/attempts, time, cost, candidate digest, actions [Inspect result] [Inspect evidence]
[Export] [Continue] [Archive]. Failed verification normally lets the collective react while
resources remain. Honest non-success terminals presented clearly (budget exhausted;
infrastructure failure; unresolved ambiguity; rejected with no funded retry). Never present
"exhausted" as success.

## 13. Product language

Lead with: agent collective, participants, task, goal, activity, result, verification, evidence,
provider, model, workspace, budget. Keep internal: orchestration, scheduler, contract/oracle/
verifier authoring, task DAG management, role assignment. Mental model: "I give ymp a task. The
collective figures out how to solve it. ymp makes sure the process stays within its rules and
that the result is actually verified."

## 14. Architectural consequence

Do not rewrite the architecture for UX. Identify and fix ownership leaks: PROJECT-CONTRACT.md as
mandatory input; verifier creation as user responsibility; manual agent creation; manual model
assignment; manual decomposition; manual team assembly; provider config conflated with
participant creation; runtime profiles exposed as user-level roles; acceptance mechanics as
mandatory workflow steps. Ownership: Operator -> intent and policy; Collective -> semantics;
Kernel -> authority/resources/lifecycle/isolation/accounting/provenance/state integrity;
Verification -> independent evidence; TUI -> simple interaction.

## 15. Provider/model orchestration decision

Catalog/pool abstraction; participants instantiated only when needed; collective chooses among
approved models subject to mechanical constraints; the kernel is not a model router. Optional
convenience pools (reasoning/general/fast/cheap) are resource policies, not semantic roles. No
hard-coded "architect model"/"coder model" assumptions.

## 16. Design deliverable

Inspect the existing repository and produce the concrete changes required: (1) revised product
mental model; (2) operator lifecycle; (3) complete TUI information architecture; (4) startup and
first run; (5) provider configuration flow; (6) model catalog and availability; (7) project/task
policy; (8) collective bootstrap; (9) dynamic recruitment; (10) /agents experience; (11)
task/activity/result views; (12) intervention semantics; (13) verification UX; (14) failure and
terminal-state UX; (15) result/export flow; (16) internal ownership of contracts/oracles/
verifiers; (17) provider/model/runtime/participant distinctions; (18) model-pool/recruitment
design; (19) exact boundaries operator/collective/kernel/verifier/TUI; (20) required domain/API/
state changes; (21) required Rust module/crate changes; (22) required tests and acceptance
scenarios; (23) migration path from the current design; (24) unresolved decisions that genuinely
require human judgment. Do not invent details to fill gaps; preserve sound constraints; change
ownership boundaries rather than removing safety mechanisms.

## 17. Acceptance criteria

1. A new operator launches ymp without authoring a contract or verifier. 2. Providers are
configurable and visible. 3. 100 models never become 100 automatic agents. 4. No manual team
construction. 5. The collective recruits dynamically from the permitted pool. 6. Recruitment is
bounded by kernel resources and authority. 7. The collective (not the kernel) makes semantic
decisions. 8. Internal contracts/verification stay strict but are not operator prerequisites.
9. Questions only on genuine intent ambiguity. 10. The TUI exposes the full lifecycle.
11. Collective activity is understandable without chain-of-thought. 12. Results carry
verification evidence. 13. Failed verification does not auto-terminate while budget/policy allow.
14. Exhaustion, infrastructure failure, cancellation and acceptance remain distinct terminals.
15. Kernel trust boundaries remain intact. 16. The experience is "give the collective a goal,
get a verified result", not "build and manage a workflow".

## 18. Final design test

    $ ymp; configure Anthropic + OpenAI + NVIDIA; enter "Implement feature X in this repository."
    No contract, oracle, verifier, team, roles, model assignments, or decomposition created by
    the operator. One participant analyzes; determines more expertise is useful; requests another
    participant; the kernel admits it; a suitable model becomes participant B; they investigate,
    disagree, resolve; one implements, another reviews; a candidate is produced; verification
    fails; the collective sees permitted diagnostics; revises; verification succeeds; the
    operator sees completed + verified and can inspect/export.

If the scenario still requires manual contract/verifier/agent/role/model/decomposition work, the
design is not complete. The implementation may be sophisticated internally; the product
experience must remain simple.
