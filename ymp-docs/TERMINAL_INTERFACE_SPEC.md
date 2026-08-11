# ymp terminal interface technical specification

## 1. Purpose and status

This document defines the technical, semantic, operational, and accessibility requirements for the
complete local terminal interface of **ymp**.

It is a normative product specification, not a screen brief. It defines what an operator must be
able to understand and do, which states and consequences must be represented, and which technical
constraints an implementation or design must respect. It deliberately does not prescribe:

- a navigation model;
- a number of screens;
- a panel, column, tab, modal, drawer, or dashboard structure;
- a visualization type;
- a color palette;
- a component library beyond the fixed implementation medium; or
- a particular information hierarchy beyond required discoverability and consequence visibility.

The interface must be designed as one coherent end-to-end product. Implementation sequencing in
`ROADMAP.md` must not divide the experience into separate product designs or produce incompatible
interaction systems.

All requirements in this specification apply to the same conceptual interface unless a capability
is explicitly unavailable on the selected runtime, model route, platform, assurance profile, or
contract. Capability absence must be represented as state, not hidden by fallback or omitted from
the design model.

## 2. Normative sources and precedence

The complete interface design must be informed by this specification and the following project
documents:

1. [README.md](README.md) — bounded product claim and distribution boundary;
2. [CONCEPT.md](CONCEPT.md) — self-organization boundary and research hypotheses;
3. [ARCHITECTURE.md](ARCHITECTURE.md) — components, trust planes, local topology, and runtime
   boundary;
4. [PROTOCOL.md](PROTOCOL.md) — entities, commands, transitions, obligations, budgets, and terminal
   outcomes;
5. [PROJECT-CONTRACT.md](PROJECT-CONTRACT.md) — contract approval, oracle validation, and
   acceptance semantics;
6. [SECURITY.md](SECURITY.md) — threats, assurance profiles, disclosure, and operator
   consequences;
7. [REPUTATION.md](REPUTATION.md) — contextual evidence and the rejection of scalar grades;
8. [DECISIONS.md](DECISIONS.md) — settled decisions, rejected mechanisms, and open experiments;
9. [INVARIANTS.md](INVARIANTS.md) — implementation contracts that the interface must not weaken;
   and
10. [ROADMAP.md](ROADMAP.md) — implementation order and shipping evidence, not interface
    segmentation.

If this specification conflicts with a governing project document, the conflict must be reported
before design or implementation. Visual composition must not silently resolve a protocol,
security, or product contradiction.

## 3. Product and execution boundary

### SYS-01 — Distribution

ymp is distributed as one installed Rust executable. Running `ymp` starts the foreground terminal
interface, trusted application core, single authoritative writer, local store, and runtime
supervisor in one main process. The same executable may start private child modes. The operator
starts only `ymp`, not agent runtimes in separate terminals.

### SYS-02 — Managed runtimes

Codex and Claude Code are managed external child runtimes. OpenCode and an ymp-native runtime may
become available through approved runtime profiles. Availability must be based on an actual probe;
the interface must not advertise a runtime, route, model, or assurance level that is unavailable.

OpenCode runtime profiles and NVIDIA Nemotron model-route profiles belong to the same unified
capability catalogue, but may appear as selectable only after explicit approval and a successful
route-feasibility or capability probe. Their presence in the product model is not evidence of
current availability.

### SYS-03 — Local foreground mode

The current product surface has no background daemon, operator socket, HTTP listener, WebSocket
endpoint, browser, WebView, Electron shell, HTML/CSS runtime, local web server, external rendering
process, remote administration client, service dashboard, PostgreSQL dependency, object-storage
service, or Kubernetes control surface.

### SYS-04 — Trusted and untrusted execution

The main process owns authoritative state and supervises child processes. Agent runtimes and
project code are untrusted. A terminal presentation must not imply that an untrusted message,
runtime event, process exit, or generated artifact can directly mutate authoritative state.

### SYS-05 — Disposable assurance boundary

The `poc_disposable` execution-assurance profile assumes an externally disposable study
environment with curated non-sensitive repositories, experiment-specific provider quota, no
production credentials, and no irreversible external actions. Private writable workspaces
preserve attribution between attempts. They are not, by themselves, evidence of hostile-code
containment.

The interface must expose the selected assurance profile and its limitations. It must not display
a sandbox, security, or containment claim that the selected platform and profile do not establish.

### SYS-06 — Bounded product claim

ymp tests whether, for declared classes of reproducible software tasks with a validated acceptance
oracle, locally coordinated search can improve independently accepted-result rate over both a
strong single agent and independent best-of-`n` search under the same total resource budget. The
interface must not present this hypothesis as an established property of a run or of the product.

### SYS-07 — Self-organization boundary

Participants control the semantics of work. They may work alone, advertise work, bid, form
temporary local contracts, recruit, delegate, compete, challenge, synthesize, or stop. The kernel
controls mechanical shared effects: identity, authentication, budgets, leases, authority, immutable
objects, integration, verification, obligation accounting, and termination.

The interface must expose self-organization without turning ymp into a global dispatcher,
semantic planner, task allocator, escalation policy, or candidate-ranking system.

## 4. Primary operator and responsibility boundary

The primary operator is a technically proficient local researcher or developer using a keyboard
in a terminal. The operator may understand Git and coding-agent runtimes but must not be required
to understand the complete ymp protocol before starting a supported run.

The operator is trusted to:

- select and inspect local project input;
- approve semantic intent and contract lineage;
- approve provider disclosure and model routes;
- approve finite budgets and authority;
- understand the selected execution-assurance profile;
- authorize one exact run configuration;
- perform explicit, attributable interventions;
- cancel execution;
- inspect terminal outcomes and independent evidence; and
- export or archive evidence according to policy.

The operator is not a hidden project manager for participants. The interface must not make the
operator responsible for:

- assigning semantic tasks to participants;
- selecting bids on behalf of a participant sponsor;
- prescribing local roles or communication topology;
- resolving participant disagreement as a normal scheduling function;
- ranking participants, offers, or candidates;
- declaring a candidate accepted without approved independent evidence; or
- treating fluent conversation as evidence of correctness, causation, intelligence, or
  leadership.

Human actions must be separately attributed and must never appear to originate from a participant
or the kernel.

## 5. End-to-end operator journey

The interface must support one coherent journey:

```text
start ymp
  → establish local ownership and storage readiness
  → select a project and capture an immutable source snapshot
  → establish runtime, model-route, authentication, and disclosure readiness
  → select or review the contract package and oracle validation
  → review assurance, authority, budgets, and study context
  → authorize one exact run configuration
  → observe bounded participant work and local commitments
  → inspect communication, candidates, integration, evidence, and limiting resources
  → intervene only through explicit actions with visible consequences
  → reach and understand one exact terminal outcome
  → export reproducible evidence and archive the run
```

The journey may be represented through any coherent information architecture. Required states and
details may use progressive disclosure, but the operator must always be able to recover the
current project, run, exact outcome state, and next valid action.

## 6. User stories

