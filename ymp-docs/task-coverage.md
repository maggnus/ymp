# Architecture-to-task coverage

This is a traceability map and decomposition rationale. The JSON records under
[tasks/records](tasks/records/) own task definitions and status; this file copies
neither status nor full task bodies. Read a task with `manage.py show ID`.

## Authority and delivery shape

The approved [self-organizing team model](self-organizing-team-domain-model.md)
defines architecture and product scope, including the final experimental rules
from `811a872` and the greenfield starting point from `0c61ff1`.
Useful self-organization is the central product question: agent initiative,
offers, commitments and their transfer, communication, objections, and team/plan
adaptation must change actual work. User tasks make their benefits, failures and
costs observable. A fixed producer/reviewer workflow is a foundation and control,
not the final product definition.

[waves_ideas.md](waves_ideas.md) supplies the six-wave order. A usable Ratatui TUI
retains broad chat, commands and information-panel traits while its layout stays
open. Key mechanisms are interchangeable through the model's ports (§5.8), not
only tunable coefficients. Run selections retain implementation/version,
recoverable effective parameters, input-view digest and basis. Authorized changes
within a session occur at recorded work boundaries and preserve earlier history.

| Wave / build stages | Concrete implementation owners | Executable outcome |
| --- | --- | --- |
| W1 / 1–5 | W1-0001–W1-0018, including W1-0015 interface | Accountable producer/reviewer session, criterion ledger, recovery and audited report through a simple TUI. |
| W2 / 6 | W2-0001–W2-0004 | Independently designed hidden checks and measured discrimination. |
| W3 / 7 | W3-0001–W3-0008 | Grant-scoped communication, voluntary allocation and evidence-driven self-organization. |
| W4 / 8 | W4-0001–W4-0003 | Sealed isolated alternatives, complete test matrix, justified selection and merge. |
| W5 / 9 | W5-0001–W5-0005 | Qualified experience, tested knowledge and delayed correction. |
| W6 / 10 | W6-0001–W6-0005 | All native backends, calibrated routing/allocation and reproducible policy evaluation. |

These ranges identify wave composition only; detailed ownership below uses
specific IDs. No wave boundary is an implicit dependency. The records contain the
actual graph. There are 43 tasks: W1 has 18, W2 has 4, W3 has 8, W4 has 3,
W5 has 5 and W6 has 5. W1 creates the entire foundation and first user journey.
The in-memory Journal (W1-0001), durable Journal/content (W1-0016), admission
(W1-0006), execution host/Scripted backend (W1-0017), and complete native Codex
backend (W1-0018) are separate behavior/ownership boundaries. Explicit dependencies
and priorities place these later-numbered IDs before their consumers. W4 has fewer because workspace isolation,
attempt production/matrix execution, and selection/merge are its three real
consumer boundaries.

Each task combines implementation, meaningful checks and affected documentation.
There are no separate enum/file tasks, repeated audit chores, or mandatory TUI
redesigns. W1-0014, W2-0004, W3-0008, W4-0003, W5-0005 and W6-0004 integrate
their wave demonstrations with substantive runtime behavior. Later features expose
their state through prerequisite projections, commands, artifacts or the simple TUI;
UI polish is not required to establish a kernel mechanism.

### Greenfield starting point and proposed implementation map

Only the approved documents and development-task tool are available as a planning
basis. No application code, API, fixture, previous result or runtime behavior is
credited as delivered. All paths below are **proposed target modules**, not claims
that the new implementation exists. W1-0001 owns the initial Rust mapping;
additional crates appear only when their concrete capability is built. No product
files are changed by this decomposition.

