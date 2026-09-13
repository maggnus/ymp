# Domain terminology

These definitions expand the concise Terms and Definitions section in [intent.md](../../intent.md#термины-и-определения). They describe the product vocabulary. They are not a claim that every field, transition or capability is implemented. The [team and effort policy](../architecture/team-and-effort-policy.md) states the corresponding operating rules.

## Task

A task is work with an expected result and acceptance conditions. A user task supplies the session's goal and constraints. Subtasks describe parts of that work, including dependencies and their own acceptance conditions. Decomposing a user task does not create a new conversation for every subtask.

## Session

A session is the durable context, history and state of solving one user task. It contains the goal, constraints, participants, task graph, assignments, board, usage and outcomes. A follow-up question or clarification retains the session; a distinct user task gets its own session. Session identity is separate from a provider's native conversation identifier.

## Provider

A provider is a configured connection to a native execution environment offering model and tool capabilities. The connected installation owns its authentication. A provider is not a team member, and its name does not identify an agent or imply a work role. A vendor, native installation and model family are related concepts, but are not interchangeable identifiers.

## Model

A model is an AI model available through a provider and selected as an execution resource. Record the requested identifier and the resolved identifier when available. Model availability, supported effort settings and defaults depend on the native installation and model. Multiple agents may use the same model; changing a model does not automatically create a new agent identity.

## Agent

An agent is an individually identified participant with instructions, execution capabilities and a distinct work context. Its ID owns authorship, assignments, usage attribution and historical participation. Provider/model bindings supply execution resources; they do not replace that ID. The interface labels a native agent with its model name/identifier and observed effort, while keeping provider identity separate. Descriptive catalog captions such as Default (recommended) do not identify a concrete model.

An agent can plan, execute or verify different work. Roles and permissions are limited to one assignment; no permanent agent hierarchy or inherited authority exists. Configuration changes are versioned for execution evidence while the stable ID preserves history. Different IDs or model names alone do not establish statistically independent reasoning or independent verification.

## Pool

The pool is the set of agents eligible for selection under current availability and user constraints. Eligibility does not imply participation, a running process or a model invocation. Capability discovery and profile inspection are distinct from asking every candidate to produce a proposal.

## Team

A team is the set of agents selected to participate in a session. Current membership differs from both the eligible pool and the agents executing at a particular instant. Joining, leaving and replacement preserve historical identities, responsibilities and resource attribution. A fixed size and a pinned roster are different constraints, as defined in the [policy](../architecture/team-and-effort-policy.md#dynamic-and-fixed-constraints).

## Assignment

An assignment binds a particular agent to bounded work, with its task/purpose, execution configuration, permissions and constraints. The runtime commits it and controls the lifetime of its authority. It is distinct from the task: reassignment changes responsibility without erasing the task, previous attempts or accepted evidence. Model and effort choices belong to assignments and are recorded for the invocations that execute them.

## Invocation

An invocation is one ymp-issued request to a native agent runtime. It may contain several native model requests and tool calls. An assignment may need multiple invocations, but their usage and attempt history remain attributable to that assignment and session. Invocation counts, native model-request counts and task attempts are different measurements.

## Effort

Effort is a model-supported control for requested reasoning intensity. A provider may expose separate thinking enablement, adaptive behavior or reasoning allowances. Use the actual supported controls rather than assuming a universal low-to-max scale.

Requested effort, the setting transmitted or acknowledged by an adapter, and provider-reported execution information are distinct. Effort is not a token budget, a measure of total cost or a permanent property of an agent. A fixed effort is an allowed-set constraint on assignments.

## Budget

A budget is the session's overall resource limit. Each dimension has an explicit unit and coverage: for example elapsed time, invocation allowances, reported tokens or a versioned cost estimate. Concurrency is an admission constraint, not itself a total-spend budget.

Planning, communication, execution, failed attempts, verification and any Oracle consultation consume shared resources. Runtime reservations are accounting commitments for in-flight work; their guarantees depend on actual adapter enforcement and usage coverage. Unknown usage must remain unknown.

## Shared board

The shared board is the durable record of participant messages, proposals, findings and decisions in a session. A message may address the team or a participant. Addressing controls intended audience and delivery; it does not imply a private channel or a broadcast that must wake every agent.

Free-text discussion is distinct from an accepted state transition. A proposal acquires operational effect only through the runtime's validated decision process. The board may reference artifacts and evidence; it does not replace the actual result files or acceptance checks.

## Knowledge

Knowledge is a retained finding with applicability and supporting evidence. A candidate finding may be stored before verification, but it remains distinguishable from accepted reusable knowledge. Source session, authorship, verification state and relevant task or artifact versions establish provenance.

Knowledge may have project or general scope. Corrections and supersession retain the earlier record and supply new supporting evidence. Session history is not automatically reusable knowledge, and an old accepted fact is not automatically applicable to a changed task.

## Runtime

The runtime is YMP's trusted execution kernel. It executes the protocol, grants assignment-scoped authority, commits assignments and records final acceptance after the required verification. Agent messages, proposals and negotiated agreements are inputs; they do not themselves change authoritative state or grant permissions. Native provider processes remain execution resources, not this protocol authority.

## Verification

Verification is the assessment of a result against acceptance conditions, with its basis recorded. It must be independent before final acceptance. A reviewer may record a reasoned acceptance even where external confirmation is unavailable, but that outcome remains explicitly unconfirmed. Naming a phase review or obtaining agreement does not establish independence.

## Confirmation

Confirmation is an acceptance basis independent of agents' judgments: relevant deterministic check results or inspectable external data. Agent agreement is not confirmation. Record sources, relevant criteria, result version and coverage so unrelated passing commands or unsupported citations cannot be mistaken for confirmation.

Acceptance and confirmation are separate facts. An independently reviewed result can be accepted without confirmation and must then earn no reputation. Rejected or unchecked work does not become accepted merely because it lacks confirmation. Runtime acceptance records must retain this distinction through recovery, export and later reuse.

## Reputation

Reputation is an evidence-based assessment of an agent's execution outcomes in a relevant configuration and task context. Accepted-but-unconfirmed results, self-assessment and peer agreement do not increase it. It supports selection; it does not grant authority to bypass constraints. Resource use and uncertainty remain available alongside quality outcomes rather than disappearing into an unexplained rank.

An agent ID, configuration version and observation ID serve different purposes: identity preserves attribution, configuration identifies the execution conditions, and observation identity prevents repeated credit for the same result. A transport failure or missing result alone does not establish a competence failure.

## Supporting concepts

- **Oracle** is an optional outside consultation under discussion. It would supply advice to the same decision process, with attributed resource use; it would not constitute automatic acceptance or unrestricted control.

Identity, state and policy are deliberately separate: entity definitions explain what exists, [principles](../../intent.md#принципы) state product rules, and the [detailed policy](../architecture/team-and-effort-policy.md) explains how selection and adaptation should respect those rules.