Each user story defines operator intent rather than a screen. A conforming design must cover the
normal path, empty state, loading state, material failure variants, stale-state behavior, and
recovery path where applicable.

### US-01 — Start the application and establish local ownership

**Story.** As a local operator, I want to start one foreground ymp instance and understand its
local data ownership, so that I do not unknowingly compete with another writer or operate on an
unusable store.

The operator must be able to:

- confirm application version and build identity;
- see the current platform and available execution-assurance capability;
- confirm or select the local data root;
- see single-writer ownership state;
- inspect store and schema version, last durable sequence, and journal integrity;
- distinguish first launch, ready state, recoverable warning, and blocking startup failure; and
- safely exit when startup cannot continue.

The interface must explain data-root lock conflict, unsupported platform, insufficient filesystem
access, storage full, incompatible schema, corrupt journal, event gap, digest mismatch, and
incomplete journal tail without implying that authoritative state was lost when recovery from the
durable journal remains possible.

### US-02 — Select a project and capture the exact source input

**Story.** As an operator, I want to select a repository and review exactly what source state ymp
will capture, so that all attempts and evidence share an attributable immutable base.

The operator must be able to:

- select a repository or supported source input;
- see the display name and canonical local path;
- inspect the base revision where available;
- review the declared treatment of modified, staged, untracked, ignored, generated, and unsupported
  files;
- review toolchain and environment identity;
- see source-capture warnings before authorization;
- confirm the resulting source digest and capture time; and
- understand that the user working tree is input, not a shared execution workspace.

Repository unavailable, unsupported repository, undeclared modification policy, and capture
failure must be distinct conditions.

### US-03 — Discover and validate agent runtimes

**Story.** As an operator, I want ymp to discover installed runtimes and explain exact capability
gaps, so that I can resolve readiness without relying on a generic installed/not-installed badge.

The operator must be able to:

- discover Codex, Claude Code, and any other approved runtime automatically;
- inspect the discovered executable path and digest;
- provide an explicit executable-path override;
- start or repeat a capability probe;
- inspect runtime kind, version, driver version, harness policy, prompt-policy version, and
  coordination-tool binding;
- see probe status, timestamp, and staleness;
- distinguish not installed, discovering, probing, ready, busy, stale, degraded, incompatible,
  unauthenticated, and unavailable; and
- follow a runtime-owned authentication or capability recovery action where one exists.

The runtime capability model must independently represent start, resume, interruption, structured
events, session continuity and opaque session capsules, MCP and other approved coordination-tool
bindings, generated-configuration isolation, descendant cancellation, native subagent suppression,
usage evidence, hard cost limiting, request cancellation, provider/model pinning, and cleanup.

### US-04 — Configure and approve model routes and disclosure

**Story.** As an operator, I want to inspect the exact model route and disclosure class behind a
runtime profile, so that authorizing Codex or Claude Code never hides a provider, model, endpoint,
account, or data-disclosure change.

The operator must be able to inspect:

- provider and deployment;
- endpoint class and wire protocol;
- account or quota scope;
- model identifier or snapshot;
- authentication mode without exposing raw credentials;
- approved data-disclosure class;
- provider usage and cost evidence;
- hard-limit and request-cancellation capability;
- known unavoidable overshoot; and
- route availability, rejection, rate limiting, cancellation, or incompatible events.

ymp must not collect raw long-lived provider credentials in its own forms. Authentication recovery
must remain attributable to the owning runtime or provider mechanism.

The interface must never use “Codex ready” or an equivalent runtime-only label as a substitute for
the complete route identity. It must not silently substitute another provider, model, account,
route, disclosure class, or weaker assurance profile.

### US-05 — Select and understand a contract package

**Story.** As an operator, I want to select a contract package and understand its public intent and
protected acceptance boundary, so that I know what the run is authorized to attempt and what
independent evidence can establish acceptance.

The operator must be able to inspect:

- package identifier, version, digest, and lineage;
- approval state, approver, approval time, and superseded package;
- public goal, scope, non-goals, observable requirements, and delivery format;
- environment manifest and permitted external services;
- visible checks;
- protected-bundle presence and digest without protected bytes;
- requirement-to-evidence coverage and uncovered requirements;
- oracle validation status;
- known-good control, invalid mutations, negative controls, and reproducibility;
- review protocol and required human authority;
- budget, evidence-disclosure, observation, runtime, model-route, and assurance policies; and
- unresolved ambiguity, inaccessible dependency, forbidden external effect, or coverage gap.

The operator must be able to distinguish no package, draft, awaiting approval, approved,
superseded, invalid digest, source mismatch, and blocked package.

### US-06 — Review, approve, supersede, or amend a contract

**Story.** As an authorized operator, I want to review and approve a structured contract lineage,
so that semantic approval is explicit and changes never rewrite the meaning of an existing run.

The interface must support the product model for:

- structured draft review;
- approval with attributable authority;
- rejection or return with explicit findings;
- package supersession;
- amendment as a new immutable package and lineage; and
- clear distinction between contract data, protected oracle material, and local display
  preferences.

Protected oracle bytes and raw secrets must not be authored or displayed through ordinary
contract forms. An active run's contract must never be edited in place. A budget or contract
amendment must create a new lineage or explicitly mark the result non-comparable according to
policy.

### US-07 — Review preflight and authorize one exact run

**Story.** As an operator, I want a concise but complete preflight review, so that I authorize one
exact combination of source, contract, runtime profile, model route, assurance profile, budget,
and study context without hidden fallback.

The operator must be able to:

- review project and captured source;
- review contract approval and oracle-validation summary;
- review coverage gaps and disqualifying failures;
- select the origin runtime profile from the approved set;
- review exact route identity and disclosure;
- review the execution-assurance profile and prerequisites;
- inspect the complete finite budget vector;
- distinguish enforced, observed, estimated, unavailable, and stale limits;
- confirm study condition, seed or repetition, stopping rule, and comparison policy where
  applicable;
- see warnings separately from blocking failures; and
- authorize one immutable run configuration.

The ready state must state exactly what will run. Provider-disclosure conflict, disallowed profile,
insufficient budget, invalid contract, invalid oracle, source mismatch, and incompatible assurance
must block authorization rather than degrade silently.

### US-08 — Start the run and understand startup progress

**Story.** As an operator, I want to see bounded startup progress and typed failure reasons, so that
I can distinguish normal initialization from a runtime, route, storage, or infrastructure failure.

The interface must represent configured, preflight, awaiting authorization, starting, running,
admission paused, cancelling, finalizing, and terminal states. It must show the origin participant,
approved runtime-profile set, start reservations, and any startup recovery action without
presenting the origin as a permanent manager.

### US-09 — Understand the current run without reading raw events

**Story.** As an operator, I want to understand current outcome state, material activity, required
attention, candidates, verification, obligations, and limiting resources at a glance, so that I do
not have to reconstruct authoritative state from an event stream.

The interface must make discoverable:

