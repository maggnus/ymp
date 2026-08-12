# Bounded protocol model

`ymp-evals` is an executable finite-state abstraction of the mechanical rules in
`ymp-docs/PROTOCOL.md`. It is a falsifier for protocol invariants, not a production kernel and not a
semantic scheduler.

## Represented state and bounds

The initial state contains one root obligation, three creation units, four invocation-start units,
two wait registrations, one protected verification query, and two sponsor-funded award slots. One
negotiated offer can receive two bids and two mechanically compatible awards. Advertising consumes
one creation unit and reserves both award slots; each award consumes its own reservation, one
creation unit, and one invocation start before it creates a child obligation whose causal parent is
the root.

Each child has an attempt, a monotonically increasing lease generation, a lease deadline, an
optional yielded invocation with a wake deadline, a stale-submission record, and at most one current
candidate. Waking or reissuing an attempt consumes another invocation-start unit. Yield consumes a
wait unit. A protected verification request consumes the query unit and binds the selected current
candidate. Every present obligation has exactly one terminal return, including returns caused by
cancellation and controller failure.

Logical time ranges from 0 through 4. The offer deadline is 2, leases last at most two time units,
and a wake remains eligible for at most one time unit. The model includes one duplicated
`advertise` delivery. The duplicate is consumed as a fault event but does not repeat the command's
budget or reservation effects.

## Exhaustive safety and termination check

The checker performs breadth-first traversal of every reachable state and eliminates repeated
states. It has no depth limit. Every enabled action is recorded as an edge, and invariant checks use
the same graph for the baseline and rule mutations. Breadth-first predecessor records therefore
produce a shortest counterexample.

After traversal, a separate termination check rejects a nonterminal sink, a transition from a
terminal state, or any reachable nonterminal cycle. For an acyclic graph whose only sinks are
terminal, reverse topological evaluation computes the maximum number of transitions from the
initial state to a terminal sink. This finite value, rather than arbitrary search truncation, proves
that every maximal generated schedule terminates within the declared abstraction.

The baseline explores explicit choices to bid, award, submit, verify, return, abstain, cancel, or
crash. These are nondeterministic participant or fault actions. Their enumeration does not express
task priority, method, skill matching, candidate quality, or a preferred participant.

## Fault assumptions

- Delivery can be duplicated once for the represented `advertise` command. Command identity remains
  stable across the duplicate.
- Delays are arbitrary interleavings within bounded logical time. There is no fairness assumption
  hidden in a depth cutoff.
- Expiry increments the lease generation before a stale submission is recorded. The stale record is
  retained as evidence but cannot become the current candidate.
- Cancellation is authorized and may occur in any running state.
- A foreground-controller crash may occur in any running state. It closes the authority interval,
  returns every present obligation as `infrastructure_error`, and exposes no successor transition.
- The trusted state representation, monotonic logical clock, and checker implementation are intact.
  Byzantine corruption of them is outside the model, as it is outside the POC recovery assumptions.

## Negative controls

The executable runs three rule mutations through the same exhaustive checker:

1. `skip_award_reservation` creates an award without debiting its separate reservation and violates
   sponsor-fund conservation.
2. `ignore_lease_fence` promotes a stale-generation submission to the current candidate set.
3. `allow_second_obligation_return` records a second terminal return for one child obligation.

Each mutation must produce a serialized violation and shortest counterexample. A mutation that
survives makes the executable return a failure status.

## Intentional omissions relative to `PROTOCOL.md`

The following behavior is intentionally absent or collapsed. Adding any of it requires a new bound
and an explicit state variable rather than an unbounded queue or semantic policy.

| Protocol behavior | Model treatment |
|---|---|
| Participant identities, runtime profiles, model routes, harnesses, MCP and ymp RPC transports | Omitted. The model starts after authentication and transport decoding and examines accepted mechanical commands only. |
| Control journal envelopes, object digests, durable append ordering, event cursors, replay after process restart and evidence export | Omitted. State transitions are atomic in memory; controller restart is prohibited after a POC crash. |
| Collaboration audiences, messages, salience, delivery receipts and communication byte budgets | Omitted. No collaboration payload exists, so it cannot carry authority or affect liveness. |
| `open_accept` and targeted offers, counter-proposals, withdrawal, sponsorship transfer, participant recruitment and proposal-stage execution | Collapsed to one funded negotiated offer with two fixed possible bidders. Mechanical consent remains explicit through separate bid and award actions. |
| Multiple offers, arbitrary obligation depth and dependency DAGs | Bounded to one root and two sibling children. Parentage is still checked separately from candidate state. |
| Lease renewal and multiple concurrent attempts for one contract | Replaced by expiry and bounded reissue of one attempt. Reissue consumes the same finite start budget and advances the fencing generation. |
| The full set of typed wake conditions, audience changes, event cursors and event coalescing | Collapsed to one matching wake class. Yield, deadline, wake eligibility and the separate wait/start charges remain explicit. |
| Model cost, tokens, wall time, CPU, memory, process count, disk, output, network, credentials, messages and external-action dimensions | Collapsed to four independent counters needed by this proof: creation, invocation starts, waits and protected queries. All omitted authority classes are unavailable, not unlimited. |
| Private workspaces, artifact bytes, base digests, integration conflicts and candidate ancestry DAGs | Collapsed to a per-child candidate record that states whether its fencing generation was current when committed. |
| Protected verifier failures, infrastructure failures, diagnostic disclosure and repeated oracle generations | Collapsed to one pending request and one passing exact-candidate result. Query conservation and acceptance preconditions remain explicit. |
| Blinded agent assessment, context reveal and human review | Omitted because the bounded model contains only mechanical verification. |
| Fine-grained `result`, `dead_end`, `declined`, runtime, model-route and integration outcomes | Collapsed to result, exhausted, abstained, cancelled and infrastructure-error returns needed for obligation and run termination. |
| Fair host admission, rate limits, concurrency slots and congestion signals | Omitted. Invocation starts are finite, and the checker explores every mechanical ordering without choosing a preferred request. |
| Strict isolation, broker enforcement, external effects and protected-oracle secrecy | Omitted by the task boundary. The model grants no network, credential or external-action capability. |
| Crash continuation, replicated controllers, SQLite recovery, server mode and cluster behavior | Omitted. A POC controller crash is terminal and deliberately has no recovery transition. |

The omission list is part of the model contract. A successful report applies only to these declared
bounds and abstractions; it is not evidence that the complete production protocol or future cluster
implementation terminates.
