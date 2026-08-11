# Roadmap

The roadmap has three product milestones: **POC → MVP → Alpha**. It is an evidence sequence. A
foreground ratatui application is the only current product interface from the first executable
checkpoint onward; interface polish grows only with demonstrated needs. Internal checkpoints
within a milestone isolate hard questions so that a failure has an interpretable cause.

Current POC execution state lives in the permanent work tree under
[work/](work/); [work/WAVES.md](work/WAVES.md) is the generated overview and
[work/STATUS.md](work/STATUS.md) is the generated unit index. This roadmap defines product sequence
and exit criteria; it is not edited as an execution journal.

## Initial scope

The first claim covers reproducible software repair and bounded code-change tasks. Each task has a
manually reviewed public specification, a prevalidated black-box oracle, pinned toolchain, no
production credential, and no irreversible external action. The evaluation includes both
decomposable and strongly sequential tasks. During POC, the whole experiment is placed in an
externally disposable environment; Linux containment implemented by ymp becomes an MVP property.

Automated contract generation, taste-based work, open-ended research, production deployment,
distributed execution, scalar reputation, dynamic complexity, and arbitrary-project support are
outside this proof.

## POC — proof of concept

The POC must test the distinctive mechanism, not merely prove that ymp can launch one agent. It is
a foreground TUI distributed as one `ymp` executable. It keeps live state in process, writes a
bounded event journal and atomic run metadata, uses a content-addressed object directory, and
supports Codex and Claude Code through separate runtime drivers. It ends with a controlled
comparison of local self-organization against both required matched-budget baselines.

The POC deliberately makes **no product-security or host-containment claim**. It runs only in a
researcher-provided disposable environment containing curated non-sensitive repositories, no real
credentials, no production access, and no irreversible external actions. Model-provider access
uses an experiment-specific account or quota. Security comes from discarding and externally
containing the environment, not from trusting POC isolation code. Arbitrary repositories and
ordinary developer machines are out of scope. The protected oracle stays outside the agent
environment and is invoked by the study harness against exported candidate digests; otherwise the
reliability result would be invalid even though product security is deferred.

### POC-0 — falsification package before product code

#### Build

- A curated task corpus with immutable source snapshots and manually approved contract packages.
- Requirement-to-evidence matrices, known-good candidates where available, invalid mutations,
  negative controls, clean-room reproduction, and oracle attack cases.
- A preregistered comparison among one strong agent, independent best-of-`n`, and locally
  coordinated participants under the same model-route cost ceiling and resource vector.
- Task labels for decomposability and sequential dependence that are assigned without looking at
  system outcomes.
- Primary outcomes, minimum practically useful effect, randomization unit, stochastic replication,
  exclusion rules, stopping rule, and blinded human audit sample.
- A communication-study plan covering message absence, neutral replacement, cross-task shuffling,
  direct delivery of the same raw evidence, delayed messages, false findings, and participant loss.
- A protocol simulator or executable state-machine model for duplicate delivery, process crash,
  lease expiry, stale writers, cancellation, budget transfer, and obligation termination.
- A runtime compatibility matrix for pinned Claude Code, Codex, and OpenCode versions. Runtime
  columns cover start, resume, structured events, configuration isolation, tool bindings,
  interruption, session persistence, descendant cleanup, and disabling native subagents,
  background sessions, remote execution, ambient plugins, and unrelated tool servers. Model-route
  columns separately cover provider and model pinning, wire protocol, brokered authentication,
  usage evidence, hard cost, request cancellation, and approved disclosure.
- An optional route-feasibility probe for NVIDIA Nemotron. Use OpenCode for the hosted NVIDIA API
  or Claude Code/Anthropic Messages for a pinned self-hosted NIM, according to the approved endpoint;
  treat Codex/OpenAI Responses as a separate pairing. The probe does not add another runtime to the
  POC exit criteria unless that profile is selected for the experiment.
- A fake deterministic runtime and MCP client for crash, duplicate-reply, malformed-message,
  yield, wake, and replay tests without model cost.

#### Exit criteria

- Every accepted test task has a discriminating oracle that rejects its invalid mutations.
- Budget conservation, stale-token rejection, idempotency, and eventual termination hold across
  generated fault schedules within the declared model.
