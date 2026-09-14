# YMP architecture: domain model of a self-organizing agent team

Status: proposal, 2026-09-14. The design is neither implemented nor tested, and the owner has not approved it. It does not authorize implementation and does not override the approved intent, the domain terminology or accepted architecture contracts. Numerical values are initial assumptions to be calibrated.

The document condenses the analysis of two accepted fifteen-puzzle observation runs and the cited research into a domain model suitable for implementation. It defines entities, trusted runtime services, replaceable strategies, interaction protocols and semi-algorithms. An implementing agent:

1. builds the components from sections 3–5;
2. connects them according to sections 6–7;
3. assembles the product in the order of section 10.

The proposal reuses the product vocabulary and follows an injectable-implementation approach with replaceable subsystem implementations. Kernel service and strategy names are proposed domain names; their mapping to Rust types has not been established. Isolated workspace copies (`IsolatedCopy`, `CopyOnWrite`) depend on post-foundation isolation work and are not part of the initial executable scope.

## 0. Notation

```text
value  Name { field: Type }                 immutable value without identity
entity Name { id: Id<Name>; ... }           entity with identity and a lifecycle
enum   Name = A | B(x: T)                   enumeration or tagged union
state  Name: S1 -> S2 [condition] / effect  lifecycle transition
event  Name { ... }                         immutable journal fact
kernel Name { op(in) -> Result<out> }       trusted runtime service: commits state; not replaceable
port   Name { op(in) -> Proposal<out> }     replaceable strategy: only proposes
rule   R-n: statement                       invariant enforced by the kernel
algo   name(args): steps                    semi-algorithm
```

Base types:

```text
Id<T>, Digest (SHA-256), Instant, Duration, Prob ∈ [0,1], Real, Int, Bool, Text, Path
List<T>, Set<T>, Map<K,V>, Opt<T>, Graph<T>, Log<T>
CostUnits = Real                                  normalized cost (section 3.7)
Ref       = { id: Id<any>; version: Digest }      versioned reference to a journal record
value PolicyRef   { port: Text; impl: Text; version: Text; params: Digest }
value Proposal<T> { value: T; rationale: Text; basis: List<Ref>; policy: PolicyRef }
value Denial      { code: Text; message: Text; refs: List<Ref> }
Result<T> = Ok(T) | Denied(Denial)
*View     = deterministic read-only projection of the journal
```

**Binding rule.**

- A strategy (`port`) receives only `*View` projections and returns a `Proposal`.
- A trusted service (`kernel`) validates invariants and records an `event`.
- A strategy that needs an AI model calls it only through an admitted assignment (section 3.4). No call is hidden or unaccounted.

## 1. Architectural principles

| # | Principle | Basis |
|---|---|---|
| 1 | Authority belongs to the runtime and intelligence to strategies: strategies propose; the kernel validates and commits | Approved intent: the runtime commits assignments, grants and acceptance |
| 2 | Coordination is organized around acceptance criteria and evidence of their satisfaction, not around messages | Stale responsibility proposals changed no admitted work in the observed runs; shared-board systems (Salemi et al.; Han & Zhang) |
| 3 | Minimal work by default; escalation only on evidence | Kim et al.; the cascade of Gao et al.; ADaPT; Snell et al. |
| 4 | Independent attempts and hidden checks come before any exchange of opinions | Choi et al.; Lorenz et al.; CodeT |
| 5 | Verification is the bottleneck: it must execute, discriminate correct from incorrect results and be independent; verifiers need tools | Brown et al.; Huang et al.; Agent-as-a-Judge; AgentCoder; ACH |
| 6 | Commitments have a lifecycle and a lease instead of being tied to a task version | Singh; Gray & Cheriton; STEAM |
| 7 | Progress and stalls are tracked explicitly; a stall leads to replanning | Magentic-One |
| 8 | Every call declares its expected contribution and cost; admission follows the value of computation; forecasts are calibrated against outcomes | Russell & Wefald; Tian et al. |
| 9 | Confirmation has grades, including discriminating checks and delayed consequences; reputation grows only from creditable grades and decays over time | intent; Jøsang & Ismail |
| 10 | Knowledge consists of verifiable procedures and strategies with a scope; its status rises after trials | AWM; ReasoningBank |
| 11 | A report statement cites evidence of a suitable class; a causal statement requires a comparison under equal conditions | Explanation errors in the observed runs; Kirchner et al. |
| 12 | Verifiers come from different model families but have comparable strength; debate only when the judge is weaker | Self-MoA; Kim E. et al.; Khan et al. |
| 13 | Only work without mutual dependencies runs in parallel; writes to one artifact are serialized or made in isolated copies | Kim et al.; Anthropic; Cognition; Malone & Crowston |
| 14 | Assignment permissions are known before the verification method is chosen | Available tool permissions prevented planned verification operations in the repeat run |
| 15 | Learning and knowledge preparation do not delay delivery of the result | In both accepted runs, the `learn` and `review_memory` assignments ran after `final_review` and before `synthesis` |

## 2. Domain map and dependencies

```text
             ┌───────────── Dispatcher (session loop, algo A1) ────────────┐
             │ calls                                                 calls │
             ▼                                                             ▼
  port strategies (section 5) ──── Proposal ────► kernel services (section 4)
  read only *View                                 Registry · Treasury · Gatekeeper · Arbiter
                                                  WorkspaceGuard · AcceptanceAuthority · ExperienceVault
                                                                 │ event
                                                                 ▼
                                                  Journal (event journal) ──► Store (*View projections)
                                                                 ▲
  ExecutionBackend (agent adapters) ── Invocation, Receipt ──────┘
```

| Context | Domain structures | Kernel service |
|---|---|---|
| Identity and pool | Agent, Provider, ModelOffering, ExecutionProfile, Capability, Pool | Registry |
| Task definition | Task, Goal, Assumption, Constraints, Pins, Criterion, AcceptanceContract, Check, Snapshot, Artifact | AcceptanceAuthority |
| Session, team and plan | Session, Team, Membership, Method, Plan, WorkItem | Dispatcher |
| Authority and execution | Contribution, Assignment, Grant, Invocation, Attempt, ResultVersion, Workspace | Gatekeeper, WorkspaceGuard |
| Coordination | Board, Notice, Solicitation, Offer, Award, Commitment, Lease, Objection, Handoff | Arbiter |
| Quality assurance | CheckRun, Mutant, Evidence, Review, Acceptance, ConfirmationGrade, Consequence, CriteriaLedger, ProgressLedger, Claim, Report | AcceptanceAuthority |
| Resources | Budget, PriceBook, Reservation, Receipt, Allowance | Treasury |
| Experience | Observation, Reputation, CalibrationRecord, Knowledge, KnowledgeTrial, Retrieval | ExperienceVault |

Dependency rules:

```text
rule D-1: a strategy does not write to Store or Journal
rule D-2: every state change is a Journal event recorded by a kernel service
rule D-3: Dispatcher defines only the call order and result handling; it contains no business rules
rule D-4: *View is a deterministic projection of Journal; a strategy decision is reproducible on the stored view
rule D-5: every decision stores the PolicyRef of the strategy used and the digest of its input view
```

## 3. Domain structures

### 3.0. Catalog

