# Decisions

This record replaces the intuition-led first draft. It separates constitutional mechanisms from
semantic work policy, states what evidence supports each choice, and records where transfer from a
different research domain is only a hypothesis. Recorded 2026-08-10.

## Product and research objectives

ymp has two separate falsifiable objectives:

1. improve independently accepted results over strong single-agent and independent-search
   baselines under the same total budget for a declared task class; and
2. make any unprescribed collective reasoning behaviour observable and causally testable without
   calling fluent dialogue consciousness, understanding, or a group mind.

Failure of the second objective does not invalidate an accepted artifact. An interesting transcript
does not rescue a rejected artifact. Neither objective permits the kernel to prescribe a semantic
plan.

## Settled design choices

### Constitution, not choreography

The kernel is deterministic only for accounting, races, containment, provenance, and exact
verification. Participants and local sponsors decide task meaning, decomposition, role formation,
recruitment, bid selection, method, synthesis, and when to stop. There is no global semantic
scheduler, skill gate, escalation tree, candidate ranker, or answer synthesizer.

This distinction is essential. Removing deterministic control from leases and budgets would make
the system unsafe; adding deterministic control over the work strategy would make it the solver the
project explicitly rejects.

### Local commitment as the coordination primitive

Work is propagated by sponsor-funded offers, participant bids, mutual local contracts, escrow, and
tracked return obligations. Sponsor and contractor are temporary task relationships. A configured
origin participant supplies the initial condition but is not a permanent manager. The kernel does
not score bids or choose the organization.

### Three logically separate planes

Authoritative control state, untrusted task-scoped collaboration, and protected verification and
security state have distinct schemas, capabilities, readers, and writers. They may share a local
store in the first implementation but not authority. A board message is inert data: it cannot execute,
fetch, grant, consent, spend, or verify. Detailed findings are task-scoped; project discovery uses
bounded summaries.

### Private workspaces and immutable candidates

There is no shared writable project tree. Each concurrent attempt receives a private workspace from
an immutable base. Submissions are content-addressed bundles; mechanical integration creates a new
immutable candidate or a typed conflict. Participants decide how to resolve semantic conflicts.

### A contract package, not prose alone

Public `PROJECT.md`, environment, visible checks, protected oracle, review protocol, budget,
evidence policy, observation policy, and approval record are separately hashed artifacts. Protected
cases may instantiate public requirements but cannot add secret requirements. Acceptance is always
relative to one exact package, candidate, and environment.

### Four constitutional constraints, plus derived invariants

The human-approved goal, finite resource and authority envelope, attempt isolation, and independent
verification remain the four product principles. They are not claimed to be a complete protocol.
Authentication, idempotency, fencing, obligation accounting, immutable bases, capability mediation,
and terminal states are necessary derived invariants. They constrain effects, not solution methods.

POC preserves logical workspace, control-state, and oracle separation through its disposable outer
study harness, but does not claim that ymp itself contains hostile code. Product-grade Linux
containment is deliberately deferred to MVP.

### Outcome evidence, not grade

The per-skill scalar grade and claim gate are removed. They would reintroduce central allocation
through a sparse, confounded, non-stationary estimate. Raw contextual outcomes and uncertainty may
inform local decisions; the kernel never returns `eligible` or denies a task on that basis.

### Behavioural evidence, not conversational theatre

ymp observes positive signaling, positive listening, task value, complementary contribution,
evidence-driven revision, warranted dissent, adaptive organization, efficiency, and resilience.
Message removal, neutral replacement, cross-task shuffling, and direct delivery of the same raw
evidence distinguish correlation from causal use. Influence, agreement, eloquence, message count,
and self-reported confidence are never rewards or admission signals. Private chain-of-thought is not
collected.

Task sponsorship, high message volume, central graph position, or the configured origin participant
is not labelled leadership. A leadership-like pattern is reported only as a behavioural finding if
the role was not prompted, changes other participants' actions under intervention, improves an
accepted outcome or calibrated abstention, and transfers or re-forms after removal. It never grants
kernel authority.

### TUI-first local application with a testable core

