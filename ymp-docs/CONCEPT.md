# Concept

**ymp** is a local environment in which coding agents organize the semantic search for a
solution while a small trusted kernel enforces the mechanical conditions required for safe
composition and independent evaluation.

## Product goal

A person writes a prompt. ymp turns it into an approved contract with a checkable acceptance
condition, raises several agents that divide the work among themselves with no assigner above them,
and returns a result an independent verifier accepted. What separates this from one strong agent is
not the interface but the absence of a single executor and of a central assigner deciding what was
produced.

Every mechanical and protective decision in this project exists to make that sentence trustworthy,
not to constrain it. The single kernel writer, the append-only control ledger, the private
per-attempt workspaces, the invocation-scoped tool bridge, and the protected acceptance bundle are
there so that a result can be believed and two ways of producing it can be compared. A decision that
hardens the system while making that path impossible has misread its purpose: it is recorded as a
divergence and resolved, not preserved as design.

The claim below is the falsifiable half of the same goal. The goal says what the product does; the
claim says what must be measured before the product may assert that doing it this way is better.

## The bounded claim

An agent is a stochastic, fallible executor. A collection of agents is not automatically more
capable or more reliable than its strongest member. ymp therefore makes a comparative claim,
scoped to an evaluated project class:

> Under the same total resource budget, locally coordinated search should produce a higher
> independently accepted-result rate than both a strong single-agent baseline and independent
> best-of-`n` search when the task is sufficiently decomposable and the acceptance oracle is valid.

The claim has three explicit boundaries:

1. It is **empirical**, not a theorem derived from agent count or topology.
2. It is **class-specific**, not a promise for arbitrary projects.
3. It is **oracle-relative**: `accepted` means that the exact candidate passed the approved
   observations. It does not certify unspecified intent or quality.

The system must be allowed to organize into one effective worker. Research on language-model
agents shows that communication can consume reasoning budget and amplify errors, especially on
sequential tasks or when a strong single-agent baseline is already high. Forcing collaboration
would contradict both the evidence and the self-organization goal.

## Two falsifiable hypotheses

ymp separates a product hypothesis from an observational research hypothesis:

1. **Instrumental reliability.** For some declared task classes, local coordination improves the
   independently accepted-result rate over both one strong agent and independent best-of-`n`
   search under the same total budget.
2. **Collective reasoning.** In some runs, participants form useful coordination patterns that
   were not prescribed by the kernel, use one another's messages causally, contribute
   complementary information, revise decisions for evidential reasons, preserve warranted
   dissent, and recover from perturbations.

The second hypothesis is deliberately behavioural. The base models already produce fluent human
language, so natural-looking conversation is not an emergent language and is not evidence of a
mind, consciousness, or understanding. What may emerge at system level are local roles,
specialization, conventions, dependency structure, and adaptive patterns of information use.

A candidate **collective-reasoning episode** has all of the following observable features:

1. the issue and response are not predetermined by a protocol role or central plan;
2. one participant publishes a bounded proposal, question, observation, or challenge with an
   artifact or evidence reference;
3. another participant independently evaluates it or supplies non-redundant information;
4. a later decision, action, revision, or justified abstention is sensitive to that exchange; and
5. the exchange improves accepted outcome, cost, calibration, or error recovery relative to an
   appropriate control.

One transcript is an anecdote. Causal use requires repeated matched-budget runs in which the
message is removed, replaced by a neutral payload, shuffled across tasks, or replaced with the same
underlying evidence delivered without another participant. The observatory records published
summaries and externally visible actions; it neither requests nor claims access to private
chain-of-thought.

## What selection can and cannot do

If one attempt independently produces a truly valid candidate with probability `p`, then `k`
attempts produce at least one with probability `1 - (1 - p)^k`. This familiar expression is an
upper-level model, not an operational guarantee: agent errors, prompts, context, tools, and
training data are correlated.

Selection also magnifies defects in the oracle. If an invalid candidate independently has
probability `q` of exploiting an acceptance blind spot, testing `k` candidates raises the chance
that at least one exploit is found to `1 - (1 - q)^k`. Repeated feedback makes attempts adaptive,
so a fixed holdout can itself be overfitted.

Consequently:

- selection cannot compensate for missing acceptance criteria;
- consensus is not acceptance;
- a different model name is not proof of independent error;
- a failed search may consume the whole budget without approaching a solution; and
- stopping with `exhausted` or `abstained` is a required correct outcome.

Protected checks, a verification-query budget, bounded diagnostic disclosure, negative
controls, and blinded review are part of the selection mechanism. See
[PROJECT-CONTRACT.md](PROJECT-CONTRACT.md).

