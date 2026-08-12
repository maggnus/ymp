# W1-EXP-01b matched-budget study protocol, version 1

## Status and authority

This protocol is preregistered before any primary ymp outcome. The canonical machine-readable
specification is `manifest-v1.json`. Its SHA-256 sidecar and the compiled `ymp-corpus` freeze
prevent a rewritten sidecar from authorizing a changed manifest.

The specification supplies technical evidence only. It cannot authenticate an owner decision,
approve a runtime profile, authorize provider expenditure, or declare a human audit valid. The
project owner separately approves the exact manifest digest, the eventual expanded corpus root,
admitted runtime-profile digests, any project-wide currency ceiling, and primary collection.

The owner-approved initial corpus root is
`e4e886bf1dfd433342f3f10f34c086415462908d60f5a53d1448824c9fe3bfc7`.
Its four packages are a reproducible initial edition, not an adequate sample. No external-package
ceiling exists. No project currency ceiling is configured.

## Conditions

The experiment estimates independently verified accepted-result probability in the pre-outcome
`decomposable` and `strongly_sequential` strata. Every matched block contains:

1. `single_strong`: one participant receives the entire arm opportunity and selects one candidate
   before the protected query.
2. `independent_best_of_n`: eleven producers work independently with one quantum each. A separate
   selector receives one quantum, sees candidate artifacts and public evidence only, and commits
   one assessment before identities, rationale, messages, arm labels, or another assessment.
3. `coordinated_ymp`: at most eleven locally recruited participants may negotiate, communicate,
   delegate, abstain, or work alone within the same total route and verification opportunity.

Participant count and communication are treatment properties. Model-route calls, token ceilings,
elapsed-time opportunity, request concurrency, protected queries, runtime profile, prompt policy,
source, contract, environment, and oracle are matched within a block.

## Randomization and repetition

The randomization unit is one `(task_id, repetition)` matched block. Each block receives each
condition once. The block seed is the unsigned big-endian integer represented by the first eight
bytes of SHA-256 over UTF-8
`"ymp-study-seed-v1\0" || manifest_sha256 || "\0" || task_id || "\0" || repetition`.
Launch order sorts condition identifiers by lowercase SHA-256 over the same framed fields with
domain `ymp-study-order-v1` and one final `"\0" || condition_id`. Each arm records its zero-based
launch position, which must map back to its condition in that exact order. Runtime-profile
assignment is balanced by the same predeclared digest order; all conditions
in a block use the same profile digest and its one exact bound model route. Each arm repeats the
block profile digest, and its sole accounted route equals the block route digest. Different
admitted profile cells remain separate routes in the report and are never pooled by currency
without their route identity.

Every task has five stochastic repetitions with preassigned seeds. Five follows
`ceil(log(1 - 0.95) / log(1 - 0.50)) = 5`: at least 95% probability of observing an event that
occurs independently with probability one half. Repetitions characterize stochastic variation
but never count as additional distinct tasks. The task is the clustering and generalization unit.

Structural labels come from the public contract and source dependency structure before outcomes.
A pre-outcome correction creates a new corpus edition; no label changes after primary collection.

## Resource scale and best-of-n

The planning alternative gives one independent producer a one-quarter viable-candidate
probability. Requiring 95% pool coverage yields
`ceil(log(1 - 0.95) / log(1 - 0.25)) = 11`, so the baseline is `best-of-11`. One further quantum
funds its blinded selector. Every condition receives twelve quanta.

The call scale uses the same explicit geometric coverage rule: if another call is independently
needed with probability one half, `ceil(log(1 - 0.95) / log(1 - 0.50)) = 5` calls cover that
continuation process with at least 95% probability. One quantum therefore contains five fully
reserved provider requests. Each request reserves at most 131,072 input tokens, 131,072
cached-input tokens, 16,384 output tokens, 16,384 reasoning-output tokens, and 600,000
milliseconds. Every arm therefore receives:

- 12 attempt quanta and 60 calls;
- 7,864,320 input and 7,864,320 cached-input tokens;
- 983,040 output and 983,040 reasoning-output tokens;
- 36,000,000 milliseconds;
- at most 11 simultaneous requests; and
- one protected query against one exact selected candidate.

Each request reserves its complete maximum before starting. No request starts when the remaining
reservation is insufficient. Provider-reported excess from an already started request is fully
charged to its arm, blocks later starts, and remains in intent-to-treat analysis. Unrecorded excess
or unreproducible usage makes the comparison noncompliant.