- run identifier and contract lineage;
- source snapshot;
- creation, start, elapsed, and terminal times;
- exact lifecycle and terminal outcome;
- autonomous, scheduled-intervention, or unscheduled human-intervened status;
- origin participant and active participant count;
- overall obligation, integration, verification, and command-queue activity;
- candidate count and verification state;
- current limiting budget or authority dimension;
- warnings and required operator actions;
- temporary absence of visible activity while funded work remains; and
- quiescence while terminal accounting is being established.

The event view may support diagnosis, but it must not be the only way to understand current state.

### US-10 — Inspect a participant, attempt, invocation, and workspace without conflation

**Story.** As an operator, I want to inspect each execution identity separately, so that I can
understand provenance, session continuity, resource use, and failure location.

Inspection must reveal:

- participant identifier and parent recruitment lineage;
- runtime profile and model route;
- current local task contract and obligation;
- participant-local budget account;
- attempt identifier, immutable base, private workspace identity, and capability summary;
- lease generation, expiry, and fencing token;
- invocation state, start or resume count, event cursor, wake conditions, and deadline;
- measured resource and model-route usage;
- typed completion, yield, limit, runtime, route, cancellation, or infrastructure reason; and
- session continuity status without exposing opaque session capsules or private reasoning.

Participant-published role labels are untrusted context. `origin`, `sponsor`, and `contractor` are
factual relationships, not grades or permanent hierarchy.

### US-11 — Follow local negotiation and work obligations

**Story.** As an operator, I want to understand how participants formed temporary commitments and
how causal work returns, so that self-organization is observable without appearing centrally
assigned.

The operator must be able to inspect:

- task scope, immutable base, path or capability scope, dependencies, sponsor, audience, and
  causation references;
- offer policy (`negotiated`, `open_accept`, or `targeted`);
- funded award slots, escrow, proposal allowance, expiry, and active-projection lifetime;
- bid author, expiry, counter-offer, expected artifact class, method-message reference, withdrawal,
  and award result;
- mutual consent record, sponsor, contractor, transferred escrow, lease policy, and child
  obligation;
- obligation parent, children, authority charge, dependencies, state, and typed return;
- open-accept race won or lost;
- current, near-expiry, expired, and renewed leases;
- stale results preserved but unable to advance state; and
- child return, cancellation, dead end, exhaustion, and complete terminal subtree.

The interface must not recommend a bid, rank an offer, assign a task, infer competence, or suggest
a prescribed escalation path.

### US-12 — Read scoped collaboration as inert untrusted data

**Story.** As an operator, I want to inspect participant communication with its exact audience,
attribution, references, and delivery state, so that I can study coordination without confusing
text with authority or evidence.

The operator must be able to inspect:

- audience identifier, scope, membership, read/publish rights, and expiry;
- message publication order, identifier, author, timestamp, kind, bounded payload, digest, reply
  relationship, and active-projection expiry;
- `proposal`, `question`, `hypothesis`, `observation`, `constraint`, `dead_end`, `challenge`,
  `confirmation`, `decision`, and `help_request` message kinds;
- task, offer, bid, candidate-review, artifact, evidence, and claimed decision-causation
  references;
- publication, delivery, citation, and read-cursor states as separate observations;
- communication charges for publication, refresh, and delivered bytes;
- active versus expired salience while immutable history remains; and
- project-discovery summary versus detailed task or review audience.

Delivery must not appear as reading; citation must not appear as agreement; agreement must not
appear as truth. Message content must be escaped and rendered as inert text. ANSI control
sequences, terminal escapes, hyperlinks, and code-like payloads must not execute or become
automatic actions.

### US-13 — Understand finite budgets, reservations, and authority

**Story.** As an operator, I want to inspect each independent resource and authority dimension, so
that I can identify the actual limiting condition and understand why spare capacity elsewhere does
not authorize an action.

The complete budget model must make discoverable:

- model-route cost and reported model tokens;
- wall time, CPU, memory, process count, disk, and output bytes;
- participant, attempt, and invocation starts;
- active and total audience grants;
- outstanding and total obligations, offers, and other creation authority;
- control commands, messages, published bytes, and delivered bytes;
- protected verification queries;
- dependency and network broker requests;
- credential uses; and
- externally consequential actions when explicitly permitted.

For each dimension, distinguish total, reserved, consumed, settled or returned, remaining, rate
where useful, projected unavoidable overshoot where known, and telemetry timestamp. Label a limit
`enforced` only when the provider, broker, operating system, or controller can stop the effect;
otherwise label it `observed` or `estimated`.

The operator must be able to understand unused, reserved, consuming, near exhaustion, exhausted,
settled, unavoidable overshoot, unavailable telemetry, and stale telemetry. One exhausted
dimension may block an action while other dimensions retain capacity.

### US-14 — Inspect submissions, integration, and candidate ancestry

**Story.** As an operator, I want to inspect immutable submissions and candidates with exact
ancestry, so that parallel results and integration failures remain attributable without inventing
a canonical mutable branch.

The operator must be able to inspect:

- submission producer, task, obligation, attempt, fencing generation, base digest, paths, artifact
  class, object digests, and submission time;
- integration queue and exact mechanical state;
- candidate digest, base, parent candidates, contributing submissions, participant and obligation
  provenance, creation time, and verification state;
- parallel candidates without a visual `main` or preferred branch;
- multi-parent synthesis;
- stale base, stale fence, unauthorized path, object mismatch, partial object, integration
  conflict, and other typed failures; and
- participant-sponsored rebase or synthesis as new work rather than automatic semantic conflict
  resolution.

When several exact root candidates pass and no approved tie-breaker exists, the interface must
show an accepted set rather than automatically selecting one winner.

### US-15 — Inspect independent verification and evidence

**Story.** As an operator, I want to inspect verification bound to one exact candidate and oracle,
so that candidate failure, acceptance, and verifier infrastructure failure cannot be confused.

The operator must be able to inspect:

- requester, candidate, contract scope, protected-query reservation, request time, and state;
- contract, candidate, environment, and oracle digests together;
- verifier and toolchain identity;
- verification regime and isolation evidence;
- approved diagnostic disclosure level;
- required machine checks, negative controls, reviewer criteria, and incomplete evidence;
- blinded initial assessment and later context reveal when required by the oracle;
- remaining protected-query budget; and
- reproducibility status from a clean verification rerun.

`passed`, `failed`, and `infrastructure_error` are mutually distinct verifier results. A failed
protected check is candidate evidence. A verifier crash, corrupt oracle, missing object, failed
negative control, or lost evidence integrity is infrastructure failure. A subtask-scope pass is
not root acceptance.

### US-16 — Diagnose events and boundary activity

**Story.** As an operator, I want an ordered diagnostic history with provenance and recovery from a
durable cursor, so that I can investigate behavior without treating notification loss as state
loss.

The operator must be able to inspect:

- ordered domain-event sequence and durable cursor;
- command identifier, correlation and causation references, subject, object, result, and failure
  class;
- process start, resume, yield, interrupt, exit, and descendant cleanup;
- lease, expiry, fencing, reservation, transfer, charge, and settlement events;
- audience, publication, delivery, integration, and verification events;
- provider, dependency, network, credential, and external-action boundary events;
- journal gap, duplicate sequence, digest mismatch, incomplete tail, and controller failure; and
- filter, search, follow, pause of visual updates, and cursor recovery.

