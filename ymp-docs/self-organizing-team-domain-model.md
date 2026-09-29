# YMP architecture: domain model of a self-organizing agent team

Status: approved architecture and product scope by owner decision, 2026-09-15;
amended 2026-09-16 by owner decision (amendment log in section 12.1).
This is the single authoritative architectural model for YMP. Approval establishes
the development target; it does not claim that every capability is implemented,
tested, or empirically beneficial. Numerical values remain initial assumptions
to be calibrated. A local build or test does not authorize native model calls,
installation, publication, or mutation of real user data.
The owner's standing authorization for bounded Codex, Claude and Glm development
experiments, including the preference for low token use and supported native
`low` settings, is recorded in [AGENTS.md](../AGENTS.md#standing-authorization-for-native-experiments).
It supplies explicit experiment permission while preserving the model's runtime
authority and resource rules.

[Intent](../intent.md) explains the independent product purpose and user
expectations. A mismatch may motivate a proposed model change; it does not
automatically erase that expectation or create a competing architecture. [Domain language](domain.md) and [implementation notes](architecture.md)
are subordinate explanations; [foundation](foundation.md) is historical
implementation material. This iteration starts from scratch: prior source code
and prior delivery claims are not its implementation baseline. Historical proposals, earlier implementations, and external
research cannot override this model or add to its scope. Changes to the model
require an explicit owner decision. Architectural approval alone does not approve
every later amendment or declare a policy empirically superior.

The document condenses the analysis of two accepted fifteen-puzzle observation runs and the cited research into a domain model suitable for implementation. It defines entities, trusted runtime services, replaceable strategies, interaction protocols and semi-algorithms. An implementing agent:

1. builds the components from sections 3–5;
2. connects them according to sections 6–7;
3. assembles the product in the order of section 10.

The model follows an injectable-implementation approach with replaceable strategy
implementations. Its domain, kernel-service, and strategy names are normative;
mapping them to Rust types and crates is an implementation responsibility.
Isolated workspace copies (`IsolatedCopy`, `CopyOnWrite`) remain required target
capabilities even where the current executable does not implement them.

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

The Rust discovery mapping distinguishes `DiscoverySource::Native`, the existing
`ScriptedFixture`, and `ProtocolFixture`. The latter identifies synthetic wire
responses from an external fixture process exercising a concrete native adapter;
it is not evidence of native model inference. This distinction preserves provider
identity while preventing protocol conformance tests from claiming native results.
`CodexParameters` binds the selected Codex adapter to its explicit executable,
observation source and finite connection/frame limits. `CodexAppServer` is the
concrete NativeDiscovery/ExecutionBackend implementation for the documented local
protocol. These are adapter/provenance mappings, not additional provider kinds or
capabilities.

CodexAppServer policy version 2 records the local native Code Mode host as a
distinct execution implementation while retaining empty environments and the
same InvocationFiles authority. Its discovery identifier differs from version 1;
the earlier schema and discovery bytes remain readable for historical replay.
The provider's computation host is an implementation detail, not a new task
capability or a route around mediated file access.

`ClaudeParameters` binds the selected Claude adapter to its explicit executable,
observation source and finite connection, frame and native round-trip limits.
`ClaudeStreamJson` is the concrete NativeDiscovery/ExecutionBackend implementation
for the installed tool's stream-json control protocol. Because that tool updates
itself, its discovery identifier carries the observed native version instead of
a fixed literal, and a call is refused when the installed version differs from
the discovered one. Files are reached only through InvocationFiles. Like the
Codex mappings, these are adapter/provenance mappings, not additional provider
kinds or capabilities.

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

The Rust implementation uses `WorkspacePath` for a validated relative path (`.`
only for the root), and a content-addressed `SnapshotTree` manifest to retain each
file's digest, byte count and ordinary Unix permission bits (`SnapshotFile`), plus
empty directories and their modes. These values preserve rerunnable content rather
than treating a digest without stored bytes as a snapshot. Symlinks, hard links,
special files and special permission bits are refused by Direct capture; ownership,
timestamps and extended attributes are not claimed as reproducible snapshot state.
`CaptureLimits` bounds entry count, nesting depth and per-file/total bytes.
`WorkspaceLocation` binds the canonical root to its observed device and inode;
it identifies an I/O target and does not certify an executor's confinement.

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

The Rust session-control mapping uses `SessionChange` for trusted Stop, explicit
Continue and a recorded phase from the existing SessionStatus values.
`SessionState` projects those control facts. The kernel forms a stop packet from
the current journal state under the storage adapter's write lock or transaction;
it revokes old authority without claiming release of unresolved effects or cost.
`RecoveryConsumed` binds advancement after recovery work to one exact completed
invocation reference. Its retained consumption prevents an earlier recovery step
from repeatedly clearing a later blocked phase.
`RecoveryIntent` distinguishes observation, a new explicit continuation and a
deterministic-only report request.
`RecoveredSession` carries newly issued local owner and budget controls for the
same retained session, never a revived Grant or a new budget. These values make
the existing A1 and section 3.9 user-control guarantees concrete; they do not add
an agent role or a second authority model. Finalization's sticky stop and an
already started reporting/delivery phase cannot be reopened by Continue.

`SessionDefinition` records the owner's workspace, version-bound `VisibleCheck`
inputs, assessment rules and finite offer window for the fixed session workflow.
It is retained input, not a second store of runtime status. `SessionStart` supplies
that input together with intake, Registry facts and accounting choices.
`SessionPolicies` groups the concrete strategy ports actually consumed by this
workflow. `Tick` describes one transient Dispatcher result; canonical progress
continues to come from Journal. These mappings implement A1's call ordering and
P1's existing offer-window parameter without moving business authority into the
Dispatcher.

The Rust `WorkItem.reference` identifies its immutable definition, excluding
state, attempt history and the accepted reference. `PlanRecord` retains the exact
Plan definition and original acceptance contract; `AttemptRecord` binds an Attempt
to its protected baseline, planned after snapshot and local owner nonce. These
implementation records preserve production provenance without treating submission
as acceptance. The initial explicit consumer supports one item with empty
dependencies. Domain Accepted/Rejected values alone do not implement transitions.
`ResultsView` is the read-only Journal projection of these definitions, attempts,
retained candidates and snapshot-transfer links.
`Results` is the implementation consumer connecting this work definition to the
existing admission, Attempt and ResultSubmitted guarantees; it cannot accept a
candidate. Its `PreparedAttempt` retains the local owner capability and exact
begin packet without recreating an execution grant. AttemptStarted and
AttemptAbandoned are implementation journal facts for the model's Pending and
Abandoned outcomes, preserving prior candidates and independent P2 obligations.

The Rust `Plans` consumer accepts completed, accounted Planner output through
owner SessionControl without restoring execution authority. `PlanningPrompt`
retains the actual contract, goal, criteria, method and effective policy supplied
to the invocation; `PaidPlanningView` binds this input and output to the original
Assignment, admission, completion and settled receipt. `PaidDecision` records the
existing Decision together with that source; `PaidRequest` supplies the current
commit boundary. These values distinguish paid result consumption from an active
agent operation and retain the basis for Derived criteria.

`IntakeOutput` contains extracted criteria and `Question` proposals with probability
of misinterpretation, rework cost, proposed assumption and rationale.
`QuestionDecision` records Ask or Assume, and `IntakeOutcome` retains criteria and
those decisions. `IntakeParameters` makes interruption cost explicit. These are
the implementation inputs and results of P6, not an additional clarification
authority. `PlanDefinition` carries the Plan and proposed WorkItems together for
coverage and dependency validation. `ExplicitPlan` is an experimental Planner
control using a predeclared definition instead of selecting the paid output's
scope; its calls remain accounted. AsNeededDecomposition initially commits one
item; diagnosed splitting and general revision remain later capabilities.

`PlanningRecorded` retains selected-policy changes, paid intake/plan decisions,
initial Team and contribution selection. `PlanningState` projects that history,
membership and consumed invocation identities. `ContributionCandidate` binds a
proposed contribution to eligible participants and its criterion priority;
`ContributionView` records those candidates, current funding/slots and limitations.
`ContributionParameters` retains uncalibrated cost, success and criterion-benefit
estimates. These mappings implement the existing replaceable ports and kernel
filters without creating another scheduler or source of execution authority.

### 3.4. Authority and execution

```text
entity Contribution {                   -- work with a declared target, forecast and cost
  id; session; kind: ContributionKind; targets: Set<Id<Criterion>>
  subject: Opt<Id<WorkItem> | Id<ResultVersion> | Id<Objection>>
  needs: Set<Capability>; forecast: Forecast; cost: CostEstimate
  difficulty: Difficulty                -- estimate recorded with the proposal; input to A4 and Observation
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

The Rust `ContributionSubject` preserves subject kind and a versioned Ref;
`ContributionRecord` retains the original acceptance contract reference so unchanged
criterion IDs cannot hide changed meanings. `CoordinationView` is a read-only journal
projection of contributions, solicitations, offers, awards and commitments. Awarded
and the corresponding Proposed Commitment are one atomic packet. The domain value
representations of Assignment, Grant and Invocation do not themselves establish that
admission, execution or later commitment transitions are implemented.

The implemented `Gatekeeper` binds a persisted Award to one attempt-bound admission
packet. The packet records the resource reservation, optional effective workspace lock,
grant digest, Active commitment and AssignmentAdmitted fact under one random nonce.
The grant secret and any prepared file capability remain local and are issued only
after the complete packet is confirmed. A journal projection cannot mint another
secret or capability. Revocation invalidates the grant and retains unresolved resource
holds; it does not imply that an invocation started or that external effects ended.

`Prompt` is the implementation value for exact invocation context text and its
committed basis. `InvocationDispatch` binds that context to the admitted Assignment,
settings, selected backend, receipt, allowance and deadline. `InvocationObserved`
records the dispatch, its complete Ready marker, attributed backend observations,
receipts, selected cost proposals and diagnostics. These are implementation facts
for the existing Invocation/accounting/recovery guarantees, not another authority
or a second journal. Dispatch and resource authorizations form one atomic packet;
`InvocationStarted` requires a later backend confirmation. `InvocationRecord` and
`ExecutionView` are replayable read-only projections of these facts.

`PreparedInvocation` and `LiveInvocation` retain local preparation and execution
capabilities without making them serializable or reconstructible from replay.
`ExecutionHandle` is an adapter-local handle distinct from the Invocation and native
session identities. `BackendEvent` carries an invocation-local sequence and a
`BackendObservation`; cumulative usage, bounded output and terminal observations
cannot grant authority. `InvocationFiles` exposes only the existing mediated file
capability. `ExecutionStatus` reports supervision or unresolved accounting; it does
not replace the model's Invocation terminal outcome. These values give the existing
execution port an explicit bounded transport while preserving R-2, R-19 and R-20.

`InvocationControl` is the original host's opaque callback for rechecking the live
stop flag, grant, exact invocation and clock before a deferred native inference
step. The callback validates existing authority; a backend cannot grant itself
permission by returning a verdict. `InvocationContinuation` records the previous
Invocation and its exact completion reference. It connects one newly admitted,
funded Invocation to previously confirmed Completed work with closed file access
and complete accounted usage in the same session/profile/backend. The reference
is single-use. It is not authority to reattach to an uncertain ongoing call,
revive an old grant or repeat a start. An absent continuation preserves earlier
serialized dispatch bytes and references.

`PathObservation` is the implementation's recorded physical ancestry (`FileIdentity`
for each existing component) and missing suffix for a WorkspacePath. An
`ObservedPathLock` pairs this observation with the model's PathLock so names alone
cannot establish independence across aliases or ancestor roots. Observations describe
filesystem facts; they do not certify an executor's permissions or confinement.
LockChanged records acquisition, authorization, revocation and separately justified
release. `CaptureRead` is an implementation value for the kernel's own temporary
root Read hold while it creates a Snapshot; it does not invent an agent Assignment.
A per-attempt owner identity distinguishes concurrent identical capture requests.
For a production baseline, `CaptureRead.protected_by` identifies an existing
admitted, unstarted mediated writer. The root Read is part of that writer's held
ownership while invocation authorization is forbidden until capture ends; the
original Write hold stays in place afterwards. This records the exact baseline
without granting the agent another capability. After validated withdrawal,
Released and the result's CaptureStarted transfer ownership in one atomic packet,
so no conflicting writer can enter before the after snapshot.
SnapshotTaken atomically publishes a completed capture and ends that hold; aborted
or unresolved capture I/O cannot be presented as a completed snapshot. Cross-session checks use active ownership derived from each session's Journal;
control space for release is protected while those holds remain active.

`JournalIdentity` records a durable journal's random identity and observed physical
file. `WorkspaceBinding` fixes a physical root to that journal; its immutable marker
prevents another physical database, including a copy, from joining the same root.
These are implementation observations for cross-store coordination, not credentials
or execution grants. Mutable holds remain in Journal. Bound roots do not nest;
relative PathLocks express subscopes within a root. Binding metadata must be protected
by the actual execution mechanism before it can justify file access.

`MediatedAccess` is the Rust implementation's non-serializable, files-only handle
issued by WorkspaceGuard for Scripted execution. `FileAccess` is one synchronous
operation's checked scope and physical WorkspaceBinding, borrowed by the I/O port.
They implement enforcement of PathLocks without adding a domain grant or treating
native cwd as confinement. The mediator exposes only permitted file operations,
protects binding metadata and drains admitted I/O before producing AccessWithdrawn
evidence. Direct supports bounded reads and writes; the constrained ReadOnly
WorkspaceProvider exposes reads and capture through the same consumer. Neither
interface permits arbitrary process execution, directory mutation or detached I/O.

`WorkspaceCoordination` is the I/O port's short physical-root coordination section;
`WorkspaceFile` retains the validated open descriptor after that section. The kernel
publishes physical Read/Write ownership for each prepared file before exposing its
bytes. `FileCreation` records an unresolved exclusive creation, with a per-attempt
owner and target observation. LockChanged records its start and physical publication
(or proven non-attempt), so a crash between creation and publication cannot erase
uncertainty. Physical file holds remain until the assignment's evidence-backed
release. These implementation values enforce the existing PathLock/R-19 contract;
they do not replace assignment grants or make external processes confined.

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

`CommitmentTerms` is the Rust representation of the recorded inputs supplied by
the already selected AwardPolicy for P2: T_lease, renewal duration and signals,
the renewal bound, and Δ_release. It keeps those choices attributable and
replaceable before Method selection, including bootstrap Plan admissions.
It introduces no strategy port or authority: Arbiter validates every lifecycle
transition and Gatekeeper retains admission and revocation authority.
`CommitmentLink` records the exact predecessor for reopening or delegation;
`Delegation` binds that predecessor to the successor's atomic admission packet.
These are implementation provenance for existing P2 transitions, not new domain
responsibilities.

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

The Rust check-execution mapping retains `CheckEnvironment` as the object behind
CheckRun.env: runner selection and parameters, platform and observed execution
identity, and `CheckLimits` for time and retained output. `CheckObservationKind`
holds the adapter's observed exit, exact-byte digest or execution error; the
kernel validates these facts and derives the CheckRun outcome. These values make
the existing check/environment binding reproducible without giving an adapter
acceptance authority. A check also retains its criterion version and, for Command,
the verifier snapshot reference, so changed artifacts cannot redefine the check.
The Rust exit value is optional when no normal process exit exists.
`VerificationDesigner/ExplicitVisible` records an already-authored visible
proposal with its effective parameters; it does not implement hidden check design.

`EvidenceRecorded` retains the model's Evidence with its exact `EvidenceScope`
(result, criterion, check and environment references), review-source references
and observation time. `ReviewRecorded` binds Review to its actual Assignment and
criterion versions. These are implementation provenance, not additional grades or
acceptance decisions. Performed Command and ExactBytes checks produce Executed
evidence for the particular check; ExactBytes provides no observation of candidate
program startup. Review statements produce Inspection without a passing CheckRun.

`ApplicabilityContext` is the consumer's expected result, criterion references and
environment set per exact check. The shared applicability traversal validates
canonical records and propagates that same context through every Review/Evidence
basis. Review assessment includes all result criteria; A8 groups the selected
evidence by criterion afterwards. This realizes the existing A7/A8/A11 applicability
rule without silently choosing a latest result or environment from history.

The Rust A8 mapping uses `BeliefView` for the kernel-filtered input, `PriorBasis`
for applicable recorded statements and their polarity, and `BeliefPrior` for a
criterion/result-scoped probability with rationale and supporting references.
`BeliefUpdate` carries the proposed LedgerEntry and prior; `BeliefResponse` binds
that proposal to its input digest. These expose experimental assumptions without
letting a strategy authorize satisfaction. `BeliefThresholds`,
`LikelihoodRatioParameters` and `StrongestSupportParameters` retain effective
numeric settings. `StrongestSupport` is the experimental maximum-support
implementation: it does not multiply weak groups, and its explicit prior is
uncalibrated. The default LikelihoodRatioTable remains A8's neutral-prior rule.

`AssessmentRules` records the mutation threshold used by a particular assessment.
`LedgerRecorded` retains its context, rules and policy-attributed decisions;
`LedgerView` projects entries and their criterion/result references while keeping
the assessment history. The current ledger excludes abandoned or superseded
candidates using the WorkItem's accepted pointer or recorded attempt order.
`AcceptanceRecorded` retains the A7 decision, exact result/attempt, assessment
contract, context, rules and CreditPolicy decision. `CreditResponse` binds the
credit proposal to the acceptance input. The concrete `ConfirmedOnly` and
`IncludeDiscriminated` policies implement the two credit variants in section 5.6;
their eligibility is separate from an actual Observation. These mappings retain
the model's existing authority and decision boundaries rather than adding roles.

The paid ordinary-review mapping uses `CandidateVerdict` for the reviewer's bounded
structured response and `PaidReviewRecorded` for its exact original Assignment,
admission, completion, receipt and prompt references together with ReviewRecorded.
This lets the kernel consume a completed, accounted reviewer after its grant was
revoked; it does not revive that grant or change historical ReviewRecorded values.
`CandidateReviewer` records the same ReviewerPolicy decision for an exact candidate
scope before admission. `ReviewerInput.subject` identifies the ResultVersion or
FinalAggregate checked by its owning consumer; the serialized key remains
`aggregate` to preserve earlier input bytes and digests.
New candidate AcceptanceRecorded writes use version 2: an already performed
conclusive Candidate run in the exact expected context requires canonical Evidence
before A7. Error and inapplicable runs do not trigger this prerequisite. This
implements R-6 without requiring new checks or Satisfied status. Version 1 remains
historically replayable and cannot be used for new append operations.

The Rust A9 mapping uses `ProgressInput`, `DiagnosisInput` and `EscalationInput`
for the exact recorded inputs of the three ports. `ProgressAssessment` contains
the ProgressRecord, stall count and decision; `MonitorParameters`,
`DiagnosisParameters` and `EscalationParameters` retain effective thresholds and
finite use/cost/time bounds. `AcceptedOnlyProgress` is an experimental monitor
requiring a new accepted version to reset its stall. `DirectFailuresOnly` is an
experimental diagnoser recognizing direct environment/check failures while
leaving inferred diagnoses Unknown. `StopOnUncertainty` is an experimental
escalation policy that stops outside its direct repair/retry cases. They provide
materially different controls through the existing ports, not additional roles.

`WorkFact` is a derived, version-bound observation of actual work for repetition
detection. `VerificationEstimate` retains a CostModel input and Decision for an
eligible verification demand; it is not calibration. `DiagnosisRecorded` binds
the diagnosis to the assessed progress boundary and its source context.
`EscalationPlan` states the proposed step, expected effect, bound, success
condition and any limitation. `ProgressRecorded` retains these decisions and
handler outcomes; `ProgressState` projects bounded work observations, history
and atomic pending work. `Progress` is the owner-authorized implementation
consumer. Monitor/Diagnosis/Escalation requests carry their exact commit boundary.

`RecoveryWork` binds a new Contribution to the step, profile, cost estimate and
exact failure context; `RecoveryOutcome` distinguishes actual reruns, created
work, stopping and explicit unavailability. Work creation is not an Invocation
completion. `ReplacementProposal` stages the full old/new check relationship,
candidate, criterion, defect and contract. `ReplacementVerdict` is the precise
reviewer statement; `ReplacementApproval` binds it to a completed, accounted
independent Reviewer invocation. These values implement A9's reviewed replacement
without granting an adapter authority or treating generic approval as check review.

The Rust A11 mapping makes `FinalAggregate` an immutable value containing the exact
session, snapshot, contract, criterion/check/environment versions, accepted sources,
producer union and optional common baseline. `FinalSource` binds a retained result
to its Acceptance; `FinalCheck` binds a check to its expected environment.
`FinalFence` retains the scoped physical read ownership from capture through report
delivery. These realize the existing final subject and stable-workspace requirement;
they do not introduce a production ResultVersion or fabricate a merge.
`Continuation` records owner authority to continue, its absence or a sticky stop.
`FinalReview` binds the final verdict to the exact aggregate and accounted independent
FinalReviewer invocation; `FinalAcceptance` retains its A7 context and credit decision.

`ContextInput` contains the actual Assignment, attributed untrusted instructions,
visible target criteria/checks and bounded journal references. `RetainedContext`
adds digest-checked snapshot bytes within a finite prompt budget. `ContextRecorded`
retains the selected ContextComposer decision consumed at dispatch. `CompactContext`
is an experimental alternative using check references instead of full definitions;
`LeastUsedReviewer` selects the eligible non-producer with the fewest prior reviews,
instead of the stable identity ordering of `AnyNonProducer`.

`ReportDraft` and `DraftClaim` contain proposed typed assertions and wording before
audit; `ClaimInput` is the exact applicable evidence projection. `ConservativeAudit`
is an experimental ClaimAuditor that excludes execution and causal statements even
when EvidenceClassRules permits them. `NarrativeWork` binds initial narration or
its one correction to the prepared report, profile, estimate and audit basis.
`ReportPrepared`, `NarrativeRecorded` and `AuditRecorded` retain those decisions;
`ReportDelivered` adds kernel-derived current outcome, retained work, criteria and
`AccountingSummary` to the model's Report and audited Claims. `FinalizationRecorded`
and `FinalizationState` retain and project these transitions. `Finalization` is the
owner-authorized implementation consumer exposed by Application. These are concrete
provenance mappings for A11, not additional roles or independent runtime authority.

### 3.7. Resources

```text
entity Budget {
  id; session; limit: CostUnits; verification_reserve: CostUnits; reporting_reserve: CostUnits
  spent: CostUnits; held: CostUnits; unknown_usage: Stop | Estimate }

entity PriceBook { version: Text; rates: Map<(Id<Provider>, Text), Rates>; fallback: Rates }
value Rates { input: Real; cache_read: Real; cache_write: Real; output: Real }

entity Reservation {
  id; budget: Id<Budget>; assignment: Id<Assignment>; amount: CostUnits
  purpose: Production | Verification | Coordination | Reporting; state: Held | Settled | Released }

entity Receipt { id; invocation: Id<Invocation>; usage: Usage; coverage: Complete | Partial | Unknown; cost: Opt<CostUnits> }
value Usage { input: Int; cache_read: Int; cache_write: Int; output: Int; reasoning: Opt<Int> }
value Allowance { cost: CostUnits; timeout: Duration; native_turns: Int; output_chars: Int }
value ReportingPlan { narration: Opt<Allowance>; correction: Opt<Allowance> }
```

`ReportingPlan` represents the bounded narration and single correction required
by A11. Both allowances are present, or both are absent for DeterministicReport;
their combined cost determines the protected reporting reserve. W1-0004 uses this
value to make ResourcePolicy's reserve derivation inspectable. The plan does not
authorize a model call or override a user stop.

### 3.8. Experience

```text
enum  Competence = Planning | CheckDesign | Implementation | Verification | Research | Synthesis
enum  Difficulty = Simple | Standard | Complex
-- competence(kind): Plan | Decompose | Clarify -> Planning; DesignChecks -> CheckDesign;
--   Produce | Alternative | Integrate -> Implementation; Verify | Review | Judge -> Verification;
--   Research | Diagnose -> Research; Narrate | Curate -> Synthesis

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

### 3.9. User interaction and session recovery

The primary human-facing interface is an interactive TUI over deterministic
`*View` projections and trusted application commands. It supports task intake,
clarification, progress, agent interaction, resource inspection, interruption,
recovery, and results. Its layout and exact command vocabulary may evolve;
by owner decisions (2026-09-16), OpenCode is the primary TUI inspiration and k9s
guides lists, tables and descriptions/detail views. The Ratatui interface uses
the `ymp2` TUI as its practical visual and interaction basis, captured in
[the TUI reference](tui-reference.md).
W1-0015 adapts its conversation, sidebar, composer, navigation and inspection
patterns to the current model. Product development prioritizes useful
communication and self-organization over visual polish. Presentation never owns
domain state or grants authority.

User actions enter through a trusted application boundary; an agent notice or
Grant cannot impersonate the user. Interruption stops new autonomous work and
requests cancellation without fabricating termination or rollback. Recovery
preserves the task, commitments, accepted results, history, expenses and holds;
it does not duplicate an unresolved invocation, repeat settlement, reset a
budget, or revive a revoked Grant. Unresolved conflicting effects keep the
relevant work blocked. Independent scopes need not be blocked by unrelated work.

Native start failure, disconnection, elapsed time, a cancellation request, or an
exited parent process alone does not prove that further writes are impossible.
The kernel validates attributable cessation or never-started evidence before
WorkspaceGuard releases conflicting access. Cessation can be established by
termination or by effective withdrawal of access. Treasury independently settles
usage or releases a verified never-started reservation; unknown usage follows A13.
The observations and release basis must survive restart through the journal.

The exact observation types, transport, control names and lifecycle transitions
are implementation choices under these guarantees. The model does not prescribe
an EffectState enum, an effects() API or a universal Cancel/Resume transition
table. User controls need appropriate authorization, concurrency and duplicate-
request handling; a stop request must not be starved by unrelated journal writes.
Interruption always permits a deterministic report of recorded facts; further
model work requires the user's applicable continuation authority.

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
  open(session, limit, verification_reserve, reporting_reserve) -> Id<Budget>
  reserve(assignment, amount, purpose) -> Result<Reservation> -- R-7, R-8
  settle(receipt) -> CostUnits                     -- through port CostModel; releases the unused reservation
  release_unstarted(assignment, basis: List<Ref>) -> Result<()> -- only verified never-started work
  remaining(purpose) -> CostUnits
}

