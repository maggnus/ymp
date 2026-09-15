# Product-development waves for YMP

Status: planning proposal, aligned with the owner's discussion decisions on
2026-09-15. This file explains wave outcomes, not implementation status. The
approved architectural model governs scope; canonical task records govern
development status. Detailed task ownership is recorded in task-coverage.md.

## Scope authority

For this planning assignment,
[self-organizing-team-domain-model.md](self-organizing-team-domain-model.md) is
the sole authoritative architecture and product-scope source. The user's
explicit approval makes the complete model binding for this proposal. Its terms, structures, services,
strategies, protocols, semi-algorithms, invariants, event vocabulary, build
order, and substantive ideas define what the final wave must implement.

No other repository document, prior YMP iteration, current implementation
limitation, or external publication can add, remove, rename, reinterpret, or
defer a requirement from that model. Proposed amendments outside the model are
not applied. Other evidence may explain the implementation baseline or help
choose an order only when that order remains consistent with the model.

The TUI remains in scope, with Ratatui and the broad chat, commands and
information-panel direction. The owner has explicitly left its detailed look and
command vocabulary open. The main product focus is useful agent communication
and self-organization, including experimental replacement of key mechanisms
through their strategy interfaces. OpenCode is general interaction and visual inspiration, not an
architectural source or dependency. The TUI is a presentation and control
adapter over the model. It displays deterministic journal projections and
invokes trusted kernel operations; it does not become a new authority, state
store, strategy, or source of domain decisions. A primitive TUI is delivered in
W1 and grows with every later wave.

## Research implementation baseline

This is the historical research baseline, not a current-code audit or scope
source. Task decomposition checks the newer App Server implementation separately.

At repository revision
`e6949fd70d278e835d6211e0a8a6f9db4fe4e5f5`, the existing executable identifies
itself as `ymp 0.1.0`. A safe `--help` run shows two commands:

- `ymp run "<task>" [--check "<shell command>"] [--data-dir PATH]`;
- `ymp show <session-id> [--data-dir PATH]`.

The implementation can persist a session, admit one Codex invocation, record
execution observations and usage, and optionally record a preliminary command
check. It does not yet demonstrate the complete session loop, independent
review, immutable accepted result, self-organizing team, independent attempts,
experience, calibrated routing, or TUI required here.

No provider invocation, credential access, user-workspace mutation, or fresh
Rust test run was performed for this planning revision. Existing code may be
reused where it conforms to the sole model, but it cannot redefine that model.

## Wave-design decision

