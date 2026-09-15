# Bounded native execution contract

This contract defines the first executable execution slice. One admitted
invocation runs from native discovery through admission, execution,
termination observation and accounting against a scripted `ExecutionBackend`.
It does not call a real provider, run a real agent or deliver a checked
result.

## Scope

The slice covers exactly one scenario end to end: discovery, admission,
invocation, termination observation and accounting. Unattended checks run
against a scripted backend adapter; no test contacts a native provider, reads
credentials or mutates user data. Real provider adapters are later tasks and
must satisfy the port defined here.

`Registry`, `Gatekeeper`, `Treasury` and `WorkspaceGuard` become kernel ports,
and `ExecutionBackend` becomes the provider-boundary port whose results are
observations. The `Dispatcher` remains the session entry point and the
`Journal` remains the only durable record: execution facts are session events
appended under the existing revision rules. The kernel alone admits work,
issues grants and records outcomes.

## ExecutionBackend port

The port exposes four operations:

- `start` an admitted invocation with its sent settings and per-invocation
  limits;
- `cancel` a started invocation;
- an event stream of execution observations while in flight;
- a receipt, the backend's final report for one invocation.

Every return value is an observation, never authority. A receipt cannot admit
another assignment, satisfy a criterion or settle acceptance; only the kernel
decides those.

Termination is typed: `completed`, `failed` with an error class, `cancelled`,
`timed out`. A termination state reports what was observed; it does not
assert a cause the error class does not state. Missing observations stay
missing and unknown metadata stays unknown.

Settings keep three distinct meanings: requested, what admission asked for;
sent, what the host passes to the backend; reported, what the backend says it
used. Sent settings are resolved during admission from the requested settings
and the offering's reported controls and are recorded in the admission
batch; `start` passes exactly those recorded settings, so nothing
unsupported or unauthorized is introduced after validation. The
`ExecutionProfile` retains all three; none overwrites another. The host
never invents a setting the native environment did not report.

## Registry

`Registry` resolves native identities through explicit discovery. A scan
supplies the actual agent identities, `ModelOffering`s and supported controls
as the native environment reports them. `Registry` keeps snapshots of what a
scan returned; it maintains no hand-curated model table and invents no
offerings. Authentication stays in the native environment; no credential is
read, copied or stored. Reading the pool or painting a surface never triggers
discovery; only an explicit scan does.

A readiness probe decides whether a discovered adapter can serve invocations
now. An unready adapter is excluded from the `Pool` with a typed reason;
exclusion is a fact about serving, not a denial of identity. The `Pool`
lists eligible agents with their exclusion reasons.

## Gatekeeper

`Gatekeeper` validates and admits assignments. Admission requires, checked
atomically against current state at commit:

- eligibility: the agent is in the `Pool` and not excluded;
- supported settings: every requested setting is supported by the offering's
  reported controls;
- independence: the agent holds no other live assignment and the role is not
  already active in the session;
- resources: `Treasury` can hold the requested reservation;
- enforceable workspace access: `WorkspaceGuard` confirms access the backend
  can actually enforce.

The final check runs at the session's current revision and commits as one
journal batch: assignment, grant, allowance, reservation and resolved sent
settings appear together or not at all. A competing commit makes the loser
stale with no partial effects. A committed admission produces an
`Assignment` carrying one role, a bounded allowance and a `Grant`. The role
exists only inside that assignment; it confers no standing before or after
it. Denials are typed: ineligible agent, unsupported settings, assignment
not independent, resources unavailable, workspace access not enforceable,
stale revision. A denial is returned to the caller without a journal
append; no revision advances, and no grant or reservation exists.

## Treasury

`Treasury` accounts for every invocation, including failed, cancelled and
coordination work. A reservation is held while the invocation is in flight
and settled at the termination observation; settlement is a journal append
whose failure returns a typed error, keeps the reservation held and is
retried, so accounting is never silently dropped. Usage the provider does
not report is recorded as unknown, never as zero.

The bounded allowance carries per-invocation limits: turns, output size and
wall-clock duration. Where the provider cannot enforce a limit, the host
enforces it: no further turns are issued past the turn bound, output past
the size bound is refused at the boundary the host controls, and expired
wall-clock stops host-side issuance without asserting the process stopped;
`timed out` is recorded only when a termination observation confirms the
expiry.
Reservations protect concurrent admission and verification capacity; they do
not promise native limits the provider cannot enforce.

## WorkspaceGuard

`WorkspaceGuard` coordinates effective access. What governs is the access
the backend can enforce, not paths a provider declares; a declared path list
is not an isolation guarantee. In this slice the scripted backend states its
effective access honestly, and that statement is test fidelity, not evidence
about real providers. Access to one workspace scope is exclusive: capturing
or verifying a result requires the hold, and the hold belongs to one
invocation at a time.

Cancellation does not prove termination; neither does a start error, a lost
observation stream or an expired wall-clock bound. The recorded invocation
states are `admitted`, `started`, `cancelling`, `terminated` and
`uncertain`. Termination requires a termination observation; once the
bounded wait deadline passes without one, the kernel records `uncertain`.
Revoking a grant asserts nothing about whether a process stopped writing.
A conflicting successor waits: its admission is refused until the
predecessor's termination or effect evidence exists.