- A stdio MCP bridge can be launched from the same `ymp` executable, and malformed or repeated tool
  calls cannot corrupt the experimental state machine or duplicate a recorded effect.
- Every POC runtime profile admitted to the primary comparison supplies a reproducible usage
  measure and per-invocation stop mechanism. Any unavoidable in-flight overshoot has a
  preregistered tolerance and is charged to the arm that incurred it; otherwise the matched-budget
  comparison cannot begin.
- The study can be run without changing its primary outcome after seeing results.

If these criteria fail, implementation pauses. A weak oracle or inconsistent experiment runtime
would make later coordination results uninterpretable.

### POC-1 — foreground TUI experimental attempt

#### Build

- One Rust executable whose default mode contains the ratatui interface, trusted application core,
  single kernel writer, and runtime supervisor. The same file supplies the private
  `ymp internal agent-mcp` and verifier process modes. There is no per-user daemon, operator socket,
  or public headless mode.
- In-memory live state with one writer, a bounded append-only JSONL control-event journal, atomic
  run metadata, and a local content-addressed object store. Journal records have monotonic sequence
  numbers and a predecessor-digest chain; gaps, corruption, or an incomplete tail end the run as
  `infrastructure_error`. Transparent recovery, migrations, backup, and restore are deferred.
- Contract and oracle-metadata store with exact digests and approval records; protected oracle
  bytes remain in the outer study harness.
- One private workspace per attempt, content-addressed submissions, mechanical integration, and
  immutable candidates. Directory separation prevents accidental overwrite but is not presented as
  hostile-code containment.
- Codex and Claude Code runtime drivers with capability probing, generated isolated configuration,
  structured event parsing, start/resume/interrupt, and explicit yield and wake. Model-route cost
  and resource use may be measured rather than treated as a containment boundary, but the
  experimental stopping rule and overshoot accounting remain mandatory.
- A per-invocation stdio MCP bridge that exposes only authorized ymp tools and never opens the
  authoritative state, object store, or oracle state directly.
- A verifier mode invoked by the outer study harness, outside the agent environment, against a clean
  candidate copy, with protected-query accounting, negative controls, bounded diagnostics, and
  honest `infrastructure_error` handling. The isolation is supplied by the experiment setup, not by
  a claimed POC security boundary.
- A minimal ratatui interface for runtime readiness, start, participant and obligation state,
  task-scoped conversation, candidate and verifier events, budgets, cancellation, and evidence
  export.

#### Exit criteria

- Installation consists of one `ymp` executable plus external state files. `ymp` makes no call to
  Paseo, a database service, a container runtime, an ymp-owned Python or Node.js helper, or another
  orchestrator. External agent harnesses and the disposable outer study harness may have their own
  prerequisites.
- The TUI detects installed and authenticated Codex and Claude Code profiles and launches their
  managed child processes itself; the operator never starts an agent runtime in another terminal.
- A single participant can produce and verify an exact candidate from its assigned disposable
  workspace.
- Replayed commands have exactly-once effects within a live controller authority interval; an
  expired attempt cannot create a current candidate or close an obligation.
- Model-route cost, wall time, CPU, memory, process count, disk, output, and verifier queries are either
  enforced or explicitly labelled observational.
- Re-running the verifier from clean inputs reproduces the recorded evidence within declared
  nondeterminism bounds.
- Lost MCP replies and yielded-session wakes preserve experimental command identity well enough to
  avoid counting one effect twice.

This checkpoint validates the oracle and experimental runtime. It does not validate containment and
does not yet test self-organization.

### POC-2 — smallest self-organizing system

#### Build

- Sponsor-funded task offers, bids, mutual awards, open acceptance, escrow, leases, fencing, and
  causal work obligations.
- Multiple resumable participants that may work alone, recruit, compete, delegate, challenge, or
  abstain using only locally owned resources.
- Event cursors, bounded wake conditions, coalescing, and finite invocation-start budgets so
  participants can wait without polling or creating an immortal dormant lease.
- A task-scoped collaboration board separated from the control ledger and verifier plane. Messages
  are bounded inert data; project-wide discovery is summary-only.
