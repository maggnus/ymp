# Roadmap

Canonical development status, ownership, dependencies, acceptance criteria, and
evidence live only in the linked task records. Use the
[development task workflow](development-tasks.md) to select work; this roadmap
describes product outcomes and does not duplicate task status.

## Product definition

The [intent](../intent.md), [domain vocabulary](domain.md),
[architecture](architecture.md), and [foundation contract](foundation.md) define the
initial direction. Product hypotheses remain hypotheses until evaluated. See
[DEV-0001](tasks/records/foundation/DEV-0001.json).

## 1. Executable foundation

Four crates provide validated task contracts, an atomic revisioned in-memory
journal, open/read/cancel through the kernel, and a truthful CLI entry point.
Acceptance tests cover invalid domain input, stale and duplicate operations,
concurrent append, and malformed history. This outcome performs no agent execution
and provides no persistent storage. See
[DEV-0002](tasks/records/foundation/DEV-0002.json).

## 2. Durable session state

Define the durable `Journal` contract before selecting a storage approach, then
implement a persistent adapter with atomic append, schema versions, corruption
handling, and restart checks through the kernel consumer. Preserve exact task
content, event order, and typed failures. See
[DEV-0004](tasks/records/persistence/DEV-0004.json) and
[DEV-0005](tasks/records/persistence/DEV-0005.json).

## 3. Bounded native execution

Introduce `Registry`, `Treasury`, `WorkspaceGuard`, `Gatekeeper`, and
`ExecutionBackend` through one admitted execution scenario. Discover native
identities, preserve authentication and settings, account for every invocation,
and distinguish cancellation from confirmed termination. Use scripted providers
for unattended checks. See [DEV-0006](tasks/records/execution/DEV-0006.json).

## 4. Checked task delivery

Introduce result versions, retained evidence, and `AcceptanceAuthority`. Demonstrate
independent review and an honest acceptance decision, including an
accepted-but-unconfirmed outcome, before adding credit. Deliver a report tied to
the checked result, with explicit unmet criteria.
See [DEV-0007](tasks/records/verification/DEV-0007.json).

## 5. Useful cooperation and terminal interaction

Introduce contribution selection, `Arbiter`, commitments, and addressed
communication. Proposals must change admitted work when appropriate. Demonstrate
useful overlap and conflict exclusion, then expose observable session controls in
the terminal UI. Public MCP uses the same trusted application commands over stdio.
See [DEV-0008](tasks/records/coordination/DEV-0008.json).

## 6. Qualified experience

Introduce `ExperienceVault` with scoped knowledge, qualified observations, and
correction. Test whether experience improves later work using disjoint tasks and
resource accounting that includes learning. Keep insufficient data explicit. See
[DEV-0009](tasks/records/experience/DEV-0009.json).

## Evaluation

Evaluate quality, resources, useful concurrency, human intervention, and the value
of experience separately. State the task set, constraints, evidence, and limits of
each conclusion. A working demonstration does not establish comparative advantage.