| Context | Structure | Meaning |
|---|---|---|
| Identity | `Agent` | Persistent participant with a stable ID; roles and authority exist only in assignments |
| | `Provider` | Native execution environment that supplies models and tools |
| | `ModelOffering` | Model and supported effort values available through a provider |
| | `ExecutionProfile` | Execution settings that change behavior; the key for experience |
| | `Capability` | Execution permission such as file writes, processes or a browser |
| | `Pool` | Agents eligible for selection, with exclusion reasons |
| Task | `Task` | User task: goal, acceptance contract and constraints |
| | `Goal` | Request with recorded assumptions and clarifications |
| | `Assumption` | Interpretation adopted instead of asking the user; listed in the report |
| | `Constraints` | Budget, verification reserve, deadline, pins and limits |
| | `Pins` | Explicit user constraints on team size, roster, models or effort |
| | `Criterion` | Atomic acceptance requirement with a kind, weight and required evidence classes |
| | `AcceptanceContract` | Versioned set of criteria and checks for a task |
| | `Check` | Executable basis for assessing one criterion, with independence and visibility |
| | `Snapshot` | Content-addressed workspace state at an instant |
| | `Artifact` | File path with its digest |
| Session | `Session` | Durable context and state of solving one task |
| | `Team` | Agents selected for the session, with a revision counter |
| | `Membership` | One agent's participation interval and reason |
| | `Method` | Work scheme with its escalation ladder |
| | `Plan` | Versioned graph of work items |
| | `WorkItem` | Subtask aimed at specific criteria, with dependencies and write paths |
| Execution | `Contribution` | Proposed work with targets, forecast and cost estimate |
| | `Assignment` | One agent's bounded work: role, access, workspace, allowance, grant and commitment |
| | `Grant` | Assignment-scoped token for team operations |
| | `Invocation` | One agent run within an assignment |
| | `Attempt` | One try at a work item |
| | `ResultVersion` | Immutable result candidate bound to before and after snapshots |
| | `Workspace` | Direct directory or isolated copy with path locks |
| Coordination | `Board` | Shared record of notices, solicitations and commitments |
| | `Notice` | Board entry: finding, question, answer, status claim or proposal reference |
| | `Solicitation` | Open request for a contribution |
| | `Offer` | Agent's response to a solicitation, with a forecast and cost |
| | `Award` | Selected offer with its rationale |
| | `Commitment` | Debtor's responsibility for a contribution, bounded by a lease |
| | `Lease` | Expiry and renewal conditions of a commitment |
| | `Objection` | Disagreement, preferably backed by an executable counterexample |
| | `Handoff` | Context transfer between assignments |
| Quality | `CheckRun` | One execution of a check against a snapshot in a stated role |
| | `Mutant` | Deliberately faulty variant that tests whether checks discriminate |
| | `Evidence` | Classified, attributed support for or against a criterion |
| | `Review` | Reviewer verdict with findings and basis |
| | `Acceptance` | Runtime decision with per-criterion confirmation grades |
| | `ConfirmationGrade` | Strength of the acceptance basis, from `Refuted` to `Confirmed` |
| | `Consequence` | External signal about a result after acceptance |
| | `CriteriaLedger` | Current status and belief for each criterion |
| | `ProgressLedger` | Progress, looping and stall records |
| | `Claim` | Report statement with its evidence and audit outcome |
| | `Report` | Delivered account of results, unmet criteria and assumptions |
| Resources | `Budget` | Session limit, verification reserve, spent and held amounts |
| | `PriceBook` | Versioned cost rates by provider and model |
| | `Reservation` | Budget amount held for an assignment and purpose |
| | `Receipt` | Reported usage and cost of one invocation |
| | `Allowance` | Per-assignment limits on cost, time, native turns and output |
| Experience | `Observation` | Creditable success or failure of an execution profile |
| | `Reputation` | Beta estimate per profile, competence and difficulty |
| | `CalibrationRecord` | Forecast compared with the actual outcome |
| | `Knowledge` | Retained finding with scope, provenance, evidence and falsifier |
| | `KnowledgeTrial` | With/without comparison that tests a knowledge record |
| | `Retrieval` | Knowledge included in or excluded from an assignment context |

### 3.1. Identity and pool

```text
entity Agent {                          -- persistent participant; carries no roles or authority
  id: Id<Agent>; name: Text; provider: Id<Provider>
  defaults: ProfileSettings; instructions: Text; enabled: Bool }

entity Provider {                       -- environment that supplies models and tools
  id: Id<Provider>; kind: Codex | Claude | Glm | Scripted | Other(Text)
  version: Text; capabilities: Set<Capability> }

enum  Capability = ReadFiles | WriteFiles | RunProcess | TempFiles | Sockets
                 | Network | Browser | VcsRead | VcsWrite
enum  Effort = Text                     -- native provider value; no shared scale

value ModelOffering   { provider: Id<Provider>; model: Text; family: Text; efforts: Set<Effort> }
value ProfileSettings { model: Opt<Text>; effort: Opt<Effort> }

value ExecutionProfile {                -- experience key: everything that changes execution behavior
  agent: Id<Agent>; provider_version: Text; model: Text; family: Text; effort: Opt<Effort> }

value Pool {
  eligible: Set<Id<Agent>>; offerings: Map<Id<Agent>, Set<ModelOffering>>
  excluded: Map<Id<Agent>, Text> }      -- exclusion reason, for example an unready adapter
```

### 3.2. Task definition

```text
entity Task { id; goal: Goal; contract: Id<AcceptanceContract>; constraints: Constraints }

value Goal { request: Text; assumptions: List<Assumption>; clarifications: List<Clarification> }
value Assumption    { text: Text; criterion: Opt<Id<Criterion>>; reason: Text }
value Clarification { question: Text; answer: Text; at: Instant }

value Constraints {
  budget: CostUnits; verification_reserve: CostUnits; deadline: Opt<Instant>
  pins: Pins; allowed: Set<Capability>; parallel_limit: Int; attempt_limit: Int; max_members: Int }
value Pins { team_size: Opt<Int>; roster: Opt<Set<Id<Agent>>>; models: Opt<Set<Text>>; efforts: Opt<Set<Effort>> }

entity Criterion {                      -- atomic requirement
  id; text: Text; kind: CriterionKind; weight: Real; required: Bool
  origin: User | Derived(Id<Assignment>); needs_class: Set<EvidenceClass> }
enum  CriterionKind = NewBehavior | Preserve | ArtifactPresence | Quality | Constraint

entity AcceptanceContract { id; task: Id<Task>; criteria: List<Id<Criterion>>; checks: List<Id<Check>>; version: Digest }

entity Check {                          -- executable basis for assessing a criterion
  id; criterion: Id<Criterion>; spec: CheckSpec; author: CheckAuthor
  independence: Independence; visibility: Visible | Hidden
  needs: Set<Capability>; version: Digest }
enum  CheckSpec = Command(program: Path, args: List<Text>, inputs: Set<Path>)
                | ExactBytes(path: Path, digest: Digest)
                | Property(harness: Path, generator: Text, oracle: Text)
                | BrowserScenario(script: Path, viewport: Opt<Text>)
                | ExternalQuery(source: Text, query: Text)
enum  CheckAuthor  = User | Agent(Id<Assignment>)
enum  Independence = ProducerAuthored | IndependentVisible | IndependentHidden | Trusted

value Snapshot { id: Id<Snapshot>; workspace: Id<Workspace>; files: Map<Path, Digest>; taken: Instant }
value Artifact { path: Path; digest: Digest }
```

### 3.3. Session, team and plan

```text
entity Session {
  id; task: Id<Task>; status: SessionStatus; method: Id<Method>
  team: Id<Team>; budget: Id<Budget>; plan: Opt<Id<Plan>>; board: Id<Board> }
enum  SessionStatus = Intake | Running | Finalizing | Delivered | Blocked(Text) | Cancelled

entity Team { id; session: Id<Session>; members: List<Membership>; revision: Int }
value Membership { agent: Id<Agent>; joined: Instant; left: Opt<Instant>; reason: Text }

entity Method {                         -- work scheme and escalation ladder
  id; kind: MethodKind; ladder: List<EscalationStep>; params: Map<Text, Text>; policy: PolicyRef }
enum  MethodKind = Solo | SoloWithVerifier | AsNeededDecomposition
                 | IndependentAttempts(k: Int) | BreadthResearch(k: Int)

entity Plan { id; session; version: Int; items: Graph<Id<WorkItem>>; rationale: Text; author: Id<Assignment> }

entity WorkItem {                       -- subtask aimed at criteria
  id; plan: Id<Plan>; title: Text; targets: Set<Id<Criterion>>; deps: Set<Id<WorkItem>>
  needs: Set<Capability>; writes: Set<Path>; state: WorkState
  attempts: List<Id<Attempt>>; accepted: Opt<Id<ResultVersion>>; parent: Opt<Id<WorkItem>> }
enum  WorkState = Open | Committed | Running | InReview | Accepted | Failed | Blocked | Superseded
```

### 3.4. Authority and execution

```text
entity Contribution {                   -- work with a declared target, forecast and cost
  id; session; kind: ContributionKind; targets: Set<Id<Criterion>>
  subject: Opt<Id<WorkItem> | Id<ResultVersion> | Id<Objection>>
  needs: Set<Capability>; forecast: Forecast; cost: CostEstimate
  proposed_by: Runtime | Agent(Id<Assignment>); basis: List<Ref> }
enum  ContributionKind = Plan | DesignChecks | Produce | Verify | Review | Research
                       | Alternative | Diagnose | Decompose | Integrate | Clarify
                       | Curate | Narrate | Judge
value Forecast     { p_success: Prob; delta_belief: Map<Id<Criterion>, Real>; source: Agent(ExecutionProfile) | Model(PolicyRef) }
value CostEstimate { expected: CostUnits; p90: CostUnits }

enum  RoleKind = Planner | CheckDesigner | Producer | Verifier | Reviewer | FinalReviewer
               | Researcher | Curator | Narrator | Judge | Advocate

entity Assignment {                     -- bounded work with settings, permissions and limits
  id; session; agent: Id<Agent>; profile: ExecutionProfile
  contribution: Id<Contribution>; role: RoleKind; access: Set<Capability>
  workspace: Id<Workspace>; allowance: Allowance; grant: Id<Grant>
  commitment: Id<Commitment>; state: Admitted | Running | Finished | Revoked }

entity Grant {                          -- operation token for the lifetime of an assignment
  id; assignment: Id<Assignment>; operations: Set<TeamOperation>; expires: Instant; token_digest: Digest }
enum  TeamOperation = BoardRead | NoticePost | ContributionPropose | OfferSubmit
                    | CommitmentRelease | CommitmentDelegate | ObjectionRaise
                    | StatusNotify | CheckPropose | KnowledgePropose

entity Invocation {                     -- one agent run within an assignment
  id; assignment; provider: Id<Provider>
  requested: ProfileSettings; sent: ProfileSettings; reported: ProfileSettings
  native_session: Opt<Text>; started: Instant; ended: Opt<Instant>
  receipt: Opt<Id<Receipt>>; terminal: Opt<Completed | Failed(ErrorClass) | Cancelled | TimedOut> }
enum  ErrorClass = Infrastructure | Environment | Protocol | Content | Unknown

entity Attempt {
  id; item: Id<WorkItem>; assignment: Id<Assignment>; workspace: Id<Workspace>
  result: Opt<Id<ResultVersion>>; outcome: Pending | Submitted | Accepted | Rejected(Text) | Abandoned }

entity ResultVersion {                  -- immutable candidate bound to snapshots
  id; item: Id<WorkItem>; producer: Id<Agent>; profile: ExecutionProfile
  before: Id<Snapshot>; after: Id<Snapshot>; artifacts: Set<Artifact>; summary: Text }

entity Workspace { id; kind: Direct | IsolatedCopy(base: Id<Snapshot>); root: Path; locks: Set<PathLock> }
value PathLock { path: Path; mode: Read | Write; holder: Id<Assignment> }
```