- Immutable candidate ancestry and explicit participant-sponsored synthesis for merge conflicts.
- Per-principal non-semantic admission, publication limits, expiring salience, finite creation
  authority, and quiescence detection.
- A read-only communication observatory that exposes task conversation, message and artifact
  provenance, independent initial positions, revisions, challenges, organization changes, and
  boundary actions.

#### Exit criteria

- No component assigns a semantic role, task priority, executor grade, decomposition, escalation,
  or winning candidate.
- Race tests show that concurrent awards, expiry, retries, and stale submissions cannot overspend,
  duplicate a shared effect, or overwrite another participant's candidate.
- A board payload cannot transfer a capability, form a task contract, invoke a tool, fetch a URL,
  enter protected verifier input, or keep a run alive without a funded control object.
- Runs terminate as `accepted`, `exhausted`, `abstained`, `cancelled`, or
  `infrastructure_error`; no-solution runs do not loop indefinitely.
- The observatory reconstructs declared provenance without claiming that temporal correlation is
  causation or exposing private chain-of-thought.

This is the smallest implementation containing the actual self-organization bet. The existing TUI
must expose the communication observatory without interpreting fluent conversation as proof of
collective reasoning.

### POC-3 — controlled evaluation

#### Instrumental reliability experiment

Randomly assign comparable tasks and seeds to:

1. one strong agent with the full budget;
2. independent attempts with no inter-attempt communication, plus a blinded selector that sees
   candidate artifacts and public evidence but no producer rationales, all within the same total
   and protected-query budgets; and
3. locally negotiated ymp coordination with the same total model-route cost and resource limits.

Report accepted-result rate, false acceptance from blinded human audit, cost, wall time,
verification-query use, infrastructure failure, and abstention with uncertainty by task stratum.
The coordinated condition must beat independent selection, not merely a single weak executor, by
the preregistered minimum useful effect. If it wins only on decomposable tasks, the product claim is
restricted accordingly.

#### Collective-reasoning experiment

For sampled communication episodes and repeated fresh runs, compare:

- the original message;
- no message;
- a neutral payload of comparable size;
- a message shuffled from another task or sender; and
- the same underlying evidence delivered directly without another participant.

Measure positive signaling separately from positive listening; accepted-outcome or calibrated
abstention change; marginal value beyond duplicated context; non-redundant contributions to the
accepted artifact; survival of correct independent dissent; communication cost; and recovery after
false information, delay, or participant removal. Role or topology change counts only when it was
not prompted and predicts useful adaptation under intervention.

No metric affects the run being measured. Influence, agreement, verbosity, eloquence, and
self-reported confidence are not rewards. Information-theoretic synergy is exploratory until it
adds predictive value beyond direct ablations.

#### POC decision point

- If coordination improves accepted results beyond both baselines at acceptable cost, proceed with
  the MVP for the supported task strata only.
- If coordination does not beat independent selection, retain the board and observatory only as
  diagnostic features and reject the reliability mechanism claim.
- If messages look intelligent but interventions show no listening or task value, report
  conversational appearance only and reject the collective-reasoning claim.
- If the oracle loses integrity, reject the result even if coordination appears useful.

The POC is complete only when this decision can be made from preregistered evidence. Completing the
runtime checkpoints without the controlled comparison is not a successful POC.

A positive POC authorizes work on MVP security and productization. It does not authorize use on
untrusted repositories, import of real credentials, or a claim that the POC can contain a malicious
agent.

## MVP — usable local product

MVP work begins only after a positive POC decision for at least one declared task stratum. It turns
the proven foreground TUI experiment into a dependable local product without broadening the claim.

### Build

- Harden the foreground TUI controller while retaining one trusted in-process kernel writer. Give
  attempts, tool bridges, runtime workers, model relays, and verifiers distinct process endpoints
  and authority; do not introduce a background daemon or external operator transport.
- Implement and attack-test the strict Linux profile: private namespaces and filesystems, delegated
  cgroups, seccomp, Landlock or an equivalent policy, no direct attempt network, no ambient host
  credentials or sockets, and complete descendant cleanup.