Pausing visual updates must not pause authoritative execution unless a separate explicit action
does so.

### US-17 — Inspect the communication observatory without causal overclaim

**Story.** As a research operator, I want to compare published communication, later decisions,
artifact changes, and controlled intervention results, so that signaling, listening, and task
value remain separate empirical questions.

The read-only observatory must make inspectable:

- published signal, delivery, reply, citation, declared evidence use, decision, artifact change,
  and verification result;
- independently committed initial positions versus later revisions;
- challenges, confirmations, unresolved disagreement, and warranted dissent;
- complementary contributions to candidate ancestry;
- locally formed and changing sponsor, contractor, audience, and communication topology;
- participant, message, dependency, or evidence removal and subsequent adaptation;
- original-message, no-message, neutral, shuffled, direct-evidence, delayed, false-message, and
  participant-loss conditions; and
- positive signaling, positive listening, and task value as separate study results.

Live provenance and temporal association are not causal evidence. Leadership-like interpretation
requires unprompted formation, causal influence, outcome value, and transfer or re-formation after
removal. Before those conditions are established, use factual labels such as sponsor, origin,
message volume, or graph centrality.

The observatory must not request, infer, display, or claim private chain-of-thought. It may contain
only deliberately published summaries, artifacts, decisions, externally visible actions, and
verifier evidence.

### US-18 — Navigate and inspect without changing run state

**Story.** As an operator, I want to navigate, search, filter, and inspect freely, so that learning
about a run never mutates authoritative state.

Non-destructive actions include:

- navigate, search, filter, sort, expand, collapse, follow, and inspect;
- change selected scope or audience view;
- copy an identifier, digest, path, or bounded text selection;
- inspect causation and provenance references;
- open contextual help and exact term definitions; and
- change local display preferences allowed by policy.

These actions require no confirmation and must not spend run authority or create a protocol event
except bounded local diagnostic/audit records explicitly defined for the application.

### US-19 — Send an attributed human message

**Story.** As an authorized operator, I want to send a scoped message with its experimental
consequence shown before confirmation, so that intervention is explicit and attributable.

Before confirmation, the interface must show:

- exact audience and scope;
- operator attribution;
- paste and final text review;
- authorization and communication-budget effect;
- whether the run is under a scheduled intervention condition; and
- whether the message marks the run human-intervened and excludes it from autonomous comparison.

The message must remain inert untrusted data and must not grant authority or execute a referenced
action.

### US-20 — Pause or resume admission with capability-aware semantics

**Story.** As an operator, I want to pause or resume supported execution admission, so that I can
control new work without assuming an in-flight provider request can be interrupted.

The interface must explain whether pause:

- blocks new participant or invocation admission;
- interrupts current invocations;
- can cancel an in-flight provider request;
- preserves leases and wake deadlines; and
- is supported by the current runtime profile.

Pause, visual follow-mode pause, invocation interruption, and run cancellation must remain distinct
actions.

### US-21 — Cancel a run

**Story.** As an operator, I want to cancel one exact run with irreversible consequences shown
before confirmation, so that cancellation cannot be mistaken for closing a view or stopping one
invocation.

Before confirmation, the interface must identify the run and explain that cancellation:

- stops new admission;
- requests supported interruption and descendant cleanup;
- revokes invocation-scoped endpoints;
- closes or cancels outstanding work according to protocol;
- finalizes the run as `cancelled` rather than `failed` or `accepted`;
- cannot be undone; and
- preserves immutable history and retained evidence.

### US-22 — Approve a contract or budget amendment

**Story.** As an authorized operator, I want to approve an explicit amendment with lineage and
comparison consequences shown, so that active evidence is never reinterpreted under silently
changed rules.

The interface must identify the exact changed fields, authority, additional reservation, new
package or run lineage, and effect on study comparability. Expanded budget must not rewrite the
original finite envelope without an attributable lineage event.

### US-23 — Handle stale selections and action races safely

**Story.** As an operator, I want the interface to detect when a selected offer, lease, grant,
reservation, or run state changed before confirmation, so that stale information cannot produce an
unintended effect.

When preconditions are stale, the interface must:

- reject or abandon the stale action without control-state mutation;
- refresh authoritative current state;
- identify the expired or changed object;
- explain why no effect occurred; and
- preserve the historical record for inspection.

### US-24 — Understand the exact terminal outcome

**Story.** As an operator, I want one exact terminal reason and its evidence, so that completion,
candidate failure, infrastructure failure, exhaustion, abstention, cancellation, and acceptance
cannot be confused.

Every run ends as exactly one of:

- `accepted`;
- `exhausted`;
- `abstained`;
- `cancelled`; or
- `infrastructure_error`.

There is no generic successful terminal state. `accepted` must identify an exact root candidate
and approved oracle evidence. Process exit, participant return, consensus, confidence, a passing
subtask, or an integrated candidate is not acceptance.

The terminal summary must make discoverable:

- exact terminal reason and time;
- obligation and quiescence accounting;
- final budget settlement;
- accepted, failed, passing, and unverified candidates;
- multiple passing candidates with no tie-breaker;
- final comparison eligibility;
- incomplete or invalidated evidence; and
- available reproduction, export, and archive actions.

### US-25 — Reproduce verification when authorized

**Story.** As an operator, I want to request or inspect a clean verification rerun when the contract
and remaining query budget permit it, so that reproducibility can be established without silently
spending protected-query authority.

The interface must show the exact candidate and four-digest binding, remaining query budget,
reservation consequence, disclosure policy, and clean-environment identity before request.

### US-26 — Export reproducible evidence

**Story.** As an operator, I want to configure and export a reproducible evidence package, including
a policy-authorized point-in-time diagnostic package during a live run, so that the run can be
independently inspected without applying candidate code or exposing protected material.

The operator must be able to:

- select destination;
- select an allowed redaction policy;
- understand included records and excluded protected material;
- observe export progress;
- receive an export digest and completion state; and
- distinguish complete export, failed export, required redaction, and rejected partial output.

Evidence export copies records and objects. It does not apply a candidate, authorize delivery, or
modify the user's working tree.

### US-27 — Archive and recover terminal runs

**Story.** As an operator, I want to archive a terminal run from default views without rewriting
history, so that local navigation remains manageable while retention and evidence rules are
preserved.

The interface must distinguish archived, restored to default views, retained by policy, and
deletion unavailable. Archival must not delete immutable evidence, rewrite history, or imply
candidate delivery.

### US-28 — Configure application behavior and access help

**Story.** As an operator, I want settings, help, glossary, and diagnostics to identify their owner
and scope, so that display preferences cannot be confused with immutable run or contract values.

The operator must be able to inspect and configure permitted settings described in Section 10,
discover keyboard bindings, access exact terminology, and understand whether each value is:

- a local display preference;
- application configuration;
- project configuration;
- runtime-profile configuration;
- immutable contract or run configuration; or
- amendable only through new lineage.

