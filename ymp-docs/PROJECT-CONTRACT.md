# Project contract and acceptance oracle

The approved contract package is the root of a run. It defines the intended observable result,
the environment in which that result is evaluated, the finite resources and authority available,
and the evidence required for acceptance.

`PROJECT.md` is the public semantic specification. It is necessary but not sufficient by itself:
executable checks, environment definitions, and review procedures are separate artifacts with
their own digests. Calling the prose file the “sole definition of done” hid this distinction and
made oracle integrity impossible to reason about.

## Package contents

An approved package contains:

1. **Public `PROJECT.md`.** Goal, intent, non-goals, scope, observable requirements, delivery
   format, and allowed ambiguity.
2. **Environment manifest.** Source snapshot, build and run environment, dependency policy,
   platform, deterministic setup steps, and permitted external services.
3. **Visible development checks.** Tests and diagnostics that attempts may run freely.
4. **Protected acceptance bundle.** Held-out cases, randomized generators, negative controls,
   and expected results that attempts cannot read or modify.
5. **Review protocol.** Any criterion that requires a human or agent reviewer, including
   blinding, rubric, conflict handling, and who has authority to decide.
6. **Budget vector.** Provider cost, wall time, execution resources, participant and task
   creation, communication, verification queries, network access, credentials, and external
   actions.
7. **Evidence policy.** What failed protected checks disclose, what is retained, and what is
   required to claim acceptance.
8. **Approval record.** Digests, provenance, approver, time, toolchain versions, and superseded
   package if any.
9. **Observation policy.** Collaboration-retention and visibility rules, whether the run may be
   used in communication experiments, what interventions are allowed, and which published
   summaries or boundary actions may be exported for analysis.
10. **Runtime and model-route policy.** Allowed agent-runtime kinds and versions, harness and
    prompt-policy digests, model providers, deployments, endpoints, wire protocols, account or
    quota scopes, model identifiers, authentication modes, data-disclosure classes, and
    coordination-tool bindings. The policy also declares whether resume, native subagents, remote
    execution, and runtime servers are permitted. Experimental comparisons pin these variables or
    balance them explicitly; a runtime name is not a proxy for a model route.
11. **Execution assurance profile.** One of the versioned supported profiles, its required
    preflight evidence, which limits are enforced versus observational, and any outer experiment
    boundary. The first release uses `poc_process_isolation`; stricter deployments may request
    `strict_linux` or an explicitly weaker profile such as `best_effort_macos`. A profile never
    weakens silently.

The public file includes the digests of the other approved artifacts but not protected content.

The observation policy is not part of the definition of candidate correctness. A run can produce
a valid accepted candidate without exhibiting collective reasoning, and an interesting exchange
cannot make an invalid candidate pass.

## Draft, approval, and amendment

Drafting, approval, execution, and acceptance are distinct authorities:

- repository analysis and a structured interview produce an agent-assisted draft;
- a human reviews the semantics, environment, budget, private oracle bundle, and observation
  policy;
- running agents receive the public specification and visible checks only;
- the verifier uses the exact approved private artifacts; and
- an amendment creates a new package and a new run lineage after fresh human approval.

The recommended drafting path is **repository analysis plus a structured interview**. A short
prompt alone cannot establish scope, threat boundaries, or a sound oracle for an unfamiliar
project. ymp must ask when intent is not operationally observable; inventing a check is not a
substitute for clarification.

Human approval establishes authority, not validity. The approval interface must show unresolved
ambiguities, criteria without checks, external effects, inaccessible dependencies, and oracle
coverage gaps rather than presenting a generated file as complete.

## Public requirements and protected cases

Protected evaluation must not create secret requirements. It may hide only instances of public
requirements: inputs, seeds, schedules, failure injections, scale, or combinations that prevent a
candidate from memorizing the visible suite.

For example, a public requirement may say that a parser accepts the declared grammar and rejects
invalid input. Visible tests demonstrate representative cases; the protected oracle uses held-out
and generated cases from the same grammar. A hidden requirement for a different file format would
be invalid even if a human approved it.

Where possible, protected checks run as a black-box harness outside the candidate process and
generate fresh cases from an approved generator. If candidate code and hidden test code must share
an address space or filesystem, secrecy is only best-effort and the contract records that weaker
verification regime.

## Oracle layers

### Mechanical integrity checks

These establish that the candidate being evaluated is the candidate that was submitted:

- contract, source, candidate, environment, and oracle digests match;
- protected files, visible tests, build entry points, and verifier configuration are unchanged;
- the submission touches only authorized paths and declares dependency changes;
- the build does not substitute fixtures or bypass the intended executable path; and
- acceptance is reproduced from a clean environment.

