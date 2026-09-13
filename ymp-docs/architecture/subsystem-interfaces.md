# Replaceable subsystem implementations

Owner direction, 2026-09-12: favor interfaces and plugins so alternative subsystem implementations can be tried without rewriting the runtime. This extends the delivery plan; it does not change the approved intent.

## Boundary

Use small, typed Rust interfaces at actual replacement points. Supply implementations when constructing the engine, with the built-in implementation as the default. Configuration may select registered implementations when alternatives exist. Each selected implementation has an ID and version recorded with the decisions or evidence it influences.

The initial extension mechanism is compiled Rust modules and dependency injection. A dynamic library ABI, plugin installer, remote plugin service and generic dependency container are not required for this release. An in-process extension is trusted executable code, not a security sandbox.

## Extension points and delivery ownership

| Subsystem | Replaceable behavior | Runtime-owned contract | Task |
| --- | --- | --- | --- |
| Provider execution | Native adapters and scripted or experimental execution backends | Captured identity/settings, admission, cancellation, resource controls, scoped grants, usage/provenance and capability-safe diagnostics | YMP-122 |
| Team and execution policy | Ranking eligible agents and proposing team/model/effort choices | User pins, provider capabilities, independence, membership bounds and resource admission | YMP-110 |
| Coordination policy | Proposing commitments, revisions and reassignment | Current plan/task versions and runtime-only state transitions | YMP-112 |
| Knowledge retrieval and update | Finding applicable entries and proposing additions or corrections | Scope, source/version resolution, context allowance, evidence binding and lifecycle transitions | YMP-113, YMP-114 |
| Confirmation checks | Executing trusted checks or typed assertions | Actual result/input capture, criterion coverage, freshness, independent acceptance and reputation eligibility | YMP-117 |
| Resource allocation | Proposing startup, execution and review allowances | Atomic counters/reservations, captured user ceilings, preserved historical spend and honest unknown usage | YMP-102, YMP-110 |

Policy implementations return proposals or observations. A returned `approved` flag, an implementation's name or its own claim of authority cannot commit an assignment, confirm a result or award reputation. Core validation remains outside the replaceable algorithm. Do not expose a mutable store as a shortcut for policy decisions.

Keep records and interface inputs domain-specific. Do not reduce all subsystem inputs to arbitrary JSON or introduce a trait around every helper. Extend existing package boundaries before adding packages. Interface design belongs to the task that owns the behavior, so this document does not freeze speculative method signatures.

Owner direction, 2026-09-13: unify library use across features. A shared responsibility should use
the existing implementation and adapter where suitable. A new or overlapping dependency needs a
demonstrated capability gap and an explicit account of integration and maintenance costs. A
replacement should identify which existing implementation and dependencies it retires; parallel
implementations are an exception to justify, not the default. Libraries providing distinct roles
within one solution, such as parsing, grammar data and display adaptation, are complementary.

## Evidence for replacing an implementation

Contract tests exercise at least the built-in implementation and a deliberately different test implementation through the actual consumer. They must show that replacement changes the intended behavior while preserving runtime constraints, attribution and failure handling. A mock that only mirrors the built-in implementation is insufficient.

Comparisons record implementation IDs/versions, configuration, workload, resources and confirmation coverage. The same evaluation scenarios can then compare strategies without changing their expected result to fit a strategy. Scripted replacements establish protocol behavior; claims about quality or efficiency still require the separately authorized comparative experiments.

## Research through alternative strategies

Owner clarification, 2026-09-13: team scaling and decision algorithms should be
investigated by implementing shared interfaces and comparing strategies, without
repeatedly rewriting the execution kernel. The owner subsequently selected
`/team` as the only UI location for the discussed policy choices; see
[team and agent surfaces](team-and-agent-surfaces.md).

The owner subsequently cautioned against parameterizing every policy. A
replaceable implementation, its algorithm parameters and public user settings
are separate concerns. Keep replacement points available for research without
requiring configuration fields or UI controls for all of them. Introduce a
parameter only for a demonstrated implementation or experiment need; record the
parameters actually used, and do not invent them for parameterless strategies.
Owner goals, resource limits and explicit team commands remain separate from
internal strategy settings. YMP-146 does not include a universal configuration
framework or an interface exposing all policies to the user.

Reuse the existing `AllocationPolicy`, `ResourceAllocationPolicy` and other
subsystem contracts. `BoardProposalPolicy` currently orders pending proposals;
it is not a replaceable implementation of the complete session workflow.
`ConfirmationChecker` executes checks; it does not independently control
acceptance. Phase transitions and provider-failure exits still include behavior
inside the engine. Recovery work under YMP-146 must identify its actual missing
replacement boundary and avoid adding a generic duplicate policy interface.

Proposed extensions should receive typed events and versioned state, then return
bounded proposed actions with their basis. Runtime validation, atomic admission,
resource accounting, owner constraints, active-write coordination and acceptance
evidence remain shared. A strategy requiring model reasoning must obtain an
ordinary admitted, recorded invocation; a synchronous proposal method is not
permission for hidden inference or unaccounted provider calls.

Record the selected strategy identity, version, explicit configuration and the
effective policy revision alongside the decisions it influences. A later strategy
change must not relabel earlier decisions or silently alter an active assignment.
Strategy configuration belongs to the session, while its UI control belongs only
to `/team`. No additional selector/default control is added to `/settings`.

Prove each newly introduced replacement point through two materially different
implementations using the same consumer and acceptance conditions. Compare actual
decisions, confirmed outcomes, elapsed time, resource use, recovery attempts and
unresolved work. Replaying recorded input compares decisions on that input;
complete executions are still needed to evaluate their downstream effects.
Existing successful substitution evidence should be reused. Do not require a new
alternative implementation for every already-proven interface as part of YMP-146.

Compiled implementations inside the standalone executable remain the initial
mechanism. Register and configure alternatives when they exist; dynamic loading,
an external plugin runtime and broad policy-framework redesign are not implied
by this research direction.

YMP-122 implements the first common execution interface. The owning tasks above implement their respective interfaces, and YMP-121 verifies their integrated contracts. This document records the requirement and responsibility split; it is not evidence that every extension point already exists.