### 3.5. Coordination

```text
entity Board { id; session; notices: Log<Id<Notice>>; solicitations: Set<Id<Solicitation>>; commitments: Set<Id<Commitment>> }

entity Notice {
  id; from: Id<Agent> | Runtime; to: Everyone | Id<Agent>
  body: Finding(Text) | Question(Text) | Answer(Id<Notice>, Text)
      | StatusChange(GoalStatusClaim) | ProposalRef(Id<Contribution>)
  refs: List<Ref>; at: Instant }
value GoalStatusClaim { criterion: Id<Criterion>; claimed: Satisfied | Unachievable | Irrelevant; evidence: List<Ref> }

entity Solicitation {                   -- request for a contribution
  id; contribution: Id<Contribution>; stimulus: Real; deadline: Instant
  eligible: Set<Id<Agent>>; visibility: Open | Sealed
  reopened: Int; state: Open | Awarded | Withdrawn | Expired }

entity Offer {                          -- contribution offer with a forecast
  id; solicitation: Id<Solicitation>; agent: Id<Agent>; profile: ExecutionProfile
  forecast: Forecast; cost: CostEstimate; approach: Text
  source: RuntimeProxy | InAssignment(Id<Assignment>) | BidAssignment(Id<Assignment>); at: Instant }

value Award { solicitation: Id<Solicitation>; offer: Id<Offer>; rationale: Text }

entity Commitment {                     -- debtor, subject, condition and lease
  id; debtor: Id<Agent>; creditor: Team | Runtime; subject: Id<Contribution>
  condition: Opt<Text>; lease: Lease; state: CommitmentState; history: List<Ref> }
value Lease { expires: Instant; renew_on: Set<ProgressSignal>; renewals_left: Int }
enum  ProgressSignal  = EvidenceAdded | CheckRun | ResultSubmitted | Heartbeat
enum  CommitmentState = Proposed | Active | Discharged | Released(Text) | Expired
                      | Cancelled(Text) | Delegated(Id<Agent>)

entity Objection {                      -- disagreement backed by a counterexample
  id; against: Id<ResultVersion> | Id<Plan> | Id<Check>; by: Id<Agent>
  counterexample: Opt<Id<Check>>; claim: Text
  state: Pending | Upheld | Dismissed | Unresolved }

entity Handoff { id; from: Id<Agent>; to: Opt<Id<Agent>>; subject: Id<Contribution>; digest: ContextDigest }
value ContextDigest { decisions: List<Ref>; open_questions: List<Text>; evidence: List<Ref>; summary: Text }
```

### 3.6. Quality assurance

```text
entity CheckRun {
  id; check: Id<Check>; target: Id<Snapshot>
  role: Baseline | Candidate | Mutant(Id<Mutant>) | Control
  exit: Int; stdout: Digest; stderr: Digest; env: Digest
  outcome: Pass | Fail | Error(ErrorClass); at: Instant }

entity Mutant { id; base: Id<Snapshot>; patch: Digest; operator: Text; equivalent: Opt<Bool> }

entity Evidence {
  id; criterion: Id<Criterion>; result: Opt<Id<ResultVersion>>
  class: EvidenceClass; independence: Independence; runs: List<Id<CheckRun>>
  discrimination: Discrimination; author: Opt<Id<Agent>>; polarity: Supports | Contradicts }
enum  EvidenceClass = StaticRead | Executed | Browser | ExternalData | Inspection
value Discrimination { baseline_fails: Opt<Bool>; candidate_passes: Bool; mutation_score: Opt<Real> }

entity Review {
  id; result: Id<ResultVersion>; reviewer: Id<Agent>; profile: ExecutionProfile
  verdict: Approve | Reject | NeedsEvidence; findings: List<Finding>; basis: List<Id<Evidence>> }
value Finding { criterion: Opt<Id<Criterion>>; text: Text; severity: Blocking | Advisory; proposed_check: Opt<CheckSpec> }

enum  ConfirmationGrade = Refuted | Unconfirmed | Discriminated
                        | Confirmed(basis: TrustedCheck | ExternalData | Consequences)
      -- order: Refuted < Unconfirmed < Discriminated < Confirmed

entity Acceptance {
  id; subject: Id<ResultVersion> | FinalAggregate(Id<Session>)
  decision: Accepted | Rejected(Text); grades: Map<Id<Criterion>, ConfirmationGrade>
  grade: ConfirmationGrade; basis: List<Ref>; at: Instant }

entity Consequence {                    -- external data received after acceptance
  id; subject: Digest; source: Text
  signal: LaterCheckPass | LaterCheckFail | Revert | Reopen | UserAccepted | UserRejected
  evidence: List<Ref>; at: Instant }

entity CriteriaLedger { session; entries: Map<Id<Criterion>, LedgerEntry> }
value LedgerEntry {
  status: Unmet | Supported | Satisfied | Contradicted; belief: Prob
  evidence: List<Id<Evidence>>; stimulus: Real; unmet_since: Instant; changed: Instant }

entity ProgressLedger { session; records: List<ProgressRecord>; stall_count: Int }
value ProgressRecord { at: Instant; satisfied: Bool; looping: Bool; progress: Real; diagnosis: Opt<Diagnosis>; rationale: Text }

entity Claim  { id; report: Id<Report>; text: Text; kind: Status | Causal | Scope | Recommendation
                evidence: List<Ref>; audit: Valid | Unsupported(Text) }
entity Report { id; session; claims: List<Id<Claim>>; unmet: List<Id<Criterion>>
                assumptions: List<Assumption>; grade: ConfirmationGrade }
```

### 3.7. Resources

```text
entity Budget {
  id; session; limit: CostUnits; verification_reserve: CostUnits
  spent: CostUnits; held: CostUnits; unknown_usage: Stop | Estimate }

entity PriceBook { version: Text; rates: Map<(Id<Provider>, Text), Rates>; fallback: Rates }
value Rates { input: Real; cache_read: Real; cache_write: Real; output: Real }

entity Reservation {
  id; budget: Id<Budget>; assignment: Id<Assignment>; amount: CostUnits
  purpose: Production | Verification | Coordination; state: Held | Settled | Released }

entity Receipt { id; invocation: Id<Invocation>; usage: Usage; coverage: Complete | Partial | Unknown; cost: Opt<CostUnits> }
value Usage { input: Int; cache_read: Int; cache_write: Int; output: Int; reasoning: Opt<Int> }
value Allowance { cost: CostUnits; timeout: Duration; native_turns: Int; output_chars: Int }
```

### 3.8. Experience

```text
enum  Competence = Planning | CheckDesign | Implementation | Verification | Research | Synthesis
enum  Difficulty = Simple | Standard | Complex

entity Observation {
  id; profile: ExecutionProfile; competence: Competence; difficulty: Difficulty
  outcome: Success | Failure; grade: ConfirmationGrade
  source: Id<Acceptance> | Id<Consequence>; at: Instant }

entity Reputation { key: (ExecutionProfile, Competence, Difficulty); alpha: Real; beta: Real; updated: Instant }

entity CalibrationRecord { id; profile: ExecutionProfile; kind: Success | Cost; forecast: Real; outcome: Real; score: Real; at: Instant }

entity Knowledge {                      -- retained finding with scope and supporting evidence
  id; kind: Fact | Procedure | Pitfall | Strategy; body: Text; scope: Scope
  status: KnowledgeStatus; provenance: List<Ref>; evidence: List<Ref>
  falsifier: Text                       -- observation that would refute the finding
  supersedes: Opt<Id<Knowledge>> }
enum  KnowledgeStatus = Candidate | Hypothesis | Validated | Confirmed | Rejected | Superseded | Retired
value Scope { project: Opt<Text>; features: Set<Text>; preconditions: List<Text> }

entity KnowledgeTrial { id; knowledge: Id<Knowledge>; session: Id<Session>; arm: With | Without; metric: Text; value: Real }
entity Retrieval { id; assignment: Id<Assignment>; query: Text
                   included: List<(Id<Knowledge>, Digest)>; excluded: List<(Id<Knowledge>, Text)> }
```

