# ADR 0002: agent identity and autonomous reasoning choices

Status: documented product direction derived from [intent.md](../../intent.md). The owner requested bounded dynamic teams, fixed user constraints and assignment-level effort to be documented on 2026-09-12. Implementation remains paused during YMP-010. Coordination topology, numerical defaults and optimization-policy performance remain open; the Oracle is an optional proposal.

## Decision

The person supplies a goal and constraints. A user task owns a session; YMP selects its method, team, executors, models and effort jointly within a shared budget, using agent proposals and verified experience. Membership is dynamic by default within recorded bounds. Explicit user pins constrain size, roster, models or effort through the same policy. Agents can accept responsibility, propose plan changes and request reassignment without requiring manual internal management.

An agent is the working unit. Provider installation, model selection and supported reasoning controls are its execution resources. The current AgentProfile and agent-ID team model are a starting point; no second orchestrator, provider-as-member abstraction or global agent daemon is required.

| Concept | Contract |
| --- | --- |
| Available pool | Agents and capabilities supplied by enabled native providers and configured profiles |
| Agent identity | Stable ID, display name, instructions and execution binding; multiple agents may share a provider/model |
| Session team | Actual participants selected for the task, with membership and configuration history |
| Assignment | Work chosen for an agent, including the requested native model/reasoning settings |
| Shared board | Team-wide and addressed communication, with persistent findings and decision evidence |
| Experience | Verified outcomes tied to their agent/configuration, plus incrementally accumulated general knowledge |

Planning, execution and review are assignments; a provider name does not impose a permanent role. The earlier unconditional no-leader requirement has been relaxed: roles and coordination are chosen for the task. A temporary planner, peer proposal mechanism or other coordinator may inform the planning policy, while the runtime validates decisions and enforces constraints. See the [domain terminology](../product/entities.md) for identity and scope definitions.

## Native reasoning controls

Effort controls requested reasoning intensity. Some providers separately expose thinking enablement, adaptive behavior or an allowance. Offer and transmit only controls supported by the selected native provider/model. The installed [capability audit](../research/evidence/effort-capabilities.json) records the initial protocol mechanisms; it does not establish a universal ladder or equal meaning for identically named levels.

The user defines constraints, YMP's planning policy selects a model and effort for each assignment, and the runtime validates and admits the resulting invocation. Agent proposals can inform the choice. An explicit fixed effort is a constraint with one allowed value. A profile's configured or inherited default is a starting condition when no assignment choice is supplied; it does not override that choice or create a second session-wide controller. Reject unsupported decisions or revise them using the actual capability set; do not silently substitute a different tier.

Distinguish requested settings, values sent or acknowledged by the native adapter, and values the provider actually reports. An inherited default may be unresolved, and an accepted setting is not a measurement of internal reasoning-token use. Record effort choices explicitly so a native session cannot accidentally inherit a previous assignment's level.

Specific policies such as low for every bid or xhigh for every final review remain hypotheses. Autonomous choice under user constraints is required by the product intent; proving the superiority of a particular policy is a separate experiment.

## Team and resource policy

The [detailed policy](../architecture/team-and-effort-policy.md) distinguishes pool, membership and active invocations, defines fixed size separately from a pinned roster, and gives one authority model for joint team/model/effort selection. Its operational rules include the following; no token-saving or optimality result is claimed.

- Opening the application or listing the eligible pool does not require inference. Activating a participant requires useful work and a resource allowance. Do not solicit a proposal and bid from every available agent as a mandatory startup procedure.
- Admission accounts for already spent resources, concurrent reservations and a protected allowance for independent verification and final reporting. Starting work must leave a feasible way to check its result. Reassignment and failure do not reset the shared accounting.
- Bound context, native model/tool turns, output where supported, elapsed time and concurrency as well as ymp invocation counts. Effort alone is not a token limit. State which adapter limits are enforceable and which are estimates; unknown usage cannot support a strict token or currency guarantee.
- A failure prompts diagnosis. Missing evidence, infrastructure errors, a poor method and a verified reasoning error need different responses. Increased effort is one possible decision, with a stated reason and an admitted allowance; there is no unconditional effort increment or unbounded escalation chain.
- Reuse verified intermediate work and publish knowledge incrementally through ordinary work and review results. Recording each finding must not require another model invocation.

## Useful concurrent work

The pool size, the participating session team and the currently executing assignments are different quantities. A pool of twenty agents must not force twenty model calls. A session with several independent ready assignments must also be able to execute them concurrently within its constraints. Any participant may propose or accept useful work, while the runtime records authoritative assignments. The coordinator topology must not accidentally serialize all independent work.

