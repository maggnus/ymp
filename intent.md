# Intent

The approved [self-organizing team architecture](ymp-docs/self-organizing-team-domain-model.md)
is the single source of architectural scope and domain rules. This document
explains its product purpose and user experience; it does not define a competing
architecture or imply that the complete target is already implemented.

## Purpose

YMP helps a user obtain a checked result from a task that benefits from reasoning,
tools and cooperation between AI agents. It must provide an interactive terminal
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
protect separate capacity for verification and reporting, including a bounded
report correction. If model-assisted reporting cannot finish, deliver a truthful
deterministic report from recorded state. Add work only for an unfinished
requirement, missing evidence or a justified alternative. Parallelism is useful
when dependencies and effective workspace permissions permit it.

The user may constrain participants, models, supported reasoning settings and
resource limits. The system must preserve these choices or state why execution
cannot continue. Unknown native metadata remains unknown.

### Acceptance has a recorded basis

Results are versioned and independently reviewed against the task's criteria.
Criterion satisfaction, acceptance and confirmation grade are separate decisions.
The kernel computes satisfaction using A8's belief threshold and applicable
evidence-class coverage. A forecast alone cannot satisfy a criterion; unrelated
or superseded evidence does not count. A7 separately requires independent approval
and assigns `Unconfirmed`, `Discriminated` or `Confirmed` from the actual evidence;
later consequences may produce `Refuted`. Agent agreement and self-assessment do
not create confirmation or competence credit. The default credit policy admits
positive observations only for `Confirmed(*)` outcomes.

Evidence identifies the result, criteria, check source and environment it covers.
A changed result requires an appropriate new check. Statements about execution,
browser behavior or causal improvement must match the evidence actually collected.

### Failure changes the next decision

Diagnose failed execution, unsuitable capabilities, defective checks and incorrect
results separately. Retry only when the next attempt has a stated purpose.
Retain completed work, consumed resources and uncertainty about side effects.
Cancellation does not imply that every external effect has been reversed.
Revoking authority does not prove that an invocation stopped writing. Conflicting
workspace access remains blocked until ended-effects or never-started evidence
is validated; usage is settled independently and conservatively. Resuming a
recoverable session preserves its task, results, budget, expenses and holds, and
cannot duplicate an unresolved invocation or reactivate a revoked grant.

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

The initial codebase establishes the contracts needed to build these capabilities.
Features become delivered only when their behavior and limitations are verified.
