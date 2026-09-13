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

YMP-122 implements the first common execution interface. The owning tasks above implement their respective interfaces, and YMP-121 verifies their integrated contracts. This document records the requirement and responsibility split; it is not evidence that every extension point already exists.