| Proposed module family under `crates/` | New implementation owner and prerequisite boundary |
| --- | --- |
| `ymp-domain/src/{lib,journal}.rs`, `ymp-kernel/src/journal.rs`, `ymp-runtime/src/memory_journal.rs` | W1-0001 creates pure values, trusted Journal/Envelope/replay, initial concrete consumer and MemoryJournal. |
| `ymp-domain/src/task.rs`, `ymp-kernel/src/intake.rs` | W1-0002 creates explicit user intake and provenance representation. W1-0011 proves actual charged Planner → Derived(Assignment). |
| `ymp-storage/src/{journal,content}.rs` | W1-0016 creates durable Journal and recoverable content/parameters. Embedded SQLite is a proposed implementation choice, not delivered infrastructure. |
| `ymp-domain/src/identity.rs`, `ymp-kernel/src/registry.rs`, `ymp-runtime/src/readiness.rs` | W1-0003 creates identity/offerings/Registry/probes; actual executor owners are listed separately. |
| `ymp-kernel/src/{treasury,workspace_guard,gatekeeper}.rs` | W1-0004/0005/0006 create independent accounting, Direct workspace ownership and atomic admission. |
| `ymp-kernel/src/arbiter.rs`, `ymp-domain/src/coordination.rs` | W1-0006/0007 create minimal award and P2 transition consumer; W3-0001/0003/0006 build full communication/allocation/dispute behavior. |
| `ymp-runtime/src/{execution_host,clock,backends/scripted}.rs` | W1-0017 creates bounded execution, observations/receipts/recovery and Scripted; connects real non-artifact discharge. |
| `ymp-runtime/src/backends/codex.rs` and focused protocol modules | W1-0018 builds complete native Codex discovery/execution and new fixtures. No ready App Server or prior adapter is assumed. |
| `ymp-domain/src/{plan,result,verification}.rs`, `ymp-kernel/src/{results,acceptance,ledger}.rs` | W1-0008/0009/0010 create immutable results, checks/evidence and A7/A8. W1-0010 connects real acceptance to P2 artifact discharge. |
| `ymp-runtime/src/policies/`, `ymp-kernel/src/{plans,progress,finalization}.rs` | W1-0011/0012/0013 create planning, diagnosis, context, independent finalization and audited narration/fallback. Later policy tasks add concrete implementations through the same contracts. |
| `ymp-runtime/src/{application,dispatcher}.rs`, `ymp-cli/src/{main,lib,tui}.rs` | W1-0014/0015 create the actual session loop and simple terminal product; W3-0008/W6-0004 add dynamic/value-based orchestration. |
| `ymp-runtime/src/checks/`, `workspace/`, `attempts.rs` | W2-0001–0004 create independent/discriminating checks; W4-0001–0003 create isolated alternatives and selection/merge. |
| `ymp-kernel/src/experience_vault.rs`, `ymp-runtime/src/consequences.rs` | W5-0001–0005 create qualified experience, calibration, knowledge/trials/retrieval and consequences. |
| `ymp-runtime/src/backends/{claude,glm}.rs` | W6-0001 and W6-0002 build complete native Claude and Glm adapters using the newly built host contract. |

Compatibility applies as this new implementation evolves. No migration from a
previous product, prior journal/API, or historical test suite is an implicit task.
Each feature includes its concrete schema/projection/consumer integration; the
initial layout does not create empty crates for future capabilities.

## Domain structures and exact variants