kernel WorkspaceGuard {                            -- workspace control
  open(kind, base: Opt<Id<Snapshot>>) -> Workspace -- through port WorkspaceProvider
  lock(assignment, paths, mode) -> Result<()>      -- only one assignment may write a given path
  release(assignment, basis: List<Ref>) -> Result<()> -- only verified ended effects or never-started work
  snapshot(workspace) -> Snapshot
  merge(result: ResultVersion, target: Workspace) -> Result<Snapshot>
}

kernel Gatekeeper {                                -- admission
  admit(contribution, award: Award) -> Result<Assignment>
    -- validates: Pins and Constraints (R-3); contribution.needs ⊆ capabilities (R-9);
    -- role independence (R-4, R-5, R-10); Treasury.reserve; WorkspaceGuard.lock;
    -- parallel_limit; attempt_limit.
    -- creates: Assignment, Grant; activates the Proposed Commitment created by Arbiter.award (P2)
    -- Denied: the Proposed Commitment moves to Cancelled(reason) and the contribution reopens
  revoke(assignment, reason) -> ()                 -- revokes Grant immediately; retains unresolved holds
    -- Dispatcher requests ExecutionBackend.cancel and observes effects.
    -- WorkspaceGuard.release and Treasury.settle/release_unstarted have independent evidence conditions.
}

