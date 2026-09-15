# Intent

The approved [self-organizing team architecture](ymp-docs/self-organizing-team-domain-model.md)
is the single source of architectural scope and domain rules. This document
explains the product purpose and user expectations. A gap between these and the
model is a reason to discuss a model revision, not to silently discard the
expectation or invent a competing architecture. Neither document's approval
implies that the complete target is already implemented.

## Purpose

YMP is an innovative product for exploring and enabling useful self-organization
among AI agents. Participants can initiate contributions, take and transfer
commitments, exchange context, raise objections, and adapt their team and plan
within kernel-enforced authority and resource limits. User tasks provide an
observable setting for discovering when these mechanisms help, when they fail,
and what they cost. A fixed task-solving workflow alone does not fulfill this purpose.

The product must also help users obtain checked results from that cooperation.
It must provide an interactive terminal
user interface (TUI) as its primary human-facing interface for stating the goal,
observing progress and resolving decisions that require the user.

The user should not need to supervise individual agent turns or manually carry
information between participants. YMP must make its actions and limitations
understandable while preserving the user's authority over scope and resources.

## Intended outcomes

1. Improve the likelihood of solving difficult tasks within an agreed resource envelope.
2. Reduce unnecessary work at comparable result quality.
3. Shorten time to a checked result where independent work can usefully overlap.
4. Reduce intervention needed to diagnose failures and continue valid work.
5. Improve later decisions through retained, qualified experience.

These are product objectives to evaluate. Agent count, message volume and a
successful process exit do not establish that any objective has been achieved.
Experiments must reveal the conditions and limits of useful self-organization;
negative or inconclusive findings remain valid outcomes rather than hidden failures.

## Product principles

### A complete interactive terminal experience

The TUI is part of the intended product scope. It must support the complete user
journey: define and refine a task, observe its progress, answer clarifications,
interrupt ongoing work, resume recoverable work, and inspect the resulting
artifacts, checks and report.

The user's goal and current result must remain easy to follow. Team activity,
assignments, resource use, acceptance criteria and decisions must be accessible
without requiring the user to supervise individual agent turns or reconstruct the
session from logs.

The TUI must display recorded session state, unmet criteria and uncertainty
truthfully. Its controls use the same kernel authority as other application
interfaces; presentation cannot grant permissions or declare a result accepted.
Its layout and detailed commands may evolve. The main product focus is useful
agent communication and self-organization, exposed through a simple working
interface before visual polish.

### One user task, one accountable session

A task has an explicit goal, constraints and acceptance criteria. Its session
retains the decisions, assignments, evidence and outcomes needed to explain what
happened. Replanning preserves the goal and records substantive changes.

### Agents participate; the kernel holds authority

An agent has its own identity, instructions and context. A provider supplies its
execution environment; a model supplies inference. These are distinct concepts.
Roles and permissions apply to an assignment, not permanently to an agent.

Agents may propose tasks, approaches, commitments and objections. The kernel alone
admits work, issues authority, settles resources and records final acceptance.
An agent's claim does not grant permission or establish that a criterion is met.

### Work follows need

Choose the method and team for the task. Start with bounded useful work and
preserve capacity for verification and a bounded explanation of the outcome.
If model-assisted reporting cannot finish, deliver a truthful report from
recorded facts without another model call. Add work only for an unfinished
requirement, missing evidence or a justified alternative. Parallelism is useful
when dependencies and effective workspace permissions permit it.

The user may constrain participants, models, supported reasoning settings and
resource limits. The system must preserve these choices or state why execution
cannot continue. Unknown native metadata remains unknown.

### Key mechanisms are replaceable for experiments

Experiments can replace the implementations of key decision mechanisms through
explicit interfaces, including assessment, contribution selection, team formation,
resource allocation, reputation and knowledge use. Adjusting a coefficient is
only one form of experimentation; a different algorithm must be connectable to
the same consumer. Record the implementation, version, effective parameters and
basis so observed differences can be explained. Preserve kernel authority and
earlier history when policies change.

### Acceptance has a recorded basis

Results are versioned and independently reviewed against the task's criteria.
Criterion satisfaction, acceptance and confirmation grade are separate decisions.
The kernel assesses criteria using the model's belief and evidence rules, requires
independent approval for acceptance, and records the strength of the actual basis.
A forecast alone cannot satisfy a criterion, and unrelated or superseded evidence
does not count. Agent agreement and self-assessment do not create confirmation or
competence credit. Experiments may select different CreditPolicy implementations
within the model's rules; their observations must remain attributable to that choice.

Evidence identifies the result, criteria, check source and environment it covers.
A changed result requires an appropriate new check. Statements about execution,
browser behavior or causal improvement must match the evidence actually collected.

### Failure changes the next decision

Diagnose failed execution, unsuitable capabilities, defective checks and incorrect
results separately. Retry only when the next attempt has a stated purpose.
Retain completed work, consumed resources and uncertainty about side effects.
Cancellation does not imply that every external effect has been reversed.
Revoking authority does not prove that further writes are impossible. Conflicting
access and financial reservations have separate release conditions. Recovery
preserves work and accounting without duplicating unresolved execution or
reviving revoked authority; concrete controls must explain their actual guarantees.

### Experience remains accountable

Retained knowledge has a source, scope, version and supporting evidence. It can be
corrected, superseded or withdrawn. Reputation uses qualified outcomes, not
confidence claims or participation volume. Learning runs after report delivery,
uses only remaining unprotected resources, and cannot delay that delivery.

## Boundaries

YMP is intended to run as a native executable on macOS and Linux. Installed agent
environments may supply their own execution runtimes and native authentication.
YMP must not copy credentials into its own state or invent supported model settings.

Workspace guarantees must describe what the execution environment can enforce.
A declared path list or a file copy is not, by itself, an isolation guarantee.
User files and external effects require explicit ownership and a truthful recovery
contract.

This iteration starts with a new implementation. Previous source material does
not count as delivered functionality. Features become delivered only when their
behavior and limitations are verified in this iteration.