For every route the record includes calls; all four token categories; elapsed time; observed
parallelism; protected queries; errors; retries; provider cost and currency; and in-flight excess
for calls, token categories, and cost. The currency ceiling has project scope only and is currently
`null`. Actual cost is mandatory even without a ceiling.

## Power, corpus breadth, and expansion

The minimum practically useful absolute effect is exactly `1/8 = 0.125`: at least one additional
accepted result per eight resource-matched task opportunities. It never changes after a negative,
small, or noisy result. The power alternative is exactly `1/4 = 0.25`, the smallest preregistered
alternative separated from the useful-effect boundary by another `1/8`. This separation, rather
than a post-outcome variance estimate, determines sample size.

Familywise one-sided alpha 0.05 is divided between two claimable strata, yielding 0.025 per stratum.
Within a stratum the claim is an intersection requiring superiority over both baselines, so those
contrasts need no additional type-I split. The analysis unit is the paired task-cluster difference,
which is bounded by `[-1, 1]` without any covariance or asymptotic-normality assumption.

Ninety-percent joint power across the four broad reliability contrasts requires 97.5% per contrast
by the union bound. The fixed-sample Hoeffding requirement is
`2 * (sqrt(log(1 / alpha)) + sqrt(log(1 / beta)))^2 / separation^2`. With a `1/8`
separation and alpha and beta both 0.025 this gives 1,888.706, rounded upward to 1,889 distinct
tasks per stratum.

The communication claim has six required broad contrasts and uses 98.33333333333334% power per
contrast, or beta `1/60`. The same bound gives 1,991.153, rounded upward to 1,992 distinct
task-linked episodes per stratum. Corpus breadth is the larger requirement: 1,992 tasks per
stratum. The initial edition is short by 1,990 tasks in each stratum, 3,980 in total.

Expansion uses no primary outcome. Within each deficient stratum, packages are considered in
ascending `(repository_url, source_commit, task_id)` order from a preregistered candidate list and
must pass every W1-EXP-01a admission check. Before primary collection, an unusable package is
replaced by the next eligible same-stratum package. After the first primary route call, no package
may be added, removed, substituted, or relabelled.

Execution freezes only after both strata contain at least 1,992 admitted packages, an exact expanded
root exists, and the owner approves it. This task adds no real package.

## Outcomes, exclusions, and stopping

Permitted terminals are exactly `accepted`, `exhausted`, `abstained`, `cancelled`, and
`infrastructure_error`. The primary value is one only for `accepted` with exact candidate digest
and a protected-verifier `passed` record bound to the frozen contract, environment, and oracle.
Every other terminal is zero. Verifier failure is not abstention; quiescence is not acceptance;
infrastructure failure is not silently discarded.

Every arm records a positive candidate-selection commit sequence. If it spends its protected
query, the protected-result reveal sequence must exist and be strictly later; without a protected
query, a reveal sequence is forbidden. The independent selector assessment commit is the same
selection event and precedes every identity reveal. A sequence equality or reversal makes the
comparison noncompliant.

Allowed exclusions occur before randomization only when:

- package integrity or admission fails;
- runtime-profile capability preflight fails;
- common disposable-environment preflight fails before any arm starts; or
- an exact duplicate schedule entry is found before assignment.

There are no post-randomization exclusions. Provider error, timeout, cancellation, missing
candidate, budget exhaustion, selector failure, and participant failure remain in the assigned
arm. Missing accounting, unequal opportunity, early selector disclosure, oracle-integrity loss,
or digest mismatch invalidates the comparison instead of deleting the affected arm.

Each run stops at a permitted terminal, a nonexchangeable resource limit, elapsed-time limit, its
single protected query, or authorized project cancellation. The study has a fixed sample with no
efficacy, futility, cost, conditional-power, promising-zone, or size-reestimation look. A later
owner-set project ceiling may prevent new blocks from starting, but cannot erase blocks, change the
target sample, or make a partial sample positive.

## Primary analysis and final rule

Acceptance is averaged across five repetitions within each task, then equally across tasks in a
stratum. Contrasts are `coordinated_ymp - single_strong` and
`coordinated_ymp - independent_best_of_n`. Standard errors cluster by distinct task. Reports give
arm rates, absolute differences, distribution-free two-sided 95% Hoeffding intervals, and explicit
one-sided alpha-0.025 lower and upper decision bounds. The empirical standard error is descriptive
only. Reports also include cost, usage, latency, parallelism, queries, infrastructure failure,
cancellation, abstention, and audit results by condition and stratum.

