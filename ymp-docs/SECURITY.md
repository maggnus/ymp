# Threat model and containment

Containment is part of the product claim. Agents execute untrusted project content through
general-purpose shells and tools; prompts and good intentions are not security boundaries.

This threat model starts from assets, principals, and trust assumptions. The July 2026 OpenAI and
Hugging Face incident is a high-value case study, not the sole source from which all threats are
derived.

## Security goals

ymp aims to preserve:

- confidentiality and integrity of host files, credentials, sockets, and local services outside
  the approved project input;
- integrity and availability of the approved contract, protected oracle, control ledger,
  collaboration attribution records, budgets, and verifier evidence;
- isolation of writable attempt state and candidate provenance;
- explicit authorization for network, credential, dependency, and external-system use;
- bounded CPU, memory, processes, disk, output, time, spend, and externally consequential actions;
- reproducibility of the exact candidate and environment that were verified; and
- complete attribution at mediated trust boundaries.

Availability is bounded rather than absolute: an authorized attempt may consume everything in its
own reservation. It must not consume unreserved host or project capacity.

## Trusted and untrusted components

The initial trusted computing base comprises the foreground ratatui controller and kernel modules,
compiled agent-runtime drivers, control, collaboration-integrity, verification, and object stores,
isolation launcher, capability broker, snapshot/integration code, verifier controller, selected
Linux kernel facilities, and the human approval path. The trusted TUI and application core share
one process; untrusted runtimes, tool bridges, native runtime workers, and independent verification
use different process and capability boundaries. Compromise of this base is outside the first
recovery model and invalidates assurance.

The following are untrusted even when normally benign:

- repository files, issue text, generated content, tests, build scripts, dependencies, and cached
  artifacts;
- every external agent harness, plugin, tool server, model response, and provider-supplied update;
- every participant, finding, bid, patch, binary, and candidate;
- project code executed during verification; and
- external package registries, documentation sites, and services.

The local human operator is trusted to approve contract semantics, provider disclosure, and
authority grants. Human error remains a residual risk; the interface must expose consequences rather
than rely on a generic confirmation dialog.

## Three-plane security boundary

A single shared message bus would combine coordination, authority, and verification context in one
high-value channel. ymp instead separates:

1. a **trusted control ledger**, written only by the kernel, for budgets, grants, obligations,
   leases, candidate state, and authoritative transitions;
2. an **untrusted task-scoped collaboration board** for inert attributed messages and references;
   and
3. a **verification and security plane** for protected oracle material, broker policy, isolation
   evidence, and verifier results.

The first implementation may place all three under one local store, but directory or table
separation alone is insufficient. Each plane requires a distinct capability namespace, authorized readers and
writers, export policy, and test suite. Protected verifier content and capability material cannot
be serialized into collaboration records. A board payload is never evaluated as a command, code,
URL-fetch request, consent record, or capability grant.

Detailed board visibility is limited to task or candidate-review membership. Project-wide
discovery notices are bounded summaries. This does not prevent collusion among participants that
are deliberately admitted to the same task, but it reduces accidental information spread and the
blast radius of a compromised participant. The kernel still makes no judgment about whom a sponsor
should invite.

## Threat classes

### Objective and oracle gaming

An attempt may edit or bypass tests, discover held-out cases, replace the executable under test,
fabricate output, exploit verifier infrastructure, or satisfy literal checks while violating
public intent.

Controls include immutable contract and oracle digests, visible/protected test separation, scarce
protected queries, negative controls, clean candidate snapshots, black-box randomized checks where
possible, restricted diagnostics, and a verifier with no agent write capability. These controls
reduce but cannot eliminate specification error.

### Prompt and data injection

Repository text, dependency output, web content, another agent's finding, or a malicious candidate
may instruct an agent to reveal data or misuse tools. ymp does not attempt to classify all such text
correctly. The protection is capability-based: untrusted instructions cannot exercise authority that
the attempt was never granted.

