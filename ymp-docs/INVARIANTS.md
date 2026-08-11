# ymp — POC invariants

Contracts that no POC change may weaken without an explicit decision in
[DECISIONS.md](DECISIONS.md). An invariant is not considered enforced until the named work unit
has accepted evidence.

## INV-1 — Semantic organization remains participant-local

The kernel may validate shared effects, but it must not assign semantic roles, rank bids, choose a
decomposition, prescribe escalation, or select a winning candidate.

**Enforced by.** Unenforced — see
[`W1-COR-03a`](work/waves/W1/W1-COR-03/tasks/W1-COR-03a.md) and
[`W1-COR-03c`](work/waves/W1/W1-COR-03/tasks/W1-COR-03c.md).

**Breaking it costs.** The POC would test a centrally planned solver rather than self-organization.

## INV-2 — Resource and authority creation is finite

Every attempt, participant, obligation, message, verification query, and external boundary action
must consume a reserved dimension of a finite budget vector. Delegation transfers capacity; it
does not create capacity.

**Enforced by.** Unenforced — see
[`W1-COR-03a`](work/waves/W1/W1-COR-03/tasks/W1-COR-03a.md) and
[`W1-COR-03b`](work/waves/W1/W1-COR-03/tasks/W1-COR-03b.md).

**Breaking it costs.** Runs may overspend, decompose without bound, or remain active indefinitely.

## INV-3 — Attempts cannot share writable project state

Every concurrent attempt receives a private writable workspace derived from an immutable base.
Submissions and candidates are content-addressed, and integration never mutates another attempt's
workspace.

**Enforced by.** Unenforced — see
[`W1-APP-02b`](work/waves/W1/W1-APP-02/tasks/W1-APP-02b.md) and
[`W1-COR-03d`](work/waves/W1/W1-COR-03/tasks/W1-COR-03d.md).

**Breaking it costs.** One participant can silently overwrite another participant's work and make
candidate provenance unverifiable.

## INV-4 — Collaboration data carries no authority

A board payload is bounded, attributed, and inert. It cannot grant a capability, form a task
contract, invoke a tool, fetch a URL, spend a budget, or enter the protected verifier input.

**Enforced by.** Unenforced — see
[`W1-COR-03c`](work/waves/W1/W1-COR-03/tasks/W1-COR-03c.md).

**Breaking it costs.** Prompt injection or malicious coordination can become an authoritative
state transition.

## INV-5 — Acceptance is exact and independently observed

Acceptance names one approved contract package, candidate, environment, and oracle digest. The
producing attempt cannot write verifier state or inspect protected oracle material before the
approved disclosure point.

**Enforced by.** Unenforced — see
[`W1-EXP-01a`](work/waves/W1/W1-EXP-01/tasks/W1-EXP-01a.md) and
[`W1-APP-02b`](work/waves/W1/W1-APP-02/tasks/W1-APP-02b.md).

**Breaking it costs.** Selection can amplify an oracle blind spot and report false acceptance.

## INV-6 — Event delivery is not authoritative storage

Committed commands and ordered events are the source of truth. In-process notification channels
may wake readers, but a lagging reader must recover from an event cursor rather than assume every
notification was received.

**Enforced by.** Unenforced — see
[`W1-APP-02a`](work/waves/W1/W1-APP-02/tasks/W1-APP-02a.md) and
[`W1-COR-03b`](work/waves/W1/W1-COR-03/tasks/W1-COR-03b.md).

**Breaking it costs.** A slow TUI view or runtime may miss state changes and act on a false local
projection.

## INV-7 — Every run reaches an honest terminal state

Finite leases, creation budgets, wake budgets, verification budgets, and obligation accounting
must eventually produce `accepted`, `exhausted`, `abstained`, `cancelled`, or
`infrastructure_error`. Quiescence is never treated as acceptance.

**Enforced by.** Unenforced — see
[`W1-COR-03b`](work/waves/W1/W1-COR-03/tasks/W1-COR-03b.md).

**Breaking it costs.** No-solution runs can consume resources without a bounded stopping
condition.

## INV-8 — The POC remains a disposable experiment

The POC runs only in an externally disposable environment with curated non-sensitive inputs,
experiment-specific provider quota, no production credentials, and no irreversible external
actions. It makes no product-containment claim.

**Enforced by.** Unenforced — see
[`W1-EXP-01a`](work/waves/W1/W1-EXP-01/tasks/W1-EXP-01a.md) and
[`W1-EVL-04a`](work/waves/W1/W1-EVL-04/tasks/W1-EVL-04a.md).

**Breaking it costs.** Experimental evidence would be obtained by exposing an ordinary host or
sensitive project to controls that the POC explicitly does not provide.
