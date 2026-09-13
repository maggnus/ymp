# Backend integration API

This is the implemented Rust API for the later UI consumer. It defines no UI
placement, policy registry, public provider tool, or generic policy settings.
The authoritative product contract remains
`ymp-docs/architecture/session-recovery-contract.md`.

## Local owner read and command boundary

- `Engine::team_control(session_id) -> Result<TeamControlView>` returns the
  effective `TeamState`, desired membership during draining, pending departures,
  current owner constraints, immutable startup policy, verified native candidates,
  active invocation/task/board/access responsibilities, the existing versioned
  `BoardSnapshot`, invocation failures, durable recovery stages and permitted
  owner/stage action families. Reading uses local metadata; it launches no scan
  or model. Views are optimistic: refresh after a stale command response.
- `Engine::owner_team_command(&OwnerTeamCommand) -> Result<OwnerTeamReceipt>`
  is the trusted local add/remove/replace/pause/continue/wait API. Commands bind
  `session_id`, `expected_revision` (the view's `TeamState` revision) and a durable
  `command_id`. A reused ID with different content is rejected. An identical
  retry returns the original receipt even if eligibility has subsequently changed.
- `Engine::recovery_stages(session_id)` exposes the same `RecoveryStage` records
  independently. `Engine::control_recovery(&RecoveryControlCommand)` accepts
  retry/continue/wait/pause for a specific stage, using its own expected revision
  and durable command ID. `RecoveryStage::manual_actions()` and
  `TeamControlView.recovery_actions` describe available controls. Current native
  work must drain before retry. Uncertain effects cannot be waived by this API.
- `Engine::run(path, prompt, Some(session_id))` continues the existing session.
  It consumes the stored plan/review/result versions. Explicit owner pauses and
  waiting controls remain in force; a plain run does not erase them. After an
  accepted continuation command, the local application can invoke this existing
  run API. Concurrent runs still require existing exclusive session/project locks.

Example local replacement (the caller selects exact IDs from the native candidates):

```rust
let view = engine.team_control(&session_id)?;
let receipt = engine.owner_team_command(&OwnerTeamCommand {
    session_id: session_id.clone(),
    expected_revision: view.revision,
    command_id: new_id(), // Retain this exact command for an idempotent retry.
    revise_pinned_roster: true, // An explicit owner amendment, when needed.
    action: OwnerTeamAction::Replace {
        agent_id: departing_id,
        replacement_id: selected_native_id,
    },
})?;
```

A provider cannot assert owner identity. The private team MCP dispatcher has no
route to either owner API; the socket test sends forged owner flags and command
names and verifies rejection with unchanged membership.

## Membership and admission semantics

`OwnerTeamState.policy_revision` increases on each explicit accepted owner
command. `TeamState.revision` also changes at automatic allocation, eligibility
and pending-departure completion boundaries. Stale commands and stale allocation
commitments fail before partial changes. Current `Engine` instances read accepted
owner state at selection, workspace, checks and native admission boundaries.
The joint invocation/grant/budget transaction rejects fresh admission to excluded
or departing members and while the owner has paused/waited.

A busy member retains its effective slot, invocation, attribution and access
until its responsibility ends. A replacement is desired first and effective only
when the old responsibility drains. Result publication and workspace release are
both considered, so closing native accounting alone does not discard a live
writer. A not-yet-admitted task selection can be withdrawn atomically, with its
attempt history retained; it produced no native effects. Existing ready board
commitments remain pending until their existing responsibility/reassignment
contract releases or transfers them. No transfer of live write ownership occurs
by editing membership. An addition that would exceed effective occupied slots is
rejected even if a departure is already desired.

Explicitly removed identities remain excluded from new automatic selection until
the owner selects them again. Fresh automatic allocation may still adapt ordinary
current membership within the owner's constraints; current membership is not
silently converted into a pinned roster. Pinned membership amendments require
`revise_pinned_roster`; fixed size, membership ceiling and explicit eligibility
remain separate and unchanged. The initial eligible-pool capture is evidence,
not an immutable allowlist. Owner amendments never change execution pins,
acceptance contracts, resource limits, spend or historical identities.

Chosen IDs must resolve to eligible native metadata (`Native`/retained `Stale`, or
`Local` for offline mocks). Editable captions and unresolved names cannot provide
native identity. Accepted native model profiles and their scan bindings let an
already running engine observe models absent from its initial agent list, while
preserving the enabled provider fingerprint. A newer native snapshot is not
replaced by an older accepted snapshot. New executable/provider definitions are
not hot-loaded: an engine without that enabled matching provider binding exposes
unavailability until configured execution is available.

## Recovery stages, policies and provenance

`RecoveryStage` schema 1 uses a stable obligation ID plus an optimistic revision.
Planning/revision, plan review, candidate review, dispute arbitration and final
review have actual durable consumers. A saved response names its originating
assignment and invocation. Completed messages are recovered only through their
recorded invocation origin. Malformed output is separate from an independently
negative verdict. Objections remain linked through plan revisions and dispute
handling; they cannot be erased by selecting a fresh reviewer.

`RecoveryAdmission` binds an assignment to the stage revision before the joint
admission transaction. This additive metadata uses existing KV storage and does
not extend UI enums or duplicate UI DTOs. The transaction rechecks the binding
before it creates invocation/grant/resource records. Stage transitions record
and validate proposal input, result identity, effective team revision and owner
policy revision atomically. Existing execution-task interruption continues to use
its task/board/result version and inspection contract; the read model exposes
that state and all invocation failures alongside the new pre-task stages.

`RecoveryPolicy` is the only new interchangeable policy. It has immutable typed
`RecoveryInput`, captured implementation ID/version, actual configuration, and a
bounded `RecoveryAction` proposal. It receives no Store or provider invocation
capability. `BoundedRecoveryPolicy(RecoveryConfiguration)` genuinely uses only
`max_attempts`, `max_provider_failures` and `delay_ms` (defaults 2, 2, 250 ms).
Injected policies still face runtime ceilings (8 recovery actions per stage,
no same-provider retry at 8 recorded failures, maximum 30 seconds per delay),
cancellation, actual-access safety and ordinary resource admission. Native output
truncation also retains the pre-existing one-retry maximum within captured
attempt limits. Provider failures are counted across the session, not reset for
each agent. Manual continuation permits one admitted attempt without resetting
those counters or any budget.

Runtime validates a proposal before effects. Retry timers are cancellable. An
independent replacement is chosen through `AllocationPolicy`, while resources
remain with `ResourceAllocationPolicy` and existing admission. Missing independent
review is a waiting condition. Saved useful work remains unaccepted until the
existing acceptance and confirmation checks pass. Identical aggregate versions
survive final-review interruption, and repeated resume cannot consume the same
final acceptance twice. Already admitted waves drain; unrelated ready work can
continue through the usual dependency/access/reviewer/resource checks while a
review obligation waits.

`RecordLinks.recovery` records full recovery input and proposal validation.
`RecordLinks.failure` retains structured classification, safe native code,
originating invocation, actual effective access and termination evidence.
Known native protocol codes are preserved by the Codex/RPC adapters and capability
redaction; a generic connection message remains `Unknown`.
`RecordLinks.policy_chain` preserves composed recovery/allocation and
allocation/board provenance. Parameterless policies need no invented settings;
absent configuration is omitted. Legacy records retain absent fields rather than
acquiring fictitious historical strategies, settings or confirmations.

## Concrete limits

This implementation does not prove remote effects have stopped after an
unobserved process loss. Unknown termination or failed write-capable work can
remain in a concrete inspection/owner-action condition. No rollback, source
isolation, detached-process recovery or generic workflow orchestration is claimed.
The existing interruption-review path for execution tasks is retained. The owner
pause drains current work; immediate cancellation continues to use the existing
engine cancellation token and native/access cleanup rules.

There is no policy-comparison marketplace, all-policy configuration schema,
strategy selector UI, changed provider authentication, new dependency, installed
release or live-session recovery. Native/provider/platform behavior beyond the
offline scripted tests remains unverified. Parent owns independent acceptance and
later UI sequencing.


Legacy plan-review invocations without stage binding enter an explicit owner-action
condition. Their prior terminal/access records are retained; missing binding does
not authorize a new write-capable call or turn an unbound message into acceptance.
The runtime counts legacy failed invocations in the shared provider restriction.
A single remaining independent nonproducer can complete a saved review; a new
producer still needs independent review eligibility. Parameterless recovery test
strategies inherit absent configuration rather than invented settings.