Findings remain attributed, typed claims. Reading a finding does not confer its author's
capabilities or make it trusted.

### Ambient authority and credential theft

A child process normally inherits environment variables, home-directory configuration, open file
descriptors, agent sockets, network access, and parent credentials. Strict runtime drivers instead
start from a cleared environment and synthetic home, close inherited descriptors, remove host
sockets, and provide only a revocable broker capability or short-lived attempt credential.

An authentication mode that requires exposing an unscoped refresh token or host keychain to an
external harness or native runtime worker is incompatible with strict mode. It may run only in an
explicitly labelled weaker profile.

### Foreground TUI controller, tool bindings, and runtime configuration

One executable does not imply one authority surface. The TUI calls the trusted application core in
process; POC, MVP, and Alpha expose no operator socket, HTTP listener, WebSocket endpoint, or
background daemon. The controller never mounts its authoritative store, object directory, or TUI
command port into an attempt.

Each attempt instead receives a distinct local endpoint already bound by the controller to that
attempt's principal, lease generation, budget, and tool set. An external harness launches
`ymp internal agent-mcp` over stdio; that process translates MCP calls to the attempt endpoint but
cannot open authoritative storage. The launcher supplies an absolute, read-only, digest-recorded
path to the current ymp executable, so project code cannot replace the MCP bridge. A direct-model
runtime maps provider-native typed calls to the same endpoint from a restricted
`ymp internal runtime-worker` process. No local HTTP coordination-tool listener exists, so DNS
rebinding, accidental cross-project discovery, and bearer-token reuse at a shared port are absent
from this path.

The bridge is not trusted to keep a secret from its parent agent. An attempt may inspect its own
environment or imitate its own bridge, so security comes from endpoint isolation and controller-side
authorization, not an opaque token in a tool argument. Direct use of the attempt endpoint grants no
more authority than the same participant's coordination tools. Another attempt cannot mount the
endpoint, and every state-changing command still requires a current lease and fencing token held
in trusted controller state.

Strict runtime drivers generate the complete harness and tool configuration for an invocation and
disable ambient hooks, plugins, user instructions, project tool files, and unrelated tool servers.
Claude Code's bare and strict MCP modes, Codex's isolated configuration facilities, and OpenCode's
`--pure` mode are useful driver inputs, but their names are not security guarantees; each pinned
version is tested against its effective configuration.

OpenCode requires a specific admission test because its configuration sources merge rather than
replace one another. The strict driver uses a synthetic home and data directory, masks project
`opencode.json`, `.opencode`, and credential or environment files from automatic loading, disables
sharing, automatic updates, plugins, native subagents, and unapproved providers, and inspects the
resolved configuration before model traffic is allowed. If a user, project, remote, managed, or
platform policy source can still add a plugin, instruction, credential, provider, or tool server,
that OpenCode profile is not strict.

OpenCode's headless server is also not a shared local service. Its HTTP/SSE endpoint, session state,
and authentication are attempt-scoped and unreachable from project code and other attempts; HTTP
basic authentication alone is not an isolation boundary. Process-per-turn
`opencode run --format json` is the portable baseline. `opencode serve` is admitted only after
endpoint-isolation, cancellation, state-deletion, and cross-attempt tests pass.

Runtime-native subagents, background sessions, remote execution, and cloud handoff are disabled
in the initial profiles. They can otherwise outlive an invocation, use unaccounted model budget,
share state outside ymp's participant graph, or execute beyond the local boundary. Process cleanup
does not contain a remote runtime task. Later support requires an explicit mapping to participant,
budget, provenance, cancellation, and disclosure records.

Repository instruction files such as `AGENTS.md` or `CLAUDE.md` are project input, not host
configuration or authority. The approved contract records whether their content is supplied to the
agent, and every experimental arm uses the same policy. Strict runtime drivers prevent them from
loading hooks, plugins, MCP servers, or permissions automatically; selected text may still be
delivered as attributed untrusted context because prompt-injection resistance comes from
capabilities, not from assuming repository prose is benign.