## 4. Trusted runtime services (kernel)

Strategies cannot replace kernel services. Every kernel operation either records an event or returns `Denied`.

```text
kernel Journal {                                   -- event journal
  append(event) -> Seq                             -- atomic and ordered
  view(session) -> SessionView                     -- deterministic projection (D-4)
}

kernel Registry {                                  -- agent registry
  pool(constraints) -> Pool                        -- excludes agents that fail ReadinessProbe
  profile(agent, settings) -> Result<ExecutionProfile>        -- whether the provider supports the model/effort
  capabilities(profile, workspace) -> Set<Capability>         -- actual execution permissions
}

kernel Treasury {                                  -- budget accounting
  open(session, limit, verification_reserve) -> Id<Budget>
  reserve(assignment, amount, purpose) -> Result<Reservation> -- R-7, R-8
  settle(receipt) -> CostUnits                     -- through port CostModel; releases the unused reservation
  remaining(purpose) -> CostUnits
}

kernel WorkspaceGuard {                            -- workspace control
  open(kind, base: Opt<Id<Snapshot>>) -> Workspace -- through port WorkspaceProvider
  lock(assignment, paths, mode) -> Result<()>      -- only one assignment may write a given path
  snapshot(workspace) -> Snapshot
  merge(result: ResultVersion, target: Workspace) -> Result<Snapshot>
}

kernel Gatekeeper {                                -- admission
  admit(contribution, award: Award) -> Result<Assignment>
    -- validates: Pins and Constraints (R-3); contribution.needs ⊆ capabilities (R-9);
    -- role independence (R-4, R-5, R-10); Treasury.reserve; WorkspaceGuard.lock;
    -- parallel_limit; attempt_limit.
    -- creates: Assignment, Grant, Commitment(Active, Lease)
  revoke(assignment, reason) -> ()                 -- revokes the Grant; releases the reservation and locks
}

kernel Arbiter {                                   -- board arbitration
  open(contribution, stimulus, deadline, eligible, visibility) -> Id<Solicitation>
  submit(offer) -> Result<Id<Offer>>               -- validates the Grant, deadline and eligibility
  award(solicitation, proposal: Proposal<Award>) -> Result<Award>
  commitment(op: Renew | Release | Delegate | Cancel | Discharge, id, reason) -> Result<CommitmentState>
  notice(notice) -> Result<Id<Notice>>
  propose(contribution_draft, grant) -> Result<Id<Contribution>>   -- contribution proposed by an agent
  objection(objection) -> Result<Id<Objection>>
  tick(now) -> List<Id<Commitment>>                -- expired leases
}

kernel AcceptanceAuthority {                       -- acceptance authority
  register_check(proposal: Proposal<Check>) -> Result<Check>  -- R-10
  run(check, target: Id<Snapshot>, role) -> CheckRun          -- through port CheckRunner
  evidence(criterion, result, runs, reviews) -> Evidence
  accept(result, reviews, evidence) -> Result<Acceptance>     -- algo A7
  finalize(session) -> Result<Acceptance>                     -- algo A11
  regrade(consequence) -> List<Acceptance>                    -- algo A7
  ledger(session) -> CriteriaLedger                           -- through port BeliefModel (algo A8)
}

kernel ExperienceVault {                           -- experience store
  observe(acceptance | consequence) -> List<Observation>      -- R-11, through port CreditPolicy
  reputation(key) -> Reputation                    -- updated through port ReputationModel
  forecast_outcome(offer, outcome) -> CalibrationRecord       -- through port CalibrationScorer
  knowledge(op: Propose | Promote(to) | Supersede(by) | Retire, id, evidence) -> Result<Knowledge>   -- R-18
}

kernel Dispatcher {                                -- session loop
  run(session) -> Report                           -- algo A1; the only component that calls strategies
}
```

## 5. Replaceable strategies (port)

Each strategy lists its signature, default implementation and alternatives. A strategy marked "role X" runs through an assignment with that role and is charged to the session budget.

### 5.1. Readiness, intake and checks

```text
port ReadinessProbe { probe(profile) -> Ready | NotReady(reason) }
     -- Default: StaticDependencyProbe: executables, adapters and dependencies; no model call

port IntakePolicy {                                -- role Planner
  criteria(goal, knowledge_view) -> Proposal<List<Criterion>>
  questions(goal, criteria) -> List<(question: Text, voi: Real)> }
     -- Default: CriteriaExtraction + VoiClarification; Alt: NoQuestions

port VerificationDesigner {                        -- role CheckDesigner
  design(criteria, base: Snapshot, excluded_authors: Set<Id<Agent>>) -> Proposal<List<Check>> }
     -- Default: IndependentHiddenDesigner with a premortem; Alt: ProducerChecks (experimental control)

port MutationStrategy { mutants(snapshot, criteria) -> List<Mutant> }
     -- Default: FaultInjectionWithEquivalenceFilter; Alt: SyntacticOperators, None

port CheckRunner { run(spec: CheckSpec, target: Snapshot, env) -> (exit, stdout, stderr, outcome) }
     -- Default: ProcessRunner; Alt: BrowserRunner, ContainerRunner
```

### 5.2. Method, plan and contribution selection

```text
port MethodRouter { choose(task_view, experience_view, budget_view) -> Proposal<Method> }
     -- Default: CascadeRouter (algo A1.1); Alt: FixedMethod (experiments), LearnedRouter

port Planner {                                     -- role Planner
  plan(session_view, method) -> Proposal<Plan>
  revise(plan, diagnosis) -> Proposal<Plan> }
     -- Default: AsNeededDecomposition (one work item; split after a failure); Alt: UpfrontDecomposition

port ContributionPolicy { next(ledger, board_view, budget_view, method) -> List<Proposal<Contribution>> }
     -- Default: OrdinalValue; Alt: VocValue (calibrated forecasts), FixedWorkflow

port BeliefModel { update(entry: LedgerEntry, evidence: List<Evidence>, criterion) -> LedgerEntry }
     -- Default: LikelihoodRatioTable (algo A8)
```

### 5.3. Self-organization

```text
port VolunteerPolicy { respond(agent_view, solicitation) -> Opt<Proposal<Offer>> }
     -- Default: ResponseThreshold (algo A4); Alt: RuntimeProxy (reputation only), AlwaysOffer

port AwardPolicy { award(solicitation, offers, reputation_view) -> Proposal<Award> }
     -- Default: CalibratedValuePerCost; Alt: ReputationRank, FirstOffer

port TeamPolicy { revise(team, ledger, board_view, pool, pins) -> Proposal<TeamChange> }
     value TeamChange { add: Set<Id<Agent>>; remove: Set<Id<Agent>>; reason: Text }
     -- Default: DemandDriven: add a member when capabilities or independence are missing; remove after idleness

port ProfilePolicy { settings(agent, contribution, offerings, calibration_view) -> Proposal<ProfileSettings> }
     -- Default: CheapestAdequate: the cheapest profile with calibrated p_success ≥ p_target

port ReviewerPolicy { pick(subject, role, candidates, reputation_view) -> Proposal<Id<Agent>> }
     -- Default: DifferentFamilyComparableStrength; Alt: AnyNonProducer

port SelectionPolicy { select(candidates: List<ResultVersion>, runs: List<CheckRun>) -> Proposal<Id<ResultVersion>> }
     -- Default: DualExecutionAgreement (algo A10); Alt: ReviewerChoice

port DisputePolicy { resolve(objection, evidence_view, strengths) -> Proposal<DisputeAction> }
     enum DisputeAction = RunCounterexample | RequestEvidence | Debate(judge: Id<Agent>) | Escalate
     -- Default: CounterexampleFirst; debate only when the judge is weaker than the debaters
```

### 5.4. Progress, diagnosis and escalation

```text
port ProgressMonitor { assess(ledger, progress_ledger, history) -> ProgressRecord }
     -- Default: EvidenceDelta(ε, stall_limit = 2) (algo A9)

port FailureDiagnoser { diagnose(runs, invocations, reviews, objections) -> Diagnosis }
     enum Diagnosis = Environment | CheckDefect | CapabilityMismatch | ArtifactDefect
                    | CapabilityLimit | PlanDefect | Ambiguity | BudgetExhausted | Unknown
     -- Default: RuleBasedDiagnoser (algo A9); Alt: ModelAssistedDiagnoser (role Researcher)

port EscalationPolicy { next(diagnosis, method, history, budget_view) -> Proposal<EscalationStep> }
     enum EscalationStep = FixEnvironment | ReplaceCheck | Reassign(needs: Set<Capability>) | Retry
                         | Decompose(Id<WorkItem>) | AddVerifier | AlternativeAttempts(k: Int)
                         | StrongerProfile | Clarify | Replan | StopPreserving
     -- Default: DiagnosisFirstLadder (algo A9)
```