The current product is a foreground ratatui application. The TUI and trusted application core are
constructed in one process, and no public headless client, local daemon, or web service precedes
them. Deterministic tests and controlled experiments may invoke the same core through an in-process
harness, but that harness is not a second product interface. Observing agent conversation,
commitments, revisions, and failures in the TUI is part of the product hypothesis rather than
deferred presentation work. Strict execution initially targets supported Linux hosts; native macOS
is best-effort. External agent harnesses remain the first execution model; the architecture does
not assume that every future runtime is a CLI.

POC intentionally does not attempt the strict profile. The complete experiment runs in an outer
disposable environment with curated non-sensitive inputs and no real credentials or consequential
external effects. This isolates the scientific hypothesis from the separate containment program.
Strict Linux isolation, credential brokering, adversarial boundary testing, and hardened verifier
separation are MVP admission criteria, not POC claims.

### One executable, multiple protected processes

ymp is delivered as one Rust executable. By default that file starts the foreground ratatui
application containing the trusted kernel, run state, and runtime supervisor. The same file also
supplies private child modes for the MCP tool bridge, native runtime worker, isolation initializer,
model relay, and verifier worker. Content-addressed objects, event journals, atomic run metadata,
logs, and workspaces are external data. Separate processes and operating-system boundaries remain
mandatory for untrusted runtimes and independent verification: “one binary” is a packaging
invariant, not permission to share an address space with them.

The initial package has no runtime dependency on a database service, container runtime, virtual
machine, ymp-owned Python or Node.js helper, or another orchestrator. Claude Code and Codex, and
later OpenCode, are external harnesses that ymp may wrap, and each may bring documented
prerequisites. Strict mode is unavailable when required Linux kernel facilities are absent; the
product does not silently add a companion virtual-machine image.

Runtime drivers are compiled into the executable. The initial design has no dynamic runtime-driver
plugin ABI or user-supplied controller code; a new lifecycle parser or authority mapping ships as a
new tested ymp build.

### Agent runtime, model route, and tool binding are separate

An agent runtime owns the harness lifecycle; a model route names the provider, deployment,
protocol, account or quota scope, model, authentication, and disclosure class; a coordination-tool
projection maps ymp commands to stdio MCP or provider-native typed calls. A versioned runtime
profile composes them with a harness and prompt policy and an execution-assurance profile.

This separation is required for OpenCode, which can run the same harness against many providers,
and for NVIDIA Nemotron, which can be reached through OpenCode, an Anthropic Messages-compatible
Claude Code route, an OpenAI Responses-compatible Codex route, or a future native runtime. The
kernel checks only approved capabilities, disclosure, and reserved resources. It does not select a
profile by task meaning, rank models, or turn this compatibility layer into central dispatch.

A direct model API is not executed in the trusted application-core writer. A future
`ymp internal runtime-worker` implements the agent and project-tool loop in a restricted process
and calls the credential broker.
This preserves the one-executable invariant without introducing a Python or Node.js SDK helper.

### Foreground application core and replaceable storage

The ratatui application owns the only trusted kernel writer. TUI actions call a typed application
interface in process; there is no operator socket, daemon lifecycle, or client handshake in POC,
MVP, or Alpha. Attempts receive narrower private endpoints and never open authoritative state or
the object directory. The POC keeps live state in memory and writes a bounded append-only event
journal, atomic run metadata, and content-addressed objects. A controller crash ends that POC run as
`infrastructure_error`; transparent crash continuation is not part of the experiment.

Storage remains behind a typed port. MVP may retain the file-backed implementation or adopt
embedded SQLite if measured recovery, resume, or query requirements justify it. The choice is not a
constitutional invariant and must not force a background service. Durable object writes precede
control references in either implementation.

This adopts the useful lifecycle shape demonstrated by tools such as Paseo—stable agent and
workspace identifiers, explicit create, send, interrupt, inspect, and archive operations, runtime
capability discovery, and asynchronous completion notification—without adopting Paseo itself.
ymp does not link, execute, configure, query, or share state with Paseo, and compatibility with its
protocol is not a goal.

### MCP at the agent edge, not inside the kernel

