# ymp

A local terminal system for bounded, self-organizing work by coding agents.

Point ymp at a repository and give it an approved project contract. Wrapped agent runtimes may
work alone, advertise work, accept locally negotiated commitments, decompose goals, challenge
findings, and submit competing candidates. A small trusted kernel preserves the conditions
under which this search remains finite, attributable, isolated, and independently testable.

The instrumental product claim is deliberately bounded:

> For a declared class of reproducible software tasks with a validated acceptance oracle, ymp
> aims to increase the independently accepted-result rate over both a strong single-agent
> baseline and independent best-of-`n` search under the same total resource budget.

This is a hypothesis to be tested, not a consequence of using more agents. Multi-agent systems
can improve decomposable work and degrade sequential work; strong agents can also exhaust the
benefit of collaboration. A passing candidate proves only what the approved oracle observes.

ymp has a second, observational research goal:

> Make any collective reasoning that arises legible enough to study: unprescribed local roles,
> complementary contributions, evidence-driven revision, preserved justified disagreement, and
> recovery after misleading messages or participant loss.

Fluent dialogue is not evidence for this claim. The wrapped models already speak human language,
and a persuasive transcript can be causally irrelevant to the result. ymp therefore calls these
**behavioural signs of collective reasoning**, not proof of consciousness, understanding, or a
group mind. Message-removal and message-replacement experiments, matched-budget baselines, and
accepted artifacts determine whether an exchange was useful.

## Constitution, not choreography

Agents control the semantics of work: whether to collaborate, what roles to form, how to split
or combine work, whom to ask, which evidence to trust, and what method to use. The kernel does
not contain a planner, skill matcher, escalation tree, or quality-ranking policy.

The kernel controls shared effects: authenticated state transitions, resource and authority
conservation, isolated workspaces, immutable candidate construction, obligation accounting,
and independent verification. This deterministic substrate makes races and failures safe; it
does not solve the project. In particular, it never rewards agreement, verbosity, influence,
role formation, or a preferred communication topology.

The original slogan, “reliability through selection, not control”, is retained only in this
precise form: **do not prescribe the solution process; select immutable results using evidence,
while controlling side effects and stopping conditions.** Selection cannot repair an invalid
oracle, create a solution where all attempts fail, or turn correlated opinions into independent
verification.

## Research stance

The coordination design draws on self-stabilizing distributed systems, termination detection,
local negotiation, blackboard systems, adaptive-data analysis, and empirical work on
language-model collaboration, emergent communication, and collective intelligence. Those results
justify local transition rules, expiring ownership, work-obligation accounting, protected
evaluation, blinded initial judgments, and controlled message interventions. They do **not**
justify a general claim that an unconstrained group will converge on a correct answer, nor do
results from humans or trained multi-agent reinforcement learning transfer automatically to
wrapped coding agents.

[DECISIONS.md](DECISIONS.md) records the primary sources, the conditions under which each result
applies, and the concrete design consequence. Mechanisms borrowed from materially different
domains, such as response-threshold models for insect or robot swarms, remain experimental until
an ablation shows value on repository work.

## Distribution and runtime boundary

ymp is distributed as one Rust executable. Its current and only user-facing mode is a foreground
ratatui application: the TUI, trusted kernel, run state, and runtime supervisor live in one main
`ymp` process. The same installed file may be launched in private child modes for the agent-facing
MCP bridge, isolation setup, a future native runtime worker, and independent verification. Claude
Code, Codex, and later OpenCode are managed child runtimes; the user starts only `ymp`.

The POC keeps authoritative live state in the main process and records a bounded append-only event
journal, atomic run metadata, content-addressed objects, logs, and private workspaces as external
data. It does not require SQLite, a background daemon, an operator socket, an HTTP listener, or a
WebSocket endpoint. One executable is a packaging invariant, not a requirement that untrusted
runtimes and verification share the TUI process.

An **agent runtime** owns one agent harness and its start, resume, event, tool, and cancellation
lifecycle. Claude Code, Codex, and OpenCode are external runtime implementations. A later
ymp-native runtime may call a model API directly from a restricted worker process. The **model
route** is recorded separately because one runtime, especially OpenCode, may use different model
providers, endpoints, accounts, and models. NVIDIA Nemotron is therefore a model route that may be
paired with OpenCode, an Anthropic Messages-compatible Claude Code profile, an OpenAI
Responses-compatible Codex profile, or a later native runtime. Each pairing is tested separately.
The same authorized ymp commands can be exposed through stdio MCP or provider-native typed tools;
MCP is a binding, not the kernel or a universal runtime interface.

