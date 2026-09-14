# Architecture

## Responsibility

YMP separates task meaning, trusted decisions, external effects and presentation.
The kernel owns admission and state transitions. Replaceable strategies can choose
how to work without choosing which rules to bypass.

The initial dependency graph is:

```mermaid
flowchart TD
    CLI[ymp-cli] --> Runtime[ymp-runtime]
    Runtime --> Kernel[ymp-kernel]
    Runtime --> Domain[ymp-domain]
    Kernel --> Domain
```

| Crate | Owns | Must not depend on |
| --- | --- | --- |
| `ymp-domain` | Identifiers, values, task contracts and pure validation. | I/O, runtime, storage implementations or presentation. |
| `ymp-kernel` | Trusted lifecycle, event projection, typed service ports and denials. | Concrete journal/provider implementations, CLI or runtime assembly. |
| `ymp-runtime` | Application assembly and adapters implementing kernel ports. | Presentation packages. |
| `ymp-cli` | Command parsing, process exit status and terminal output. | Concrete domain mutation or journal writes. |

The first journal adapter is an in-memory implementation in `ymp-runtime`.
Dedicated storage, provider and terminal-interface crates should be introduced
when they have executable responsibilities; the foundation has no empty packages.

## State and decisions

`Journal` appends a batch to one session stream against an expected revision.
Comparison and append are atomic: a stale revision or rejected batch changes
nothing. Reads return a coherent ordered history. Kernel projection validates the
history and produces a `SessionView`; a malformed history is an error, not a
successful empty session.

State transitions record facts. The event schema is independent of terminal
messages. A user-facing explanation cannot substitute for the authoritative
decision or its evidence.

For model-assisted decisions, retain the policy version, the actual supplied
input and the observed response. Reconstructing a decision means resolving those
records, not expecting a repeated model call to return identical text.

## Authority and effects

An assignment requires current eligibility, supported settings, independence,
available resources and enforceable workspace access. These checks belong to
trusted admission, including the final atomic check against current state.

Database commit and native process execution have different failure boundaries.
Record admitted, started, cancelling, terminated and uncertain states explicitly
when execution is implemented. Revoking a grant does not prove that a process has
stopped writing. A conflicting successor waits for the required termination or
effect evidence.

Unknown resource usage is not zero. All admitted work, including coordination,
failed attempts, verification and learning, belongs to the session's accounting.
Reservations protect concurrent admission and verification capacity; they do not
promise native limits that the provider cannot enforce.

## Acceptance

Independent review, criterion satisfaction and confirmation remain separate.
Applicable failures cannot be overruled by agreement. Evidence covers exact
versions of the result, criteria and checker. Acceptance and credit are committed
by the kernel after those bindings have been validated.

Finalization requires stable result contents and resolution of relevant running
work. Reports state unmet conditions and uncertainty. Optional knowledge
preparation should have a durable, bounded lifecycle that does not hold up delivery.

## Extension boundaries

Add a port when two useful implementations need the same enforced contract.
Do not make every helper a strategy. Start with simple deterministic policies;
estimated value, calibrated routing and experience-based choices need data before
their effectiveness can be claimed.

Native model discovery supplies actual identifiers and supported controls.
Authentication stays in the native environment. Adapters must preserve typed
failure causes, invocation attribution and incomplete observations.

Persistence, native execution, public MCP over stdio and a terminal UI are planned
capabilities. Their absence is explicit in the foundation and its command-line
output. The product will expose one native executable; development tools are not
automatically application runtime dependencies.