### 5.5. Resources

```text
port CostModel {
  cost(receipt, pricebook) -> CostUnits
  estimate(contribution, profile, history_view) -> CostEstimate }
     -- Default: PriceWeighted (algo A13)

port ResourcePolicy { allowance(contribution, profile, budget_view) -> Allowance }
     -- Default: PurposeBounded: limits by contribution kind, difficulty and remaining budget
```

### 5.6. Experience

```text
port CreditPolicy { creditable(grade: ConfirmationGrade) -> Bool }
     -- owner decision; Default: Confirmed(*) only; experimental: Discriminated

port ReputationModel { update(rep, observation) -> Reputation; estimate(rep) -> (mean: Prob, var: Real) }
     -- Default: BetaWithForgetting(λ); Alt: MeanOnly

port CalibrationScorer {
  score(forecast, outcome) -> Real
  calibrate(profile, raw: Prob) -> Prob }
     -- Default: BrierIsotonic; with insufficient data returns raw, marked uncalibrated

port KnowledgeCurator { review(session_view) -> List<Proposal<Knowledge>> }
     -- role Curator; Default: AfterActionReview (asynchronous, after delivery)

port RetrievalPolicy { select(assignment_view, knowledge_view) -> Retrieval }
     -- Default: ScopedLexical: status ∈ {Validated, Confirmed} and a matching scope

port TrialPolicy { arm(knowledge, session_view) -> Opt<With | Without> }
     -- Default: AlternatingArms for hypotheses with a matching scope

port ConsequenceSource { poll(since: Instant) -> List<Consequence> }
     -- implementations: LaterChecks, VcsReverts, UserFeedback
```

### 5.7. Context, report and execution

```text
port ContextComposer { prompt(assignment, retrieval, handoff: Opt<ContextDigest>, projection) -> Prompt }
     -- Default: CriteriaProjection + JournalDigest: criterion texts and visible checks;
     --          hidden checks excluded (R-10); a condensed decision history instead of the full journal

port NarrativeComposer { compose(session_view) -> Proposal<Report> }     -- role Narrator

port ClaimAuditor { audit(claim, evidence_view) -> Valid | Unsupported(Text) }
     -- Default: EvidenceClassRules (algo A11)

port WorkspaceProvider { open(kind, base) -> Workspace; merge(result, target) -> Result<Snapshot> }
     -- Default: Direct; Alt: CopyOnWrite (for IndependentAttempts)

port ExecutionBackend {
  start(assignment, prompt, grant, allowance) -> Handle
  cancel(handle) -> ()
  events(handle) -> Stream<ProgressSignal | ToolDenied(Capability) | Output>
  receipt(handle) -> Receipt }
     -- implementations: Codex, Claude, Glm, Scripted
```

**Replacement rule.** Contract tests exercise each strategy through its real consumer with at least two substantially different implementations. An experiment or session records the `PolicyRef` of every implementation it used.

## 6. Protocols

### P1. Contribution solicitation and voluntary response (Contract Net with thresholds)

```text
0. Contribution sources: ContributionPolicy templates (algo A3) and agent proposals
   (Arbiter.propose through the ContributionPropose operation). Both enter one candidate list.
1. Dispatcher: s ← Arbiter.open(c, stimulus = S(c) (algo A4), deadline = now + T_offer, eligible = E, visibility)
   E is the set of idle team members for which c.needs ⊆ Registry.capabilities
2. Offer source for an agent a ∈ E:
   a) RuntimeProxy: VolunteerPolicy without a model call; the forecast comes from reputation and calibration;
   b) InAssignment: the agent inside its current assignment, through the OfferSubmit operation;
   c) BidAssignment: a separate cheap assignment, only if the cost estimate c.cost.p90 > B_bid.
   Arbiter.submit(offer)
3. At the deadline: award ← AwardPolicy.award(s, offers, reputation_view); Arbiter.award; Gatekeeper.admit(c, award)
4. No offers: s.stimulus ← s.stimulus · (1 + κ); s.reopened += 1; reopen.
   When reopened ≥ N_open: TeamPolicy.revise (add a pool member with the required capabilities),
   then FailureDiagnoser (CapabilityMismatch | CapabilityLimit).
```

### P2. Commitment lifecycle

```text
state Commitment:
  Proposed -> Active     [Gatekeeper.admit = Ok] / lease.expires = now + min(allowance.timeout, T_lease)
  Active   -> Active     [signal in lease.renew_on ∧ renewals_left > 0] / renew the lease
  Active   -> Discharged [Acceptance(subject) = Accepted ∨ a contribution without an artifact result finished]
  Active   -> Released   [agent: release(reason)] / the contribution reopens; stimulus += Δ_release
  Active   -> Expired    [now > lease.expires] / Gatekeeper.revoke; FailureDiagnoser
  Active   -> Cancelled  [runtime: target criteria Satisfied ∨ plan revised]
  Active   -> Delegated  [agent: delegate(to) ∧ Gatekeeper.admit(to) = Ok] / a new commitment for to

rule R-12: an assignment has exactly one active commitment; an agent cannot accept responsibility again for its own current work
```

### P3. Goal status notice (joint intention)

```text
A participant that considers a criterion satisfied, unachievable or irrelevant posts:
  Arbiter.notice(StatusChange{criterion, claimed, evidence})
At the next work boundary ContributionPolicy must respond:
  Satisfied    → a Verify contribution for the criterion if the evidence is insufficient for Satisfied
  Unachievable → FailureDiagnoser → EscalationPolicy (Clarify | Replan | StopPreserving)
  Irrelevant   → Planner.revise; related commitments move to Cancelled
```

### P4. Objection with a counterexample

```text
Arbiter.objection(o)
if o.counterexample = chk:
    AcceptanceAuthority.register_check(chk, independence = IndependentVisible)
    r ← run(chk, candidate.after, Candidate)
    r = Fail  → o.Upheld;    Evidence(polarity = Contradicts); the result is rejected
    r = Pass  → o.Dismissed
    r = Error → o.Unresolved; FailureDiagnoser (Environment | CheckDefect)
else:
    DisputePolicy.resolve(o):
      RequestEvidence → a DesignChecks contribution for the disputed criterion
      Debate(judge)   → two Advocate assignments (for and against) and a Judge assignment; the outcome is recorded as a Review
      Escalate        → EscalationPolicy
rule R-13: an objection without an executable counterexample does not override a passing applicable check
```

### P5. Context handoff

```text
Triggers: Released, Delegated, Expired, a new contribution for the same work item.
ContextComposer builds a ContextDigest: decisions made, open questions and evidence references.
The basis is JournalDigest without a model call; if that is insufficient, a Narrator assignment on a cheap profile.
The next assignment receives the digest instead of the full history.
```

### P6. User clarification

```text
for (q, voi) in IntakePolicy.questions(goal, criteria):
    voi = P(misinterpretation) · rework cost
    voi > cost_interrupt → ask the question; record a Clarification
    otherwise            → add Assumption(q, chosen interpretation) to Goal; the assumption appears in the Report
```

### P7. Independence window

```text
Applies to IndependentAttempts and to CheckDesigner assignments:
  solicitation.visibility = Sealed
  a participant sees the criteria, its own context and visible checks;
  results and board entries of other participants in the same window stay hidden
  until all candidates are submitted or the deadline passes
```

## 7. Semi-algorithms

### A1. Session loop: Dispatcher.run

```text
algo run(session):
  # 1. Readiness and budget, without a model call
  pool ← Registry.pool(constraints)                     -- unready agents excluded with a typed reason
  Treasury.open(session, constraints.budget, constraints.verification_reserve)

  # 2. Intake
  a_plan ← admit(Contribution{kind: Plan}, role Planner)
  criteria ← IntakePolicy.criteria(goal, knowledge_view)            -- through a_plan
  ask_or_assume(IntakePolicy.questions(goal, criteria))             -- P6
  base ← WorkspaceGuard.snapshot(workspace)
  checks ← design_checks(criteria, base)                            -- A2
  method ← MethodRouter.choose(task_view, experience_view, budget_view)   -- A1.1
  plan ← Planner.plan(session_view, method)

  # 3. Work loop over work boundaries
  loop:
    ledger ← AcceptanceAuthority.ledger(session)                    -- A8
    if ∀ required k: ledger[k].status = Satisfied: break
    rec ← ProgressMonitor.assess(ledger, progress_ledger, history)  -- A9
    if rec.stall:
        d ← FailureDiagnoser.diagnose(recent runs, invocations, reviews, objections)
        step ← EscalationPolicy.next(d, method, history, budget_view)
        apply(step)                                                 -- A9
        if step = StopPreserving: break
    if TeamPolicy triggers: apply(TeamPolicy.revise(team, ledger, board_view, pool, pins))
    candidates ← ContributionPolicy.next(ledger, board_view, budget_view, method)   -- A3
    if no running assignments ∧ (candidates = ∅
       ∨ Treasury.remaining(Verification) < minimum verification cost of unmet criteria): break   -- stop rule, A13
    for c in candidates: solicit_and_award(c)                       -- P1
    wait until: an invocation ends ∨ a board event ∨ a lease expires ∨ ConsequenceSource
    for finished assignment a:
        Treasury.settle(ExecutionBackend.receipt(a))
        match a.role:
          Producer              → result ← submit_result(a); verify(result)          -- A6
          CheckDesigner         → register_check for each proposed check              -- A2
          Verifier | Researcher → AcceptanceAuthority.evidence(...) | Arbiter.notice(Finding)
          Reviewer | Judge      → Review; AcceptanceAuthority.accept(...)            -- A7
    for expired in Arbiter.tick(now): FailureDiagnoser(...); reopen the contribution

  # 4. Completion
  finalize(session)                                                 -- A11
  report ← compose_and_audit(session)                               -- A11
  deliver(report); session.status ← Delivered | Blocked(reason)
  spawn async learn(session)                                        -- A12, R-15
```