At the audited implementation baseline, the ready-task selection uses take(1), explicitly serializing execution and verification even though the code uses JoinSet and a configurable parallel limit. Removing that selection limit alone would not establish correct concurrent execution: dependencies, agent occupancy, shared resources, in-flight budget reservations and verification eligibility must all be respected. YMP-115 tracks this missing capability.

Waiting is justified by a dependency, resource constraint or lack of useful work. Keeping agents busy is not an objective by itself. Record enough scheduling evidence to distinguish unavoidable waiting from ready work that was unnecessarily serialized, and assess accepted contributions, elapsed time and total resource use rather than rewarding message volume.

## Open option: external Oracle

An Oracle could provide a bounded outside consultation when the team encounters a documented impasse or needs to compare consequential alternatives. This is an option under discussion, not an accepted mandatory component or a permanent team leader. External means outside the current working conversation; it need not introduce another provider or authentication mechanism.

A consultation would receive the goal, relevant evidence, attempted approaches, available execution capabilities and remaining budget. It would return a proposed next action and a way to check it. The team would record how it used that advice; the runtime would continue to enforce constraints and charge the consultation to the shared budget. Advice would not count as verified knowledge or final acceptance merely because it came from the Oracle.

The initial proposal is one bounded consultation for an unresolved issue, with no recursive Oracle calls and no repeat without new evidence and a fresh admission decision. This limits consultation frequency; an adapter must still bound the work inside the invocation. An Oracle that authorizes every assignment would serialize the team and consume resources before useful work begins. It cannot substitute for concurrent task execution or solve effort selection merely by relocating it to another model.

The theoretical motivation is to compare the expected decision improvement with the resources consumed by additional computation, as described in [Russell's account of rational metareasoning](https://aima.eecs.berkeley.edu/~russell/research-bo.html). Applying that principle to an ymp Oracle is a design inference; the current traces do not establish consultation value or calibrated thresholds. Verified consultation outcomes could later inform when to seek advice, with unsupported or inconclusive outcomes retained as such.

## Identity and history

Provider bindings do not replace agent IDs in membership, board authorship, tasks or usage. Renaming an agent changes a label, not its identity. Clone a profile with a new ID when two independently acting copies are needed.

Capture a participant's settings when it joins. Record membership or assignment-setting changes as new decisions, preserving previous attribution. Later profile edits do not rewrite historical participants. Model, instructions and reasoning changes define distinguishable execution variants for reputation while usage remains attributed to the same agent.

Existing configurations retain their IDs and native defaults; do not silently rename user profiles or discard legacy experience whose effort was unknown. Authentication remains native-provider-owned.

## Incremental knowledge

Board data, intermediate findings and verified results are persisted during work. General reusable knowledge and earned reputation are available to later sessions with their source evidence and verification state. Incorrect or outdated knowledge can be corrected or superseded by newer verified information while preserving history. Its production must not depend exclusively on a final post-success learning phase. Keep project-specific facts and unverified observations identifiable rather than silently presenting them as general verified knowledge.

## Verification, resilience and explanation

Independent verification precedes final acceptance. An individual agent failure does not invalidate completed verified work or authorize blind replay of side effects. Preserve progress, inspect unfinished work and let the team revise assignments within its remaining budget.

Record brief decision rationales, supporting evidence, resource use and setting changes so a person can follow the work without directing every internal step. This is decision provenance, not a requirement to expose private internal reasoning traces.

## Delivery

- **YMP-109:** complete the available agent pool and distinguish membership from active invocations.
- **YMP-111:** expose assignment-level native reasoning controls and explicit user pins.
- **YMP-110:** apply bounded dynamic or fixed team constraints and joint executor/model/effort selection.
- **YMP-112:** coordinate commitments, plan changes, additional participants and reassignment through the board.
- **YMP-115:** execute independent assignments concurrently within dependencies, resource constraints and the shared budget.
- **YMP-102:** enforce the shared resource budget and supplied constraints.
- **YMP-113:** accumulate task information and general knowledge incrementally across sessions.
- **YMP-114:** correct or supersede knowledge using newer verified evidence.
- **YMP-101:** record and export decision/configuration evidence for improvement and auditing.
- **YMP-203:** evaluate particular effort and coordination policies; this experiment does not gate the existence of autonomous choice.

The earlier [UI review](../research/evidence/agent-profile-ui-contract.md) predates this policy clarification. Any UI contract revision or implementation must use Claude Opus 5 max and the current terminology; no UI work is included in this documentation change. Application metadata remains under ~/.ymp2.