The same executable provides an invocation-specific stdio MCP server because Claude Code, Codex,
and OpenCode can consume that interface. The bridge projects an authorized subset of ymp commands
and derives the acting principal from a private endpoint. It has no database or operator
capability, opens no HTTP listener, and treats MCP request identifiers as transport identifiers
rather than durable idempotency keys.

Authoritative commands, events, budgets, leases, obligations, candidates, and session lifecycle use
the versioned ymp domain protocol. Runtime-native structured streams remain behind drivers. A
direct-model runtime exposes the same command schemas as provider-native typed tools rather than
emulating MCP. This decision avoids forcing richer session semantics into MCP and prevents an agent
integration standard from becoming the replication, transaction, or future cluster protocol.

### Local application ports for later external modes

Cluster execution is not part of POC, MVP, or Alpha. The local design nevertheless
keeps transactional state, immutable objects, invocation execution, and command/event transport
behind narrow interfaces. Stable identifiers, idempotency, fencing, event cursors, and object
digests do not depend on PIDs, paths, file offsets, or database row numbers.

The current TUI uses an in-process command and event adapter. After Alpha, a separate product
decision may add `ymp server`, a remotely connected ratatui client, or a web gateway by adapting
the same application port. No network listener, authentication scheme, browser security model, or
multi-client consistency claim is implied by preserving this seam.

A later Kubernetes profile may run the same executable as controller, runtime worker, tool bridge, and
verifier processes, with PostgreSQL and object storage. Its first control model uses one elected
active controller and multiple worker replicas. SQLite on shared storage and MCP between cluster
components are explicitly rejected.

## Research basis and applicability

### Distributed self-organization