### A1.1. Cascading method choice: CascadeRouter

```text
algo choose(task_view, experience_view, budget_view):
  ladder ← [Retry, ReplaceCheck, Reassign, Decompose, AlternativeAttempts(2), StrongerProfile, Clarify, StopPreserving]
  f ← features: difficulty (planner estimate), number of criteria, number of independent write groups,
      required evidence classes, presence of breadth (research) questions, history of similar tasks
  if f.research_breadth ≥ 2 ∧ the groups are independent:   return BreadthResearch(k = min(groups, parallel_limit)) with ladder
  if f.independent_write_groups ≥ 2 ∧ the budget allows it: return AsNeededDecomposition(parallel by group) with ladder
  if similar-task history shows frequent CapabilityLimit:  return IndependentAttempts(k = 2) with ladder
  return SoloWithVerifier with ladder                       -- default
```

### A2. Check design and discrimination

```text
algo design_checks(criteria, base):
  designer ← ReviewerPolicy.pick(subject = criteria, role = CheckDesigner, candidates = team)
  a ← admit(Contribution{kind: DesignChecks, targets: criteria}, role CheckDesigner, visibility Sealed)
  -- the instructions include a premortem: "how could the checks pass while the criterion is unmet?"
  for chk in VerificationDesigner.design(criteria, base, excluded_authors = ∅):
      AcceptanceAuthority.register_check(chk)          -- IndependentHidden: the author is recorded for R-10
      r0 ← run(chk, base, Baseline)
      expected ← (criterion.kind = NewBehavior) ? Fail : Pass
      r0 = Error(_)      → mark FixEnvironment before work starts
      r0 ≠ expected      → the check is NonDiscriminating: request a replacement from the designer
  -- an empty initial state makes baseline_fails a weak signal; the main signal is mutation_score (A6)
```

### A3. Contribution selection: ContributionPolicy

```text
algo next(ledger, board_view, budget_view, method):
  C ← agent proposals from the board (ProposalRef) ∪ templates for criteria in Unmet | Supported | Contradicted:
       no result candidate                          → Produce (Decompose if this work item hit CapabilityLimit)
       candidate without discriminating evidence    → Verify | DesignChecks
       Contradicted                                 → Diagnose, then Produce (retry) | Alternative
       open questions or StatusChange on the board  → Research | Verify | Clarify
  for c in C:
     p     ← CalibrationScorer.calibrate(profile_hint(c), c.forecast.p_success)
     gain  ← Σ_{k ∈ c.targets} weight(k) · p · jump(c, k) · (1 − belief(k))
     cost  ← CostModel.estimate(c, profile_hint(c), history_view).expected
     score ← gain / cost
     -- OrdinalValue sorts lexicographically instead: (required, status: Contradicted > Unmet > Supported, kind)
  filter:
     deps(c) accepted; c.needs ⊆ Registry.capabilities(profile_hint(c), workspace)
     no path write conflict with running assignments
     for Verify | Review: the candidate agent ∉ producers(subject)
     purpose(c) ≠ Verification ⇒ cost ≤ Treasury.remaining(Production)
  return top-m by score, m ≤ parallel_limit − running, score ≥ θ_min
  -- jump(c, k): belief gain from the LR table (A8) if the contribution succeeds
```

### A4. Stimulus and response threshold: ResponseThreshold

```text
S(c)    = max_{k ∈ c.targets} weight(k) · (1 − belief(k)) · (1 + γ · (now − unmet_since(k)) / T_ref)
θ(a, c) = θ0 · (1 − ρ · mean Reputation(profile(a), competence(c), difficulty(c))) · (1 + load(a))
P(offer | a, c) = S(c)² / (S(c)² + θ(a, c)²)
offer if random() < P; the forecast p̂ comes from reputation (RuntimeProxy) or is stated by the agent (InAssignment | BidAssignment)
-- an accepted contribution raises belief and lowers S: specialization emerges without permanent roles
```

### A5. Assignment execution

```text
algo execute(assignment):
  retrieval  ← RetrievalPolicy.select(assignment_view, knowledge_view)      -- event RetrievalRecorded
  projection ← criteria and visible checks for the targets; hidden checks and their data excluded (R-10)
  prompt     ← ContextComposer.prompt(assignment, retrieval, handoff_digest, projection)
  h ← ExecutionBackend.start(assignment, prompt, grant, allowance)
  on ProgressSignal         → Arbiter.commitment(Renew, commitment)
  on ToolDenied(capability) → record CapabilityMismatch for FailureDiagnoser
  on end → Invocation.terminal; Receipt
     ErrorClass ∈ {Infrastructure, Environment} → no competence observation is created (R-11)
```

### A6. Result verification

```text
algo verify(result):
  for chk in applicable checks for result.item.targets:
      AcceptanceAuthority.run(chk, result.after, Candidate)
  if MutationStrategy ≠ None:
      for k in targets with kind = NewBehavior:
          M ← MutationStrategy.mutants(result.after, {k}); exclude equivalent = true
          for m in M: run(checks of k, m, Mutant(m))
          mutation_score(k) ← |killed mutants| / |M|
  evidence ← AcceptanceAuthority.evidence(k, result, runs, ∅) for each k
  reviewer ← ReviewerPolicy.pick(result, Reviewer, team \ {result.producer})          -- R-4
  review ← a Reviewer assignment; its access includes RunProcess | Browser when needs_class(k) contains Executed | Browser
  objections → P4
  acceptance ← AcceptanceAuthority.accept(result, [review], evidence)                 -- A7
  if acceptance.decision = Rejected:
      FailureDiagnoser.diagnose(runs, invocations, [review], objections) → ProgressLedger
```

### A7. Acceptance and confirmation grades

```text
algo accept(result, reviews, evidence):
  for k in result.item.targets:
     if ∃ e ∈ evidence(k): e.polarity = Contradicts ∧ e.class ∈ {Executed, Browser, ExternalData}:
         return Rejected("failing applicable check")                               -- R-6
  if ¬∃ r ∈ reviews: r.reviewer ≠ result.producer ∧ r.verdict = Approve:
         return Rejected("no independent approval")                                -- R-4
  for k in targets:
     grade_k ←
       Confirmed(TrustedCheck)  if ∃ e: e.independence = Trusted ∧ e.class ∈ {Executed, Browser} ∧ candidate_passes
       Confirmed(ExternalData)  if ∃ e: e.class = ExternalData ∧ e.polarity = Supports
       Discriminated            if ∃ e: e.independence = IndependentHidden ∧ e.class ∈ {Executed, Browser}
                                     ∧ candidate_passes ∧ (kind = NewBehavior ⇒ baseline_fails)
                                     ∧ (mutants not applicable ∨ mutation_score ≥ μ)
       Unconfirmed              otherwise
  grade ← min(grade_k) over required criteria
  return Accepted(grades, grade)

algo regrade(consequence):
  S ← acceptances whose artifacts have digest = consequence.subject
  LaterCheckFail | Revert | UserRejected → grade ← Refuted; ExperienceVault.observe(consequence)
  LaterCheckPass | UserAccepted          → positive(subject) += 1
      positive ≥ n_corroborate ∧ no negative signals → grade ← max(grade, Confirmed(Consequences))
  record Regraded; review the knowledge that relies on these acceptances (A12)
```

### A8. Criteria ledger: BeliefModel LikelihoodRatioTable

```text
algo update(entry, evidence, criterion):
  if ∃ e: e.polarity = Contradicts ∧ e.class ∈ {Executed, Browser, ExternalData}:
      return { status: Contradicted, belief: 0 }
  prior ← 0.5 or the planner's Forecast;  odds ← prior / (1 − prior)
  for each group g of Supports evidence with the same (independence, class, discrimination):
      odds ← odds · LR(g)             -- repeated evidence of one group is correlated and counts once
  belief ← odds / (1 + odds)
  τ ← (criterion.kind = Quality) ? τ_quality : τ_behavior
  status ← Satisfied if belief ≥ τ ∧ the classes in criterion.needs_class are covered
           Supported if belief ≥ τ_support
           Unmet     otherwise

Initial LR values (calibrated from observations):
  Trusted ∧ (Executed | Browser)                             50
  ExternalData                                               30
  IndependentHidden ∧ (Executed | Browser) ∧ discriminating  20
  IndependentVisible ∧ (Executed | Browser)                   5   -- also non-discriminating hidden checks
  Inspection by an independent reviewer (Approve)           1.5
  ProducerAuthored ∧ Executed                               1.5
  StaticRead                                                1.1
τ_behavior = 0.9;  τ_quality = 0.6;  τ_support = 0.6
```