- Add the attempt-scoped model-provider relay with pinned route, destination, protocol, model,
  request, rate, and cost accounting; keep long-lived credentials in the trusted controller and
  relay boundary. Runtime profiles that cannot use this arrangement remain best-effort.
- Move protected verification into a stricter isolated process with exact-digest inputs, no board
  access, no agent credentials, negative controls, and authenticated evidence.
- Expand the existing ratatui interface around the same typed in-process application port, showing
  obligation, local-contract, participant, invocation, and immutable-candidate graphs.
- Add task-scoped untrusted conversation, unresolved challenges, independent first assessments,
  revisions, and message-to-decision-to-artifact-to-verification provenance.
- Implement budget reservations and consumption by dimension, runtime profile, model route, and
  isolation-boundary events, explicit pause, cancellation, contract approval or amendment, and
  evidence export.
- Retain conforming Codex and Claude Code implementations and add OpenCode only after it passes the
  same runtime and configuration-isolation suite. Runtime-native capabilities remain behind drivers
  and unsupported strict features remain visibly unavailable. The target Nemotron path is OpenCode
  against the hosted NVIDIA API or Claude Code/Messages against a pinned self-hosted NIM, according
  to the approved endpoint; Codex/Responses remains an independent conformance profile.
- Keep a native direct-model driver outside MVP unless the external runtimes cannot satisfy the
  approved Nemotron route, brokered authentication, or cancellation requirements. Its interfaces
  and fake-runtime conformance tests remain in scope so adding it later does not change domain
  commands.
- Dependable foreground startup and shutdown, bounded logs, workspace cleanup, and object garbage
  collection. Add crash continuation, backup and restore, and embedded SQLite migrations only if
  the MVP storage decision adopts SQLite; otherwise document crash termination and file-store
  recovery explicitly.
- Red-team repository content, board messages, MCP and provider-native tool calls, dependencies,
  candidate code, verifier interfaces, lease races, broker destinations, and attempted authority
  laundering. Record residual covert channels rather than claiming their elimination.
- One-executable packaging for supported Linux targets and an explicitly labelled native macOS
  best-effort build. No bundled virtual machine or external orchestration service is introduced.

### Exit criteria

- A new user with an installed supported agent runtime can use the TUI to start, list, inspect,
  follow, send an attributed intervention, pause, resume, cancel, archive, and export a run from the
  single `ymp` executable without editing hidden runtime or model-provider configuration.
- Every supported runtime driver passes the same fake-runtime and domain conformance suite; its
  capability differences are recorded rather than normalized by terminal scraping.
- Strict Linux preflight rejects a deliberately missing namespace, cgroup, filesystem, network, or
  descendant-cleanup control and never silently falls back to best-effort.
- Killing the TUI controller, tool bridge, runtime worker, external harness, verifier, or
  network relay at injected commit points either recovers the exact authoritative state or ends the
  run as `infrastructure_error`.
- The interface exposes local choices and evidence without assigning roles, ranking bids, choosing
  candidates, or rewarding conversation.

## Alpha — limited external trial

Alpha distributes the MVP to a small set of consenting users and held-out repositories inside the
already supported task class. It is a compatibility, safety, and usability trial, not permission
to revive the arbitrary-project claim.

### Build and evaluate

- Run a blinded held-out product trial. Evaluate interface usefulness separately from coordination
  effectiveness so presentation cannot mask a negative mechanism result.
- Pin and continuously test a declared compatibility window for every supported runtime profile,
  whether Claude Code, Codex, or OpenCode, including effective configuration, tool-binding
  revision, session resume, structured events, cancellation, and cost evidence. Test every
  supported Nemotron model route against the exact hosted NVIDIA or NIM version and wire protocol.
- Repeat oracle mutation tests, controller and storage fault injection, adversarial board and tool-call
  inputs, model-relay abuse, cross-attempt access tests, and verifier attacks on release builds.
- Exercise upgrades, downgrades where supported, evidence export, cleanup, and the documented
  crash-termination or recovery path on real Linux hosts. Test backup restoration if the selected
  MVP store supports continuation. Publish all weaker native macOS guarantees.
- Validate that communication-observatory views remain causally modest in user studies: fluent
  traces are not labelled reasoning without the preregistered interventions.