## Self-organization by local commitment

There is no global task allocator. Work propagates through local, temporary agreements:

1. An origin participant receives the approved root goal as the initial condition.
2. Any participant may retain work, advertise part of it, or ask for another participant by
   transferring part of its own budget into escrow.
3. Other active participants may bid, decline, counter-propose, or publish an unsolicited
   challenge.
4. The task sponsor chooses among bids from its local perspective. A binding task contract is
   formed only by mutual consent.
5. A contractor may in turn become a sponsor for work it creates. Roles are therefore local,
   temporary, and not assigned by the kernel.

This follows the useful part of the Contract Net model: negotiation is distributed and the
manager/contractor distinction is dynamic. It does not assume that a single scalar rank can
capture a multidimensional task or that an agent's self-description is true.

The root participant is an ignition point, not a permanent orchestrator. It may work alone,
create peers, delegate, hand off sponsorship, or be replaced after expiry. Different runs may
form a hierarchy, a set of competing branches, a peer network, or no collaboration at all.

An offer or bid may request an approved **runtime profile**: an agent harness and prompt policy,
model route, coordination-tool binding, and execution-assurance profile. This is a resource and
provenance constraint, not a semantic assignment. NVIDIA Nemotron, for example, is a model route
that may be paired with OpenCode, a compatible Claude Code or Codex profile, or a future native
runtime. The kernel checks declared capabilities, disclosure policy, and reserved resources but
does not decide that a task “needs” a particular runtime or model.

## The board is an untrusted medium, not an organizer

A task-scoped collaboration board exposes attributed offers, bids, messages, findings,
challenges, and artifact references to authorized participants. Agents adapt to these
observations. The board does not rank semantic importance, decide the next action, execute a
payload, fetch a referenced resource, transfer a capability, or serve as authoritative control
state.

The trusted control ledger separately records budgets, leases, capabilities, obligations, and
authoritative transitions. The verifier and security controller form a third plane that does not
consume participant conversation when producing an initial independent result. These may share one
local store implementation, whether the POC journal or a later embedded database, but they have
distinct schemas, write authorities, visibility rules, and capability namespaces.

This separation matters because a shared board can accelerate both useful discovery and harmful
coordination. A participant may publish a false or malicious instruction, but another participant
cannot turn that message into authority: every consequential request is mediated against the
requester's own non-delegable capability and task scope.

Persistent history and active salience are distinct. Audit records and message digests remain
immutable, while task offers and finding projections expire unless refreshed. This prevents stale
statements from remaining permanently prominent without deleting evidence. Publication and
refresh consume bounded communication resources. Project-wide discovery notices contain only
bounded summaries; detailed discussion is confined to a task, candidate-review group, or
explicitly admitted audience.

A blackboard architecture still has a control problem: someone must decide which possible
action to perform. In ymp that decision remains with each participant and each local task
sponsor. It is not hidden inside the board implementation.

## Constitutional constraints and derived invariants

The original “exactly four fixed things” formulation mixed product principles with the many
protocol rules needed to make them real. The four constitutional constraints remain:

1. a human-approved definition of done;
2. a finite resource and authority envelope;
3. isolation between untrusted attempts and trusted state; and
4. verification independent of the candidate-producing attempt.

“Isolation” has two acceptance levels. The coordination experiment requires separate writable
workspaces, immutable candidates, trusted control state outside agent reach, and an oracle outside
the producing attempt so results are attributable. During POC, a disposable outer study harness
supplies those boundaries. Hostile-code containment implemented and evidenced by ymp is a separate
MVP requirement. Therefore POC can test the coordination hypothesis without making a product
security claim, but it cannot discard logical attempt or oracle separation without invalidating the
experiment.

They are necessary but not a complete state machine. The following mechanical invariants are
derived from them and do not prescribe a solution method:

- approved contract inputs, protected checks, and base snapshots are immutable and addressed
  by digest;
- no task, message, attempt, external action, or verification can be created without reserved
  capacity from a finite budget vector;
- each concurrent attempt has a private writable workspace and explicit capabilities;
- shared-state commands are authenticated, idempotent, and validated atomically;
- expiring leases carry monotonic fencing tokens, so stale work cannot win a race;
- every delegation creates a tracked work obligation that must be discharged or cancelled;
- submissions name an immutable base, and integration produces a new immutable candidate;
- only the verifier can create acceptance evidence, and agents cannot write its control state;
- collaboration messages are inert untrusted data and cannot contain delegable authority;
- verifier inputs exclude the collaboration board until any required independent initial
  assessment has been committed; and