With these values, producer-authored checks, static reads and reviewer approval together give belief 0.71 and do not move a behavioral criterion to `Satisfied`. A3 therefore requests an independent executable check.

### A9. Progress, diagnosis and escalation

```text
algo assess(ledger, progress_ledger, history):                    -- EvidenceDelta
  satisfied   ← ∀ required k: Satisfied
  progress    ← Σ_k weight(k) · (belief_now(k) − belief_prev(k))
  looping     ← the same (kind, targets, outcome) ≥ 2 times in a row ∨ the same rejection reason ≥ 2 times
  stall_count ← (progress < ε ∨ looping) ? stall_count + 1 : 0
  stall       ← stall_count ≥ stall_limit

algo diagnose(runs, invocations, reviews, objections):            -- RuleBasedDiagnoser, first match wins
  Error(Environment | Infrastructure) in runs or invocations                      → Environment
  a check fails on the Control reference ∨ a NewBehavior check passes on Baseline  → CheckDefect
  ToolDenied ∨ needs ⊄ assignment access                                          → CapabilityMismatch
  a discriminating check fails on the candidate ∧ attempts < attempt_limit        → ArtifactDefect
  ArtifactDefect across ≥ 2 profiles ∨ calibrated p_success < p_min               → CapabilityLimit
  conflicting StatusChange notices or objections about criterion interpretation   → Ambiguity
  criteria incompatible with the plan structure                                   → PlanDefect
  Treasury.remaining(Verification) < minimum verification cost of unmet criteria  → BudgetExhausted
  otherwise                                                                       → Unknown

algo next(diagnosis, method, history, budget_view):               -- DiagnosisFirstLadder
  Environment        → FixEnvironment; if that fails, ReplaceCheck (the replacement requires independent review)
  CheckDefect        → ReplaceCheck
  CapabilityMismatch → Reassign(needs): a profile or workspace with the required permissions
  ArtifactDefect     → Retry: the same producer, with the counterexample and failing runs in context
  CapabilityLimit    → the first unused step of [Decompose(item), AlternativeAttempts(2), StrongerProfile]
  PlanDefect         → Replan (Planner.revise)
  Ambiguity          → Clarify (P6)
  BudgetExhausted    → StopPreserving
  Unknown            → AddVerifier (a Researcher assignment for diagnosis), then reassess
  every step records its expected effect, limit and the condition for evaluating its outcome

rule R-14: changing a profile, adding a member or raising effort requires a recorded diagnosis
```

### A10. Independent attempts and selection

```text
algo independent_attempts(item, k):
  base ← WorkspaceGuard.snapshot(target)
  W ← k × WorkspaceGuard.open(IsolatedCopy, base)
  producers ← k distinct agents of comparable strength by reputation, from different families when available
  independence window (P7); every Produce contribution must include its own tests T_i
  after submission: run matrix run(t, cand_j) for t ∈ H ∪ ⋃T_i, where H is the set of hidden checks
  DualExecutionAgreement:
     admissible ← { j : ∀ h ∈ H: Pass }
     clusters   ← admissible candidates grouped by identical sets of passed tests from ⋃T_i
     score(cl)  ← |cl| · |tests passed by cl|
     choice     ← argmax score; an independent reviewer chooses within the cluster
  WorkspaceGuard.merge(choice, target); the other candidates remain in the journal
  no admissible candidates → FailureDiagnoser (CapabilityLimit | CheckDefect)
```

### A11. Finalization, report and claim audit

```text
algo finalize(session):
  integrated ← WorkspaceGuard.snapshot(target) after all merges
  rerun all applicable checks on integrated (role Candidate)
  producers ← ∪ producer(accepted versions)
  final ← ReviewerPolicy.pick(aggregate, FinalReviewer, team \ producers)
     no candidate → TeamPolicy.revise if Pins allow it; otherwise Blocked("final_review_pending")   -- R-5
  AcceptanceAuthority.accept(aggregate, [final_review], evidence(integrated))

algo compose_and_audit(session) -> Report:
  report ← NarrativeComposer.compose(session_view)
  for claim in report.claims: ClaimAuditor.audit(claim, evidence_view)     -- EvidenceClassRules
     Status "accepted"                  → reference to the Acceptance and its confirmation grade
     "renders", "works in the browser"  → Browser-class evidence
     "runs", "passes the check"         → Executed-class evidence with a CheckRun reference
     Causal "X fixed Y"                 → two CheckRuns of the same check on before and after with the same env
     Scope "always", "for all"          → a Property check; otherwise the wording narrows to the verified scope
     Recommendation                     → reference to a diagnosis or an unmet criterion
  Unsupported → one NarrativeComposer revision; Unsupported again → the claim is replaced by an uncertainty note
  report.unmet ← required criteria without Satisfied status; report.assumptions ← Goal.assumptions
  return report                                                            -- A1 delivers it before learning starts (R-15)
```

### A12. Experience: reputation, calibration, knowledge and consequences

```text
algo learn(session):                                               -- asynchronous
  for acc in acceptances(session):
     ExperienceVault.observe(acc):
        Accepted ∧ CreditPolicy.creditable(acc.grade)              → Success for the invocations that produced accepted versions
        Rejected by Contradicts ∧ diagnosis ∈ {ArtifactDefect, CapabilityLimit} → Failure
        diagnosis ∈ {Environment, CheckDefect, CapabilityMismatch}
          ∨ ErrorClass ∈ {Infrastructure, Environment}             → no observation
     ReputationModel.update (BetaWithForgetting): α ← λ·α + 1[Success]; β ← λ·β + 1[Failure]
  for offer with a known outcome:
     CalibrationScorer.score(p̂, outcome) → CalibrationRecord; update the profile's isotonic curve
  for d in KnowledgeCurator.review(session_view):                   -- after-action review, role Curator
     ExperienceVault.knowledge(Propose, d)                           -- Candidate: scope and falsifier are mandatory
     independent review of the scope → Hypothesis | Rejected

  Knowledge lifecycle:
     Hypothesis → Validated  : TrialPolicy assigns With/Without arms to sessions with a matching scope;
                               With outperforms Without in ≥ n_trial sessions on a predefined metric
     Validated  → Confirmed  : the knowledge is supported by evidence graded Confirmed(*)
     any        → Superseded : new knowledge with the same scope and a stronger basis contradicts it
     any        → Retired    : the falsifier is observed or a Refuted consequence arrives
  RetrievalPolicy: automatic context uses only Validated | Confirmed; Hypothesis only in the With arm, labeled as such

algo consequences():                                               -- periodic
  for c in ConsequenceSource.poll(last_poll): AcceptanceAuthority.regrade(c); ExperienceVault.observe(c)
```

### A13. Budget

```text
algo cost(receipt, pricebook):                                     -- PriceWeighted
  -- Usage.input is total input including cache reads and writes; adapters normalize native fields
  r ← pricebook.rates[(provider, model)] or pricebook.fallback
  uncached ← input − cache_read − cache_write
  return r.input·uncached + r.cache_read·cache_read + r.cache_write·cache_write + r.output·output
  -- fallback in relative units: input 1, cache_read 0.1, cache_write 1.25, output 4
  coverage = Unknown:
     unknown_usage = Stop     → new admissions are denied, except Verification within the reserve
     unknown_usage = Estimate → cost = allowance.cost

algo reserve(assignment, amount, purpose):                         -- Treasury
  Production | Coordination: spent + held + amount ≤ limit − verification_reserve
  Verification:              spent + held + amount ≤ limit
  otherwise Denied("budget")

algo settle(receipt): spent += cost(receipt); held −= reservation.amount; the remainder is released

stop rule (in A1): no running assignments ∧ (max score (A3) < θ_min
                   ∨ Treasury.remaining(Verification) < minimum verification cost of unmet criteria)
                   → finalize; unmet criteria appear in the Report
```

## 8. Invariants