Source: model [section 3](self-organizing-team-domain-model.md#3-domain-structures).
The owner of a structure includes its substantive fields, references and replay
behavior; listed variants identify branch coverage, not separate tasks.

| Structure/family and important branches | Concrete owners |
| --- | --- |
| Base `Id<T>`, SHA-256 `Digest`, versioned `Ref`, `PolicyRef`, `Proposal<T>`, `Denial`, `Ok/Denied`, deterministic `*View`; validated time/probability/numeric values | W1-0001; durable refs/parameter content W1-0016; value-specific validation W1-0002, W1-0004 |
| Agent; Provider `Codex/Claude/Glm/Scripted/Other(Text)`; ProfileSettings; ModelOffering; ExecutionProfile; Pool and exclusion reasons; native `Effort = Text` | W1-0003; execution Scripted W1-0017, Codex W1-0018, Claude W6-0001 and Glm W6-0002 |
| Capability `ReadFiles/WriteFiles/RunProcess/TempFiles/Sockets/Network/Browser/VcsRead/VcsWrite` | W1-0003, W1-0005, W1-0006; concrete browser W2-0003, isolation W4-0001 |
| Task, Goal, Assumption, Clarification, Constraints, Pins; Criterion with CriterionKind `NewBehavior/Preserve/ArtifactPresence/Quality/Constraint`, origin `User/Derived(Assignment)`, weight/required/needs_class; versioned AcceptanceContract | W1-0002, W1-0011; work-boundary P6 and revisions W3-0004 |
| Check; CheckSpec `Command/ExactBytes` | W1-0009 |
| CheckSpec `Property`, `BrowserScenario`, `ExternalQuery` | W2-0002 (Property), W2-0003 (BrowserScenario/ExternalQuery) |
| CheckAuthor `User/Agent(Assignment)`; Independence `ProducerAuthored/IndependentVisible/IndependentHidden/Trusted`; visibility `Visible/Hidden` | W1-0009; hidden design/author exclusion W2-0001; counterexample authoring W3-0006 |
| Snapshot, Artifact, immutable content and version refs | W1-0005/W1-0016 content, W1-0008 results; isolated/merged snapshots W4-0001 |
| Session; SessionStatus `Intake/Running/Finalizing/Delivered/Blocked(Text)/Cancelled` | W1-0002 representations, W1-0014 transitions/recovery; dynamic continuation W3-0008 and W6-0004 |
| Team, Membership participation intervals/reasons and revision | W1-0011 initial team; W3-0002 dynamic changes |
| Method/ladder/PolicyRef; MethodKind `Solo/SoloWithVerifier/AsNeededDecomposition/IndependentAttempts(k)/BreadthResearch(k)` | W1-0011 fixed SoloWithVerifier; W3-0004 decomposition; W4-0002 independent attempts; W6-0003 all methods and routing |
| Plan graph/version/author; WorkItem targets/deps/needs/writes/parent/accepted; WorkState `Open/Committed/Running/InReview/Accepted/Failed/Blocked/Superseded` | W1-0008 result-linked graph, W1-0011 planning, W3-0004 revisions/state, W3-0008 concurrent execution |
| Contribution; Forecast `Agent(ExecutionProfile)/Model(PolicyRef)`; CostEstimate expected/p90; proposed_by `Runtime/Agent(Assignment)` | W1-0004, W1-0006; agent proposal flow W3-0001 and value selection W6-0004 |
| ContributionKind `Plan/Produce/Verify/Review/Diagnose/Decompose/Clarify/Narrate` | W1-0006, W1-0011/0012/0013; complete Decompose/Clarify W3-0004/0007 |
| ContributionKind `DesignChecks/Research/Alternative/Integrate/Curate/Judge` | W2-0001; W3-0007 and W6-0003 Research; W4-0002 Alternative; W6-0003 Integrate; W5-0003 Curate; W3-0006 Judge |
| RoleKind `Planner/CheckDesigner/Producer/Verifier/Reviewer/FinalReviewer/Researcher/Curator/Narrator/Judge/Advocate` | W1-0006 typed admission; substantive consumers W1-0011/0013, W2-0001, W3-0006/0007, W4-0002, W5-0003, W6-0003/0004 |
| Assignment states `Admitted/Running/Finished/Revoked`; Grant scoped operations/expiry/token; Invocation native session/requested/sent/reported/start/end/receipt | W1-0006 admission, W1-0017 actual invocation; grant transport W3-0001 |
| TeamOperation `BoardRead/NoticePost/ContributionPropose/StatusNotify` | W3-0001; P3 response W3-0004 |
| TeamOperation `OfferSubmit/CommitmentRelease/CommitmentDelegate/ObjectionRaise/CheckPropose/KnowledgePropose` | W3-0001 transport; actual owners W3-0003, W1-0007/W3-0005, W3-0006, W2-0001/W3-0006, W5-0003 respectively |
| Invocation terminals `Completed/Failed(ErrorClass)/Cancelled/TimedOut`; ErrorClass `Infrastructure/Environment/Protocol/Content/Unknown` | W1-0017 lifecycle; W1-0018/W6-0001/W6-0002 native mappings; W1-0012 diagnosis |
| Attempt `Pending/Submitted/Accepted/Rejected(Text)/Abandoned`; ResultVersion producer/profile/before/after/artifacts | W1-0008; acceptance W1-0010; competing attempts W4-0002/0003 |
| Workspace `Direct/IsolatedCopy(base)`; PathLock `Read/Write` | W1-0005 Direct, W4-0001 IsolatedCopy/merge |
| Board; Notice to `Everyone/agent`, body `Finding/Question/Answer/StatusChange/ProposalRef`; GoalStatusClaim `Satisfied/Unachievable/Irrelevant` | W3-0001 storage/transport; W3-0004 work response |
| Solicitation visibility `Open/Sealed`, state `Open/Awarded/Withdrawn/Expired`, stimulus/deadline/eligible/reopened; Offer and Award | W1-0006 fixed baseline; full W3-0003, sealed attempts W4-0002 |
| Offer sources `RuntimeProxy/InAssignment/BidAssignment` | W3-0003 |
| Commitment debtor/creditor/subject/condition/history; Lease; ProgressSignal `EvidenceAdded/CheckRun/ResultSubmitted/Heartbeat`; CommitmentState `Proposed/Active/Discharged/Released/Expired/Cancelled/Delegated` | W1-0006 admission and W1-0007 transition consumer; W1-0017 actual non-artifact and W1-0010 artifact discharge; W3-0001 native operations |
| Objection against `ResultVersion/Plan/Check`, state `Pending/Upheld/Dismissed/Unresolved`; DisputeAction `RunCounterexample/RequestEvidence/Debate/Escalate` | W3-0006 |
| Handoff; ContextDigest decisions/open_questions/evidence/summary | W3-0005 |
| CheckRun roles `Baseline/Candidate/Mutant/Control`, outcomes `Pass/Fail/Error`, stdout/stderr/env digests | W1-0009 baseline/candidate/control; W2-0004 mutant; complete cross-matrix W4-0002 |
| Mutant base/patch/operator/equivalent `true/false/unknown`; Discrimination baseline_fails/candidate_passes/mutation_score | W2-0004 |
| `EvidenceClass` `StaticRead/Executed/Browser/ExternalData/Inspection`; polarity `Supports/Contradicts`; author/result/criterion/run attribution | W1-0009; Browser/ExternalData W2-0003; grade use W1-0010/W2-0004 |
| Review `Approve/Reject/NeedsEvidence`; Finding `Blocking/Advisory`, criterion and proposed_check | W1-0009/0010; dispute review W3-0006 |
| Acceptance subject `ResultVersion/FinalAggregate`, decision `Accepted/Rejected`; per-criterion/minimum ConfirmationGrade `Refuted/Unconfirmed/Discriminated/Confirmed(TrustedCheck/ExternalData/Consequences)` | W1-0010/0013; Discriminated W2-0004, ExternalData W2-0003, consequences/refutation W5-0005 |
| CriteriaLedger/LedgerEntry `Unmet/Supported/Satisfied/Contradicted`, subject-bound belief/evidence/stimulus/unmet_since/changed, neutral default prior and applicable Supports coverage | W1-0010; stimulus W3-0003, calibrated allocation W6-0004 |
| ProgressLedger/ProgressRecord progress/satisfaction/looping/diagnosis/stall count | W1-0012; complete recovery W3-0007 |
| Claim kinds `Status/Causal/Scope/Recommendation`, audits `Valid/Unsupported`; Report claims/unmet/assumptions/grade | W1-0013; Property scope evidence W2-0002 |
| Budget with verification_reserve and policy-derived reporting_reserve, PriceBook/Rates, CostUnits; unknown_usage `Stop/Estimate`; Reservation purpose `Production/Verification/Coordination/Reporting`, state `Held/Settled/Released`; Receipt coverage `Complete/Partial/Unknown`; Usage and Allowance | W1-0004 policies/accounting, W1-0006 admission, W1-0017 receipts; W1-0013 Reporting/fallback; full integration W6-0004 |
| Observation `Success/Failure`, complete ExecutionProfile key; Competence `Planning/CheckDesign/Implementation/Verification/Research/Synthesis`; Difficulty `Simple/Standard/Complex`; Reputation alpha/beta/updated | W5-0001 |
| CalibrationRecord kinds `Success/Cost`, forecast/outcome/score | W5-0002; empirical comparisons W6-0005 |
| Knowledge kinds `Fact/Procedure/Pitfall/Strategy`; Scope; provenance/evidence/falsifier/supersedes; KnowledgeStatus `Candidate/Hypothesis/Validated/Confirmed/Rejected/Superseded/Retired` | W5-0003; trial-based Validated W5-0004, consequence retirement W5-0005 |
| KnowledgeTrial arms `With/Without`; predefined metric/value; Retrieval included exact digests and excluded reasons | W5-0004 |
| Consequence signals `LaterCheckPass/LaterCheckFail/Revert/Reopen/UserAccepted/UserRejected` with subject/source/evidence/time | W5-0005; Reopen is recorded for investigation, not invented as a pass/fail transition |
| Diagnosis `Environment/CheckDefect/CapabilityMismatch/ArtifactDefect/CapabilityLimit/PlanDefect/Ambiguity/BudgetExhausted/Unknown` | W1-0012 rules, W3-0007 complete execution/model alternative |
| EscalationStep `FixEnvironment/ReplaceCheck/Retry/AddVerifier/StopPreserving` | W1-0012, completed orchestration W3-0007 |
| EscalationStep `Reassign/Decompose/StrongerProfile/Clarify/Replan/AlternativeAttempts(k)` | W3-0007 with W3-0002/0004/0005; AlternativeAttempts W4-0003 |

Contribution-to-role mappings use existing model names. W6-0004 states the complete
mapping; in particular artifact Integrate work is Producer work, and P4 Advocates
are admitted Research contributions. Roles never become permanent Agent properties.

## Trusted service operations

Source: model [section 4](self-organizing-team-domain-model.md#4-trusted-runtime-services-kernel).

| Kernel service | Operation owners and real consumer |
| --- | --- |
| Journal | W1-0001 MemoryJournal/Envelope/replay; W1-0016 durable append/content/restart; each feature owns its payloads. New runtime sessions consume both adapters. |
| Registry | W1-0003 pool/profile/capabilities using readiness and native metadata; Gatekeeper and team policies consume them. |
| Treasury | W1-0004 open/reserve/settle/release_unstarted/remaining, including derived Reporting capacity; W1-0006 admission; W1-0017 actual receipt/cessation separation; W1-0014/W6-0004 stopping. |
| WorkspaceGuard | W1-0005 Direct open/lock/snapshot/release(basis), using W1-0016 content; W1-0017 actual cessation integration; W4-0001 IsolatedCopy/merge. |
| Gatekeeper | W1-0006 admit/revoke, atomic grant/commitment/reservation/locks; W1-0007 lifecycle; W2-0001 author exclusion; W3-0002 dynamic pins/growth. |
| Arbiter | W1-0006 minimal open/submit/award, W1-0007 commitment/tick; W3-0001 notice/propose; W3-0003 full open/submit/award; W3-0006 objection. |
| AcceptanceAuthority | W1-0009 register_check/run/evidence; W1-0010 accept/ledger; W1-0013 finalize; W2-0001/0004 independent design/discrimination; W5-0005 regrade. |
| ExperienceVault | W5-0001 observe/reputation; W5-0002 forecast_outcome; W5-0003 knowledge Propose/Promote/Supersede/Retire; trial promotion W5-0004; consequences W5-0005. |
| Dispatcher | W1-0014 run baseline; W3-0008 dynamic coordination; W4-0003 alternatives; W5-0003/0005 asynchronous learning/consequences; W6-0004 complete loop. |

## Strategies, defaults and alternatives

Source: model [section 5](self-organizing-team-domain-model.md#5-replaceable-strategies-port).
Each owning task exercises the strategy through its actual service/Dispatcher
consumer with at least two materially different implementations. Where the model
names only a default, a concrete deterministic or adversarial test implementation
provides the second behavior; it is not an invented product subsystem. W6-0005
collects reproducible comparisons, not replacement tests deferred from early waves.

| Port and exact implementation names | Task owners |
| --- | --- |
| ReadinessProbe — StaticDependencyProbe | W1-0003 |
| IntakePolicy — CriteriaExtraction + VoiClarification; NoQuestions | W1-0011, P6 consumer W3-0004 |
| VerificationDesigner — IndependentHiddenDesigner; ProducerChecks | W2-0001 |
| MutationStrategy — FaultInjectionWithEquivalenceFilter; SyntacticOperators; None | W2-0004 |
| CheckRunner — ProcessRunner; BrowserRunner; ContainerRunner | W1-0009; W2-0003 BrowserRunner; W2-0002 ContainerRunner |
| MethodRouter — FixedMethod; CascadeRouter; LearnedRouter | W1-0011 FixedMethod; W6-0003 all alternatives |
| Planner — AsNeededDecomposition; UpfrontDecomposition | W1-0011; full revision/alternative W3-0004 |
| ContributionPolicy — OrdinalValue; FixedWorkflow; VocValue | W1-0011; W6-0004 VocValue and integration |
| BeliefModel — LikelihoodRatioTable | W1-0010; discrimination W2-0004; calibration W6-0005 |
| VolunteerPolicy — ResponseThreshold; RuntimeProxy; AlwaysOffer | W3-0003 |
| AwardPolicy — CalibratedValuePerCost; ReputationRank; FirstOffer | W3-0003; learned inputs W5-0002/W6-0004 |
| TeamPolicy — DemandDriven; TeamChange add/remove/reason | W3-0002 |
| ProfilePolicy — CheapestAdequate | W3-0002; learned inputs W5-0002/W6-0004 |
| ReviewerPolicy — AnyNonProducer; DifferentFamilyComparableStrength | W1-0013; W3-0002 family/strength selection |
| SelectionPolicy — DualExecutionAgreement; ReviewerChoice | W4-0003 |
| DisputePolicy — CounterexampleFirst | W3-0006 |
| ProgressMonitor — EvidenceDelta | W1-0012 |
| FailureDiagnoser — RuleBasedDiagnoser; ModelAssistedDiagnoser (Researcher) | W1-0012; W3-0007 model-assisted consumer |
| EscalationPolicy — DiagnosisFirstLadder | W1-0012; W3-0007/W4-0003 full action handlers |
| CostModel — PriceWeighted; ResourcePolicy — PurposeBounded, including reporting_reserve(task_view,pool_view) | W1-0004; complete policy replacement and resolved parameter evidence W6-0005 |
| CreditPolicy — Confirmed(*) only; experimental Discriminated | W1-0010 policy; W5-0001 actual observation consumer |
| ReputationModel — BetaWithForgetting; MeanOnly | W5-0001 |
| CalibrationScorer — BrierIsotonic, raw/uncalibrated branch | W5-0002 |
| KnowledgeCurator — AfterActionReview (Curator) | W5-0003 |
| RetrievalPolicy — ScopedLexical; TrialPolicy — AlternatingArms | W5-0004 |
| ConsequenceSource — LaterChecks; VcsReverts; UserFeedback | W5-0005 |
| ContextComposer — CriteriaProjection + JournalDigest | W1-0013; hidden protection W2-0001; handoff W3-0005; retrieval W5-0004 |
| NarrativeComposer — admitted Narrator charged to Reporting; DeterministicReport alternative/fallback; ClaimAuditor — EvidenceClassRules | W1-0013, zero-call user-stop integration W1-0014 |
| WorkspaceProvider — Direct; CopyOnWrite | W1-0005; W4-0001 |
| ExecutionBackend — Scripted; Codex; Claude; Glm | W1-0017 host/Scripted; W1-0018 complete Codex; W3-0001 team transport; W6-0001 Claude; W6-0002 Glm |

### Experimental interface contract (§5.8)

W1-0001 and W1-0016 retain implementation/version, resolvable parameter values,
input digest, proposal and basis. W1-0011 creates actual run-level selection of
the first named strategies. Each later owner installs concrete alternate
implementations through that same consumer contract. W3-0008 and W6-0004 handle
authorized changes at recorded work boundaries; W6-0005 compares their effects.

The obligation applies to the assessment method, cost/resource allocation,
contribution/offer selection, team/profile choice, commitment policy inputs,
verification/discrimination, diagnosis/escalation, reputation and knowledge use.
A factor may be computed differently from history/current conditions through its
owning strategy instead of remaining a scalar constant. Replacing an algorithm
must not require changing its kernel consumer. No per-number interface wrapper,
generic service container or extra TUI form for every setting is required.

Named/validated/defaulted parameters, native model/effort choices and experiment
conditions are explicit and recoverable; a bare hash does not suffice. Both
CreditPolicy implementations are available for experiment selection and credited
observations retain that choice. New selections do not rewrite earlier decisions,
observations or reputation history, and cannot disable kernel invariants.

## Protocols and algorithms

| Exact model behavior | Concrete owners and observable result |
| --- | --- |
| P1 | W3-0001 proposal transport; W3-0003 all offer sources, deadlines, thresholds, awards/reopen; W3-0002 capability/independence repair. |
| P2 | W1-0006 admission; W1-0007 transition consumer; real non-artifact discharge W1-0017 and artifact acceptance/discharge W1-0010/0014. |
| P3 | W3-0004 status claims cause Verify, diagnose/escalate, or revise/cancel at the next boundary. |
| P4 | W3-0006 counterexample Fail/Pass/Error and request-evidence/debate/escalation paths, with weaker judge and R-13. |
| P5 | W3-0005 bounded handoff on release/delegation/expiry/successor, including paid Narrator fallback. |
| P6 | W1-0002/0011 intake values/proposals; W3-0004 real ask/assume/answer/resume and report attribution. |
| P7 | W2-0001 CheckDesigner window; W3-0001 participant Board projections; W4-0002 attempt submission/deadline closure. Hidden checks stay out of producer context after closure. |
| A1 | W1-0014 minimal loop; W3-0008 concurrent coordination; W5-0003/0005 post-delivery learning/consequence events; W6-0004 complete loop. |
| A1.1 | W6-0003 exact CascadeRouter branches, features, ladder and executable methods. |
| A2 | W2-0001 premortem, baseline expectations, Error/replacement and author isolation; W2-0004 integration. |
| A3 | W1-0011 templates/OrdinalValue; W3-0001 agent proposals; W6-0004 calibrated gain/cost/filters/stopping. |
| A4 | W3-0003 stimulus, threshold/load/reputation and recorded random decision; calibrated inputs W5-0002/W6-0004. |
| A5 | W1-0006 admission, W1-0017 execution/receipt, W1-0018 Codex; W1-0013 context, W3-0005 handoff, W5-0004 retrieval and W5-0001 credit filtering. |
| A6 | W1-0008/0009 result/checks, W1-0010 review/acceptance; W2-0004 mutation/complete verification; W3-0006 objections. |
| A7 | W1-0010 acceptance/grades; W2-0004 discrimination; W5-0005 exact-artifact regrading/corroboration. |
| A8 | W1-0010 subject:Ref, applicability before polarity, prior=0.5 default, criterion/result-scoped experimental prior, correlated grouping and positive Supports coverage; W2-0004 discrimination. |
| A9 | W1-0012 default diagnosis/progress; W3-0007 handlers including StrongerProfile; W4-0003 actual Decompose → AlternativeAttempts → StrongerProfile sequence; W6-0005 evaluation. |
| A10 | W4-0001 isolation; W4-0002 candidates and full H ∪ T_i matrix; W4-0003 admissible clusters/reviewer/merge/no-admissible diagnosis. |
| A11 | W1-0013 stable aggregate/final review/applicable claim audit, bounded paid narration/correction and DeterministicReport; W1-0014 no model calls after stop; W4-0003/W6-0004 integration. |
| A12 | W5-0001 credit/reputation, W5-0002 forecasts, W5-0003 asynchronous curation/lifecycle, W5-0004 trials/retrieval, W5-0005 consequences. |
| A13 | W1-0004 derived reporting capacity, both reserves, Partial/Unknown, idempotent settlement/release_unstarted; W1-0017 receipts; W1-0013/0014 report/stopping; W6-0004 all roles. |

### User interaction and recovery (§3.9)

W1-0014/0015 own the trusted user boundary and simple evolving TUI. W1-0017/0018
own attributable execution observations; W1-0005 and W1-0004 independently validate
scoped cessation/access release and financial settlement. W1-0016 preserves their
basis across restart. W3-0008/W6-0004 prevent control starvation and duplicate
dynamic work. A user stop permits a deterministic report but no new autonomous
model call without applicable continuation authority. Unrelated independent scopes
need not be blocked by unresolved work elsewhere.

These are guarantees, not mandatory EffectState, effects(), SessionCommand,
control() or universal transition-table abstractions. Implementers choose concrete
forms in their owning tasks; no extra architecture task is hidden here.

## Principles and invariants

| Architectural principle (section 1) | Concrete owner/evidence |
| --- | --- |
| 1 Authority in runtime | W1-0001/0006 proposal denial and no adapter verdict authority. |
| 2 Criteria/evidence-centered coordination | W1-0011, W3-0001/0004/0008 proposals/notices affect admitted criterion work. |
| 3 Minimal work and evidence-based growth | W1-0011/0012, W3-0002/0007, W6-0003 default and diagnosis. |
| 4 Independence before opinion exchange | W2-0001, W4-0002 native/projection leak controls. |
| 5 Executable discriminating independent verification | W1-0009/0010, W2-0001–0004 check/run/mutation/review outcomes. |
| 6 Commitment lifecycle/lease | W1-0007 expiry/renew/delegation scenarios. |
| 7 Explicit stalls and replanning | W1-0012, W3-0004/0007 failed-progress recovery. |
| 8 Contribution/cost/value/calibration | W1-0004/0006, W5-0002, W6-0004/0005. |
| 9 Graded confirmation/consequences/reputation decay | W1-0010, W5-0001/0005. |
| 10 Scoped verifiable knowledge promoted by trials | W5-0003/0004. |
| 11 Evidence-class and equal-condition causal claims | W1-0013, W6-0005 matched comparisons. |
| 12 Different families/comparable strength/weaker judge | W3-0002/0006, W4-0002. |
| 13 Dependency-safe parallelism/serialized or isolated writes | W1-0005/0006, W3-0008, W4-0001. |
| 14 Permissions before verification design | W1-0003/0006/0009, W2-0001/0003. |
| 15 Delivery before learning | W1-0013/0014, W5-0003/0005 event-order and failure scenarios. |

| Invariant / dependency rule | Concrete owners |
| --- | --- |
| D-1/D-2/D-4/D-5; R-1/R-17 | W1-0001 read-only views/event/proposal/policy/outcome; W1-0016 durable resolvable parameter content; each feature’s payloads; full replay W6-0004. |
| D-3 | W1-0011/0012/0013 service/policy business rules, W1-0014 and W3-0008/W6-0004 call-order-only Dispatcher. |
| R-2 | W1-0006/0007 and W3-0001 grant scope/expiry/native sender validation. |
| R-3 | W1-0002/0003/0006, W3-0002/0003, W6-0003/0004 pins/constraint denial controls. |
| R-4 | W1-0006/0010, W3-0002/0006, W4-0003 actual producer-independent review. |
| R-5 | W1-0013, W3-0002, W4-0003 final producer-union exclusion or Blocked. |
| R-6 | W1-0010, W2-0003/0004, W3-0006 executable contradiction defeats approval. |
| R-7/R-8 | W1-0004 both reserves and derived Reporting capacity; W1-0017 actual charges; W1-0013 bounded report/fallback; bids W3-0003, debate W3-0006, unprotected learning W5-0003, all-role W6-0004. |
| R-9 | W1-0003/0005/0006; browser/container W2-0002/0003; isolation W4-0001. |
| R-10 | W2-0001 author/context/native access; W3-0001/0005 Board/handoff; W4-0002 windows; W5-0004 retrieval. |
| R-11 | W5-0001 qualified source/diagnosis matrix; delayed correction W5-0005. |
| R-12 | W1-0006/0007 atomic admission/commitments; concurrent allocation W3-0008. |
| R-13 | W3-0006 opinion-only objection control. |
| R-14 | W1-0012, W3-0002/0007, W6-0003/0004 diagnosis before member/profile/effort growth. |
| R-15 | W1-0013/0014, W5-0003/0005 report available before charged learning. |
| R-16 | W1-0005/0007/0008/0014, W4-0001/0003 immutable accepted work and truthful effects/recovery. |
| R-18 | W5-0003 provenance/scope/basis and stronger supersession; W5-0004 trial promotion; W5-0005 retirement. |
| R-19 | W1-0004 financial settle/release_unstarted; W1-0005 scoped cessation/release; W1-0006/0007 revocation retains holds; W1-0017/0018 actual observations; W4-0001 isolation. |
| R-20 | W1-0016 durable history/content; W1-0017/0018 no duplicate unresolved start; W1-0014 user recovery/budget/grant preservation; W3-0008/W6-0004 dynamic recovery. |

## Journal event families

Source: model [section 9](self-organizing-team-domain-model.md#9-journal-events).
W1-0001 owns the common Envelope; W1-0016 implements its durable storage. Every
feature adds its new payload/projection behavior while preserving records produced
by this new implementation. Recovery observations and user actions use attributable
journal facts; the model does not require three additional named recovery events.

| Exact event names | Concrete owners |
| --- | --- |
| SessionOpened; CriteriaCommitted; ClarificationRecorded; AssumptionRecorded | W1-0002, W1-0011; interactive revisions W3-0004 |
| BudgetOpened; ReservationChanged; ReceiptSettled | W1-0004/0006, lifecycle W1-0007 |
| CheckRegistered; CheckRunRecorded; EvidenceRecorded; ReviewRecorded | W1-0009; hidden W2-0001, new runners W2-0002/0003, discrimination W2-0004, objections W3-0006 |
| MutantRecorded | W2-0004 |
| MethodChosen; PlanCommitted; PlanRevised | W1-0011, W3-0004, W6-0003 |
| ContributionProposed; SolicitationOpened; SolicitationChanged; OfferSubmitted; Awarded | W1-0006 fixed baseline, W3-0001 proposals, W3-0003 full lifecycle |
| AssignmentAdmitted; AssignmentRevoked; GrantIssued; CommitmentChanged | W1-0006/0007, dynamic native operations W3-0001 |
| WorkspaceOpened; LockChanged; SnapshotTaken; ResultMerged | W1-0005 Direct, W4-0001 isolation/merge |
| InvocationStarted; InvocationEnded; ResultSubmitted | W1-0017 invocation, W1-0008 result; native Codex W1-0018 and Claude/Glm W6-0001/0002 |
| ObjectionRaised; ObjectionResolved | W3-0006 |
| AcceptanceRecorded; LedgerUpdated; Regraded | W1-0010/0013, W2-0004; Regraded W5-0005 |
| ProgressAssessed; Diagnosed; Escalated | W1-0012, W3-0007, W4-0003 |
| TeamChanged; NoticePosted; HandoffCreated; ReportDelivered | W3-0002; W3-0001; W3-0005; W1-0013/0014 respectively |
| ObservationRecorded; ReputationUpdated; CalibrationRecorded | W5-0001; W5-0002 |
| KnowledgeChanged; TrialRecorded; RetrievalRecorded; ConsequenceIngested | W5-0003/0004/0005; trial/retrieval W5-0004; ingestion W5-0005 |

## Calibration, verification and implementation choices

| Parameter/obligation | Mechanism owner | Empirical owner |
| --- | --- | --- |
| P1 T_offer/B_bid/κ/N_open; A4 γ/T_ref/θ0/ρ | W3-0003 | W6-0005 |
| P2 T_lease/Δ_release | W1-0007 | W6-0005 |
| A3 θ_min; ProfilePolicy p_target | W6-0004; W3-0002 | W6-0005 |
| A7 μ/n_corroborate | W2-0004; W5-0005 | W6-0005 |
| A8 LR table/τ_behavior/τ_quality/τ_support | W1-0010 | W6-0005 |
| A9 ε/stall_limit/p_min | W1-0012/W3-0007 | W6-0005 |
| A12 λ/n_trial | W5-0001/W5-0004 | W6-0005 |
| P6 cost_interrupt | W3-0004 | W6-0005 |
| PriceBook/fallback/unknown usage; policy-derived reporting reserve; creditable-grade choice | W1-0004; W1-0010/W5-0001 | W6-0005 records resolved values/conditions and evaluated mechanisms |
| Brier/isotonic fitting, insufficient-data fallback, success/cost attribution | W5-0002 | W6-0005 separated fitting/evaluation and honest inconclusive outcomes |
| Every strategy’s real-consumer replacement contract | Each owning feature task above | W6-0005 reproducible comparisons |

AGENTS.md applies to every implementation task: before a code commit run
`cargo build --workspace --offline`, `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --offline -- -D warnings`, and
`cargo test --workspace --offline`; inspect affected consumers and obtain
independent review for public contracts, ownership, persistence or execution
authority. New tasks remain proposals until scheduled. A fixture is not a native
run, a passing check is not necessarily discriminating, an accepted result is not
necessarily confirmed, and an implemented policy is not measured improvement.

The genuine implementation choices are the concrete Rust layout created by
W1-0001, durable encoding/content details from W1-0016, new native adapter wire
protocols, check/browser/container tooling, enforceable Direct/CopyOnWrite
mechanics, bounded context formatting, offer timing, calibrated values/datasets
and the evolving TUI layout. Reporting capacity is derived by ResourcePolicy into
Budget; it is not a mandatory user Constraints field. A11 narration/correction is
bounded and paid, with audited DeterministicReport after user stop or unavailable
narration. A7, A8 and claim audit share evidence applicability; A8 also requires
nonempty applicable Supports and required-class coverage. Owners above must
resolve these choices for their consumer. They do not permit omitting a named strategy/variant, adding permanent agent roles,
turning A8 into advisory-only belief, or replacing kernel decisions with messages.

CreditPolicy has a Confirmed(*)-only default and an implementation also crediting
Discriminated, selected for an experiment and recorded with its observations.
Architectural approval does not make one experiment's choice permanently mandatory.
Empirical evaluation may retain raw uncalibrated values or find no advantage. Such an outcome is valid evidence;
fabricated benefit or unowned scope is not.