The external harnesses and the operating-system facilities they require remain platform
prerequisites. ymp itself does not require a database service, container runtime, Python or Node.js
helper, Paseo, or any other orchestrator; a selected external runtime may have its own documented
prerequisites. Paseo is only a comparative implementation reference for lifecycle ideas such as
stable identifiers, explicit interruption, and asynchronous completion notification; ymp neither
calls it nor reads or shares its state.

The TUI invokes a typed application interface in process. That interface keeps storage, object,
execution, and event delivery replaceable without building a network service now. A later,
separately approved mode may place the same application core behind `ymp server`, let the TUI
connect as a client, or add a web interface. A still later Kubernetes mode may use PostgreSQL,
object storage, worker pods, and an elected active controller. None of these external transports is
part of POC, MVP, or Alpha, and MCP is not their internal protocol.

## Documents

| Document | Contents |
|---|---|
| [CONCEPT.md](CONCEPT.md) | Bounded claim, self-organization boundary, guarantees, and failure conditions |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Three-plane trust boundary, local commitments, immutable candidates, and communication observatory |
| [PROTOCOL.md](PROTOCOL.md) | Negotiation, scoped inert messages, obligations, leases, budgets, and termination |
| [PROJECT-CONTRACT.md](PROJECT-CONTRACT.md) | Public specification, protected oracle, approval, and oracle validation |
| [REPUTATION.md](REPUTATION.md) | Why grade was removed; contextual evidence, uncertainty, cold start, and diversity |
| [SECURITY.md](SECURITY.md) | Threat actors, trust boundaries, enforceable containment, and residual risk |
| [DECISIONS.md](DECISIONS.md) | Settled decisions, rejected mechanisms, research applicability, and open experiments |
| [ROADMAP.md](ROADMAP.md) | POC, MVP, Alpha, causal communication audit, and shipping criteria |
| [INVARIANTS.md](INVARIANTS.md) | POC contracts that no implementation change may weaken silently |
| [research/README.md](research/README.md) | Research index and boundary between scientific prose and executable study artifacts |
| [research/RES-001-calibration.md](research/RES-001-calibration.md) | Development agent ladder, pinned low-effort profiles, measured runs, and remaining blockers |
| [research/RES-002-weak-diagnostic.md](research/RES-002-weak-diagnostic.md) | Matched-budget weak-participant diagnostic protocol, stop/go rule, and implementation boundary |
| [research/RES-003-mechanism-map.md](research/RES-003-mechanism-map.md) | Falsifiable coordination mechanisms, interventions, expected null strata, and scientific stop rules |
| [VISUAL_CONCEPT.md](VISUAL_CONCEPT.md) | Chat-first composition, operator path, semantic constraints, and known implementation gaps |
| [design/ymp_chat_tui.dc.html](design/ymp_chat_tui.dc.html) | Exact terminal screens, state variants, fixtures, and reusable-structure handoff |
| [design/ymp_chat_tui.pdf](design/ymp_chat_tui.pdf) | Primary fixed-layout review and reading version of the terminal-interface concept |
| [work/WAVES.md](work/WAVES.md) | Generated POC execution overview; detailed current state is in [work/STATUS.md](work/STATUS.md) |

## Status

Early POC implementation exists in the production Rust workspace at
[`../ymp-rust`](../ymp-rust/README.md). It already builds one `ymp` executable with a typed event
core, append-only digest-linked journal, content-addressed objects, pinned Codex and Claude Code
process drivers, protected development-calibration oracles, opaque digest-bound verifier evidence,
a deterministic protocol model, and k9s-style Ratatui runtime and run views. The POC is an early
feature stage of the production codebase rather than a disposable
implementation. The Ratatui interface is present from the first executable because observing and
controlling agent interaction is part of the product hypothesis. Reproducible automated tests call
the same application core without becoming a second public interface. Strict execution initially
targets supported Linux hosts; native macOS execution is
explicitly best-effort in the initial one-executable release. Documents of record are in English.
POC experiments still run in an externally disposable study environment and make no
product-security claim; strict containment is an MVP requirement only if the POC passes.
