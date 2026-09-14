# Roadmap

## Product definition

The [intent](../intent.md), [domain vocabulary](domain.md),
[architecture](architecture.md) and [foundation contract](foundation.md) define the
initial direction. Product hypotheses remain hypotheses until evaluated.

## 1. Executable foundation — planned

Deliver four compiling crates, validated task contracts, a revisioned session
journal, a kernel lifecycle and a truthful CLI entry point. The foundation contract
contains its acceptance conditions. This outcome performs no agent execution.

## 2. Durable session state — planned

Implement a persistent `Journal` adapter with atomic append, schema versions and
restart checks through the kernel consumer. Preserve exact task content, event
order and typed failures. Define storage location and lifetime before introducing
application-owned files.

## 3. Bounded native execution — planned

Introduce `Registry`, `Treasury`, `WorkspaceGuard`, `Gatekeeper` and
`ExecutionBackend` through one admitted execution scenario. Discover native
identities, preserve authentication and settings, account for every invocation,
and distinguish cancellation from confirmed termination. Use scripted providers
for unattended checks.

## 4. Checked task delivery — planned

Introduce result versions, retained evidence and `AcceptanceAuthority`. Demonstrate
independent review and an honest accepted/unconfirmed outcome before adding credit.
Deliver a report tied to the checked result, with explicit unmet criteria.

## 5. Useful cooperation and terminal interaction — planned

Introduce contribution selection, `Arbiter`, commitments and addressed communication.
Proposals must change admitted work when appropriate. Demonstrate useful overlap
and conflict exclusion, then expose observable session controls in the terminal UI.
Public MCP uses the same trusted application commands over stdio.

## 6. Qualified experience — planned

Introduce `ExperienceVault` with scoped knowledge, qualified observations and
correction. Test whether experience improves later work using disjoint tasks and
resource accounting that includes learning. Keep insufficient data explicit.

## Evaluation

Evaluate quality, resources, useful concurrency, human intervention and the value
of experience separately. State the task set, constraints, evidence and limits of
each conclusion. A working demonstration does not establish comparative advantage.