- every run ends as `accepted`, `exhausted`, `cancelled`, `abstained`, or `infrastructure_error`.

Changing these rules may change what shared effects are possible, but it does not select a role,
task decomposition, model, method, or answer.

## Safety, stabilization, liveness, and success are different claims

The protocol defines a **legitimate control state** as one in which budgets are conserved,
ownership is unambiguous, immutable objects retain their digests, capabilities remain within
their grants, and all outstanding work has a recorded obligation. Valid commands preserve this
set.

Within the stated fault model, replay after a process crash reconstructs a legitimate state from
the durable control ledger. Expiry and fencing prevent an old worker from changing current state.
Fair, non-semantic admission prevents one principal from monopolizing finite execution slots.
Obligation accounting detects quiescence when delegated work has returned.

These properties support three limited guarantees:

- **safety:** an invalid shared-state transition is rejected;
- **recovery:** declared process and delivery faults converge back to a legitimate control
  state; and
- **termination:** finite budgets and finite leases eventually stop the run.

They do **not** guarantee convergence to a solution. Whether local interactions improve the
chance of acceptance is the project hypothesis and must be tested experimentally.

## Negative feedback without semantic dispatch

The environment supplies bounded negative feedback:

- a child task or new participant consumes budget already owned by its sponsor;
- every created work obligation consumes a non-refundable authority unit, making the total
  amount of decomposition finite;
- expiring offers, leases, and finding salience remove stale coordination state;
- per-principal round-robin admission and rate limits damp message and spawn storms;
- task age, failed attempts, bid count, congestion, deadline, and remaining escrow are visible
  as raw local signals; and
- verification queries are scarce, preventing unlimited adaptive probing of protected checks.

The kernel does not turn those signals into a priority, grade, required level, or prescribed
escalation. Agents may respond to unmet demand by decomposing, retrying, recruiting, simplifying,
or stopping. Response-threshold and activity-damping policies are candidates for later ablation,
not hidden rules in the first implementation.

## A run

1. ymp captures an immutable source snapshot and loads an approved contract package.
2. The kernel creates the root obligation and starts the configured origin participant. Starting
   peers or competitors is permitted but consumes the same run budget.
3. Participants work, advertise tasks, negotiate commitments, and publish bounded findings on
   task-scoped boards. Every request with shared effects uses the requester's own capability and
   locally delegated resources rather than authority implied by a message.
4. Each attempt works in a private snapshot and submits a patch or artifact bundle against an
   explicit base digest.
5. The integrator mechanically constructs immutable candidates. Semantic conflicts become new
   work; the integrator does not resolve them.
6. An authorized participant spends a reserved verification query on a candidate. The kernel
   verifies the exact digest but does not choose which candidate deserves testing.
7. A passing candidate produces contract-relative acceptance evidence. If obligations and funded
   actions end first, the run terminates without claiming success.

## Falsifiability

The first evaluation compares, at matched total cost:

- one strong agent;
- independent attempts plus a blinded candidate selector and the same verifier; and
- locally negotiated self-organization.

Tasks are stratified by decomposability and sequential dependence. Acceptance uses protected
checks plus a blinded human audit sample. Coordination is useful only if it improves the true
accepted-result rate beyond independent selection by enough to justify its overhead. If it does
not, the board is an observability feature rather than the claimed reliability mechanism.

The separate collective-reasoning analysis measures:

- **positive signaling:** whether a message reflects information available to its sender;
- **positive listening:** whether a receiver's later action changes when the message changes;
- **task value:** whether correct messages outperform absent, neutral, or shuffled messages;
- **separate-participant value:** whether another participant adds value beyond exposing the same
  raw evidence to one executor;
- **complementarity:** whether the accepted artifact causally combines non-redundant contributions
  and beats the strongest individual and independent-selection baselines;
- **revision quality:** whether verified evidence changes decisions while unsupported social
  pressure does not erase a correct independent view;
- **adaptive organization:** whether unprompted roles and communication topology change usefully
  after task or participant perturbations; and
- **efficiency and resilience:** outcome gain per communication cost and recovery after delayed,
  missing, false, or conflicting messages.

These are experimental measurements, not rewards or admission rules. Optimizing message count,
agreement, self-reported confidence, eloquence, or influence would invite performance theatre and
manipulation. A general collective-intelligence claim requires repeated out-of-sample performance
across many tasks and groups; it cannot be inferred from a single successful run.

The protocol and evaluation plan are detailed in [PROTOCOL.md](PROTOCOL.md) and
[ROADMAP.md](ROADMAP.md).