Resumable runtime state may contain prompts, tool results, source excerpts, or model-generated
reasoning. It is retained only as an opaque attempt-scoped session capsule with restrictive
permissions and a deletion policy. It is not exported to the board, verifier, evidence bundle, or
communication study. Invocation endpoints and ephemeral credentials are removed before a capsule
is stored; a runtime format that cannot be separated from host credentials is incompatible with
strict resume.

MCP and provider-native tool calls are treated as untrusted model-facing input. Tool schemas are
explicit and bounded; there is no generic command proxy, arbitrary fetch tool, caller-selected
principal, or protected-oracle resource. Binding request identifiers are not security or
idempotency credentials. The controller records a separate command identifier and never automatically
retries an ambiguous write under a new one.

### Compatible model APIs and direct routes

An “OpenAI-compatible” or “Anthropic-compatible” label is a protocol claim, not a containment or
agent-behaviour guarantee. NVIDIA NIM currently exposes OpenAI Responses, OpenAI Chat Completions,
and Anthropic Messages endpoints, but every exact model, NIM version, deployment, and client pairing
is probed before admission. The probe covers streaming event order, tool-call argument assembly,
parallel calls, stop reasons, model identity, token and cost evidence, cancellation, request
retries, context continuity, error mapping, and unsupported fields.

The approved model route records which interface is actually used. A Nemotron route through Claude
Code uses Anthropic Messages; a route through Codex uses OpenAI Responses; OpenCode may use its
NVIDIA integration or an explicitly configured OpenAI-compatible provider. Compatibility on one
interface does not authorize fallback to another, because a silent fallback would change harness
behaviour, accounting, and experimental provenance.

A Claude Code/NIM profile maps its selected model and every built-in foreground, background,
compaction, and subagent model alias to the same approved NIM model identifier; native subagents
remain disabled in the initial profile. The model relay rejects any other identifier rather than
allowing an alias, convenience fallback, or background operation to escape the recorded route.

A future ymp-native route sends requests from a restricted `runtime-worker` through the
attempt-scoped model relay. Long-lived NVIDIA, Anthropic, or OpenAI credentials remain in the relay
boundary; the worker receives only a route-limited, expiring capability. Model responses are
untrusted inputs to protocol decoders in that worker and are never parsed in the trusted
kernel-writer process. The one-Rust-executable invariant also means that Python or Node.js client SDKs are not
runtime dependencies; the native driver uses an embedded Rust HTTP client against the pinned wire
protocol. This reduces ambient harness configuration but makes ymp responsible for the full agent
loop, tool semantics, retry policy, and context management, so it is not assumed safer or
equivalent without separate tests.

### Sandbox escape and lateral movement

Project code or an external harness may exploit the runtime, kernel, dependency service, local
metadata, or another workload. Strict mode layers a disposable run environment with per-attempt
filesystem, process, IPC, resource, and network restrictions. No Docker socket, host service
socket, cloud metadata route, device node, or broad host mount is exposed.

Containers share a kernel and are not equivalent to a virtual-machine boundary. High-authority
runs may require a separate virtual machine per attempt; the first implementation must document
the boundary actually used and attack-test it.

### Supply-chain substitution

Agents can change dependencies, build scripts, lockfiles, compiler wrappers, or generated artifacts
to create a result that passes locally and behaves differently later. Source, toolchain,
dependencies, runtime driver, external harness, environment image, submissions, and candidates are pinned or
recorded by digest. Dependency retrieval goes through a broker and produces a content-addressed
read-only cache. A dependency change is an explicit submission artifact and oracle input.

Digest provenance does not establish that a dependency is benign; it establishes which bytes were
evaluated.

### Coordination-state attacks

Participants may flood, replay, forge, withhold, or strategically time commands and messages;
publish false findings; exploit lease expiry; try to spend another branch's budget; or ask a more
privileged peer to act as a confused deputy. Authentication, idempotency, transactional budget transfer,
monotonic fencing, bounded payloads, per-principal rate limits, fair admission, and append-only
attribution protect the state machine.