kernel Arbiter {                                   -- board arbitration
  open(contribution, stimulus, deadline, eligible, visibility) -> Id<Solicitation>
  submit(offer) -> Result<Id<Offer>>               -- validates the Grant, deadline and eligibility
  award(solicitation, proposal: Proposal<Award>) -> Result<Award>   -- records Awarded; creates Commitment(Proposed)
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

port BeliefModel { update(entry: LedgerEntry, evidence: List<Evidence>, criterion, subject: Ref) -> LedgerEntry }
     -- Default: LikelihoodRatioTable (algo A8)
     -- the kernel supplies the version-bound result/snapshot currently assessed for this criterion
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

port ResourcePolicy {
  allowance(contribution, profile, budget_view) -> Allowance
  reporting_reserve(task_view, pool_view) -> CostUnits }
     -- Default: PurposeBounded: limits by contribution kind, difficulty and remaining budget
     -- protected reporting capacity is derived by this policy; no mandatory user-facing reserve field
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
     --          participant-authored text is quoted as attributed, untrusted content (R-21)

port NarrativeComposer { compose(session_view) -> Proposal<Report> }
     -- Default: admitted Narrator assignment, charged to Reporting
     -- Alt/fallback: DeterministicReport from recorded state, with no model invocation

port ClaimAuditor { audit(claim, evidence_view) -> Valid | Unsupported(Text) }
     -- Default: EvidenceClassRules (algo A11)

port WorkspaceProvider { open(kind, base) -> Workspace; merge(result, target) -> Result<Snapshot> }
     -- Default: Direct; Alt: CopyOnWrite (for IndependentAttempts)

port ExecutionBackend {
  start(assignment, prompt, grant, allowance) -> Handle
  cancel(handle) -> ()                    -- requests cancellation; does not prove termination or ended writes
  events(handle) -> Stream<ProgressSignal | ToolDenied(Capability) | Output
                          | OperationRequest(op: TeamOperation, args: Text, correlation: Text)>
  reply(handle, correlation, result: Text) -> ()   -- returns the kernel's typed result or Denial
  receipt(handle) -> Receipt }
     -- implementations: Codex, Claude, Glm, Scripted
     -- OperationRequest is forwarded by the host to the owning kernel service under the
     -- invocation's Grant; the actor is the admitted assignment, never the text (R-2, R-21)
```

**Replacement rule.** Contract tests exercise each strategy through its real consumer with at least two substantially different implementations. An experiment or session records the `PolicyRef` of every implementation it used.

### 5.8. Experimental replaceability of key mechanisms

Useful self-organization is the product's central research question. Key decision
mechanisms are explicit strategy interfaces with interchangeable implementations,
not formulas hidden in Dispatcher or a fixed producer/reviewer workflow. This
applies to contribution and offer selection, team/profile changes, commitments'
policy inputs, assessment, verification design, diagnosis/escalation, resource
allocation, reputation, curation/retrieval and method routing.

A critical factor that is initially a constant may later be calculated from
history or current conditions through its owning strategy. Replacing a mechanism
must not require rewriting its kernel consumer. Use existing ports for these
responsibilities; add a boundary only for a coherent decision responsibility,
not an interface wrapper around each number or a generic service container.

An experiment selects implementations and parameters explicitly. The journal
retains the effective PolicyRef, resolvable parameter values and input-view digest
at each decision; a parameter hash without recoverable values is insufficient.
Changing a selection does not rewrite earlier decisions or reputation history.
Changes within a session take effect only at a recorded work boundary with the
appropriate user authority; admission still enforces grants, constraints and
resource limits. Different policies must be exercised through real consumers,
and experimental effects must remain attributable when results are compared.

Named defaults are starting implementations, not universal winners. Alternate
implementations must preserve the interface contract and kernel invariants.
Experiments may reveal useful, ineffective or harmful self-organization; a fixed
workflow is a useful baseline, not a replacement for the required agent initiative.

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
-- Arbiter.award creates Commitment(Proposed) for the awarded offer; admission activates or cancels it
state Commitment:
  Proposed -> Active     [Gatekeeper.admit = Ok] / lease.expires = now + min(allowance.timeout, T_lease)
  Proposed -> Cancelled  [Gatekeeper.admit = Denied] / the contribution reopens
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
  # Resume continues at the recorded boundary; do not repeat committed intake,
  # native starts, settlements or finalization. Unknown external effects require recovery first.
  # 1. Readiness and budget, without a model call
  pool ← Registry.pool(constraints)                     -- unready agents excluded with a typed reason
  report_reserve ← ResourcePolicy.reporting_reserve(task_view, pool)
  Treasury.open(session, constraints.budget, constraints.verification_reserve, report_reserve)
  -- open once for a new session; Resume reuses the existing budget and every spent/held amount

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
    if the user has stopped autonomous work without authorizing continuation: break
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
    wait until: an invocation ends ∨ a board event ∨ a lease expires ∨ ConsequenceSource ∨ a user control
    for finished assignment a:
        Treasury.settle(ExecutionBackend.receipt(a))
        match a.role:
          Producer              → result ← submit_result(a); verify(result)          -- A6
          CheckDesigner         → register_check for each proposed check              -- A2
          Verifier | Researcher → AcceptanceAuthority.evidence(...) | Arbiter.notice(Finding)
          Reviewer | Judge      → Review; AcceptanceAuthority.accept(...)            -- A7
    for expired in Arbiter.tick(now): FailureDiagnoser(...); reopen the contribution

  # 4. Completion
  cancellation_requested ← the user has stopped autonomous work without authorizing continuation
  if not cancellation_requested:
    finalize(session) if effects permit a stable final snapshot      -- A11
      otherwise session.status ← Blocked("effects_uncertain")
    finalization denied(reason) → session.status ← Blocked(reason)
  -- Under a user stop, preserve the recorded stop/block state; do not admit final reviewers.
  report ← compose_and_audit(session, allow_narration = not cancellation_requested)                               -- A11
  deliver(report); preserve Cancelled/Blocked, otherwise session.status ← Delivered
  if not cancellation_requested ∧ session.status = Delivered:
    spawn async learn(session) only within remaining unprotected budget -- A12, R-15
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
  retain only reviews and evidence applicable to this result, criterion/check versions and run environment
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
algo update(entry, evidence, criterion, subject):
  evidence ← only records applicable to the assessed criterion, current result/snapshot,
              check version and execution environment; superseded or unrelated evidence is excluded
  if ∃ e: e.polarity = Contradicts ∧ e.class ∈ {Executed, Browser, ExternalData}:
      return { status: Contradicted, belief: 0 }
  supporting ← applicable evidence with polarity = Supports
  prior ← 0.5 for the default LikelihoodRatioTable
  -- an experimental BeliefModel may supply a criterion/result-scoped prior; require finite 0 < prior < 1
  odds ← prior / (1 − prior)
  for each group g of supporting evidence with the same (independence, class, discrimination):
      odds ← odds · LR(g)             -- repeated evidence of one group is correlated and counts once
  belief ← odds / (1 + odds)
  τ ← (criterion.kind = Quality) ? τ_quality : τ_behavior
  status ← Satisfied if belief ≥ τ ∧ supporting is nonempty
                       ∧ criterion.needs_class ⊆ {e.class : e ∈ supporting}
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

The default prior is neutral. A forecast of a future assignment's success is not
a probability that the present result satisfies a criterion. An experimental
prior must define that meaning and record its policy and basis; it does not
bypass evidence applicability, Supports coverage or hard contradictory checks.
Participant forecasts still inform contributions and offer selection.

Belief participates in criterion satisfaction exactly as A8 specifies; it is not
merely advisory. A forecast alone cannot satisfy a criterion, provide a
`ConfirmationGrade`, or create competence credit. A7 separately requires
independent approval and applies evidence-based grading; A12 separately applies
`CreditPolicy`. The same evidence-applicability rules apply to A7, A8 and report
claim audit. A7 acceptance without stronger confirmation remains explicitly
`Unconfirmed`, and unmet criteria remain visible in the report.

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
  if any invocation can still change the aggregate workspace, including revoked or disconnected work:
      return Blocked("effects_uncertain") without snapshot, merge or final acceptance
  integrated ← WorkspaceGuard.snapshot(target) after all merges
  rerun all applicable checks on integrated (role Candidate)
  producers ← ∪ producer(accepted versions)
  final ← ReviewerPolicy.pick(aggregate, FinalReviewer, team \ producers)
     no candidate → TeamPolicy.revise if Pins allow it; otherwise Blocked("final_review_pending")   -- R-5
  AcceptanceAuthority.accept(aggregate, [final_review], evidence(integrated))

algo compose_and_audit(session, allow_narration) -> Report:
  if allow_narration and a bounded Reporting assignment is admissible:
    report ← NarrativeComposer.compose(session_view) through that assignment
  else: report ← DeterministicReport(session_view)
  for claim in report.claims: ClaimAuditor.audit(claim, evidence_view)     -- EvidenceClassRules
     Status "accepted"                  → reference to the Acceptance and its confirmation grade
     "renders", "works in the browser"  → Browser-class evidence
     "runs", "passes the check"         → Executed-class evidence with a CheckRun reference
     Causal "X fixed Y"                 → two CheckRuns of the same check on before and after with the same env
     Scope "always", "for all"          → a Property check; otherwise the wording narrows to the verified scope
     Recommendation                     → reference to a diagnosis or an unmet criterion
  Unsupported → at most one admitted NarrativeComposer revision, only if allow_narration
                and the remaining Reporting reserve and execution authority permit it
  unavailable/failed narrator or insufficient reserve → DeterministicReport from recorded state
  Unsupported after the allowed revision or in the fallback → replace the claim by an uncertainty note
  report.unmet ← required criteria without Satisfied status; report.assumptions ← Goal.assumptions
  return report                                                            -- A1 delivers it before learning starts (R-15)
```

The reporting reserve covers initial narration and its one bounded correction.
ResourcePolicy derives the amount; its value is recorded when Treasury opens the
budget. A user need not configure a separate reserve manually.
Reporting cannot start production, consume the verification reserve, or inspect
an unstable workspace as a completed result. A blocked or cancelled session still
gets a report of known results, expenses, unmet criteria, and unresolved effects.
The deterministic fallback uses the same claim audit and records its policy; it
does not infer acceptance, confirmation, or termination. Narration and correction
receipts are settled or conservatively held before `ReportDelivered`, so the
report can reference their accounted expenses and unknowns. This does not change
the result being assessed or require another model call to describe the accounting.
A fallback report describes what happened; it cannot replace an unfinished task
artifact or claim the task was completed. A nominal reserve is not proof of
available funds when earlier consumption has no defensible upper bound; in that
case use the deterministic report and retain the uncertainty.

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
     unknown_usage = Stop     → new admissions are denied, except Verification and Reporting within their reserves
     unknown_usage = Estimate → cost = allowance.cost
  -- Partial coverage without a defensible complete cost is treated conservatively as Unknown.
  -- Stop keeps the unresolved amount held; an estimate must stay labeled as estimated.

algo reserve(assignment, amount, purpose):                         -- Treasury
  -- At session opening: limit and both reserves are finite and nonnegative;
  -- verification_reserve + reporting_reserve ≤ limit. Reporting capacity covers
  -- the selected bounded narration/correction plan or its deterministic fallback.
  require spent + held + amount ≤ limit
  Production | Coordination: spent + held + amount ≤ limit − verification_reserve − reporting_reserve
  Verification:              spent + held + amount ≤ limit − reporting_reserve
  Reporting:                 spent_reporting + held_reporting + amount ≤ reporting_reserve
  -- Reporting is only narration/claim correction after work/finalization stops; it cannot extend production.
  -- Per-purpose spent/held amounts are projections of reservations and settlement events.
  otherwise Denied("budget")

algo settle(receipt): once for its invocation/reservation, spent += cost(receipt);
                     held −= reservation.amount; the unused remainder is released
  -- A repeated matching receipt is idempotent; conflicting receipt data is denied.
  -- Idempotence applies to committed settlements; an unsettled Unknown observation
  -- may be resolved by a later attributable receipt before settlement is committed.
  -- Unknown cost never becomes zero: retain the hold under Stop or settle the labeled allowance estimate.
  -- Revocation alone does not settle usage. Only verified never-started work releases without a receipt.

stop rule (in A1): no running assignments ∧ (max score (A3) < θ_min
                   ∨ Treasury.remaining(Verification) < minimum verification cost of unmet criteria)
                   → finalize; unmet criteria appear in the Report
minimum verification cost of unmet criteria =
    min over unmet required k of CostModel.estimate(Contribution{kind: Verify, targets: {k}},
                                                    cheapest eligible profile, history_view).expected
    -- no eligible profile or no estimate → treated as exceeding the remaining verification reserve
```

## 8. Invariants

```text
R-1  Only the kernel changes state; a strategy decision is a Proposal with a PolicyRef
R-2  Roles and authority apply within one Assignment and do not carry over to later ones
R-3  An assignment does not violate Pins or Constraints
R-4  A Reviewer is not the producer of the result version under review
R-5  The FinalReviewer is not among the producers of the final result; without such an agent the session is Blocked
R-6  An applicable executable check with outcome Fail overrides any approvals
R-7  Production and Coordination cannot consume verification or reporting reserves;
     Verification cannot consume the reporting reserve; Reporting is bounded by its own remaining reserve
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
R-19 Revocation is not termination: conflicting workspace holds persist until validated ended-effects
     or never-started evidence; financial holds persist until conservative settlement or verified non-execution
R-20 Explicit recovery preserves the session's task, accepted results, spent/held resources and history;
     it cannot revive a revoked grant, duplicate an invocation, or reset a budget
R-21 Agent-authored text (notices, offers, objections, handoff summaries, role output) enters another
     participant's context only as attributed, untrusted content; it carries no runtime, user or grant authority
```

## 9. Journal events

Common event envelope:

```text
event Envelope { seq: Int; session: Id<Session>; at: Instant; actor: Runtime | Id<Agent>
                 policy: Opt<PolicyRef>; input: Opt<Digest>; refs: List<Ref>; payload }
```

Minimum set; every kernel state change in section 4 maps to one of these events (D-2):

```text
SessionOpened · PoolRecorded · BudgetOpened · CriteriaCommitted · ClarificationRecorded · AssumptionRecorded
CheckRegistered · CheckRunRecorded · MutantRecorded · MethodChosen · PlanCommitted · PlanRevised
ContributionProposed · SolicitationOpened · SolicitationChanged · OfferSubmitted · Awarded
AssignmentAdmitted · AssignmentRevoked · GrantIssued · CommitmentChanged · ReservationChanged
WorkspaceOpened · LockChanged · SnapshotTaken · ResultMerged
InvocationStarted · InvocationEnded · ReceiptSettled · ResultSubmitted · EvidenceRecorded · ReviewRecorded
ObjectionRaised · ObjectionResolved · AcceptanceRecorded · Regraded · LedgerUpdated
ProgressAssessed · Diagnosed · Escalated · TeamChanged · NoticePosted · HandoffCreated · ReportingStarted · ReportDelivered
ObservationRecorded · ReputationUpdated · CalibrationRecorded · KnowledgeChanged · TrialRecorded
RetrievalRecorded · ConsequenceIngested
```

`PoolRecorded` records a Registry observation: discovery and dependency inputs,
the selected ReadinessProbe implementation and effective parameters, its
profile-specific proposals and input digests, and the kernel-derived Pool.
W1-0003 uses this event to satisfy D-2, D-5 and R-17 without repeating discovery
or filesystem inspection during replay. Recording readiness does not start an
invocation or confer assignment authority.

`BudgetOpened` records the immutable PriceBook and policy-derived ReportingPlan.
`ReservationChanged` records funding, single-use invocation authorization, receipt
observations, revocation and evidence-backed release. `ReceiptSettled` records the
selected CostModel decision. Observations precede pricing so pricing failure cannot
hide usage. These are financial facts; authorization does not establish that an
Invocation started or that its external effects ended.

`ReportingStarted` records Treasury's transition to bounded narrated reporting or
deterministic reporting. It makes A11's prohibition on extending production
replayable before ReportDelivered. It may downgrade from narrated to deterministic,
but cannot reopen production, override a user stop or grant execution authority.
After delivery, Curate remains subject to unprotected capacity under R-15.
The session lifecycle and report delivery remain owned by their kernel services.

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

The architecture is approved. The remaining choices concern implementation,
calibration and optional experimental policies, not whether the model is binding.

- `CreditPolicy` remains replaceable. Implement both the specified Confirmed-only
  default and an experimental variant that also credits `Discriminated`. The owner
  selects the policy for an experiment; architectural approval does not establish
  one permanent choice. Record the policy with observations so results from
  different choices are not silently conflated.
- Numerical values in A4, A8, A13 and section 10.1 are initial assumptions; their effect requires calibration before any claim.
- Concrete Rust boundaries, storage/adapters, supported native protocols and
  evolving TUI presentation are implementation choices within this approved model.
  The protected reporting reserve, recovery evidence rules and A8 evidence guard
  are architectural requirements, not options to be silently dropped.

### 12.1. Amendments (owner decision, 2026-09-16)

Recorded after the start review. Each item names the sections changed and the
tasks that own the behavior; none narrows earlier scope.

1. **Team operations through the execution port** (section 5.7, R-21). `ExecutionBackend.events`
   gains `OperationRequest(op, args, correlation)` and the port gains `reply`. The host forwards
   a request to the owning kernel service under the invocation's Grant; the actor is derived
   from the admitted assignment. Owner: W3-0001 (transport), W1-0017 (host contract).
2. **Stop rule input defined** (A13). "Minimum verification cost of unmet criteria" is the
   minimum expected cost of a `Verify` contribution over unmet required criteria for the
   cheapest eligible profile; absence of an estimate counts as exceeding the reserve.
   Owner: W1-0004 (estimate), W1-0014 (stop rule).
3. **Difficulty and competence bound to contributions** (sections 3.4, 3.8). `Contribution`
   carries `difficulty`; `competence(kind)` maps every `ContributionKind` to a `Competence`.
   Owner: W1-0006 (value), W1-0011 (estimate), W5-0001 (observation key).
4. **Proposed commitments have a creator** (section 4, P2). `Arbiter.award` creates
   `Commitment(Proposed)`; `Gatekeeper.admit` activates it or cancels it on denial.
   Owner: W1-0006, W1-0007.
5. **Untrusted participant text** (section 5.7, R-21). Text written by one agent reaches
   another only as attributed, untrusted content. Owner: W1-0013 (ContextComposer),
   W3-0001 (Board projections), W3-0005 (handoff).
6. **Declared implementation choices** (section 12): `load(a)` in A4 is the agent's count of
   `Running` assignments divided by `parallel_limit`, owned by VolunteerPolicy (W3-0003);
   "strengths" in DisputePolicy are `ReputationModel.estimate` for (profile, Verification,
   difficulty), and unknown or uncalibrated strength denies the debate path (W3-0006);
   "similar-task history" in A1.1 is the set of ExperienceVault observations whose scope
   matches the task (W6-0003); kernel concurrency serializes journal appends per session
   under the expected revision while independent sessions proceed concurrently, with the
   concrete form fixed by W1-0014 and preserved by W3-0008.
7. **TUI basis** (section 3.9): OpenCode is the primary inspiration; k9s guides
   lists, tables and descriptions/detail views. Use the `ymp2` TUI as the practical
   visual and interaction basis, with its revision and observed patterns recorded in
   [tui-reference.md](tui-reference.md). W1-0015 adapts it to the new application's
   operations and projections; current domain names, runtime authority and
   greenfield implementation rules continue to apply.