The authoritative model already supplies a ten-stage dependency order in
[section 10](self-organizing-team-domain-model.md#10-build-order). Stages 1–4
produce necessary internals but no complete user result; stage 5 explicitly
defines the minimal product. The proposal therefore groups stages 1–5 into W1
and assigns stages 6–10 to five successive waves. This yields **six waves**:

| Wave | Authoritative build stages | User-visible growth |
| --- | --- | --- |
| W1 | Stages 1–5 | A minimal TUI runs one accountable producer/reviewer session and delivers an audited report. |
| W2 | Stage 6 | The TUI shows independently designed hidden checks, mutation discrimination, and a justified `Discriminated` grade. |
| W3 | Stage 7 | The team self-organizes through solicitations, offers, commitments, notices, objections, handoffs, clarification, and evidence-driven membership/profile changes. |
| W4 | Stage 8 | Capability limits can trigger sealed independent attempts in isolated copies, followed by evidence-based selection and merge. |
| W5 | Stage 9 | Qualified outcomes create reputation, calibrated forecasts, scoped knowledge, controlled retrieval trials, and later regrading. |
| W6 | Stage 10 plus full-scope audit | YMP uses calibrated routing and value-of-computation selection, exposes the entire model through the TUI, and leaves no required capability deferred. |

This count is derived from useful product increments rather than subsystem
folders. Every wave ends with an executable scenario that exposes its new runtime behavior
through a simple TUI, commands, projections or inspectable artifacts.
Intermediate simplifications are named and removed by a later wave.

## TUI delivery direction

The TUI is an evolving presentation/control adapter over the approved model. A
primitive working interface belongs to W1 and can grow as capabilities arrive.
Chat, commands and an information panel are broad directions, not fixed screen
layouts or exhaustive per-wave command lists. Existing projections and application
operations should serve both experimentation and the interface.

The unique product work is agent initiative, proposals, offers, commitments,
handoffs, objections, team/plan adaptation and experience. UI polish or repeated
screen redesign is not a completion gate for those mechanisms. Each wave must
expose its actual behavior through a real consumer; a mockup or a successful
isolated unit check alone does not demonstrate product growth. The interface
must preserve authority, hidden-context boundaries and truthful unknowns.

## Evidence required from every wave

Each wave closes when its new product outcome is observable through a real
consumer and the underlying behavior is verified through the kernel.
Its evidence package must contain:

1. the exact executable version and a reproducible user scenario;
2. a recording, transcript or inspectable result showing the improvement over the previous
   wave;
3. journal references for the relevant session, policy input, proposal,
   decision, assignment, invocation, result, evidence, and report entities
   available in that wave;
4. a negative or counterexample case demonstrating that the trusted rule can
   refuse an invalid transition or defective result;
5. resource reservations, receipts, unknown coverage, and final accounting for
   every invocation used by the scenario;
6. an explicit statement of which later-wave mechanisms are still simplified;
7. contract tests for every introduced strategy through its real consumer,
   including two substantially different implementations as required by
   [section 5](self-organizing-team-domain-model.md#5-replaceable-strategies-port).

Internal scaffolding, a data type, a test count, or a TUI mockup cannot close a
wave without the end-to-end user scenario.

## W1 — Minimal accountable product with a primitive TUI

### Product outcome and user benefit

A user can open YMP, state a goal and constraints, inspect the eligible pool,
start a bounded `SoloWithVerifier` session, observe its plan and resource use,
and receive a report whose claims are audited against recorded evidence. The
result may be `Unconfirmed`; that limitation is shown rather than hidden.

This is the first complete product loop defined by build stages 1–5: a producer,
an independent reviewer, a criteria ledger, diagnosis-driven escalation, and a
report with claim audit
([section 10](self-organizing-team-domain-model.md#10-build-order)).

### User scenario and visible improvement

The primitive TUI accepts a small task with one `NewBehavior` criterion and one
`Preserve` criterion. It shows:

1. readiness results and the eligible/excluded agents;
2. the normalized budget and protected verification reserve;
3. criteria, recorded clarification or assumption, method, one-item plan, and
   direct workspace;
4. the producer assignment, active commitment, grant, invocation, allowance,
   progress, and receipt;
5. the immutable candidate, applicable checks, independent review, criteria
   ledger, acceptance grade, unmet criteria, and audited final report.

A scripted provider makes this scenario reproducible. The existing Codex
backend may provide a separately authorized real-provider demonstration once it
conforms to the same ports. Compared with the current baseline, the user sees a
complete accountable session rather than one invocation and a preliminary
check.

### Included authoritative capabilities

**Reproducible state and domain values.**

- Base values `Id<T>`, `Digest`, time/duration, probability, normalized
  `CostUnits`, collections, versioned `Ref`, `PolicyRef`, `Proposal<T>`,
  `Denial`, and deterministic read-only `*View` projections
  ([section 0](self-organizing-team-domain-model.md#0-notation)).
- `Journal`, `Store`, atomic ordered events, deterministic views, and
  decision records containing policy identity and input-view digest.
- The complete task intake values required for the scenario: `Task`, `Goal`,
  `Assumption`, `Clarification`, `Constraints`, `Pins`, `Criterion`,
  versioned `AcceptanceContract`, user/visible `Check`, `Snapshot`, and
  `Artifact`
  ([section 3.2](self-organizing-team-domain-model.md#32-task-definition)).

**Identity, readiness, and execution.**

- `Agent`, `Provider`, `ModelOffering`, `ExecutionProfile`,
  `Capability`, and `Pool`, including native model/effort values and typed
  exclusion reasons
  ([section 3.1](self-organizing-team-domain-model.md#31-identity-and-pool)).
- `Registry` with `StaticDependencyProbe`; reading readiness performs no
  model call.
- `ExecutionBackend(Scripted)` as the reference implementation and a
  conforming first native adapter where available. Requested, sent, and reported
  settings remain distinct.
- `Contribution`, `Assignment`, `Grant`, `Invocation`, `Attempt`,
  `ResultVersion`, and direct `Workspace` with path locks
  ([section 3.4](self-organizing-team-domain-model.md#34-authority-and-execution)).
- `Gatekeeper` validates constraints, pins, actual capabilities, role
  independence, reservations, path locks, parallel and attempt limits before it
  creates an assignment, grant, allowance, and commitment.

**Resources and quality.**

- `Budget`, `PriceBook`, `Reservation`, `Receipt`, and `Allowance`
  with `PriceWeighted` cost and `PurposeBounded` allocation
  ([sections 3.7 and A13](self-organizing-team-domain-model.md#37-resources)).
- Production and coordination cannot spend the verification reserve; every
  invocation, including verification, reporting, and coordination, is charged.
  Unknown receipt coverage follows `Stop` or `Estimate` rather than becoming
  zero.
- Direct `WorkspaceProvider`, `ProcessRunner`, `AcceptanceAuthority`,
  `LikelihoodRatioTable`, and `CreditPolicy`.
- `CheckRun`, `Evidence`, independent `Review`, `Acceptance`,
  `ConfirmationGrade`, `CriteriaLedger`, `ProgressLedger`, `Claim`, and
  `Report`
  ([section 3.6](self-organizing-team-domain-model.md#36-quality-assurance)).
- Applicable contradicting executable evidence overrides approval. A reviewer
  cannot be the producer. A final reviewer cannot be among the aggregate's
  producers; otherwise the session is blocked.

**Minimal session loop and coordination.**

- `Session`, `Team`, `Membership`, fixed `SoloWithVerifier` `Method`,
  versioned `Plan`, and `WorkItem`
  ([section 3.3](self-organizing-team-domain-model.md#33-session-team-and-plan)).
- `Dispatcher.run` with intake, checks, method, plan, work loop, finalization,
  report delivery, and post-delivery learning hook; the learning hook remains
  inert until W5.
- `Arbiter` support required for P2 commitments; the full P1 and P3–P7
  protocols arrive in W3.
- `Commitment` transitions Proposed → Active → renewed, Discharged, Released,
  Expired, Cancelled, or Delegated, with exactly one active commitment per
  assignment
  ([P2](self-organizing-team-domain-model.md#p2-commitment-lifecycle)).
- Deterministic initial strategies: `CriteriaExtraction + VoiClarification`,
  `FixedMethod(SoloWithVerifier)`, `AsNeededDecomposition`,
  `OrdinalValue`, `AnyNonProducer`, `EvidenceDelta`,
  `RuleBasedDiagnoser`, `DiagnosisFirstLadder`,
  `CriteriaProjection + JournalDigest`, narrator-role
  `NarrativeComposer`, and `EvidenceClassRules`.

**Primitive TUI.**

- Goal/constraint intake, pool/readiness, criteria, plan, current assignment,
  commitment, progress, resource, result, evidence, acceptance, and report
  views.
- Actions are limited to trusted operations available in W1. The TUI cannot
  write `Store` or `Journal` directly, issue a grant, override failed
  evidence, or manufacture a status.
- Hidden checks, dynamic offers, independent alternatives, reputation, and
  learned routing are visibly marked unavailable until their owning waves.

### Necessary internal work

Implement build stages 1–5 as one vertical slice. The kernel owns every state
transition; strategies read projections and return proposals; the
`ExecutionBackend` returns observations. The `Dispatcher` owns call order but
no business rules. Each recorded decision includes the exact `PolicyRef`,
input-view digest, proposal, basis, and committed decision. Add only the event
kinds required by this wave without changing their later meaning.

### Dependencies

The current journal, execution, and check code can accelerate this wave only
after conformance review against sections 0–8. W1 has no dependency on another
product wave.

### Testable completion conditions

- The TUI completes the full scripted scenario from intake to
  `ReportDelivered`, then reopens the same deterministic `SessionView`.
- A strategy cannot mutate state; an adapter-returned verdict cannot accept a
  result.
- Unsupported profile settings, excluded agent, pin violation, missing
  capability, insufficient reservation, conflicting path lock, stale view, and
  unavailable independent reviewer each return a recorded or typed denial with
  no partial assignment.
- A failed applicable executable check rejects the candidate despite reviewer
  approval.
- Producer-authored execution plus inspection can remain `Unconfirmed`; the
  TUI does not relabel it.
- Resource totals include production, review, report generation, and failed
  work; unknown coverage remains unknown.
- Claims about execution, browser behavior, causality, scope, and
  recommendations are accepted or narrowed according to A11.

### Risks and unspecified implementation choices

- The model does not choose the concrete `Store` technology, Rust type layout,
  evolving TUI layout, native provider protocol, `PriceBook` source, or direct
  workspace mechanism. W1 must choose implementations that preserve the model's
  contracts; these choices cannot remove a model element.
- User-supplied or fixed visible checks are an explicit W1 simplification.
  Independent hidden design and mutation discrimination are completed in W2.
- Reputation-dependent choices use neutral/uninformative priors until W5
  produces qualified observations.

## W2 — Discriminating independent verification

### Product outcome and user benefit

The user can see not only that a candidate passed a check, but that independently
designed hidden checks distinguish the candidate from the baseline and from
deliberately faulty variants. A qualifying result can receive the model's
`Discriminated` grade.

### User scenario and visible improvement

Before production, a sealed `CheckDesigner` receives the criteria and base
snapshot, performs a premortem, and proposes hidden checks. A new-behavior check
must fail on the baseline; a preservation check must pass. After the producer
submits a result, YMP runs the checks on the candidate and on non-equivalent
mutants. The TUI shows hidden-check identity only after the producer window
closes, baseline/candidate/mutant/control runs, mutation score, evidence
polarity, independent review, and the resulting grade.

Compared with W1, a passing producer-visible check no longer supplies the
strongest available basis.

### Included authoritative capabilities

- `VerificationDesigner` with `IndependentHiddenDesigner` and
  `ProducerChecks` as the experimental control
  ([section 5.1](self-organizing-team-domain-model.md#51-readiness-intake-and-checks)).
- `MutationStrategy` with
  `FaultInjectionWithEquivalenceFilter`, `SyntacticOperators`, and `None`.
- The complete `CheckSpec` set: `Command`, `ExactBytes`, `Property`,
  `BrowserScenario`, and `ExternalQuery`; corresponding
  `ProcessRunner`, `BrowserRunner`, and `ContainerRunner` paths.
- `CheckRun` roles Baseline, Candidate, Mutant, and Control; full
  stdout/stderr/environment digests and typed errors.
- `EvidenceClass` values StaticRead, Executed, Browser, ExternalData, and
  Inspection; support/contradiction polarity and `Discrimination`.
- A2 check design, A6 result verification, and A7 grade calculation
  ([A2](self-organizing-team-domain-model.md#a2-check-design-and-discrimination),
  [A6](self-organizing-team-domain-model.md#a6-result-verification),
  [A7](self-organizing-team-domain-model.md#a7-acceptance-and-confirmation-grades)).
- Hidden check data never reaches producer context, and a hidden-check author
  cannot produce the result for that criterion (R-10).
- TUI check-design, run-matrix, mutant, evidence, finding, and grade views. Before
  independence closes, hidden content is omitted rather than merely concealed
  cosmetically.

### Necessary internal work

Add sealed check-design assignments, hidden-context projections, baseline and
control execution, mutant materialization, equivalence decisions, evidence
grouping, mutation scores, and grade rules. `AcceptanceAuthority` remains the
only service that registers checks, runs them, records evidence, and accepts.

### Dependencies

W1 supplies snapshots, result versions, check execution, reviewer independence,
criteria ledger, resource accounting, and report auditing.

### Testable completion conditions

- A new-behavior check that passes on the baseline is rejected as
  non-discriminating; an environment error causes `FixEnvironment` rather than
  a false failed criterion.
- Producers cannot read hidden check specifications or data before the relevant
  window closes.
- Equivalent mutants are excluded; surviving non-equivalent mutants lower the
  mutation score and can prevent `Discriminated`.
- Contradicting Executed, Browser, or ExternalData evidence rejects the result.
- Repeated evidence from one correlated independence/class/discrimination group
  counts once in A8.
- A deliberately leaky TUI projection and a producer/hidden-author role collision
  fail negative tests.

### Risks and unspecified implementation choices

The model fixes the semantics but not the property generator format, browser
driver, container environment, mutation operators, or equivalence detector.
Their limitations must be recorded in evidence. Initial likelihood ratios,
`μ`, and corroboration thresholds remain model-specified assumptions to
calibrate in W6.

## W3 — Self-organizing cooperation

### Product outcome and user benefit

YMP can form and revise a team around unmet criteria, accept agent-proposed
contributions, allocate work through offers, coordinate through accountable
notices and commitments, and change the next action when evidence or an
objection warrants it. The TUI shows why each participant, profile, and
contribution exists.

### User scenario and visible improvement

A task has an implementation criterion, a browser criterion, and an ambiguous
constraint. The initial team lacks browser capability. The TUI shows:

1. intake either asks the high-value clarification or records an assumption;
2. P1 opens a contribution solicitation, gathers RuntimeProxy,
   InAssignment, or justified BidAssignment offers, and awards one;
3. a no-offer browser contribution is reopened with increased stimulus, then
   `TeamPolicy` adds an eligible participant;
4. two independent, non-conflicting contributions overlap, while conflicting
   writes wait;
5. a participant posts a goal-status notice and another raises an objection
   with an executable counterexample;
6. a released or delegated commitment creates a bounded handoff;
7. the plan and team change, verification reruns, and the final report explains
   the decisions and cost.

Compared with W2, coordination changes admitted work instead of merely showing
multiple messages.

### Included authoritative capabilities

- Full P1 contribution solicitation: open/sealed `Solicitation`, stimulus,
  deadline, eligibility, all three `Offer` sources, `Award`, reopening,
  `N_open`, and capability diagnosis
  ([P1](self-organizing-team-domain-model.md#p1-contribution-solicitation-and-voluntary-response-contract-net-with-thresholds)).
- Full P3 goal-status notices for Satisfied, Unachievable, and Irrelevant claims
  and their required work-boundary responses
  ([P3](self-organizing-team-domain-model.md#p3-goal-status-notice-joint-intention)).
- Full P4 objection handling: executable counterexample, evidence request,
  debate with two Advocates and a Judge, or escalation. Debate is allowed only
  under the model's weaker-judge condition; opinion alone cannot override a
  passing applicable check
  ([P4](self-organizing-team-domain-model.md#p4-objection-with-a-counterexample)).
- P5 `Handoff` and `ContextDigest`; P6 value-of-information clarification;
  P7 independence windows for check designers
  ([protocols](self-organizing-team-domain-model.md#6-protocols)).
- `Board`, addressed/global `Notice`, questions, answers, findings,
  `GoalStatusClaim`, proposal references, commitments, solicitations, and
  objections
  ([section 3.5](self-organizing-team-domain-model.md#35-coordination)).
- `VolunteerPolicy(ResponseThreshold)`, `AwardPolicy(CalibratedValuePerCost)`,
  `TeamPolicy(DemandDriven)`, `ProfilePolicy(CheapestAdequate)`,
  `ReviewerPolicy(DifferentFamilyComparableStrength)`, and
  `DisputePolicy(CounterexampleFirst)`
  ([section 5.3](self-organizing-team-domain-model.md#53-self-organization)).
- Every `ContributionKind` and `RoleKind` from section 3.4, including
  Research, Alternative, Diagnose, Decompose, Integrate, Clarify, Curate,
  Narrate, Judge, and Advocate.
- A3 ordinal contribution selection, A4 response threshold, A5 assignment
  execution, and A9 progress/diagnosis/escalation.
- TUI surfaces for team membership, pool exclusions, solicitations, offers,
  awards, commitments/leases, addressed notices, handoffs, objections,
  clarifications, plan revisions, progress, diagnoses, escalations, and actual
  resource growth.

### Necessary internal work

Complete `Arbiter` operations, all P1 and P3–P7 events, dynamic
`TeamPolicy`, capability-aware solicitations, bounded context composition,
goal-status responses, dispute processing, and plan revision. The final atomic
admission still belongs to `Gatekeeper`; strategy selection never creates
authority.

### Dependencies

W1 supplies the session loop, P2 commitments, plan, admission, resource and
workspace rules. W2 supplies trustworthy checks and hidden-context handling.
Reputation and calibrated probabilities are not yet learned: strategies use
recorded neutral/raw values marked uncalibrated until W5–W6.

### Testable completion conditions

- An agent proposal enters the same candidate list as policy templates and
  either changes admitted work or receives a valid denial.
- No-offer reopening increases stimulus, then adds a capable pool member or
  records CapabilityMismatch/CapabilityLimit.
- Pins, actual capabilities, independence, path conflicts, parallel limits,
  attempt limits, and verification reserve still constrain dynamic choices.
- Satisfied, unachievable, and irrelevant notices cause the exact P3 response at
  the next work boundary.
- A failing executable counterexample upholds an objection and rejects the
  result; a passing one dismisses it; an error remains unresolved and is
  diagnosed.
- A released, delegated, or expired commitment transfers a bounded digest
  without hidden checks.
- Adding a member, selecting a stronger profile, or raising effort without a
  recorded diagnosis is denied (R-14).
- The TUI can trace every dynamic decision to policy, proposal, basis, input
  view, kernel decision, and cost.

### Risks and unspecified implementation choices

The model fixes the protocol but leaves TUI interaction design, offer timing,
native agent notification, and exact context condensation format open. All
numeric policy values remain initial assumptions. Dynamic cooperation must not
be presented as beneficial merely because it ran.

## W4 — Independent attempts and isolated selection

### Product outcome and user benefit

When one approach reaches a diagnosed capability limit, YMP can commission
independent alternatives without cross-contamination, evaluate them against a
shared test matrix, select an admissible candidate, and merge only that result.
The user can inspect both the chosen basis and the preserved alternatives.

### User scenario and visible improvement

Two profiles fail the same discriminating criterion, producing
`CapabilityLimit`. The escalation ladder selects
`AlternativeAttempts(2)`. YMP snapshots the target, opens two
`IsolatedCopy` workspaces, assigns distinct comparable producers from
different families when available, and seals their results and board entries.
Each producer supplies tests `T_i`; YMP runs every hidden check and every
producer test against every candidate, clusters admissible candidates, and asks
an independent reviewer to choose inside the leading cluster. The TUI shows the
sealed phase, matrix after disclosure, excluded candidates, rationale, merge,
and unchanged target if no candidate is admissible.

Compared with W3, escalation can now explore competing solutions safely rather
than only reassigning or decomposing one shared approach.

### Included authoritative capabilities

- `WorkspaceProvider(CopyOnWrite)`, `WorkspaceGuard.open(IsolatedCopy)`,
  base/candidate snapshots, path locks, and validated merge
  ([section 5.7](self-organizing-team-domain-model.md#57-context-report-and-execution)).
- `SelectionPolicy(DualExecutionAgreement)` and alternative
  `ReviewerChoice`.
- Full A10 independent-attempt algorithm: distinct producers, comparable
  strength, family diversity where available, P7 sealed window, mandatory
  producer tests, complete `H ∪ T_i` cross-run matrix, admissible clusters,
  cluster score, independent choice, selected merge, and preserved alternatives
  ([A10](self-organizing-team-domain-model.md#a10-independent-attempts-and-selection)).
- `IndependentAttempts(k)` method and its place after Decompose and before
  StrongerProfile in the diagnosis-first ladder.
- R-13 serialization/isolation principle from section 1 and R-10 hidden-context
  enforcement.
- TUI views for independent-attempt group, isolation base, producer/profile,
  sealed state and deadline, test matrix, admissibility, cluster score,
  reviewer choice, merge result, and retained non-selected candidates.

### Necessary internal work

Implement isolated workspace materialization and content-addressed merge,
deadline-based disclosure, cross-candidate execution, cluster scoring, selection
proposal, and kernel merge validation. Make all attempt/result/workspace
associations durable. Equal-prior reputation is sufficient until W5 supplies
qualified estimates, but the prior must be explicit.

### Dependencies

W2 provides hidden discriminating checks and result evidence. W3 provides full
P7 windows, dynamic assignments, different-family reviewer selection,
diagnosis, and alternative escalation.

### Testable completion conditions

- Before disclosure, one producer cannot read another candidate, tests, or
  same-window board records.
- A path escape or shared-write attempt cannot modify the target or another
  isolated copy.
- Every `T_i` and hidden check runs against every candidate; selective testing
  cannot bias the cluster.
- Any hidden-check failure excludes a candidate. The reviewer chooses only
  within the leading admissible cluster.
- Only the selected `ResultVersion` merges; other candidates and evidence
  remain immutable in the journal.
- If no candidate is admissible, no merge occurs and
  `FailureDiagnoser` records CapabilityLimit or CheckDefect.
- The TUI presents the same selection rationale that the journal records.

### Risks and unspecified implementation choices

The model does not prescribe the copy-on-write mechanism, merge algorithm, or
equivalence of workspace-level conflicts. These implementations must preserve
snapshot identity and isolation. Different model families are preferred “when
available”; absence is displayed and does not justify inventing a family.

## W5 — Qualified experience and accountable learning

### Product outcome and user benefit

YMP can use prior qualified outcomes to estimate competence, calibrate offers,
retain scoped knowledge, and test whether that knowledge helps later matching
sessions. Later evidence can regrade acceptances and retire dependent knowledge.
Learning never delays delivery of the current report.

### User scenario and visible improvement

After a delivered session, the asynchronous `Curator` proposes a scoped
procedure with provenance, evidence, and a falsifier. Independent scope review
makes it Hypothesis or Rejected. Matching later sessions are assigned With and
Without arms; the TUI shows the retrieved or excluded knowledge version, reason,
metric, cost, and outcome. After the predefined number of successful trials,
the procedure becomes Validated; Confirmed evidence can promote it further. A
later failed check or revert regrades the acceptance and retires knowledge whose
falsifier was observed.

Compared with W4, later assignments can benefit from accountable experience
rather than starting from unqualified memory.

### Included authoritative capabilities

- `ExperienceVault` operations for observations, reputation, forecast
  outcomes, and knowledge lifecycle
  ([section 4](self-organizing-team-domain-model.md#4-trusted-runtime-services-kernel)).
- `Observation` keyed by complete `ExecutionProfile`, `Competence`, and
  `Difficulty`; Success/Failure rules tied to `Acceptance` or
  `Consequence`.
- `Reputation` using `BetaWithForgetting(λ)` and alternative `MeanOnly`.
- `CalibrationRecord` using `BrierIsotonic`; insufficient data returns the
  raw forecast marked uncalibrated.
- `Knowledge` kinds Fact, Procedure, Pitfall, and Strategy; project/features/
  preconditions `Scope`; provenance, evidence, falsifier, supersession, and
  all statuses Candidate, Hypothesis, Validated, Confirmed, Rejected,
  Superseded, and Retired
  ([section 3.8](self-organizing-team-domain-model.md#38-experience)).
- `KnowledgeTrial` With/Without arms and `Retrieval` with exact included
  versions and excluded reasons.
- `CreditPolicy`, `ReputationModel`, `CalibrationScorer`,
  `KnowledgeCurator(AfterActionReview)`, `RetrievalPolicy(ScopedLexical)`,
  `TrialPolicy(AlternatingArms)`, and `ConsequenceSource` implementations
  LaterChecks, VcsReverts, and UserFeedback
  ([section 5.6](self-organizing-team-domain-model.md#56-experience)).
- Full A12 learning and consequence processing. Only Validated/Confirmed
  knowledge enters ordinary automatic context; Hypothesis enters only its
  labeled With arm.
- TUI views for observations, reputation distributions, forecast calibration,
  knowledge version/scope/status/provenance/evidence/falsifier, trial arm,
  retrieval inclusion/exclusion, consequence, regrading, supersession, and
  retirement.

### Necessary internal work

Implement `ExperienceVault` projections and events, after-delivery curation,
credit filtering, reputation updates, forecast scoring, scoped retrieval,
trial assignment, consequence polling, acceptance regrading, and knowledge
review. Retrieval still passes through `ContextComposer` and is recorded before
execution.

### Dependencies

W1 supplies acceptance grades, reports, profiles, receipts, and the asynchronous
learning hook. W2 supplies discriminating evidence. W3–W4 supply calibrated
offer inputs, diverse assignments, and independent outcomes.

### Testable completion conditions

- The configured `CreditPolicy` creates Success only from a creditable accepted
  grade and Failure only from contradicting executable evidence with
  ArtifactDefect/CapabilityLimit diagnosis.
- Self-assessment, agent agreement, infrastructure/environment failure, check
  defect, and capability mismatch create no competence observation (R-11).
- Candidate knowledge lacking scope or falsifier is denied. Scope review moves
  it only to Hypothesis or Rejected.
- Promotion to Validated requires With to outperform Without for at least
  `n_trial` matching sessions on the predefined metric; Confirmed requires
  confirmed evidence.
- Stronger contradictory basis supersedes; observed falsifier or Refuted
  consequence retires.
- Only current matching Validated/Confirmed knowledge appears in normal context;
  every exclusion has a recorded reason.
- `ReportDelivered` precedes learning events, and curation failure cannot delay
  delivery (R-15).
- The TUI can show a positive, negative, or insufficient-data trial result
  without fabricating improvement.

### Risks and unspecified implementation choices

The model explicitly leaves the creditable-grade choice to `CreditPolicy`;
that decision must be made before positive observations affect reputation.
Storage/indexing and consequence polling cadence are implementation choices.
`λ` and `n_trial` remain calibration parameters until W6.

## W6 — Calibrated routing and complete model

### Product outcome and user benefit

YMP matches effort and cooperation method to the task from recorded evidence,
selects the next contribution by expected value per cost, and exposes the
complete authoritative model through the TUI. Simple work remains bounded;
parallel breadth, decomposition, independent attempts, or stronger profiles are
used only when their conditions are met.

This is the final scope wave. Every required structure, service, strategy,
protocol, algorithm, invariant, event, default/alternative implementation, and
provider backend named by the authoritative model must be implemented and
reachable by its real consumer. No required model capability remains “later” or
“future”.

### User scenario and visible improvement

A frozen scenario suite contains:

- a simple task routed to `SoloWithVerifier`;
- a task with two independent write groups routed to
  `AsNeededDecomposition`;
- a research task with at least two independent breadth groups routed to
  `BreadthResearch(k)`;
- a task whose similar history shows CapabilityLimit, routed to
  `IndependentAttempts(2)`;
- a diagnosed hard case that escalates to `StrongerProfile`;
- a budget-limited case that stops while preserving verified work and reports
  unmet criteria.

The TUI explains the task features, policy version, calibrated/raw probability,
belief gain, expected and p90 cost, selected method, selected contribution,
resource reservation, stopping decision, and final evidence. A later matching
task also demonstrates knowledge retrieval and consequence-driven regrading.
Compared with W5, experience now changes method, profile, award, and
contribution allocation rather than only supplying context and inspection.

### Included authoritative capabilities

**Data-driven routing and contribution choice.**

- `MethodRouter(CascadeRouter)`, alternatives `FixedMethod` and
  `LearnedRouter`, using difficulty, criteria count, independent write groups,
  required evidence classes, research breadth, similar-task history, and budget
  ([A1.1](self-organizing-team-domain-model.md#a11-cascading-method-choice-cascaderouter)).
- `ContributionPolicy(VocValue)`, alternatives `OrdinalValue` and
  `FixedWorkflow`, using calibrated success probability, criterion weight,
  belief gap/jump, expected cost, dependencies, capability, write conflicts,
  producer independence, purpose budget, parallel capacity, and `θ_min`
  ([A3](self-organizing-team-domain-model.md#a3-contribution-selection-contributionpolicy)).
- Fully calibrated `ResponseThreshold`,
  `CalibratedValuePerCost`, `CheapestAdequate`,
  `DifferentFamilyComparableStrength`, `DualExecutionAgreement`,
  `EvidenceDelta`, and `LikelihoodRatioTable`.

**Complete resources, providers, and execution.**

- `PriceWeighted` accounting normalizes provider input, cache-read,
  cache-write, output, and optional reasoning usage; the model fallback rates
  remain available.
- `Stop` and `Estimate` unknown-coverage paths, verification reserve,
  settlement, release, and A13 stop rule are complete.
- Conforming `ExecutionBackend` implementations for Codex, Claude, Glm, and
  Scripted, plus typed readiness and actual capability reporting. Unsupported
  native offerings are excluded rather than invented.
- All `TeamOperation`, `ContributionKind`, `RoleKind`, `MethodKind`,
  `CheckSpec`, `EvidenceClass`, diagnosis, escalation, status, grade,
  consequence, knowledge, and lifecycle variants in sections 3 and 5.

**Complete session and TUI.**

- Full A1 loop from readiness through asynchronous learning, including board,
  invocation, lease, and consequence wakeups
  ([A1](self-organizing-team-domain-model.md#a1-session-loop-dispatcherrun)).
- Full A1.1 and A2–A13 behavior; P1–P7; D-1–D-5; R-1–R-18.
- Every minimum journal event in section 9, with the common envelope and
  attributable actor, policy, input, and references.
- TUI access to the complete set of model projections and permitted operations:
  intake, pool, team, profiles, plans/work items, contributions,
  solicitations/offers/awards, assignments/grants/invocations, commitments,
  board/handoffs/objections, workspaces/attempts/results, checks/mutants/evidence,
  reviews/acceptance/ledgers, budget/receipts, diagnoses/escalations,
  reports/claims, reputation/calibration, knowledge/trials/retrieval, and
  consequences.
- The TUI preserves hidden-check and sealed-window exclusions and never makes a
  presentation state authoritative.

**Calibration and replacement discipline.**

- Calibrate or explicitly retain as uncalibrated every parameter in
  [section 10.1](self-organizing-team-domain-model.md#101-parameters-to-calibrate):
  `T_offer`, `B_bid`, `κ`, `N_open`, `T_lease`, `Δ_release`,
  `θ_min`, `γ`, `T_ref`, `θ0`, `ρ`, `μ`, `n_corroborate`, the LR
  table, `τ_behavior`, `τ_quality`, `τ_support`, `ε`, `stall_limit`,
  `p_min`, `λ`, `n_trial`, `cost_interrupt`, and `p_target`.
- Each strategy is exercised through its real consumer with at least two
  substantially different implementations; sessions record the implementation
  and parameters actually used.
- Insufficient calibration data stays marked uncalibrated and cannot support an
  effectiveness claim. Complete implementation does not guarantee a favorable
  empirical result.

### Necessary internal work

Connect accumulated W1–W5 observations to routing, contribution, award, profile,
and response policies; complete all target backends and variants; run the full
session loop; calibrate parameters on separated data; and audit all consumers,
invariants, and journal projections. Remove every temporary fixed path that
would bypass a target strategy or trusted service.

### Dependencies

All prior waves. Calibration data used to select parameters must be separated
from scenarios used to assess the resulting policy, and insufficient data must
remain explicit as required by `CalibrationScorer`.

### Testable completion conditions

- Each frozen scenario routes to the method dictated by A1.1 or records why
  input evidence is insufficient; pins and budget always win over a strategy
  proposal.
- A3 admits only contributions whose dependencies, capabilities, paths,
  independence, purpose budget, parallel capacity, and score pass.
- `StopPreserving` leaves completed work, accepted versions, and unmet criteria
  in the final report.
- All four provider implementations pass the shared backend contract; real
  provider evidence is separated from Scripted evidence and never invents
  unsupported metadata.
- Every strategy has two materially different real-consumer tests and records
  its `PolicyRef`.
- Replay reconstructs every decision from the journal and produces the same
  `*View` at the same sequence.
- The full TUI scenario exercises every model context, exposes no unavailable
  action as active, leaks no hidden data, and reaches Delivered or a truthful
  Blocked state.
- The coverage audits below have no unowned or post-W6 requirement.

### Risks and unspecified implementation choices

Calibration may favor simple policies, show no advantage, or remain
insufficient. In that case YMP retains the model's explicit uncalibrated status
or simpler alternative rather than claiming improvement. Concrete TUI layout,
storage engine, adapter protocols, workspace mechanism, check tooling, and
calibration dataset remain implementation choices, but W6 must resolve them
without narrowing the authoritative model.

## Approved consistency and experiment requirements

The approved model additionally clarifies protected reporting with a deterministic
fallback, scoped cessation and settlement before conflicting resource reuse,
recovery without duplicated work or reset accounting, and evidence applicability
with positive class coverage in A8. The default prior is 0.5; alternate assessment
and CreditPolicy implementations remain available for explicit experiments.
R-19/R-20 capture recovery guarantees without mandating a particular EffectState
representation or universal user-command transition table.

These requirements belong to existing W1 resource, admission, evidence, report
and session work, with experimental policy integration in W5/W6. They do not
justify another wave or a task for each setting. The task coverage map binds the
current model, including these clarifications, to concrete implementation tasks.

## Coverage audit

All requirement links in the following tables point to the sole authoritative
scope file. TUI rows derive only from the user's explicit mandatory-interface
decision and do not add domain authority.

### Build-order coverage

| Model stage | Required contents | Proposed wave | Observable evidence |
| --- | --- | --- | --- |
| 1 | Journal, Store, base types, `*View` | W1 | Reopened TUI session reproduces the same projection and decision references. |
| 2 | Registry, Scripted backend, readiness | W1 | TUI shows eligible/excluded agents; scripted invocation runs. |
| 3 | Treasury, CostModel, ResourcePolicy | W1 | TUI shows held/spent/released/unknown resources and reserve denial. |
| 4 | Direct workspace, CheckRunner, AcceptanceAuthority, BeliefModel, CreditPolicy | W1 | Candidate, checks, evidence, review, acceptance grade, and ledger are visible. |
| 5 | Gatekeeper, Arbiter P2, Dispatcher, minimal strategies | W1 | Producer/reviewer session delivers an audited report. |
| 6 | Hidden check design, mutation, discrimination | W2 | Baseline/candidate/mutant evidence yields or denies `Discriminated`. |
| 7 | P1 and P3–P7, self-organization strategies | W3 | Offers, team changes, objections, handoffs, and replanning affect admitted work. |
| 8 | CopyOnWrite, SelectionPolicy, A10 | W4 | Sealed alternatives are compared and only the selected version merges. |
| 9 | ExperienceVault and all experience strategies | W5 | Later task retrieves qualified knowledge and records a trial/consequence. |
| 10 | Data-driven MethodRouter and ContributionPolicy | W6 | Different task structures receive recorded method and contribution choices. |

Source:
[section 10](self-organizing-team-domain-model.md#10-build-order).

### Architectural-principle coverage

| Principle | Wave(s) | Completion evidence |
| --- | --- | --- |
| 1. Strategies propose; kernel validates and commits. | W1–W6 | Strategy and adapter negative controls cannot mutate authoritative state. |
| 2. Coordination centers on criteria and evidence, not message volume. | W1, W3 | Every contribution/notice targets criteria or an evidence gap and changes work only through the kernel. |
| 3. Minimal work by default; evidence-based escalation. | W1, W3, W6 | SoloWithVerifier default and recorded diagnosis before growth. |
| 4. Independent attempts and hidden checks precede opinion exchange. | W2, W4 | Hidden/sealed data remains inaccessible until submission/deadline. |
| 5. Verification executes, discriminates, is independent, and has tools. | W1, W2 | CheckRunner, baseline/control/mutant runs, independence, and capability access. |
| 6. Commitments have lifecycle and lease. | W1, W3 | Full P2 transitions appear in TUI and journal. |
| 7. Progress/stalls are explicit and cause replanning. | W1, W3 | EvidenceDelta, loop detection, diagnosis, and plan revision. |
| 8. Calls declare contribution/cost; allocation uses value and calibration. | W1, W3, W5, W6 | Forecast/cost/allowance on every invocation; calibrated A3 decision. |
| 9. Confirmation grades and delayed consequences govern reputation. | W1, W5 | Grade, consequence regrading, credit filter, and decaying reputation. |
| 10. Knowledge is scoped, verifiable, and promoted by trials. | W5 | Provenance/evidence/falsifier and With/Without promotion. |
| 11. Report claims require suitable evidence; causal claims require matched comparison. | W1 | ClaimAuditor accepts, narrows, or replaces each claim. |
| 12. Reviewers differ by family at comparable strength; debate follows the weaker-judge rule. | W3, W4, W6 | Reviewer rationale and debate admission evidence are recorded. |
| 13. Only independent work overlaps; conflicting writes serialize or isolate. | W3, W4 | Timing/path-lock evidence and isolated-attempt negative controls. |
| 14. Permissions are known before verification is chosen. | W1, W2 | CheckDesigner sees actual capabilities; unsupported check design is denied/reassigned. |
| 15. Learning does not delay delivery. | W5 | `ReportDelivered` precedes curation and learning events. |

Source:
[section 1](self-organizing-team-domain-model.md#1-architectural-principles).

### Domain and kernel coverage

| Context | Exact authoritative elements | Wave(s) |
| --- | --- | --- |
| Base | `CostUnits`, `Ref`, `PolicyRef`, `Proposal<T>`, `Denial`, `Result<T>`, `*View` | W1 |
| Identity | `Agent`, `Provider`, `ModelOffering`, `ExecutionProfile`, `Capability`, `Pool` | W1, W3, W6 |
| Task | `Task`, `Goal`, `Assumption`, `Clarification`, `Constraints`, `Pins`, `Criterion`, `AcceptanceContract`, `Check`, `Snapshot`, `Artifact` | W1, W2 |
| Session | `Session`, `Team`, `Membership`, `Method`, `Plan`, `WorkItem` | W1, W3, W6 |
| Execution | `Contribution`, `Assignment`, `Grant`, `Invocation`, `Attempt`, `ResultVersion`, `Workspace` | W1, W4 |
| Coordination | `Board`, `Notice`, `Solicitation`, `Offer`, `Award`, `Commitment`, `Lease`, `Objection`, `Handoff` | W1, W3 |
| Quality | `CheckRun`, `Mutant`, `Evidence`, `Review`, `Acceptance`, `ConfirmationGrade`, `Consequence`, `CriteriaLedger`, `ProgressLedger`, `Claim`, `Report` | W1, W2, W5 |
| Resources | `Budget`, `PriceBook`, `Reservation`, `Receipt`, `Allowance` | W1, W6 |
| Experience | `Observation`, `Reputation`, `CalibrationRecord`, `Knowledge`, `KnowledgeTrial`, `Retrieval` | W5 |
| Kernel | `Journal`, `Registry`, `Treasury`, `WorkspaceGuard`, `Gatekeeper`, `Arbiter`, `AcceptanceAuthority`, `ExperienceVault`, `Dispatcher` | W1, W2, W3, W5 |

Sources:
[domain catalog](self-organizing-team-domain-model.md#30-catalog) and
[kernel services](self-organizing-team-domain-model.md#4-trusted-runtime-services-kernel).

### Exact variant coverage

This table gives every substantive enum/lifecycle branch an explicit home rather
than relying on the phrase “all variants”.

| Model type | Exact variants | Wave(s) |
| --- | --- | --- |
| `Provider.kind` | `Codex`, `Claude`, `Glm`, `Scripted`, `Other(Text)` | W1, W3, W6 |
| `Capability` | `ReadFiles`, `WriteFiles`, `RunProcess`, `TempFiles`, `Sockets`, `Network`, `Browser`, `VcsRead`, `VcsWrite` | W1, W3 |
| `Effort` | Native `Text` value with no shared scale | W1, W6 |
| `CriterionKind` | `NewBehavior`, `Preserve`, `ArtifactPresence`, `Quality`, `Constraint` | W1, W2 |
| Criterion origin | `User`, `Derived(Assignment)` | W1, W3 |
| `CheckSpec` | `Command`, `ExactBytes`, `Property`, `BrowserScenario`, `ExternalQuery` | W1, W2 |
| `CheckAuthor` / `Independence` / visibility | `User`, `Agent(Assignment)`; `ProducerAuthored`, `IndependentVisible`, `IndependentHidden`, `Trusted`; `Visible`, `Hidden` | W1, W2, W3 |
| `SessionStatus` | `Intake`, `Running`, `Finalizing`, `Delivered`, `Blocked(Text)`, `Cancelled` | W1, W6 |
| `MethodKind` | `Solo`, `SoloWithVerifier`, `AsNeededDecomposition`, `IndependentAttempts(k)`, `BreadthResearch(k)` | W1, W3, W4, W6 |
| `WorkState` | `Open`, `Committed`, `Running`, `InReview`, `Accepted`, `Failed`, `Blocked`, `Superseded` | W1, W3, W4 |
| `ContributionKind` | `Plan`, `DesignChecks`, `Produce`, `Verify`, `Review`, `Research`, `Alternative`, `Diagnose`, `Decompose`, `Integrate`, `Clarify`, `Curate`, `Narrate`, `Judge` | W1–W5 |
| `RoleKind` | `Planner`, `CheckDesigner`, `Producer`, `Verifier`, `Reviewer`, `FinalReviewer`, `Researcher`, `Curator`, `Narrator`, `Judge`, `Advocate` | W1–W5 |
| Assignment state | `Admitted`, `Running`, `Finished`, `Revoked` | W1, W3 |
| `TeamOperation` | `BoardRead`, `NoticePost`, `ContributionPropose`, `OfferSubmit`, `CommitmentRelease`, `CommitmentDelegate`, `ObjectionRaise`, `StatusNotify`, `CheckPropose`, `KnowledgePropose` | W1, W3, W5 |
| Invocation terminal / `ErrorClass` | `Completed`, `Failed(ErrorClass)`, `Cancelled`, `TimedOut`; `Infrastructure`, `Environment`, `Protocol`, `Content`, `Unknown` | W1, W3 |
| Attempt outcome | `Pending`, `Submitted`, `Accepted`, `Rejected(Text)`, `Abandoned` | W1, W4 |
| Workspace / lock | `Direct`, `IsolatedCopy(base)`; `Read`, `Write` | W1, W4 |
| Notice addressing/body | `Everyone`, agent address; `Finding`, `Question`, `Answer`, `StatusChange`, `ProposalRef` | W3 |
| `GoalStatusClaim` | `Satisfied`, `Unachievable`, `Irrelevant` | W3 |
| Solicitation visibility/state | `Open`, `Sealed`; `Open`, `Awarded`, `Withdrawn`, `Expired` | W3 |
| Offer source | `RuntimeProxy`, `InAssignment`, `BidAssignment` | W3 |
| `ProgressSignal` | `EvidenceAdded`, `CheckRun`, `ResultSubmitted`, `Heartbeat` | W1, W3 |
| `CommitmentState` | `Proposed`, `Active`, `Discharged`, `Released`, `Expired`, `Cancelled`, `Delegated` | W1, W3 |
| Objection state | `Pending`, `Upheld`, `Dismissed`, `Unresolved` | W3 |
| Check-run role/outcome | `Baseline`, `Candidate`, `Mutant`, `Control`; `Pass`, `Fail`, `Error(ErrorClass)` | W1, W2 |
| `EvidenceClass` / polarity | `StaticRead`, `Executed`, `Browser`, `ExternalData`, `Inspection`; `Supports`, `Contradicts` | W1, W2 |
| Review/finding | `Approve`, `Reject`, `NeedsEvidence`; `Blocking`, `Advisory` | W1, W2 |
| `ConfirmationGrade` | `Refuted`, `Unconfirmed`, `Discriminated`, `Confirmed(TrustedCheck | ExternalData | Consequences)` | W1, W2, W5 |
| Acceptance decision | `Accepted`, `Rejected(Text)` | W1, W2 |
| `Consequence.signal` | `LaterCheckPass`, `LaterCheckFail`, `Revert`, `Reopen`, `UserAccepted`, `UserRejected` | W5 |
| Ledger status | `Unmet`, `Supported`, `Satisfied`, `Contradicted` | W1, W2, W6 |
| Claim kind/audit | `Status`, `Causal`, `Scope`, `Recommendation`; `Valid`, `Unsupported` | W1 |
| Budget unknown policy | `Stop`, `Estimate` | W1, W6 |
| Reservation purpose/state | `Production`, `Verification`, `Coordination`; `Held`, `Settled`, `Released` | W1, W6 |
| Receipt coverage | `Complete`, `Partial`, `Unknown` | W1, W6 |
| `Competence` | `Planning`, `CheckDesign`, `Implementation`, `Verification`, `Research`, `Synthesis` | W5 |
| `Difficulty` / Observation outcome | `Simple`, `Standard`, `Complex`; `Success`, `Failure` | W5 |
| Knowledge kind/status | `Fact`, `Procedure`, `Pitfall`, `Strategy`; `Candidate`, `Hypothesis`, `Validated`, `Confirmed`, `Rejected`, `Superseded`, `Retired` | W5 |
| Trial arm | `With`, `Without` | W5 |
| `DisputeAction` | `RunCounterexample`, `RequestEvidence`, `Debate`, `Escalate` | W3 |
| `Diagnosis` | `Environment`, `CheckDefect`, `CapabilityMismatch`, `ArtifactDefect`, `CapabilityLimit`, `PlanDefect`, `Ambiguity`, `BudgetExhausted`, `Unknown` | W1, W3 |
| `EscalationStep` | `FixEnvironment`, `ReplaceCheck`, `Reassign`, `Retry`, `Decompose`, `AddVerifier`, `AlternativeAttempts`, `StrongerProfile`, `Clarify`, `Replan`, `StopPreserving` | W1, W3, W4, W6 |

Sources:
[identity/task/session/execution variants](self-organizing-team-domain-model.md#31-identity-and-pool),
[coordination variants](self-organizing-team-domain-model.md#35-coordination),
[quality/resource/experience variants](self-organizing-team-domain-model.md#36-quality-assurance),
and [strategy variants](self-organizing-team-domain-model.md#53-self-organization).

### Strategy coverage

| Strategy group | Exact ports and required implementations | Wave(s) |
| --- | --- | --- |
| Readiness/intake/checks | `ReadinessProbe` (StaticDependencyProbe), `IntakePolicy` (CriteriaExtraction + VoiClarification; NoQuestions), `VerificationDesigner` (IndependentHiddenDesigner; ProducerChecks), `MutationStrategy` (FaultInjectionWithEquivalenceFilter; SyntacticOperators; None), `CheckRunner` (ProcessRunner; BrowserRunner; ContainerRunner) | W1, W2 |
| Method/plan/contribution | `MethodRouter` (CascadeRouter; FixedMethod; LearnedRouter), `Planner` (AsNeededDecomposition; UpfrontDecomposition), `ContributionPolicy` (OrdinalValue; VocValue; FixedWorkflow), `BeliefModel` (LikelihoodRatioTable) | W1, W3, W6 |
| Self-organization | `VolunteerPolicy` (ResponseThreshold; RuntimeProxy; AlwaysOffer), `AwardPolicy` (CalibratedValuePerCost; ReputationRank; FirstOffer), `TeamPolicy` (DemandDriven), `ProfilePolicy` (CheapestAdequate), `ReviewerPolicy` (DifferentFamilyComparableStrength; AnyNonProducer), `SelectionPolicy` (DualExecutionAgreement; ReviewerChoice), `DisputePolicy` (CounterexampleFirst) | W1, W3, W4, W6 |
| Progress/escalation | `ProgressMonitor` (EvidenceDelta), `FailureDiagnoser` (RuleBasedDiagnoser; ModelAssistedDiagnoser), `EscalationPolicy` (DiagnosisFirstLadder) | W1, W3 |
| Resources | `CostModel` (PriceWeighted), `ResourcePolicy` (PurposeBounded) | W1, W6 |
| Experience | `CreditPolicy`, `ReputationModel` (BetaWithForgetting; MeanOnly), `CalibrationScorer` (BrierIsotonic), `KnowledgeCurator` (AfterActionReview), `RetrievalPolicy` (ScopedLexical), `TrialPolicy` (AlternatingArms), `ConsequenceSource` (LaterChecks; VcsReverts; UserFeedback) | W5, W6 |
| Context/report/workspace/execution | `ContextComposer` (CriteriaProjection + JournalDigest), `NarrativeComposer`, `ClaimAuditor` (EvidenceClassRules), `WorkspaceProvider` (Direct; CopyOnWrite), `ExecutionBackend` (Codex; Claude; Glm; Scripted) | W1, W3, W4, W6 |

Source:
[section 5](self-organizing-team-domain-model.md#5-replaceable-strategies-port).

### Protocol and algorithm coverage

| Exact model behavior | Wave(s) | Observable completion evidence |
| --- | --- | --- |
| P1 solicitation and voluntary response | W3, W6 | Eligible offers, award, reopen, member addition/diagnosis. |
| P2 commitment lifecycle | W1, W3 | Every transition and one-active-commitment invariant. |
| P3 goal-status notice | W3 | Verify, diagnose/escalate, or revise/cancel response. |
| P4 objection with counterexample | W3 | Upheld/dismissed/unresolved and evidence/debate/escalation branches. |
| P5 context handoff | W3 | Bounded digest on release/delegation/expiry/reassignment. |
| P6 user clarification | W1, W3, W6 | Ask or record assumption according to value of information. |
| P7 independence window | W2–W4 | Hidden/sealed information remains unavailable until closure. |
| A1 Dispatcher loop | W1–W6 | Full loop in W1; all dynamic wakeups and learning complete by W6. |
| A1.1 CascadeRouter | W6 | Solo, decomposition, breadth, and independent-attempt routing cases. |
| A2 check design/discrimination | W2 | Premortem, baseline expectation, replacement/fix-environment branch. |
| A3 ContributionPolicy | W1, W3, W6 | Ordinal selection first; calibrated value per cost final. |
| A4 ResponseThreshold | W3, W6 | Stimulus/load/reputation probability and calibrated parameters. |
| A5 assignment execution | W1, W3, W5 | Retrieval, context, backend, progress, denial, receipt, and credit filtering. |
| A6 result verification | W1, W2 | Candidate checks, mutants, evidence, review, objection, acceptance. |
| A7 acceptance/regrading | W1, W2, W5 | Rejection precedence, grades, and consequence transitions. |
| A8 criteria ledger | W1, W2, W6 | Correlated grouping, thresholds, required evidence classes. |
| A9 progress/diagnosis/escalation | W1, W3 | Stall/loop cases cover every diagnosis and next step. |
| A10 independent attempts | W4 | Isolated candidates, test matrix, cluster, review, merge/no-admissible branch. |
| A11 finalization/report audit | W1, W3 | Integrated snapshot, final review, claim audit, unmet/assumption report. |
| A12 experience | W5, W6 | Credit, reputation, calibration, knowledge, retrieval, trials, consequences. |
| A13 budget | W1, W6 | Price calculation, reserve protection, settlement, unknown usage, stop rule. |

Sources:
[protocols](self-organizing-team-domain-model.md#6-protocols) and
[semi-algorithms](self-organizing-team-domain-model.md#7-semi-algorithms).

### Invariant coverage

| Invariants | Wave(s) | Required evidence |
| --- | --- | --- |
| R-1: only the kernel changes state; strategies return attributed proposals. | W1 | Strategy and adapter attempts to mutate state are refused. |
| R-2: roles and authority exist only inside one assignment. | W1, W3 | A finished or revoked assignment cannot reuse its role or grant. |
| R-3: assignments obey Pins and Constraints. | W1, W3 | Pin, capability, limit, and budget violation controls are denied. |
| R-4: a Reviewer is not the result producer. | W1, W2 | Producer/reviewer identity collision blocks acceptance. |
| R-5: a FinalReviewer is outside the final producer set. | W1, W3 | No eligible final reviewer produces a truthful Blocked session. |
| R-6: applicable executable failure overrides approvals. | W1, W2 | Contradicting Executed/Browser/ExternalData evidence rejects. |
| R-7: verification reserve is unavailable to Production/Coordination. | W1, W6 | Production reservation cannot consume the protected amount. |
| R-8: every invocation is charged, including coordination, verification, learning, and reporting. | W1, W5, W6 | Receipts reconcile to the session totals for every role. |
| R-9: contribution needs fit actual assignment permissions. | W1, W3 | Missing capability denies admission or records ToolDenied. |
| R-10: hidden check isolation and author exclusion | W2–W4 | Data-leak and role-collision controls. |
| R-11: success/failure observations follow qualified credit and diagnosis rules. | W5 | Non-creditable outcome matrix creates no observation. |
| R-12: an assignment has exactly one active commitment. | W1, W3 | Duplicate responsibility denial and lifecycle transitions. |
| R-13: unsupported objection cannot override a passing applicable check. | W3 | Opinion-only objection control. |
| R-14: member/profile/effort growth requires diagnosis. | W3, W6 | Unjustified resource increase is denied. |
| R-15: learning and knowledge preparation follow report delivery. | W5 | Journal event ordering. |
| R-16: accepted result versions remain immutable and survive assignment failure/revocation. | W1, W4 | Revocation/failure does not remove accepted versions. |
| R-17: every decision is reconstructable from the journal. | W1–W6 | Replay uses input digest, strategy, proposal, and decision. |
| R-18: knowledge retains provenance, scope, basis, and stronger-basis supersession. | W5 | Invalid promotion and weak supersession are denied. |

Source:
[section 8](self-organizing-team-domain-model.md#8-invariants).

### Journal-event coverage

| Event group | Exact events | Wave(s) |
| --- | --- | --- |
| Session/intake | `SessionOpened`, `BudgetOpened`, `CriteriaCommitted`, `ClarificationRecorded`, `AssumptionRecorded` | W1 |
| Checks/method/plan | `CheckRegistered`, `CheckRunRecorded`, `MutantRecorded`, `MethodChosen`, `PlanCommitted`, `PlanRevised` | W1, W2, W3 |
| Contribution/coordination | `ContributionProposed`, `SolicitationOpened`, `SolicitationChanged`, `OfferSubmitted`, `Awarded` | W1, W3 |
| Authority/resources/workspace | `AssignmentAdmitted`, `AssignmentRevoked`, `GrantIssued`, `CommitmentChanged`, `ReservationChanged`, `WorkspaceOpened`, `LockChanged`, `SnapshotTaken`, `ResultMerged` | W1, W4 |
| Execution/result/quality | `InvocationStarted`, `InvocationEnded`, `ReceiptSettled`, `ResultSubmitted`, `EvidenceRecorded`, `ReviewRecorded`, `ObjectionRaised`, `ObjectionResolved`, `AcceptanceRecorded`, `Regraded`, `LedgerUpdated` | W1–W5 |
| Progress/team/report | `ProgressAssessed`, `Diagnosed`, `Escalated`, `TeamChanged`, `NoticePosted`, `HandoffCreated`, `ReportDelivered` | W1, W3 |
| Experience | `ObservationRecorded`, `ReputationUpdated`, `CalibrationRecorded`, `KnowledgeChanged`, `TrialRecorded`, `RetrievalRecorded`, `ConsequenceIngested` | W5, W6 |

Every event uses the common `Envelope` with sequence, session, time, actor,
optional policy/input, references, and payload
([section 9](self-organizing-team-domain-model.md#9-journal-events)).

## Final completeness statement

The six-wave plan covers:

- all 15 architectural principles;
- all domain structures and lifecycle variants in section 3;
- all nine trusted kernel services in section 4;
- every strategy port, named default, and named alternative in section 5;
- protocols P1–P7;
- algorithms A1, A1.1, and A2–A13;
- invariants R-1–R-20;
- every minimum journal event and the event envelope;
- build stages 1–10;
- all parameters in section 10.1;
- Codex, Claude, Glm, and Scripted backend implementations;
- the `CreditPolicy` choice and calibration obligations from section 12;
- the evolving TUI as the user-facing adapter, with core mechanisms observable
  through real consumers rather than mandatory per-wave screen redesigns.

Nothing from another repository document, a historical YMP iteration, an
external publication, or a proposed amendment is used to define this list.
No TUI feature creates a second authority or enlarges the domain model. W6 owns
all remaining integration and calibration work; no authoritative capability is
deferred beyond it.

## Real unspecified implementation choices

These choices are genuinely left open by the authoritative model. They must be
resolved during the owning waves without narrowing its scope:

1. mapping approved domain/service/strategy names to concrete Rust types and
   crate boundaries;
2. concrete `Store` persistence, projection, and migration mechanisms;
3. evolving TUI layout, navigation, streaming, and accessibility behavior while
   preserving the broad Ratatui direction, projections and authority rules;
4. native protocols for Codex, Claude, and Glm adapters and how each reports
   readiness, capabilities, settings, progress, cancellation, and receipts;
5. `PriceBook` data source and normalization into `CostUnits`;
6. Direct and CopyOnWrite workspace implementations and validated merge;
7. ProcessRunner, BrowserRunner, ContainerRunner, property generation,
   mutation, and equivalence implementations;
8. experiment-specific `CreditPolicy` selection, including the default
   Confirmed-only and the variant admitting `Discriminated`;
9. parameter values in section 10.1 and the separated evidence used to
   calibrate them;
10. consequence polling cadence and the storage/indexing implementation for
    reputation, calibration, knowledge, trials, and retrieval.

These are implementation decisions, not permission to omit an element or import
a replacement architecture.

## Research-process note

The previously installed `autoresearch`, `brainstorming-research-ideas`, and
`creative-thinking-for-research` skills were used in a bounded way to
reconsider the wave count, distinguish infrastructure stages from visible
product increments, test whether a TUI-only or team-first decomposition would
hide dependencies, and audit coverage. No skill was reinstalled, no recurring
schedule was created, and no model-training or provider experiment was started.
The research methods influenced ordering only; the authoritative model and the
explicit TUI requirement alone define product scope.
