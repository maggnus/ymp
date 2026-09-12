# Useful concurrency in the direct MVP workspace

YMP-115 coordinates a bounded wave of independent ready tasks through the public
`Engine` and the existing native/backend execution path. It does not implement
isolated candidates, copying, merging or recoverable publication. The selected
working directory remains the actual location of file outputs. Application state
contains metadata and evidence. Production workspace isolation belongs to YMP-124.

## Explicit task authority and replaceable access decisions

`PlanTask.access` and `Task.access` are `read_only` or `write`. Omitted legacy values
mean `write`. Access is independent of competence: analysis and synthesis can
produce files when their task requests write authority. Read-only contributions
return their findings in the recorded response. The planning prompt explains this
distinction and independent plan review judges the declared choice. Plan commitment
checks the exact access value, and `TaskDefinition.access` binds it into the
candidate reviewed at acceptance. Legacy write defaults remain omitted when
serialized so historical plan/result digests do not gain an invented field.

The engine sends the actual choice as `TurnRequest.read_only` and captures the
corresponding requested permission mode in the assignment and invocation. The
public storage admission boundary rejects a write invocation for a read-only task.
A workspace policy cannot enlarge that task's native authority.

`ExecutionBackend::workspace_access(&TurnRequest)` reports the access that the
trusted compiled backend actually enforces. The default for third-party compiled
backends is `WriteAll`. Native Codex/Claude read-only requests and the built-in
offline mock use `ReadAll`; unrestricted native writes use `WriteAll`. ACP remains
`WriteAll` even for a requested read-only mode: a mode name alone is not a reliable
filesystem restriction. Existing native permission observations retain that limit.

`Engine.with_workspace_access_policy(Arc<dyn WorkspaceAccessPolicy>)` installs a
policy that can retain or broaden the backend's resource claim, or reject it. The
runtime rejects claims narrower than actual backend access. The initial policy
is `ymp.direct-mvp`, version 1. No planner-supplied path is treated as enforcement.
A scoped compiled backend may report explicit read/write paths only when its
operations enforce them. Relative path validation rejects traversal, symlink
aliases and hard-linked files. The scripted acceptance backend has no shell or
general filesystem tool; its only I/O uses literal paths from that same access
contract. This stronger fixture capability is not a native sandbox claim.

## Coordination, acceptance and recovery

The process coordinator atomically claims agent occupancy, active capacity and
effective resources before budget admission. A blocked selection has no native
invocation, token reservation or live grant. Existing storage admission still owns
atomic budget reservation, team membership checks and permission grants. The
existing project lock prevents concurrent team runs in another process; follow-up
native work uses that same project lock. Separate selected roots and external
programs are not contained by an OS sandbox.

Readers can overlap readers. Scoped independent accesses can overlap. A writer
excludes every reader/writer of its resource; an unrestricted writer owns the whole
directory. Nested selected roots are coordinated conservatively. A lease stays
held through production's artifact snapshot. Verification takes exclusive access
through snapshotting, deterministic checks, native inspection and acceptance,
because arbitrary check commands may write. Result completion never grants
acceptance. Existing exact artifact/input/verifier freshness checks remain active.

Each completed task reaches independent review even when a sibling has already
failed. Failure and cancellation close invocations, retain observed usage and end
grants; dropping a lease releases ownership. Restart ends historical resource
ownership, preserves accepted siblings, and issues fresh inspection for admitted
uncertain work. Selections cancelled before native admission return to ready work
without inventing effects to inspect or erasing their recorded attempts. Native
continuation is loaded after resource/agent admission, with its existing backend,
settings and usage-baseline compatibility checks.

The scheduler admits bounded waves up to the captured parallel ceiling.
The updated heuristic is recorded as `ymp.bounded-allocation`, version 2. Ordinary
independent ready work can justify concurrent producers plus an eligible independent
reviewer, within member and budget limits. Allocation input includes active and
committed agent identities, and runtime/storage validation prevents removing them.
The membership target includes a distinct selected reviewer alongside those retained
actors, so two failed producers do not displace an otherwise feasible sibling review.
Actual member ceilings and roster/size pins still apply.
This sizing rule is an explicit bounded heuristic, not an optimality claim. The
next wave waits for the current wave's work and reviews; it is not a continuously
replenished queue. No bidding or filler invocation counts as useful overlap.

## Observable records

`RecordLinks.workspace_access` carries `WorkspaceAccessDecision`:

- `reservation_id`, policy and backend ID/version;
- canonical actual `directory`;
- `backend_access`, `effective_access`, and the rationale describing direct MVP
  execution without isolation or rollback.

Decision kinds are `workspace_access_acquired`, `workspace_access_admitted`, and
`workspace_access_released`. Only the admitted record links an actual assignment
and invocation. Acquired/released reservation IDs also identify runtime verification
ownership. Releases explicitly do not mean acceptance. Restart records release of
historical acquisitions that have no terminal record.

`assignment_waiting` carries `RecordLinks.workspace_wait`: stable `code`, optional
`holder` reservation ID, and readable `detail`, with the proposed effective access
when available. Codes include `resource_conflict`, `agent_busy`, `concurrency_limit`
and `dependencies`. Waiting native agents also publish an existing `AgentStatus`
update. These shared records are the UI/MCP integration surface; no new UI behavior
is delivered by this backend outcome.

## Offline verification and limits

`ymp-runtime/tests/concurrency.rs` walks public `Engine::run` with causal barriers:
ordinary default-policy read-only production overlap through actual built-in offline
native execution; scoped writer/independent-reader overlap; conflicting native and
scoped writer serialization; confirmed file-producing analysis alongside failed
synthesis; cancellation while waiting; restart inspection without uncertain replay;
and rejection of unsupported policy guarantees or enlarged task authority.
The retained R1 regression uses three simultaneous producers under default
allocation: two fail before the third finishes, and the third still reaches exact-byte
confirmed acceptance through a distinct reviewer while failure usage and unresolved
responsibilities remain recorded. The same review input still rejects a member
ceiling or fixed size of two. Correction commands and observed failure/pass outputs
are in [R1 correction evidence](../research/evidence/ymp115-r1-correction-checks.json).

The suite also retains existing confirmation freshness, attribution, native
continuation, provider authority and atomic budget tests. Negative controls remove
useful scheduling width, resource exclusion, sibling review, scope validation,
read-only admission protection and exact reviewed access validation. Additional
controls retain a stale hard-exit resource record and make the offline native
read-only path write a file.
Each fails its targeted consumer check. Commands and real outputs are retained in
[concurrency evidence](../research/evidence/ymp115-concurrency-checks.json).
No real-provider inference, credentials, quality comparison or native narrow-path
sandbox validation is part of this outcome.