A stratum supports the reliability mechanism only when:

- its complete fixed sample is compliant;
- both one-sided alpha-0.025 lower bounds exceed `1/8`;
- the measurement and audit conditions below hold; and
- the owner confirms the evidence belongs to the authorized execution.

Two supported strata permit only the declared two-stratum claim; one supports only that restricted
claim. If the upper alpha-0.025 bound against independent `best-of-11` is below `1/8`, the mechanism
claim is rejected for that stratum. Every other complete result is `inconclusive`. Negative and
inconclusive results complete the evidence obligation without changing success.

## Measurement reliability and false acceptance

Before collection, every exact runtime/profile/route combination must pass fixtures for version,
effective configuration, subagent suppression, structured completion, usage categories, retry
identity, timeout, cancellation, descendant cleanup, protected-query identity, and in-flight
excess. Missing usage or provider cost makes the profile ineligible.

Mandatory accounting and digest/assignment/blinding integrity require 100% completeness. A positive
claim requires each arm's upper one-sided 97.5% infrastructure-or-cancellation bound no greater
than `1/32`, one quarter of the useful effect, and the upper bound on an absolute arm difference no
greater than `1/64`.

Every verifier-accepted candidate enters a blinded human audit; this is a census. The auditor sees
the contract, artifact, and permitted evidence, but not condition, producer, rationale, messages,
other assessments, or aggregate. False acceptance receives a 95% binomial interval by condition
and stratum. Every upper bound must be no greater than `1/128`, one sixteenth of the useful effect.
The tool validates audit presence, order, and digests but cannot confer auditor or owner authority.

## Communication interventions

Communication analysis is separate from acceptance and allocation. Private chain-of-thought is not
collected. At most one episode is selected per distinct task: a message delivered after at least
two active participants and before candidate selection or a protected query. The smallest
`SHA-256(manifest_sha256 || task_id || message_id)` wins without inspecting content or outcome.
Before fresh runs, the receiver's typed next-action endpoint and seeds are committed.

Each episode has five fresh repetitions of all eight matched-budget variants:

- original message;
- no message;
- neutral payload padded to the same byte length;
- length-bucketed message shuffled from another task or sender;
- identical underlying evidence delivered directly without another participant;
- delayed original message;
- a frozen false finding; and
- removal of the originating participant at delivery.

Positive signaling is descriptive sender association. Positive listening is the
original-minus-no-message change in the committed receiver action. Task value is accepted result
for these solvable tasks. A confirmatory stratum requires the one-sided lower bound to exceed
`1/8` for original-minus-no-message listening, original-minus-no-message task value, and
original-minus-direct-evidence task value. Other variants report specificity and recovery but
cannot rescue failure. Fewer than 1,992 distinct task-linked episodes is inconclusive. Fluent text,
citations, agreement, centrality, roles, or confidence never substitute for listening and value.

## Reproduction and negative controls

`study-check` verifies the initial root, manifest bytes, compiled digest, protocol, power inputs,
power report, calculation code, and frozen outputs. `study-power` recomputes breadth, repetitions,
`best-of-n`, and resource scale. `study-dry-run` validates synthetic route records and emits the
declared primary, measurement, audit, and communication analyses without outcome-dependent
defaults.

`study-negative-controls` changes an arm budget, reveals identity before selector commitment,
applies a post-randomization exclusion, substitutes acceptance for failed verification, and
changes arm/profile and arm/route bindings, reveals a protected result before selection, changes
the deterministic block seed, reverses launch order, breaks arm/order correspondence, substitutes
the corpus root, and rewrites the frozen budget, exclusion, primary outcome, stopping, and manifest
identity while also rewriting the sidecar. `study-negative-control --case ID` exposes every case
through the same public validation route and must end with a nonzero code; the aggregate command
requires every invalid case to be rejected.

W1-EVL reaches these checks by exporting the block assignment and condition records with the same
fields consumed by `study-dry-run`: corpus root, task, repetition, seed, order, block profile and
route, arm profile, route accounting, selection commit, and protected-result reveal. A conforming
synthetic record passes. The controls distinguish mismatched bindings, event order, deterministic
assignment, and corpus identity, but they cannot establish that future W1-EVL telemetry is truthful,
that a profile was externally admitted, or that human and owner authority is valid.