```text
R-1  Only the kernel changes state; a strategy decision is a Proposal with a PolicyRef
R-2  Roles and authority apply within one Assignment and do not carry over to later ones
R-3  An assignment does not violate Pins or Constraints
R-4  A Reviewer is not the producer of the result version under review
R-5  The FinalReviewer is not among the producers of the final result; without such an agent the session is Blocked
R-6  An applicable executable check with outcome Fail overrides any approvals
R-7  The verification reserve is unavailable to Production and Coordination contributions
R-8  Every invocation is charged to the session budget, including coordination, verification, learning and reporting
R-9  contribution.needs ⊆ the actual permissions of the assignment
R-10 Hidden checks and their data never enter producer context;
     the author of a hidden check for a criterion is not assigned as a producer for that criterion
R-11 Success observations arise only from grades that CreditPolicy credits; failure observations arise only from
     contradicting executable evidence with a competence diagnosis; self-assessment, agent agreement,
     infrastructure failures and check defects create no observations
R-12 An assignment has exactly one active commitment
R-13 An objection without an executable counterexample does not override a passing applicable check
R-14 Resource growth (member, profile, effort) requires a recorded diagnosis
R-15 Learning and knowledge preparation do not delay report delivery
R-16 Accepted result versions are immutable; an agent failure or assignment revocation does not remove accepted work
R-17 Every decision is reconstructable from Journal: input view digest, strategy, proposal and decision
R-18 Knowledge retains provenance, scope and basis; only a stronger basis supersedes it
```

## 9. Journal events

Common event envelope:

```text
event Envelope { seq: Int; session: Id<Session>; at: Instant; actor: Runtime | Id<Agent>
                 policy: Opt<PolicyRef>; input: Opt<Digest>; refs: List<Ref>; payload }
```

Minimum set; every kernel state change in section 4 maps to one of these events (D-2):

```text
SessionOpened · BudgetOpened · CriteriaCommitted · ClarificationRecorded · AssumptionRecorded
CheckRegistered · CheckRunRecorded · MutantRecorded · MethodChosen · PlanCommitted · PlanRevised
ContributionProposed · SolicitationOpened · SolicitationChanged · OfferSubmitted · Awarded
AssignmentAdmitted · AssignmentRevoked · GrantIssued · CommitmentChanged · ReservationChanged
WorkspaceOpened · LockChanged · SnapshotTaken · ResultMerged
InvocationStarted · InvocationEnded · ReceiptSettled · ResultSubmitted · EvidenceRecorded · ReviewRecorded
ObjectionRaised · ObjectionResolved · AcceptanceRecorded · Regraded · LedgerUpdated
ProgressAssessed · Diagnosed · Escalated · TeamChanged · NoticePosted · HandoffCreated · ReportDelivered
ObservationRecorded · ReputationUpdated · CalibrationRecorded · KnowledgeChanged · TrialRecorded
RetrievalRecorded · ConsequenceIngested
```

## 10. Build order

| Stage | Components | Default strategies | Outcome |
|---|---|---|---|
| 1 | Journal, Store, base types, `*View` projections | — | Reproducible state |
| 2 | Registry, ExecutionBackend(Scripted), ReadinessProbe | StaticDependencyProbe | A scripted agent runs |
| 3 | Treasury, CostModel, ResourcePolicy | PriceWeighted, PurposeBounded | Cost accounting and reserves |
| 4 | WorkspaceGuard, WorkspaceProvider(Direct), CheckRunner, AcceptanceAuthority, BeliefModel, CreditPolicy | ProcessRunner, LikelihoodRatioTable | Acceptance with confirmation grades |
| 5 | Gatekeeper, Arbiter (P2 commitments), Dispatcher (A1) | FixedMethod(SoloWithVerifier), AsNeededDecomposition, OrdinalValue, AnyNonProducer, EvidenceDelta, RuleBasedDiagnoser, DiagnosisFirstLadder, CriteriaProjection + JournalDigest, a Narrator-role NarrativeComposer, EvidenceClassRules | **Minimal product**: a producer, an independent reviewer, the criteria ledger, diagnosis-driven escalation and a report with claim audit |
| 6 | VerificationDesigner, MutationStrategy, hidden checks, discrimination (A2, A6) | IndependentHiddenDesigner, FaultInjectionWithEquivalenceFilter | `Discriminated` grade |
| 7 | Protocols P1, P3–P7; VolunteerPolicy, AwardPolicy, TeamPolicy, ProfilePolicy, DisputePolicy, ReviewerPolicy | ResponseThreshold, CalibratedValuePerCost, DemandDriven, CheapestAdequate, CounterexampleFirst, DifferentFamilyComparableStrength | Self-organization |
| 8 | WorkspaceProvider(CopyOnWrite), SelectionPolicy, A10 | DualExecutionAgreement | Independent attempts |
| 9 | ExperienceVault, ReputationModel, CalibrationScorer, KnowledgeCurator, RetrievalPolicy, TrialPolicy, ConsequenceSource | BetaWithForgetting, BrierIsotonic, AfterActionReview, ScopedLexical, AlternatingArms | Accumulated experience |
| 10 | Data-driven MethodRouter and ContributionPolicy | CascadeRouter, VocValue | Effort matched to the task |

Every stage requires contract tests that exercise each strategy through its real consumer with two substantially different implementations. New event kinds are added without changing previously recorded events.

### 10.1. Parameters to calibrate

```text
P1:  T_offer, B_bid, κ, N_open          P2:  T_lease, Δ_release
A3:  θ_min                              A4:  γ, T_ref, θ0, ρ
A7:  μ, n_corroborate                   A8:  LR table, τ_behavior, τ_quality, τ_support
A9:  ε, stall_limit, p_min              A12: λ, n_trial
P6:  cost_interrupt                     ProfilePolicy: p_target
```

## 11. Sources

- Kim et al., [Towards a Science of Scaling Agent Systems](https://arxiv.org/abs/2512.08296v3)
- Gao et al., [Single-agent or Multi-agent Systems? Why Not Both](https://arxiv.org/abs/2505.18286)
- Prasad et al., [ADaPT](https://arxiv.org/abs/2311.05772)
- Snell et al., [Scaling LLM Test-Time Compute Optimally](https://arxiv.org/abs/2408.03314)
- Choi et al., [Debate or Vote](https://arxiv.org/abs/2508.17536)
- Lorenz et al., [How social influence can undermine the wisdom of crowd effect](https://www.pnas.org/doi/10.1073/pnas.1008636108)
- Chen et al., [CodeT](https://arxiv.org/abs/2207.10397)
- Brown et al., [Large Language Monkeys](https://arxiv.org/abs/2407.21787)
- Huang et al., [LLMs Cannot Self-Correct Reasoning Yet](https://arxiv.org/abs/2310.01798)
- Zhuge et al., [Agent-as-a-Judge](https://arxiv.org/abs/2410.10934)
- Huang et al., [AgentCoder](https://arxiv.org/abs/2312.13010)
- Foster et al., [Mutation-Guided LLM-based Test Generation at Meta](https://arxiv.org/abs/2501.12862)
- Zhong et al., [ImpossibleBench](https://arxiv.org/abs/2510.20270)
- Singh, [An ontology for commitments in multiagent systems](https://link.springer.com/article/10.1023/A:1008319631231)
- Gray & Cheriton, [Leases](https://dl.acm.org/doi/10.1145/74850.74870)
- Tambe, [Towards Flexible Teamwork (STEAM)](https://jair.org/index.php/jair/article/view/10193)
- Fourney et al., [Magentic-One](https://arxiv.org/html/2411.04468)
- Russell & Wefald, [Principles of metareasoning](https://www.sciencedirect.com/science/article/abs/pii/000437029190015C)
- Tian et al., [Just Ask for Calibration](https://arxiv.org/abs/2305.14975)
- Jøsang & Ismail, [The Beta Reputation System](https://aisel.aisnet.org/bled2002/41/)
- Wang et al., [Agent Workflow Memory](https://arxiv.org/abs/2409.07429)
- Ouyang et al., [ReasoningBank](https://arxiv.org/abs/2509.25140)
- Kirchner et al., [Prover-Verifier Games](https://arxiv.org/abs/2407.13692)
- Li et al., [Rethinking Mixture-of-Agents](https://arxiv.org/abs/2502.00674)
- Kim E. et al., [Correlated Errors in LLMs](https://arxiv.org/abs/2506.07962)
- Khan et al., [Debating with More Persuasive LLMs](https://arxiv.org/abs/2402.06782)
- Anthropic, [Multi-agent research system](https://www.anthropic.com/engineering/built-multi-agent-research-system)
- Cognition, [Don't Build Multi-Agents](https://cognition.com/blog/dont-build-multi-agents)
- Malone & Crowston, [The interdisciplinary study of coordination](https://dl.acm.org/doi/10.1145/174666.174668)
- Salemi et al., [LLM-Based Multi-Agent Blackboard System](https://arxiv.org/abs/2510.01285)
- Han & Zhang, [LLM MAS Based on Blackboard Architecture](https://arxiv.org/abs/2507.01701)
- Bonabeau, Theraulaz & Deneubourg, [Fixed threshold model](https://doi.org/10.1098/rspb.1996.0229)
- Klein, [Performing a Project Premortem](https://hbr.org/2007/09/performing-a-project-premortem)

## 12. Open decisions

- The owner decides which confirmation grades are creditable (`CreditPolicy`), including whether `Discriminated` counts.
- Numerical values in A4, A8, A13 and section 10.1 are initial assumptions; their effect requires calibration before any claim.
- Proposed next step: after the `CreditPolicy` decision, plan stages 1–5 as separate tasks.