### Behavioral checks

These observe the public requirements using examples, property tests, randomized cases, fault
injection, invariants, benchmarks with tolerance, or schema validation. A plain zero exit status is
not enough unless the check's semantics and coverage are reviewed.

### Negative controls

The bundle includes tests expected to fail for known invalid candidates or mutations. If these
controls pass, the oracle or harness is not discriminating and the run must end with
`infrastructure_error`, not acceptance.

### Reviewer-judged criteria

Some qualities cannot be reduced to executable checks. Each such criterion needs a rubric and an
authorized reviewer. An agent reviewer records an initial assessment without seeing the producer's
rationale, other votes, or reputation evidence; social information may be shown only after that
independent record exists. A human remains required for criteria whose consequences exceed the
delegated authority.

Agreement among reviewers is diagnostic evidence. It is not automatically a verdict, because
language-model errors can be strongly correlated.

## Adaptive evaluation and feedback limits

Repeated attempts adapt to every disclosed failure. A frozen hidden suite therefore becomes less
independent with each query even when agents cannot edit it. The contract package must define:

- a small protected-query budget distinct from visible test runs;
- disclosure no richer than needed for the next authorized decision;
- whether final checks use fresh approved random seeds or cases;
- how often a holdout is retired or refreshed through a new approved package; and
- which evidence remains blinded from later producers and reviewers.

The kernel does not choose a candidate to test. A participant spends a scarce query reservation on
an exact candidate. A failed query does not automatically reveal raw hidden-test output or fund a
retry.

## Coordination-study protocol

Runs used to evaluate collective reasoning require a study specification approved separately from
the candidate oracle. It declares:

- task-class and decomposability strata;
- single-agent, independent-search, and locally coordinated conditions;
- matched agent-runtime, harness-policy, model-route, execution, communication, and verification
  budgets;
- which messages may be absent, replaced, shuffled, delayed, or delivered as raw evidence;
- randomization unit, stochastic repetitions, exclusions, stopping rule, and primary outcomes;
- the minimum effect considered practically useful before results are observed; and
- retention, redaction, and publication rules for repository data and collaboration messages.

The study records intentionally published summaries, artifacts, decisions, capability-boundary
actions, and verifier evidence. It does not request private chain-of-thought. A message that sounds
insightful is not labelled causally useful unless the preregistered intervention changes subsequent
behaviour or outcome. Conversely, an efficient coordination pattern may be useful even when its
messages are terse.

Communication measures never feed candidate acceptance, participant eligibility, resource
allocation, or an intrinsic reward during the run being measured. This avoids selecting for
agreement, verbosity, performative explanations, or manipulation of other participants.

## Oracle validation before agent search

Before approval, the package should be tested against:

- the unchanged source baseline;
- at least one known-good candidate where available;
- deliberately invalid mutations for every major requirement;
- attempts to edit, bypass, or replace the harness;
- clean-room reproducibility; and
- timeout, nondeterminism, and infrastructure-failure cases.

A requirement-to-evidence matrix records which checks and review criteria cover each public
requirement. Missing coverage is shown explicitly. The percentage of prose requirements with
valid independent observations is a property of the contract package, not of the agents.

## Acceptance semantics

`accepted` means all required evidence in one approved package was obtained for one exact candidate
digest under one exact environment. It does not mean:

- that every reasonable user expectation was specified;
- that the candidate is best among alternatives;
- that a reviewer consensus is true;
- that a benchmark score transfers to another project class; or
- that the candidate is authorized for external delivery or deployment unless delivery itself is
  an approved capability and criterion.

Several candidates may pass. If the public contract does not define a mechanical tie-breaker, ymp
presents the accepted set and evidence; the kernel does not invent a quality ranking.

## Supported-project boundary

A project is ingestible only when ymp can capture a reproducible source state, build an isolated
environment, express public requirements, validate a sufficiently discriminating oracle, and
bound external authority. Projects based primarily on taste, open-ended research, unavailable
services, or irreversible real-world effects are outside the initial claim.

“Any project” is therefore not a defensible first product claim. Extending the supported boundary
requires a new evaluated contract pattern and evidence, not merely another prompt template.

## The design-critical question

Contract and oracle quality is the decision that can sink the project. If the oracle is easy to
pass without satisfying intent, additional attempts make false acceptance more likely. If it is
too strict, flaky, or incomplete, the system burns budget against an impossible target. No task
allocation or reputation mechanism repairs this layer after the search begins.