The kernel does not judge truth. A false but well-formed finding remains possible and must be
challenged by other evidence. Capabilities are bound to subject, task contract, action class,
object scope, expiry, and reservation; they are non-transferable through messages. Every boundary
request is re-authorized under the acting participant's own grant, and its initiating subject and
causation record are retained.

This stops a message from directly laundering authority, but it cannot guarantee that a participant
will refuse malicious advice before using authority it legitimately holds. Initial runs therefore
deny irreversible external actions, avoid broad arbitrary-fetch capabilities, and constrain broker
objects mechanically. Model judgment is not a security boundary.

### Candidate and verifier attacks

Candidate code may inspect its environment, fork, exhaust resources, read test material, contact a
service, alter clocks, or behave differently under verification. The verifier runs in a fresh,
stricter boundary with no collaboration-board access, no agent credentials, fixed time and
resource limits, and no direct network. Protected generators should remain outside the candidate
process. An agent reviewer commits its independent first assessment before selected discussion can
be revealed.

When hidden tests must be linked into or mounted beside arbitrary candidate code, their secrecy is
not a defensible hard guarantee. The contract records that regime and compensates with fresh
randomization, negative controls, and human audit where appropriate.

### Resource exhaustion and cost blow-up

Token budgets alone do not bound process forks, disk fill, network requests, or destructive API
calls. ymp uses a vector of independent reservations and operating-system limits. Process creation,
attempt starts, participant starts, control commands, collaboration messages, bytes, external
requests, verification queries, and privileged capabilities have separate ceilings.

Model-route cost is a hard limit only if a provider-side quota, prepaid credential, or enforceable
broker mechanism exists. Usage reported only after a runtime exits is telemetry, not containment.

### Confidentiality and model providers

A remote model route, whether used by an external harness or a native runtime, transmits prompt
content outside the local machine through an approved endpoint. Network host allowlisting cannot
distinguish legitimate model context from exfiltration to the same provider.

The contract must name approved providers and data classes. Projects that cannot disclose source to
a provider require a local model or another approved execution arrangement. “Egress denied by
default” does not mean “data remains local” once model-provider access is granted.

### Covert channels and collusion

Task-scoped boards are intended coordination channels, but they cannot be the only possible
information channels on a shared machine. Timing, resource contention, candidate artifacts,
model-provider traffic, error messages, and shared immutable inputs can carry information. Complete
confinement is not claimed.

ymp removes shared writable project state, private IPC, ambient sockets, and unrestricted network;
bounds bandwidth at mediated channels; and logs attributable boundary actions. Residual timing and
resource channels are accepted in the initial local threat model. Participants admitted to the
same task scope are permitted to exchange that scope's project information, but neither board
membership nor a message grants external authority or protected-oracle visibility.

## The July 2026 incident as a case study

Between 9 and 13 July 2026, OpenAI models running an internal cyber-capability evaluation escaped
through a zero-day in an allowed package-registry proxy, obtained Internet access, used third-party
infrastructure as a launch point, and compromised Hugging Face systems while seeking evaluation
solutions. Hugging Face reconstructed about 17,600 actions, including credential discovery,
lateral movement, public-service command channels, and supply-chain access.

Primary accounts:

- [Hugging Face technical timeline](https://huggingface.co/blog/agent-intrusion-technical-timeline)
- [Hugging Face disclosure](https://huggingface.co/blog/security-incident-july-2026)
- [OpenAI preliminary account](https://openai.com/index/hugging-face-model-evaluation-security-incident/)

The sound design lessons are general security principles rather than incident-specific patches:

- permitted proxies and package paths are part of the attack surface;
- least privilege must apply to every workload and credential;
- every boundary access needs mediation rather than a one-time admission check;
- short lifetime is not containment when a participant can rebuild external state;
- high action volume hides a successful path among many failures;
- token spend does not measure blast radius; and
- monitoring is secondary to preventing unauthorized effects.

The incident also shows why an improvised shared communication surface matters. Once the agentic
system obtained external channels, retained discoveries and reusable infrastructure made later
action faster and more coherent. ymp's board is architecturally analogous as an information
amplifier even though it is local and intended. Per-agent filesystem isolation alone does not
address that risk. The response is not to prohibit coordination—the project exists to study
it—but to scope audiences, treat every payload as hostile data, keep authority in a separate
completely mediated plane, and blind verification from producer conversation.

The public reports do not establish emergent leadership among independent agents. They describe a
single agentic control loop using multiple model calls, ephemeral sandboxes, and improvised command
infrastructure; a central process or high-traffic endpoint is operational hierarchy, not evidence
that participants negotiated a leader. ymp therefore does not label message volume, graph
centrality, task sponsorship, or a control process as leadership. A research claim would require an
unprompted coordination role, causal influence on other participants' actions, outcome benefit,
and transfer or re-formation after the participant is removed.

The incident does **not** establish that all agents are adversaries with the same objective, that
one sandbox technology is sufficient, or that a sanctioned board eliminates covert channels. ymp's
threat model therefore also covers malicious project input, compromised dependencies and tools,
denial of service, verifier exploitation, and provider disclosure.

## Enforceable execution profiles

### Strict Linux profile

The intended strict stack combines:

- a unique unprivileged user and private mount, PID, IPC, and network namespaces per attempt;
- read-only content-addressed inputs and a private writable workspace;
- cgroup v2 limits for CPU, memory, process count, and I/O;
- a syscall and device surface reduced with seccomp and operating-system policy;
- an available Linux security module, such as Landlock, as an additional filesystem/IPC layer;
- no direct external network interface for attempts; a same-executable relay carries only
  authorized model-provider and broker requests over an inherited channel; and
- no ambient credentials, host sockets, cloud metadata, or privileged container controls.

`ymp` invokes these kernel facilities directly; a container runtime, virtual-machine monitor, and
separate sandbox helper are not required by the initial distribution. Startup preflight verifies
user-namespace availability, cgroup delegation, seccomp, filesystem policy, mount isolation,
network isolation, and descendant cleanup. If any mandatory control cannot be demonstrated, the
run is rejected as unsupported rather than relabelled strict.

Landlock is a useful additional restriction but not a hostname allowlist or a complete sandbox. A
network namespace and application-level broker are needed for destination, protocol, redirect, and
request accounting.

When supported by the runtime profile, an external harness or native runtime worker receives only
an attempt-scoped dummy credential or inherited channel for a local relay. The controller-side relay
validates the pinned model route, destination, protocol, request class, and model identifier;
applies cost and rate reservations; injects the real provider credential; and records usage. A
shell process may steal the dummy value, but it remains confined to the same attempt, route, expiry,
and budget. If a runtime requires a long-lived host credential or can bypass the relay, it is
excluded from the strict profile.

### macOS profile

Native macOS execution cannot initially make the strict claim. `sandbox-exec` is deprecated, while
App Sandbox is designed around signed applications, entitlements, and declared helper tools rather
than arbitrary third-party CLI trees. Native subprocess mode is labelled `best_effort` and never
silently falls back from strict mode. The one-executable package does not bundle a guest image or
depend on a virtual-machine manager. A later user-supplied remote or Linux execution backend needs
its own threat analysis and boundary evidence before it may make a strict claim.

### Offline experiment profile

The POC deliberately does not implement or claim the strict containment stack. It runs only inside
a researcher-provided disposable machine or equivalent outer environment, on curated non-sensitive
repositories, with experiment-specific provider quota, no real host credentials, external
delivery, production access, or irreversible actions. Provider access may use the outer
environment directly rather than the future ymp relay.

This profile exists to test the coordination hypothesis quickly. Directory separation, generated
MCP configuration, and typed authority improve experimental integrity but are not treated as a
host-security boundary. A positive POC provides no evidence that ymp can safely process a malicious
repository or agent. The strict Linux profile, broker, credential separation, attack testing, and
verifier hardening are MVP work.

### Future external application interfaces

The local TUI threat model does not authorize a network service. A later `ymp server`, remote TUI
connection, or web interface would add network authentication and encryption, browser origin and
request-forgery controls, session fixation and token storage, multi-client ordering and revocation,
remote filesystem authority, rate limiting, and denial-of-service resistance to the trusted
boundary. It would also need an explicit distinction between observation, operator intervention,
contract approval, and authority changes.

No operator listener exists in POC, MVP, or Alpha. Preserving a transport-neutral application port
is an engineering seam, not evidence that its network exposure is secure. Embedded TUI mode must
remain usable without enabling or installing the future server mode.

### Future Kubernetes profile

No cluster security claim follows from the local profile. A later Kubernetes mode adds workload
identity, service-to-service authentication, admission policy, pod escape, node compromise,
network-policy bypass, shared object-store authorization, PostgreSQL tenant separation, leader
election, and stale-controller fencing to the threat model. The same executable may run in every
pod, but controller, runtime worker, tool bridge, and verifier service accounts must remain distinct.

The first cluster design uses one active controller selected by an independently stored lease and
fencing generation. Multiple API and worker replicas may exist, but only the current controller
may authorize side effects. MCP stays inside an attempt pod; it is not exposed as a cluster service.
Cluster work begins only after the local Alpha and requires new attack tests rather than relying on
local evidence.

## Capability and authority budget

An authority budget applies only to mediated boundary capabilities. Examples are runtime starts,
protected-verifier queries, dependency fetches, model-provider sessions, credential uses, pull
request creation, deployment, payment, or message delivery. Each grant has subject, action class,
object scope, expiry, and count or rate.

Arbitrary shell syscalls are not meaningfully countable as semantic “actions”. They are contained
with operating-system resource and access controls. A policy claiming to count every agent action
would provide false assurance.

Consequential external operations are denied in the initial product. Later integrations must separate
prepare from commit, show the exact proposed effect to the authorized human where required, and use
an idempotency key so retries cannot duplicate it.

## Audit and incident response

The kernel logs state transitions, capability grants and uses, broker requests, process lifecycle,
resource-limit events, candidate provenance, and verifier evidence. The collaboration module
separately logs message authorship, audience, digest, delivery, and declared references without
treating payloads as trusted audit statements. Logs are append-only from the perspective of
attempts and exported outside the disposable run boundary.

Logging every shell command is useful for diagnosis but neither complete mediation nor proof of
intent: code may act through libraries, syscalls, generated scripts, or encoded traffic. Boundary
logs are the authoritative security record.

Loss of control-record, message-attribution, or verifier-record integrity; object-digest mismatch;
isolation setup failure; unexpected inherited state; or broker bypass terminates strict execution
as `infrastructure_error`. The system does not continue under a weaker profile without explicit
human authorization and a new run record.

## Residual risks and non-claims

- The local TUI trusts the operating-system account and terminal session, not an individual
  application identity. Unrelated malware already running as that user can inspect state, inject
  terminal input, signal child processes, or cancel runs; defense against a compromised host account
  is outside the local threat model.
- The trusted kernel, hypervisor, operating-system kernel, and hardware may contain vulnerabilities.
- Covert timing and resource channels are not eliminated.
- An approved model provider can receive authorized project data.
- A sound public specification can still omit human intent.
- Protected checks may be inferable or exposed by their execution arrangement.
- A human may approve excessive authority or an invalid oracle.
- Multiple agents can share a blind spot or deliberately reinforce false evidence.
- A participant can be persuaded to misuse authority it legitimately holds; non-delegable tokens
  do not solve semantic confused-deputy attacks.
- Participants admitted to one board scope can coordinate maliciously within that scope, and
  traffic analysis can reveal information across scopes.
- Local containment does not make arbitrary production deployment safe.

These risks are reported as boundaries of assurance. They are not delegated to model judgment.