Supported configuration and recovery must be accessible without requiring the operator to find
and edit an undocumented or hidden configuration file.

### US-29 — Continue safely under high volume and degraded conditions

**Story.** As an operator, I want current state, limiting conditions, focus, and available actions
to remain stable under large volumes and partial failures, so that the interface remains useful
without hiding contradictions.

The design must support hundreds of messages, dozens of obligations and candidates, and thousands
of events without unbounded visual growth, focus loss, control displacement, or an unreadable
event waterfall. Missing, stale, corrupt, partial, incompatible, unauthorized, rate-limited, and
infrastructure-error states must have explicit representations and recovery paths where available.

## 7. Stable vocabulary and distinctions

The following distinctions are normative. A design may propose concise display labels, but
inspection must preserve the exact concepts.

| Term | Required meaning |
|---|---|
| Project | Selected repository input and its captured immutable source snapshot. |
| Public `PROJECT.md` | Human-readable goal, scope, requirements, non-goals, and delivery format; not the whole acceptance package. |
| Contract package | Public specification plus environment, checks, protected oracle metadata, review, budget, evidence, observation, runtime policy, assurance profile, and approval record. |
| Run | One immutable contract lineage and finite resource envelope from start to one terminal outcome. |
| Runtime | External coding-agent harness such as Codex, Claude Code, OpenCode, or an approved ymp-native worker. |
| Model route | Provider, deployment, endpoint, protocol, account or quota scope, model, authentication mode, and disclosure class; not the runtime. |
| Runtime profile | Exact driver, harness and prompt policy, model route, coordination-tool binding, and assurance profile; provenance, not role or grade. |
| Participant | One resumable agent-runtime session with a principal identifier and locally owned budget account. |
| Attempt | Bounded execution of one participant against one task contract and immutable base in a private writable workspace. |
| Invocation | One supervised process slice that starts or resumes a participant within an attempt. |
| Origin participant | Initial participant and ignition point; not a permanent manager or leader. |
| Task scope | Bounded semantic and artifact scope referenced by offers, contracts, audiences, and obligations. |
| Sponsor / contractor | Temporary factual roles in one mutually formed local task contract; not global hierarchy. |
| Offer | Sponsor-funded invitation for scoped work under expiry and escrow. |
| Bid | Participant's expiring consent and resource counter-proposal; not proof of competence. |
| Task contract | Mutually formed local commitment with sponsor, contractor, scope, base, escrow, obligation, and lease policy. |
| Work obligation | Outstanding causal work that must return a typed terminal result; parentage forms a termination-accounting tree. |
| Lease / fencing token | Time-bounded right and monotonic generation preventing stale work from advancing current state. |
| Audience grant | Expiring read or publish access to a detailed scoped conversation; information access, not authority. |
| Collaboration message | Attributed, bounded, inert, untrusted data in one audience; cannot execute, grant, spend, verify, or form a contract. |
| Active projection | Temporarily salient view of immutable history; expiry removes salience, not the record. |
| Submission | Immutable patch or artifact bundle against one base snapshot. |
| Candidate | Immutable mechanically integrated result with exact digest and ancestry; several may coexist or pass. |
| Verification request | Participant-selected use of scarce protected-query authority on one exact candidate. |
| Verification evidence | Independent result bound to exact contract, candidate, environment, and oracle digests. |
| Human intervention | Attributed operator action that adds or changes run input and may change comparison eligibility. |
| Evidence export | Reproducible records and objects for inspection; does not apply a candidate or authorize delivery. |

Never merge participant, attempt, invocation, runtime, model route, and workspace into one generic
`agent` record. A compact overview may summarize them, but focused inspection must reveal the
identity and lifecycle layers.

## 8. Required information catalogue

This section defines discoverability, not default visibility. Progressive disclosure is allowed.

### INF-01 — Application and local ownership

- application version and build identity;
- platform and execution-assurance capability;
- data-root path and ownership;
- store and schema version;
- last durable sequence and journal integrity;
- current project and run;
- bounded log and retention policy; and
- startup failure and recovery state.

### INF-02 — Project and source snapshot

- display name and repository path;
- source digest and capture time;
- base revision;
- modified, staged, untracked, ignored, and generated-file policy;
- toolchain and environment identity;
- source warnings and unsupported conditions; and
- working-tree versus execution-workspace boundary.

### INF-03 — Contract and approval

- package identity, version, digest, lineage, and supersession;
- approval state, authority, and time;
- public intent and delivery contract;
- requirement/evidence coverage;
- environment and permitted dependencies;
- visible checks and protected-bundle metadata;
- oracle controls and reproducibility;
- review protocol;
- budget, disclosure, observation, runtime, route, and assurance policy; and
- ambiguity, dependency, effect, or coverage blockers.

### INF-04 — Run and study context

- run identity and contract lineage;
- source snapshot and timestamps;
- current lifecycle and exact terminal outcome;
- ordinary or study context, experimental condition, repetition or seed, and study specification;
- comparison eligibility and exact intervention/exclusion event;
- origin and approved runtime profiles;
- assurance profile and disposable-environment limitation;
- obligation, integration, verification, and command activity; and
- export, archive, and retention state.

### INF-05 — Runtime profiles and model routes

- discovery and path override;
- runtime, executable, driver, harness, prompt, and coordination binding identity;
- probe state and time;
- authentication readiness without credential disclosure;
- complete provider/route/model/account/disclosure identity;
- lifecycle, isolation, cancellation, telemetry, and hard-limit capabilities;
- required, optional-with-fallback, observational, and disqualifying capability gaps; and
- exact readiness, degradation, incompatibility, authentication, or availability reason.

### INF-06 — Participants, attempts, and invocations

- stable and parent identifiers;
- runtime profile and model route;
- task contract and obligation;
- local budget;
- base, workspace, capability, lease, and fence;
- invocation lifecycle, cursor, wake conditions, and deadline;
- measured resource and route usage;
- typed completion or failure; and
- safe session-continuity status.

### INF-07 — Offers, bids, contracts, and obligations

- scope, base, capability/path restrictions, dependencies, parties, audience, and causal references;
- offer policy, slots, escrow, proposal allowance, expiry, and projection lifetime;
- bid consent, counter-offer, artifact class, method reference, withdrawal, and result;
- mutual formation, transfer, lease, and child obligation;
- obligation tree, authority charge, dependencies, state, and return; and
- race, expiry, cancellation, duplication, stale return, and parent/child result distinctions.

### INF-08 — Collaboration

- audience, membership, rights, and expiry;
- publication order and immutable message metadata;
- message kind and bounded escaped payload;
- reply and domain references;
- publication, delivery, citation, and read-cursor observations;
- communication charges;
- active and expired salience; and
- summary versus detailed audience boundary.

### INF-09 — Budgets and authority

- every dimension listed in US-13;
- total, reserved, consumed, returned/settled, remaining, and rate;
- unavoidable overshoot;
- enforced/observed/estimated classification;
- telemetry timestamp and staleness; and
- action rejection caused by absent reservation or authority.