## The admitted scenario

1. Discovery. An explicit scan finds the scripted provider. `Registry`
   records its agent identity and one offering with its supported controls;
   the readiness probe admits it. A scan that returns nothing leaves the
   `Pool` empty with that typed reason, and any admission fails as
   ineligible.
2. Request. The kernel receives an assignment request from the session
   caller: agent, role, requested settings, allowance, the workspace areas
   with the required operations in each, and the reservation size with its
   purpose. `Treasury` and `WorkspaceGuard` check exactly these request
   values; neither derives its inputs from other state. An invalid or
   incomplete request is refused with a typed validation error before any
   check or journal append.
3. Admission. `Gatekeeper` validates the five requirements at the current
   revision. Any typed denial is returned to the caller without a journal
   append; state, revision, grants and reservations are unchanged.
4. Commit. Assignment, grant, allowance, reservation and resolved sent
   settings are appended as one batch; the `Invocation` exists as
   `admitted`. A concurrent admission at the same expected revision loses
   and is stale. An `IndeterminateCommit` is resolved by the durable
   journal contract's rule — a stored exact match of the batch is
   committed, a retry happens only while the history still ends at the
   original expected revision, and any other advancement is stale — and
   `start` is forbidden until the batch's presence is exactly established.
   Any other typed journal failure fails admission with no effects.
5. Start. The host calls `start` with the settings recorded in the
   admission batch; the invocation becomes `started`. A typed start failure
   that confirms the invocation never started skips directly to accounting
   as `failed` with its error class. A start error with an unknown outcome
   is not a confirmed failure: the invocation is `uncertain`, retains the
   reservation and workspace hold, and admits no conflicting successor.
6. Observation. The event stream and receipt deliver one termination state;
   reported settings are recorded beside requested and sent. A lost event
   stream is not a termination: the invocation waits for its receipt until
   the bounded deadline, and a receipt that never arrives leaves it
   `uncertain` after that deadline with reservation and hold intact.
   Host-enforced expiry follows the same rule: without a termination
   observation it records `uncertain`, not `timed out`.
7. Cancellation, when requested. `cancel` moves the invocation to
   `cancelling`. A termination observation within the bounded wait moves it
   to `terminated`; when the deadline passes without one, the kernel must
   record `uncertain`, holding the reservation and workspace hold.
8. Successor. A conflicting successor is refused while the predecessor is
   `cancelling` or `uncertain` and is admitted only after termination or
   effect evidence.
9. Accounting. `Treasury` settles the reservation against observed usage,
   counting the invocation, its failures and the coordination around it;
   unknown usage stays unknown, and a failed settlement append is retried
   as the `Treasury` rules define.

## Acceptance criteria

The follow-up implementation task demonstrates, through the kernel against
scripted adapters:

1. Scripted backend lifecycle: one invocation passes start, observation and each typed termination — completed, failed with error class, cancelled and timed out — while the `ExecutionProfile` retains requested, sent and reported settings separately.
2. Typed admission denials: ineligible agent, unsupported settings, assignment not independent, resources unavailable, workspace access not enforceable and stale revision each return their own failure, append nothing and leave journal, revision, grants and reservations unchanged.
3. Atomic admission: assignment, grant, allowance, reservation and sent settings land as one batch at one revision, `start` passes exactly the recorded sent settings, of two admissions at one expected revision exactly one commits with the other stale and no grant or reservation, and an indeterminate commit is resolved by the durable-journal rule before any `start`.
4. Start failure branches: a confirmed never-started failure is accounted as `failed`, while a start error with unknown outcome becomes `uncertain` with reservation and workspace hold intact and the conflicting successor refused.
5. Host-enforced limits: no turns are issued past the turn bound, output past the size bound is refused, and expired wall-clock, a lost observation stream or a lost receipt without a termination observation records `uncertain` after the bounded deadline.
6. Bounded cancellation: once the bounded wait deadline passes without a termination observation the state is recorded as `uncertain` with reservation and workspace hold intact, and the conflicting successor is refused until termination or effect evidence.
7. Accounting totals include failed invocations: completed, failed, cancelled and coordination work all appear in the session totals, unreported usage is accounted as unknown, not zero, and a failed settlement append returns a typed error and is retried with the reservation still held.
8. No real provider execution: every check runs against scripted adapters, discovery is simulated by the scripted scan, and no check accesses the real native environment, credentials, network or user data.

## Honest limits

This contract decides the trusted shape of one bounded execution. It does
not decide real adapter protocols or their failure modes, cost normalization
across providers, durable execution records beyond the session journal, or
any UI. An implementation must state in its own evidence which resource and
workspace guarantees it enforces and which remain unknown. The workspace
rules here rest on a scripted backend's honest statement, not on a real
provider's behavior. Accounting records what the host observes; native usage
a provider does not report remains unknown. Nothing in this slice
establishes the capability, quality or usefulness of executed work, and a
written contract is not implemented functionality.