### Exit criteria

- Held-out accepted-result, false-acceptance, cost, latency, infrastructure-failure, and abstention
  results remain within the POC decision bounds for the supported stratum.
- No critical containment, state-integrity, cross-attempt, or verifier-boundary defect remains open.
- Release artifacts still consist of one `ymp` executable per target platform; state compatibility
  and recovery are documented and tested.
- Unsupported repositories, runtime profiles, model routes, platforms, and authority requests fail
  explicitly rather than falling through to a weaker unlabelled path.

Another project class, broader board visibility, agent-review criterion, external action, or
platform profile is added only with a new threat analysis, validated contract pattern, and
held-out evaluation. Cross-project outcome evidence retains full context; no scalar reputation or
kernel allocation policy is planned.

## After Alpha — optional client/server and web experiment

An external application transport is not an Alpha acceptance criterion. If local use demonstrates
a need for detached execution, multiple operator interfaces, or remote observation, a later
experiment may add `ymp server`, let the ratatui application connect as a client, and optionally
serve a web interface. All three modes must use the same application commands, events, authorization
rules, and store interface as embedded TUI mode. Embedded mode remains available and requires no
background process.

This experiment adds a new threat model for network authentication, browser origins, request
forgery, session fixation, multi-client races, remote filesystem authority, and denial of service.
Merely serializing the in-process application port is not sufficient evidence. The mode is rejected
if it duplicates kernel semantics in a gateway, weakens subject binding, or makes a service a
prerequisite for local TUI use.

## After Alpha — optional cluster experiment

Kubernetes support is not an Alpha acceptance criterion. If local demand and evidence justify it,
the next experiment runs the same executable in controller, runtime-worker, tool-bridge, and
verifier modes; uses PostgreSQL and object storage; and begins with one elected active controller
plus multiple worker replicas. It must pass the existing store and execution conformance suite
together with new partition, stale-controller, duplicate-job, pod-loss, workload-identity, and
network-policy tests.

The experiment is rejected if it requires semantic central scheduling, SQLite on shared storage,
Paseo, or MCP as the inter-controller protocol. Cluster success changes placement and availability,
not the self-organization constitution.

## Work that was secretly hard in the original first cut

The plan combines at least eleven independent research and engineering problems:

1. deriving a valid acceptance oracle from an unfamiliar project;
2. confining arbitrary external agent harnesses and their descendants without inherited
   credentials;
3. enforcing model-route cost and cancellation rather than observing usage after the fact;
4. making resumable local negotiation survive duplication, expiry, and process failure;
5. composing concurrent patches without a shared mutable working tree;
6. protecting adaptive evaluation from oracle extraction;
7. distinguishing causally useful communication from persuasive transcripts;
8. normalizing incompatible runtime session, configuration, event, tool-binding, authentication,
   and cancellation interfaces without scraping terminal screens;
9. proving that a compatible NVIDIA, OpenAI, or Anthropic endpoint preserves the exact streaming,
   tool, usage, identity, and error semantics required by its runtime;
10. implementing a complete coding-agent harness, context policy, and project-tool loop if direct
    model access is added; and
11. recovering or honestly terminating foreground runs across controller and store failures while
   preserving optional client/server and cluster boundaries.

The TUI does not by itself prove any of them. The milestone plan makes these difficulties explicit
and permits an early negative result.

## Definition of done for the initial product claim

ymp is sound enough for an initial supported release only when all of the following hold:

- the exact supported task class and oracle regime are published;
- the coordinated condition exceeds both preregistered matched-budget baselines by the minimum
  useful effect, with uncertainty and negative results reported by task stratum;
- false acceptance, infrastructure failure, cost, latency, and abstention stay within predeclared
  bounds;
- no known test bypasses subject-bound authority, private workspaces, candidate immutability,
  protected verifier separation, or budget conservation;
- no-solution and failure runs terminate honestly;
- any claim about collective reasoning is supported by repeated causal message interventions, not
  transcript inspection; and
- release packaging contains one `ymp` executable and does not conceal a dependency on Paseo or
  another orchestrator.

This definition supports an initial bounded release. It never justifies “accepted results on
arbitrary projects.”