### INF-10 — Submissions, candidates, and integration

- complete submission provenance;
- integration queue and state;
- candidate digest, base, parents, contributions, provenance, time, and verification;
- parallel and synthesis candidates;
- typed integration failures; and
- plural accepted set where no tie-breaker exists.

### INF-11 — Verification and evidence

- requester, exact candidate, scope, reservation, time, and state;
- four exact digests;
- verifier, toolchain, regime, isolation, and disclosure;
- checks, controls, review, and completeness;
- independent/blinded review state;
- query budget; and
- clean-rerun reproducibility.

### INF-12 — Event and boundary history

- durable order, cursor, correlation, causation, object, subject, result, and failure;
- lifecycle and cleanup events;
- authority and accounting events;
- communication, integration, and verification events;
- provider, network, dependency, credential, and external-action events;
- integrity failures; and
- search, filter, follow, visual pause, and cursor recovery.

### INF-13 — Communication observatory

- signal, delivery, reply, citation, declared use, decision, artifact change, and result;
- independent initial position and revision;
- challenge, confirmation, disagreement, and dissent;
- complementary contribution and candidate ancestry;
- local topology and topology change;
- removal and adaptation;
- all controlled intervention conditions listed in US-17; and
- signaling, listening, and task-value outcomes separately.

## 9. Action and consequence requirements

### ACT-01 — Consequence preview

Every state-changing action must name, before confirmation:

- exact object and scope;
- initiating human authority;
- resource or authority charge;
- reversibility;
- runtime and provider capability limitation;
- experimental/comparison consequence;
- lineage consequence; and
- expected control-state or terminal effect.

One generic confirmation message is insufficient.

### ACT-02 — Distinct cancellation concepts

Closing a view, cancelling text input, stopping paste review, pausing visual follow mode,
interrupting an invocation, pausing admission, cancelling a verification request, and cancelling a
run must be distinct in wording, keys, and confirmation behavior.

### ACT-03 — No silent fallback

No action may silently fall back to another runtime, model route, provider, account, model,
assurance profile, contract, source snapshot, budget, disclosure class, or weaker capability.

### ACT-04 — Candidate application boundary

Applying a selected candidate to the user's working tree is outside the current terminal product
surface. Evidence export must never appear to perform application or delivery. If candidate
application enters scope in a separately approved change, it must identify one exact candidate,
preview conflicts, require separate external authority, and remain distinct from acceptance.

### ACT-05 — External authority boundary

Irreversible external effects and production credentials are outside the current authorized
execution profile. The interface must not expose dormant advanced controls for unsupported
external authority.

## 10. Settings and configuration

Settings may be distributed across application, project, contract, runtime profile, and run
contexts. The specification does not require one settings screen.

### CFG-01 — Application preferences

- data root and ownership status;
- default project location;
- retention for terminal runs, workspaces, bounded logs, session capsules, and unreferenced
  objects subject to evidence policy;
- local versus UTC time and precision;
- theme, contrast, color capability, Unicode/ASCII mode, motion reduction, and density;
- keyboard bindings, optional mouse, paste behavior, and permitted confirmation preferences;
- default follow, wrapping, truncation, and identifier display; and
- export destination and redaction defaults.

### CFG-02 — Runtime and route configuration

- discovered and overridden executable paths;
- approved runtime, driver, harness, prompt, and coordination-tool versions;
- generated versus ambient configuration status;
- start, resume, interrupt, cancellation, structured-event, session, and subagent capabilities;
- provider, deployment, endpoint, protocol, model, account/quota, authentication, and disclosure;
- usage/cost evidence, hard-limit availability, cancellation, and overshoot; and
- probe history, compatibility, and recovery actions.

### CFG-03 — Contract, execution, and study configuration

- package and approval lineage;
- assurance profile and prerequisites;
- approved runtime profiles and origin profile;
- complete budget vector, reservations, and settlement;
- concurrency, creation, lease, renewal, yield, wake, communication, and query limits;
- visible versus protected diagnostics;
- observation, retention, intervention, and export policy;
- experimental condition, seed/repetition, stopping rule, and comparability; and
- immutability, amendability through lineage, or display-preference classification for every
  value.

### CFG-04 — Explicitly absent settings

Do not expose settings for SQLite, daemon operation, HTTP, WebSocket, server/client attachment,
browser access, PostgreSQL, object-storage credentials, Kubernetes placement, leader election,
dynamic runtime plug-ins, central task assignment, semantic priority, grades, scalar task
complexity, automatic escalation, or hidden assurance fallback. These are absent product concepts,
not hidden advanced options.

## 11. State and failure matrix

Every material normal, empty, loading, stale, degraded, incompatible, unauthorized, exhausted,
cancelled, failed, and infrastructure-error condition requires a defined presentation and recovery
path where recovery exists.

### STATE-01 — Application and project

- first launch;
- no project;
- no historical runs;
- loading or probing with useful progress and safe cancellation;
- ready;
- data root owned by another foreground process;
- unsupported platform;
- unavailable assurance profile;
- filesystem access failure;
- storage full;
- schema incompatibility;
- journal corruption;
- event gap;
- digest mismatch;
- incomplete journal tail;
- repository unavailable or unsupported;
- modified repository under undeclared capture policy;
- source capture failure; and
- terminal too small.

### STATE-02 — Runtime and model route

- not installed;
- discovering;
- probing;
- ready;
- busy;
- stale probe;
- unauthenticated;
- authentication expired;
- account/quota unavailable;
- incompatible version;
- missing required capability;
- weaker declared capability;
- invalid generated configuration;
- start failure;
- resume failure;
- structured-event failure;
- session-capsule failure;
- interruption/cancellation failure;
- descendant-cleanup failure;
- route unavailable, rejected, rate-limited, or cancelled; and
- incompatible route events.

### STATE-03 — Contract and preflight

- no package;
- draft;
- awaiting approval;
- approved;
- superseded;
- invalid digest;
- source mismatch;
- unvalidated oracle;
- failed negative control;
- missing known-good control;
- flaky/non-reproducible check;
- coverage gap;
- inaccessible dependency;
- forbidden external effect;
- provider-disclosure conflict;
- disallowed profile;
- insufficient budget;
- incompatible assurance; and
- ready to authorize with an exact configuration summary.

### STATE-04 — Run

- configured;
- preflight;
- awaiting authorization;
- starting;
- running;
- admission paused;
- cancelling;
- finalizing;
- autonomous comparison eligible;
- scheduled intervention condition;
- unscheduled human-intervened;
- temporarily no visible activity with funded work;
- quiescent during terminal accounting; and
- terminal `accepted`, `exhausted`, `abstained`, `cancelled`, or `infrastructure_error`.

### STATE-05 — Local commitments and execution

- funded;
- advertised;
- bid received;
- awarded;
- contracted;
- active;
- submitted;
- returned;
- withdrawn;
- expired;
- declined;
- dead end;
- exhausted;
- cancelled;
- infrastructure error;
- open-accept race won or lost;
- lease current, near expiry, expired, or renewed with newer fence;
- stale result preserved but unable to advance state;
- participant requested, starting, active, yielded, eligible to wake, waiting for admission,
  returned, or failed;