| Primary source | Result used | Design consequence | Transfer limit |
|---|---|---|---|
| [Dijkstra, “Self-stabilizing Systems in Spite of Distributed Control”](https://www.cs.utexas.edu/~EWD/transcriptions/EWD04xx/EWD426.html) | Local rules can recover a legitimate state under explicit scheduling assumptions. | Define legitimate control states and invariant-preserving transitions. | It proves neither repository correctness nor convergence to a solution. |
| [Dijkstra and Scholten, “Termination Detection for Diffusing Computations”](https://www.cs.utexas.edu/~EWD/transcriptions/EWD06xx/EWD687a.html) | Delegated work can be paired with return accounting to detect termination. | Every child task creates a causal obligation that must return. | The obligation tree records activity, not semantic dependencies or success. |
| [Smith, “The Contract Net Protocol”](https://reidgsmith.com/The_Contract_Net_Protocol_Dec-1980.pdf) | Local announcement, bidding, and award permit distributed task sharing with temporary manager and contractor roles. | Use mutual local task contracts and sponsor-owned escrow. | Bidding has overhead and can explode combinatorially; it is an experimental option, not a universal optimum. |
| [Hayes-Roth, “A Blackboard Architecture for Control”](https://www.sciencedirect.com/science/article/pii/0004370285900633) | A shared blackboard does not remove the need to decide which action occurs next. | Keep semantic action choice in participants and local sponsors. | The original architecture includes explicit control machinery; ymp does not copy its scheduler. |
| [Gray and Cheriton, “Leases”](https://doi.org/10.1145/74850.74870) | Time-bounded ownership improves availability under failures. | Use expiry plus monotonically increasing fencing tokens. | Clock, storage, and stale-writer assumptions must be tested; a timeout can duplicate work. |

These sources justify a reliable substrate for self-organization, not the proposition that an
uncontrolled semantic search converges. Finite budgets guarantee stopping, not progress.

### Multi-agent selection and evaluation

| Primary source | Result used | Design consequence | Transfer limit |
|---|---|---|---|
| [Kim et al., “Capable language models can outgrow the benefits of collaboration”](https://www.nature.com/articles/s42256-026-01268-y) | Under fixed compute, collaboration gains depend strongly on task structure and can become negative for strong models and sequential work. | Compare one strong agent, independent best-of-`n`, and coordination at matched cost; stratify by decomposability. | Published tasks and topologies do not identify the best ymp policy on repository work. |
| [Lorenz et al., “How social influence can undermine the wisdom of crowd effect”](https://www.pnas.org/doi/10.1073/pnas.1008636108) | Social information can reduce diversity without improving accuracy. | Commit independent first assessments before discussion. | Human estimation groups are not language-model reviewers; the mechanism remains experimental. |
| [Kohli et al., “Nine Judges, Two Effective Votes”](https://arxiv.org/abs/2605.29800) | Nominally distinct language-model judges can have highly correlated errors. | Record independence lineage; never equate model count with independent verification. | This is a recent preprint and does not estimate every executor or task class. |
| [Gao et al., “Scaling Laws for Reward Model Overoptimization”](https://proceedings.mlr.press/v202/gao23h.html) | Repeated optimization against a proxy can degrade the true objective. | Validate negative controls and limit adaptive oracle feedback. | Reward-model optimization is an analogy; each acceptance oracle needs its own attack study. |
| [Dwork et al., “The reusable holdout”](https://pubmed.ncbi.nlm.nih.gov/26250683/) and [Blum and Hardt, “The Ladder”](https://proceedings.mlr.press/v37/blum15.html) | Adaptive reuse of evaluation data invalidates naive holdout reasoning; controlled disclosure can reduce leakage. | Budget protected queries, restrict diagnostics, and refresh approved holdouts. | Their formal guarantees require mechanisms and assumptions ymp does not yet implement. |
| [Ismail and Jøsang, “The Beta Reputation System”](https://aisel.aisnet.org/bled2002/41/) | Binary outcomes can be summarized with explicit uncertainty under exchangeability assumptions. | Permit descriptive evidence cards with sample size and intervals. | Self-selected, drifting, oracle-relative tasks violate the assumptions needed for a permission gate. |

### Emergent communication and collective intelligence

| Primary source | Result used | Design consequence | Transfer limit |
|---|---|---|---|
| [Lowe et al., “On the Pitfalls of Measuring Emergent Communication”](https://www.ifaamas.org/Proceedings/aamas2019/pdfs/p693.pdf) | Messages may correlate with sender behaviour while having no effect on a receiver; positive signaling and positive listening are distinct. | Do not infer communication from a transcript; intervene on message content and receiver behaviour. | The experiments use trained reinforcement-learning agents in simple games, not pretrained coding agents. |
| [Jaques et al., “Social Influence as Intrinsic Motivation for Multi-Agent Deep Reinforcement Learning”](https://proceedings.mlr.press/v97/jaques19a.html) | Counterfactual influence can quantify how one agent changes another's action. | Treat influence as an offline observational variable. | Rewarding influence in ymp could select manipulation and theatre; the proposed intrinsic reward is deliberately not adopted. |
| [Woolley et al., “Evidence for a Collective Intelligence Factor in the Performance of Human Groups”](https://pubmed.ncbi.nlm.nih.gov/20929725/) and [Riedl et al., “Quantifying Collective Intelligence in Human Groups”](https://pubmed.ncbi.nlm.nih.gov/34001598/) | Human-group performance can exhibit a cross-task factor, but it is established over many groups and diverse tasks. | Reserve any collective-intelligence claim for repeated out-of-sample task batteries. | Human results do not establish model cognition, and one compelling run cannot estimate a general factor. |
| [Riedl, “Emergent Coordination in Multi-Agent Language Models”](https://arxiv.org/abs/2510.05174) | Information decomposition and randomized interventions can separate temporal coupling from performance-relevant synergy in a simple language-model game. | Consider later information-theoretic analysis and perturbation tests for complementarity. | This is a recent preprint in a narrow game without direct communication; it is not proof of general intelligence. |

ymp therefore studies **behavioural signs of collective reasoning**. It does not claim to detect
consciousness, expose hidden reasoning, or discover a new natural language. The potentially
emergent objects are roles, conventions, information dependencies, and adaptive coordination
patterns.

### Security and confinement

| Primary source | Result used | Design consequence | Transfer limit |
|---|---|---|---|
| [Saltzer and Schroeder, “The Protection of Information in Computer Systems”](https://doi.org/10.1109/PROC.1975.9939) | Least privilege, complete mediation, and least common mechanism reduce authority and shared attack surface. | Re-authorize every boundary action and separate plane capabilities. | Principles do not select or validate a concrete Linux isolation stack. |
| [Lampson, “A Note on the Confinement Problem”](https://doi.org/10.1145/362375.362389) | General confinement includes covert channels that ordinary access control does not eliminate. | State residual timing, resource, provider, and artifact channels explicitly. | ymp cannot claim the board is the only communication channel. |
| [Hugging Face technical timeline](https://huggingface.co/blog/agent-intrusion-technical-timeline), [Hugging Face disclosure](https://huggingface.co/blog/security-incident-july-2026), and [OpenAI account](https://openai.com/index/hugging-face-model-evaluation-security-incident/) | Allowed infrastructure, credentials, external communication, and shared discoveries amplified an autonomous intrusion. | Treat proxies, dependencies, boards, credentials, and verifiers as attack surfaces; separate information from authority. | One incident is a case study, not a complete threat taxonomy or proof that all agents collude. |

### Runtime integration and protocol boundaries

| Primary source | Result used | Design consequence | Transfer limit |
|---|---|---|---|
| [MCP transport specification, revision 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) and [versioning](https://modelcontextprotocol.io/docs/learn/versioning) | MCP negotiates revisions and supports stdio JSON-RPC; the client launches a local stdio server. | Run `ymp internal agent-mcp` per invocation and test supported revisions. | Transport interoperability supplies neither ymp authorization nor transactional semantics. |
| [MCP tasks specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks) | Durable MCP tasks are experimental and use their own polling and result lifecycle. | Do not map work obligations, yields, or termination to MCP tasks in the first implementation. | A later stable revision may justify a compatibility projection, but not ownership of kernel state. |
| [MCP security best practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices) | Token passthrough and confused-deputy behavior break authorization and attribution. | Bind attempts at a controller-owned private endpoint, keep authority in the application core, and never forward agent-supplied bearer tokens. | Most OAuth guidance targets HTTP servers; ymp's initial MCP transport is local stdio. |
| [Codex App Server](https://developers.openai.com/codex/app-server/) and [Codex configuration reference](https://developers.openai.com/codex/config-reference/) | Codex exposes a bidirectional JSON-RPC lifecycle over newline-delimited JSON on stdio by default, while custom model providers are configurable by base URL but currently use the Responses wire API. | Spawn App Server as an attempt-scoped child behind a runtime driver and record its model route separately; a compatible NVIDIA NIM route is possible only through Responses. | App Server and compatible endpoints remain version-specific; endpoint naming does not prove tool, event, or cancellation fidelity. |
| [Claude Code programmatic mode](https://code.claude.com/docs/en/headless), [LLM gateway configuration](https://code.claude.com/docs/en/llm-gateway), and [model configuration](https://code.claude.com/docs/en/model-config) | Claude Code exposes structured non-interactive sessions and can use a custom Anthropic Messages endpoint and model identifier. | Treat Claude Code plus a Nemotron Messages route as a distinct profile and generate all endpoint, model, MCP, and capability settings per invocation. | Claude Code may assume Anthropic-specific beta features and model behaviour; a Messages-compatible endpoint is not automatically a conforming Claude Code backend. |
| [OpenCode CLI](https://opencode.ai/docs/cli/), [providers](https://opencode.ai/docs/providers), [configuration](https://opencode.ai/docs/config/), and [server](https://opencode.ai/docs/server/) | OpenCode exposes JSON events, sessions, stdio MCP, an HTTP/SSE server, explicit NVIDIA and local NIM routes, and many model providers. Its configuration sources merge. | Add OpenCode as a runtime implementation; prefer process-per-turn for the portable baseline and prove effective configuration isolation before strict use. | `--pure`, provider allowlists, and generated files do not by themselves suppress every ambient or managed source; the server creates a separate local HTTP attack surface. |
| [NVIDIA NIM LLM API reference](https://docs.nvidia.com/nim/large-language-models/latest/api-reference.html), [Claude Code integration](https://docs.nvidia.com/nim/large-language-models/latest/ai-assistant-integrations/claude-code.html), and [Nemotron 3 Ultra guide](https://docs.nvidia.com/nim/large-language-models/2.0.6/day-0/get-started-nemotron-3-ultra.html) | Current NIM LLM exposes OpenAI Responses, Chat Completions, and Anthropic Messages with streaming and tool calling; NVIDIA documents Claude Code as a NIM client. | Model the wire protocol in `ModelRoute`; probe each exact Nemotron model, NIM version, deployment, and runtime pairing. | API compatibility is not equivalence of agent prompts, tool semantics, usage fields, server-side state, or quality. Hosted NVIDIA endpoints and self-hosted NIM may differ. |
| [Anthropic client SDK overview](https://platform.claude.com/docs/en/cli-sdks-libraries/overview) | Official Messages clients support configurable endpoints but no official Rust client is listed. | Use the protocol directly from an embedded Rust client for a future native runtime rather than adding a Python or Node.js helper. | Reimplementing the wire client still requires conformance tests and makes ymp responsible for retries, streaming, and agent-loop semantics. |

## Rejected mechanisms

- **Central semantic planning, fixed roles, required levels, cheapest-capable selection, and
  prescribed escalation.** These make the system a deterministic solver.
- **A learned grade as a claim gate.** Learning the number does not stop the gate from being
  central allocation; realistic sample sizes make it mostly noise during cold start.
- **Dynamic complexity that rises after failure.** Failure is evidence about an attempt and oracle,
  not a measurement of latent task complexity.
- **One globally readable board.** It spreads false information and coordinates misuse across the
  whole project. Scoped boards preserve local choice with a smaller blast radius.
- **A shared writable repository.** It permits silent overwrite and invalidates provenance.
- **Consensus or majority vote as acceptance.** Correlated agents can agree on the same error.
- **Rewarding influence, agreement, role formation, or message activity.** These measurements would
  become targets and select theatrical or manipulative behaviour.
- **Exactly five permanent protocol verbs.** Versioned explicit effects matter; verb count does
  not provide minimality or safety.
- **Automated contract drafting as part of the first coordination proof.** It confounds oracle
  quality with coordination quality.
- **A TUI-specific kernel and an arbitrary-project claim.** The current product starts with
  ratatui, but widgets do not own protocol semantics and the initial claim remains bounded.
- **A runtime dependency on Paseo or another agent orchestrator.** It would make ymp's lifecycle,
  failure model, storage compatibility, and future deployment boundary depend on a different
  product. Comparative patterns may be reimplemented behind ymp's own tests.
- **One process merely because distribution uses one executable.** The trusted TUI and application
  core intentionally share a foreground process, but untrusted provider children, tool bridges,
  native runtime workers, and independent verification retain explicit process boundaries.
- **MCP as the authoritative internal bus.** MCP is valuable at the model-tool boundary but does
  not define ymp's transactions, subject binding, fencing, budget conservation, obligation tree,
  or replicated-controller semantics.
- **One “provider adapter” that conflates harness, model provider, and tool protocol.** OpenCode and
  compatible NVIDIA endpoints make those axes independent; conflation would corrupt provenance and
  force route-specific assumptions into the lifecycle interface.
- **Calling a model API from the trusted kernel writer.** Untrusted response parsing and tool execution
  would collapse the attempt and control boundaries. A native harness, if added, runs as a
  restricted worker and reaches credentials through the broker.
- **SQLite on shared cluster storage.** The local single-writer design is not a distributed
  database; a later cluster uses a store with serializable transactions and explicit failover.

## Open implementation decisions and recommendations

### 1. Contract and oracle construction — design-critical

**Recommendation:** use manually reviewed, prevalidated contract packages for the first controlled
evaluation. Repository analysis plus a structured human interview may draft later packages, but no
generated oracle becomes authoritative without requirement-to-evidence review, negative controls,
and clean-room reproduction.

This is the decision most capable of sinking the product. A weak oracle makes more search increase
false acceptance; an impossible or flaky oracle burns budget indefinitely. No coordination policy
can repair it after the run starts.

### 2. First proving ground

**Recommendation:** hermetic software repair and bounded code-change tasks on Linux, with protected
black-box checks and no production credentials or external effects. Include both decomposable and
sequential strata. Begin with repositories whose toolchains can be pinned; do not generalize to
open-ended research or taste-based work. During POC, the outer study harness supplies hermeticity;
ymp's own strict Linux profile is evaluated only in MVP.

### 3. Isolation stack

**Recommendation:** support strict mode only on Linux hosts that pass a startup probe for private
user, mount, PID, IPC, and network namespaces; cgroup v2 delegation; seccomp; an available Linux
security module; private filesystems; and complete descendant cleanup. Attempts have no direct
external network and use a same-executable relay. Attack-test this exact composition. Native macOS
is best-effort; a virtual machine may be a later user-supplied execution backend but is not a
packaged dependency.

### 4. Board visibility

**Recommendation:** detailed task and candidate-review scopes, bounded project-discovery notices,
explicit local invitation, and no protected or capability data. Test whether this still permits
useful cross-branch discovery before considering broader visibility.

### 5. Verifier implementation

**Recommendation:** core-owned orchestration for machine checks and exact-digest evidence. Use a
blinded agent or human only for approved rubric criteria that cannot be mechanized. Never let a
wrapped producer certify its own candidate.

### 6. Budget denomination

**Recommendation:** retain a vector. Use actual provider cost as the primary matched-comparison
quantity, with wall time, model tokens, CPU, memory, disk, messages, starts, queries, and authority
reported separately. Call a ceiling hard only when the provider or broker can enforce it.

### 7. Collective-reasoning threshold

**Recommendation:** preregister the primary causal contrast and minimum useful effect before data
collection. Require replication across task and group samples. Treat information-theoretic synergy
as exploratory until it predicts accepted outcomes beyond simpler ablations.

### 8. Local state and cluster evolution

**Recommendation:** use one in-process kernel writer, a bounded append-only JSONL event journal,
atomic run metadata, and a local immutable object directory in POC. Treat a controller crash as
`infrastructure_error` instead of implementing transparent recovery before the coordination
hypothesis is tested. Keep a typed store interface and decide during MVP whether embedded SQLite is
justified by measured resume, recovery, and query needs. Define store, object, execution, and
transport conformance tests now, but do not implement a daemon, PostgreSQL, or Kubernetes in
parallel with the POC. If a later cluster is justified, begin with one elected active controller,
multiple worker replicas, PostgreSQL, and object storage; require stale-controller and partition
tests before claiming failover safety.

### 9. First runtime driver and Nemotron route

**Provisional recommendation:** implement Codex and Claude Code as the two real POC runtime
drivers, with the fake deterministic runtime preceding both. Codex App Server is the first narrow
vertical integration because it provides a structured JSON-RPC lifecycle suitable for a Rust TUI.
Claude Code follows in the same POC so the runtime abstraction is tested against a different
lifecycle and the experiment is not tied to one harness. The locally inspected Claude Code
`2.1.226` exposes bare non-interactive execution, explicit MCP configuration, structured streaming
input and output, session resume, tool selection, and a per-invocation `--max-budget-usd` limit.
Either driver is excluded from matched comparisons if its pinned profile cannot disable native
subagents, isolate configuration, terminate descendants, or produce reproducible usage evidence.

The first **Nemotron route probe** depends on the approved endpoint rather than a global runtime
preference. For NVIDIA's hosted API, OpenCode is the documented direct integration. For a pinned
self-hosted NIM that exposes Anthropic Messages, Claude Code is the documented integration and
reuses the provisional POC driver. The locally inspected OpenCode `1.18.15` exposes
`run --format json`, sessions, `--pure`, selectable `provider/model` routes, stdio MCP, and a
headless server; its official provider configuration directly names both NVIDIA and local NIM.
Its merged configuration and lack of an established per-invocation hard-cost control must still be
measured before it can replace Claude Code in the primary experiment or enter strict mode.

The locally inspected Codex CLI `0.147.0` exposes structured `exec` output, session resume, stdio
MCP configuration, a richer App Server, and custom Responses providers. Its App Server remains
experimental in the inspected build, and the inspected `exec` interface exposes no equivalent hard
cost flag. Codex plus NVIDIA NIM remains a separate route because both Codex and the exact NIM
deployment must agree on Responses streaming and tool semantics.

The POC requires conforming Codex and Claude Code implementations. MVP may add OpenCode, especially
for a hosted NVIDIA route, after its merged configuration and process or endpoint lifecycle pass
the same conformance suite. Runtime choice is an engineering and local participant constraint, not
kernel allocation policy.

### 10. Direct model API

**Recommendation:** preserve the interface now but do not build a native agent harness in POC.
First probe the exact NVIDIA route with a minimal Rust protocol harness outside the experimental
runtime. If external harnesses cannot provide brokered credentials, exact route provenance, or
enforceable cancellation, or if later cluster workers need a smaller dependency surface, implement
`ymp internal runtime-worker` after the coordination mechanism passes.

The native driver should begin with one wire protocol selected by conformance evidence, most likely
OpenAI Responses because both current Codex custom providers and current NIM LLM expose it. An
Anthropic Messages binding remains valid for Claude Code and for a later native driver, but adding
an official Anthropic client SDK as a non-Rust helper would violate the one-Rust-executable boundary
and still would not supply a coding-agent harness. Such an SDK may serve as a development-only
reference implementation in protocol conformance tests, but it is not a shipped runtime component. Direct
access changes the harness and therefore must be evaluated as its own runtime profile rather than
treated as a transparent transport swap.

## Owner decisions of 2026-08-13

These entries record decisions taken during execution. Each states what was decided, why, and what
it changes in already accepted work, so a later reader is not left comparing two documents that
disagree.

### The product goal governs the mechanics

A person writes a prompt; ymp turns it into an approved contract with a checkable acceptance
condition, raises several agents that divide the work among themselves with no assigner above them,
and returns a result an independent verifier accepted. Every protective and mechanical decision
exists to make that sentence trustworthy. A decision that hardens the system while making that path
impossible is a divergence to resolve, not a design to preserve. The full statement is in
[`ymp-docs/CONCEPT.md`](https://github.com/maggnus/ymp/blob/e97fd1ec269c80d1ba45c90a2089ec31a9f868fd/ymp-docs/CONCEPT.md).

### Everything the interface can do is available as a command

The terminal interface is a convenient way to reach the system core, not the only way. Any action
the interface offers — starting a run from a prompt, authorizing a contract, cancelling, exporting
evidence, applying an accepted candidate — must also exist as a command of the same executable, with
the same authority checks and the same journal path through the kernel writer.

This supersedes the constraint recorded in accepted subtask `W1-APP-02a.2`, which forbade a public
mode outside the interface. That constraint was taken to prevent a second, weaker path into the
kernel; the risk it addressed is answered by routing every command through the same writer and the
same authorization, not by removing the command surface. The accepted closure record of that subtask
stands as history and is not rewritten; its restriction no longer binds new work. The line
`ymp apply cd-32` in the visual concept, which an audit reported as contradicting that subtask, is
consistent again.

A command surface is not an automation surface: a command that mutates state remains subject to the
same decision surfaces, including typing a run identifier to confirm an irreversible action.

### The full suite runs before integration, not at the end of a card

Every card ended with a full workspace run, which cost about seven minutes each time and repeated
what the narrow tests had already settled. The suite now runs once, immediately before a merge into
the release branch, where it answers the only question it is good at: whether the combination of
independently accepted changes still holds together. A worker proves its own card with the narrow
tests for what it changed and their negative halves, plus formatting and lint on the packages it
touched. A reviewer runs the suite only to settle a hypothesis that the combined tree alone can
answer, and says which one.

### The drawn interface sources are references, not a frozen contract

The HTML source and its fixed-layout export describe an intended interface. What suits the product
is taken; what does not is changed, and the change is recorded in the work tree. The HTML source is
the only carrier of correspondence, and the export is a reading convenience that is never cited as
the source of a screen or a fixture.

### The interface is one framework bound to real state

Screens compose one shared component and layout layer, and every displayed value derives from an
application projection. A concept the current domain cannot produce is shown as unavailable rather
than invented. Both properties are enforced by checks that must fail on violation, because an
interface that fabricates a value corrupts the judgement the comparison depends on.

## Change rule

A semantic allocation mechanism does not enter the trusted kernel. A new mechanical mechanism must
state its invariant, fault assumptions, security effect, and a preregistered matched-budget ablation.
If the same benefit can be obtained as a participant-local policy, it remains outside the kernel.