- invocation running, explicit completion, yielded, interrupted, limit exceeded, runtime error,
  route error, or infrastructure error; and
- obligation with open, returned, cancelled, or terminal child subtree.

### STATE-06 — Collaboration

- no messages;
- active conversation;
- unread delivery;
- direct message;
- audience request;
- membership granted, denied, or expired;
- active and expired salience;
- unanswered challenge;
- revision after challenge;
- unresolved disagreement;
- independent first assessment not revealed;
- published but not delivered;
- delivered but not causally evaluated;
- causally tested under controlled study; and
- malformed, over-budget, unauthorized, cross-scope, protected-reference, or oversized message
  rejected without control-state mutation.

### STATE-07 — Candidate and verification

- no submission;
- submission recorded;
- integrating;
- integrated;
- integration failed;
- parallel candidates;
- multi-parent synthesis;
- stale base;
- stale fence;
- path violation;
- object mismatch;
- conflict;
- partial object;
- unverified;
- verification requested;
- verifying;
- passed;
- failed;
- verifier `infrastructure_error`;
- root-scope versus subtask-scope pass;
- multiple passing candidates without tie-breaker;
- failed negative control; and
- evidence-integrity loss.

### STATE-08 — Budgets and limits

- unused;
- reserved;
- consuming;
- near exhaustion;
- exhausted;
- settled;
- unavoidable measured overshoot;
- enforced;
- observational;
- estimated;
- unavailable;
- stale telemetry;
- one exhausted dimension while others remain; and
- action rejected for absent authority or reservation.

### STATE-09 — Export and archive

- export configuration;
- export in progress;
- complete export with digest;
- failed export;
- rejected partial output;
- redaction required;
- archived;
- restored to default views;
- retained by policy; and
- deletion unavailable.

## 12. Terminal implementation constraints

### TUI-01 — Fixed implementation medium

- Rust is the implementation language.
- Ratatui is the terminal UI framework.
- Crossterm is the terminal backend and input-event layer.
- Ratatui `TestBackend` is the deterministic rendering surface for screen-state tests.
- `tui-textarea` is an optional reviewed dependency for multiline input; the design must remain
  implementable with core Ratatui primitives if it is rejected.
- Crate versions are selected during implementation and must not be assumed by visual design.

### TUI-02 — Cell-grid reality

The render surface is a terminal-cell grid. Each cell carries a rendered symbol plus foreground
color, background color, and supported text modifiers; a wide grapheme may occupy more than one
cell. A conforming design must not depend on:

- variable font size or weight beyond terminal modifiers;
- CSS-style letter spacing, line height, opacity, blur, shadow, or gradients;
- sub-cell placement for text and controls;
- rounded geometry that cannot be represented by terminal symbols;
- pixel-perfect alignment;
- images or a browser rendering engine; or
- hover-only disclosure.

### TUI-03 — Widgets and custom rendering

The design may use concepts implementable with Ratatui `Layout`, `Block`, `Paragraph`, `Line`,
`Span`, `List`, `Table`, `Tabs`, `Gauge`, `LineGauge`, `Scrollbar`, `Sparkline`, `Chart`, `Canvas`,
or bounded custom `Widget`/`StatefulWidget` implementations that write to `Buffer`.

No separate graph framework is selected. Graph and timeline concepts must degrade to
terminal-native lists, trees, tables, box-drawing symbols, text, or bounded custom widgets. Core
meaning must not depend on Braille/Octant markers, force-directed layout, animation, or a font's
special glyph coverage.

### TUI-04 — Terminal sizes

- `120 × 40` cells is the normal design target.
- `80 × 24` cells is a constrained compatibility mode. It must preserve exact run state,
  required action or recovery, limiting condition, essential navigation, and safe exit. It is not
  required to show every diagnostic detail simultaneously.
- `180 × 50` cells or wider may expose dense research and observatory information but must not be
  required for essential operation.
- Below the supported minimum, the interface must show a stable minimal state, required
  dimensions, and safe resize/exit guidance rather than corrupt output.

### TUI-05 — Input and focus

- keyboard-first operation is required;
- optional mouse input must not be necessary;
- contextual help and bindings must be discoverable;
- no essential command may depend on a memorized single-letter binding without visible discovery;
- focus, selection, search scope, scroll position, unseen updates, and follow mode must be
  perceptible;
- paste into state-changing input must be explicit and reviewable;
- resize and focus events must preserve a coherent state; and
- action locations must remain stable under event volume.

### TUI-06 — Color and symbols

- 16- or 256-color operation is the baseline;
- true-color RGB may enhance but cannot carry required meaning;
- color must be redundant with text, shape, symbol, or placement;
- high-contrast and monochrome modes are required;
- red/green-only semantics are prohibited;
- Unicode line drawing may enhance the interface;
- ASCII fallback must remain meaningful; and
- emoji may decorate but cannot be the only command or status symbol.

### TUI-07 — Text handling

- paths, identifiers, digests, code, and multilingual text must wrap or truncate predictably;
- full values must remain inspectable and copyable;
- wide characters and zero-width behavior must not corrupt adjacent cells;
- untrusted text must be escaped before rendering; and
- raw control characters must never reach the terminal as executable sequences.

### TUI-08 — Deterministic testing

Material states must be reproducible through deterministic Ratatui `TestBackend` snapshots or
equivalent cell-buffer assertions. Tests must cover normal, empty, loading, stale, degraded,
failure, terminal, monochrome, ASCII, long-text, hostile-text, and supported-size variants.

## 13. Accessibility and usability requirements

### ACC-01 — Progressive disclosure

The interface must provide a useful overview and precise inspection without hiding contradictions
or requiring every field on one screen. Detailed protocol expertise must not be a prerequisite for
the first supported run.

### ACC-02 — Exact language

Use exact typed outcome and failure language. Generic `success`, `healthy`, `failed`, or color-only
labels are insufficient where the protocol distinguishes acceptance, candidate failure, runtime
failure, infrastructure failure, exhaustion, abstention, and cancellation.

### ACC-03 — Uncertainty and assurance

Missing data, stale telemetry, observational limits, estimated values, provisional capability,
incomplete evidence, and weak assurance must be visible rather than collapsed into one health
indicator.

### ACC-04 — Action clarity

The operator must be able to distinguish current state, available action, required action, blocked
action, and unavailable capability. Empty states must explain the next valid action without
inventing a central workflow for participants.

### ACC-05 — Terminology

Contextual help must define exact domain terms. Concise display labels may be used, but inspection
must reveal the normative entity and consequence.

## 14. Security, privacy, and trust presentation

### SEC-01 — Three planes

Authoritative control, untrusted collaboration, and independent verification must remain distinct
in both color and monochrome presentations. Human actions, enforced limits, observational values,
immutable objects, and failures must also have unambiguous treatment.

### SEC-02 — Secret and capability exclusion

The interface and evidence export must never expose:

- raw credentials;
- protected oracle bytes;
- bearer capability material;
- private session capsules;
- ambient user configuration excluded by the assurance profile; or
- private chain-of-thought.

### SEC-03 — Inert content

Messages, runtime output, logs, repository text, paths, links, and code are untrusted content. They
must not execute, issue terminal control, open a URL automatically, transfer authority, or become a
state-changing action without separate authenticated operator intent.

### SEC-04 — Assurance honesty

The interface must distinguish logical attempt isolation, workspace attribution, execution
assurance, provider controls, and hostile-code containment. It must not imply strict containment on
native macOS or any platform/profile without corresponding evidence. Strict Linux boundary
assurance may be shown only when the selected profile's prerequisites and recorded evidence
establish it.

## 15. Volume, performance, and stability

### VOL-01 — Representative load

The design must remain usable with at least:

- eight participants;
- dozens of obligations;
- dozens of candidates;
- hundreds of collaboration messages; and
- thousands of durable events.

These values are fixtures, not protocol limits.

### VOL-02 — Stable interaction

High event rates must not cause flicker, focus loss, selection reset, action displacement,
unbounded visual growth, or replacement of current state by a scrolling event stream.

### VOL-03 — Durable recovery

Notification loss must be recoverable from a durable cursor where authoritative history remains
intact. The interface must distinguish notification delay, stale projection, journal integrity
failure, and authoritative state loss.

### VOL-04 — Bounded text

All lists, logs, messages, identifiers, and diagnostics require bounded rendering, explicit
truncation/wrapping, and navigable overflow. No untrusted content may expand the interface without
limit.

## 16. Required representative fixtures

The following internally consistent scenarios must be available to validate design coverage. Do
not use lorem ipsum or contradictory placeholder state.

1. **First launch:** no runtime found, no project, and no historical run.
2. **Mixed readiness:** Codex ready, Claude Code installed but unauthenticated, and no fallback.
3. **Ready preflight:** approved contract, validated oracle, exact source, route, assurance, and
   finite budget ready for authorization.
4. **Single participant running:** one active attempt, current lease, enforced wall-time limit,
   observational tokens, and no candidate.
5. **Single participant accepted:** one immutable root candidate passed with exact four-digest
   evidence and export available.
6. **Candidate failure:** protected checks failed with bounded diagnostics and remaining query
   budget.
7. **Infrastructure failure:** verifier or journal integrity lost with no candidate labelled
   rejected or accepted.
8. **Budget exhaustion:** one required dimension reaches zero while other dimensions remain;
   terminal `exhausted` follows obligation accounting and quiescence.
9. **Local negotiation:** three participants, one negotiated offer, two bids, one award, one
   expired bid, and a child obligation without central assignment.
10. **Competing work:** two candidates from one base, one typed integration conflict, and a third
    participant-sponsored synthesis candidate.
11. **Useful challenge candidate:** one observation challenged, revised, cited by a decision, and
    incorporated into a candidate; live view shows provenance, not causal value.
12. **Controlled communication result:** repeated intervention evidence establishes signaling but
    not listening, or listening without task value, despite fluent messages.
13. **Human intervention:** an operator sends a scoped message and the run becomes visibly
    intervened and non-comparable for autonomous analysis.
14. **Stale action race:** selected offer or lease expires before confirmation; current state
    refreshes and no effect occurs.
15. **High volume:** the representative load from VOL-01 without loss of current outcome,
    limiting condition, focus, or actions.
16. **Constrained terminal:** the active run at `80 × 24` with essential state and recovery.
17. **Terminal too small:** stable required-size and safe-exit guidance.
18. **Multiple passing candidates:** two exact root candidates pass with no approved tie-breaker;
    accepted set remains plural.
19. **Contract amendment:** changed budget or contract creates new lineage and explicit comparison
    consequence.
20. **Export failure:** redaction required or partial output rejected without presenting partial
    evidence as complete.

## 17. Prohibited semantics

A conforming design must not imply:

- a global dispatcher, project manager, central work queue, or kernel-generated semantic plan;
- required skill level, per-skill grade, best participant, competence rank, cheapest-capable
  selection, or winner recommendation;
- scalar task complexity that grows after failure;
- permanent roles or leadership based on origin, sponsorship, verbosity, message volume, or graph
  centrality;
- automatic escalation, forced collaboration, or preferred communication topology;
- majority vote, consensus, confidence, or praise as acceptance;
- one shared writable repository or mutable canonical candidate;
- a global free-form board, message-carried authority, or action executed from message content;
- protected bytes, credentials, capability material, session capsules, or private reasoning in a
  view or export;
- runtime exit, participant return, integration, or subtask pass as root acceptance;
- automatic candidate selection when several candidates pass;
- event arrival order as proof of causation;
- fluent communication as proof of consciousness, understanding, group mind, collective
  intelligence, or leadership;
- workspace separation as a hostile-code sandbox;
- strict containment where the platform/profile does not establish it;
- hidden fallback to another runtime, route, provider, model, account, assurance profile,
  contract, source, disclosure, or budget;
- background service, browser administration, database service, cluster placement, or Kubernetes
  control in the current product surface; or
- implementation that depends on pixel positioning, hover, web animation, variable font size, or
  a browser canvas.

## 18. Conformance requirements

A complete interface design or implementation conforms to this specification only when:

- [ ] The full operator journey in Section 5 is coherent and recoverable.
- [ ] Every user story has a normal path, material failure variants, consequences, and recovery
      where available.
- [ ] Every required information item is discoverable without forcing all details into the default
      view.
- [ ] Participant, runtime, model route, attempt, invocation, workspace, obligation, submission,
      candidate, and evidence remain distinct.
- [ ] Local negotiation and self-organization are visible without ranking, assignment, or inferred
      leadership.
- [ ] Control, collaboration, verification, human action, assurance, and limit enforcement remain
      distinguishable without color.
- [ ] Live provenance is not presented as causal evidence.
- [ ] `accepted`, candidate `failed`, verifier `infrastructure_error`, `exhausted`, `abstained`, and
      `cancelled` cannot be confused.
- [ ] Every budget value identifies its dimension and enforcement classification.
- [ ] Human message, pause, interruption, cancellation, amendment, archive, export, and candidate
      application boundary have distinct consequences.
- [ ] The disposable-environment assurance limitation is explicit without becoming a false
      product-security claim.
- [ ] Raw secrets, protected oracle content, capability material, session capsules, and private
      reasoning never appear.
- [ ] The interface is feasible in Ratatui's terminal-cell model at the required sizes.
- [ ] Keyboard, focus, search, paste, resize, long text, wide characters, color limitations, and
      hostile terminal content are covered.
- [ ] High volume preserves current state, focus, limiting conditions, and actions.
- [ ] Material states are testable deterministically with `TestBackend`.
- [ ] Open assumptions and document conflicts are recorded rather than silently settled through
      visual design.

The specification intentionally leaves composition to the designer. Conformance is determined by
semantic accuracy, operator capability, consequence clarity, complete state coverage, terminal
feasibility, and traceability to the governing project documents—not by a predetermined layout.
