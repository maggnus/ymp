# YMP-146: recovery and owner-controlled team changes

Status: authorized backend implementation contract, 2026-09-13. UI placement of
strategy selection remains open. This does not amend intent.md or authorize
changing resource limits, replaying the owner's live session, or implementing an
editor. Parent owns this contract and final acceptance.

## Outcome and delivery sequence

Continue a session from its saved unfinished stage after agent failures, and
provide typed owner commands to add, remove and replace session participants
while work is active. Preserve completed work, provenance, objections, actual
usage, user pauses and independent acceptance. Implement backend behavior and
interfaces first; Claude Code owns the later UI consumer. Deliver coherent local
commits for recovery and owner control, with focused evidence for each.

Use the current implementation's allocation, resources, board, access, execution,
storage and confirmation contracts. Add only the missing recovery replacement
point. Full workflow orchestration, model-assisted strategy execution and a
strategy-selection UI are not required by this backend assignment. Do not claim
those broader algorithms are interchangeable merely because recovery is.

## Recovery policy and durable state

A narrow typed `RecoveryPolicy` receives a versioned snapshot of the interrupted
stage, failure classification, saved result and reviews, previous attempts,
participant/provider availability, uncertain effects, outstanding responsibility,
team-policy revision and resource admission state. It returns a bounded proposal:
resume the pending review, inspect uncertain effects, retry, reassign, wait for a
condition, request owner action or stop. Exact public type names may be chosen
within this semantic contract and documented for the UI consumer.

The runtime validates and records the proposal before effects. Selection of a
replacement executor stays with `AllocationPolicy`; actual invocation allowance
and admission stay with existing resource interfaces. Strategies receive no
mutable Store, direct permission grant or unaccounted provider-call capability.
Keep decision input/version, strategy ID/version/configuration, proposal,
validation result and originating records. New fields must read legacy records
honestly. For composed decisions, preserve the originating policy chain instead
of overwriting it with only the last policy's identity; make only the directly
needed additive provenance correction.

The built-in recovery algorithm is finite and configurable: bounded delays,
attempt counts and shared-provider retry accounting within existing session
allowances. State the chosen defaults in evidence. A distinct manual-wait test
implementation must change actual behavior through the same consumer while
preserving runtime invariants. Reuse existing allocation substitution tests.
No strategy-comparison marketplace, dynamic ABI or new dependency is needed.

Persist stage/result versions so process restart, cancellation or failure cannot
turn a saved plan proposal into an implicit instruction to plan again. Reuse
existing provenance records as appropriate; do not infer acceptance from a
message or manufacture missing evidence. Handle each unresolved stage explicitly
and retain existing interrupted-execution review. Stage storage must support
atomic transition checks, repeated resume and stale-result rejection.

## Failure and continuation rules

- A failed plan review retains the exact proposal and resumes its review. A
  negative review retains objections and requests revision or existing dispute
  handling. Malformed output is a separate bounded failure, not a negative or
  positive verdict. Do not shop reviewers until one approves.
- Distinguish known transient transport failures, authentication, exhausted
  quota, unsupported configuration, timeout/cancellation, malformed response and
  unknown failure. Preserve available structured native evidence with normal
  redaction. Do not pretend a generic connection error proves its cause.
- Repeated failures from a common provider share recovery restrictions; N failed
  agents do not each receive fresh unlimited retry allowances. Appropriate
  replacements must meet current capability and independent-review constraints.
- Drain already admitted work and preserve accepted neighboring results. Admit
  unrelated ready work only where dependencies, access and reserved verification
  resources permit. A missing required reviewer creates a truthful waiting or
  owner-action condition, never self-acceptance.
- Establish termination of prior execution and inspect uncertain effects before
  replaying side-effecting work. Requested read-only mode cannot override an
  actually write-capable backend. If safe continuation cannot be established,
  preserve a concrete waiting/owner-action reason. No rollback or source-isolation
  guarantee is introduced.
- Preserve owner pauses and cancellation across restart. Timers are cancellable
  and do not spin when no admissible action exists. Resuming or replacing an
  agent never resets consumed resources or unknown usage.

## Owner commands and live team changes

Expose a typed read model containing current members, pending changes, eligible
native candidates, active responsibilities, policy revision, saved recovery
stage, reason/condition and permitted manual actions. Expose typed commands with
session ID, expected revision and a durable command ID for idempotent retry.
Provide add/remove/replace and explicit continuation/retry/wait/pause controls
needed by the later UI. Reject or report stale commands without partial changes.

Owner commands enter through the trusted local application API. Do not add a
provider-facing tool that can assert owner identity or amend its own authority.
Resolve chosen agent IDs against verified native metadata and applicable owner
constraints; do not accept invented profiles or display aliases as native IDs.

Additions and removal of idle members can take effect at a validated boundary.
By default, removing or replacing a busy member marks a pending departure:
already issued work can finish, its result remains attributable to that member,
and no new work is assigned to it. Apply the final membership change once the
responsibility is released or explicitly transferred. An explicit cancellation
uses the existing cancellation/access rules and inspects uncertain effects;
changing a list entry must not prematurely release a live writer.

Represent requested and effective state so a pending departure is visible. Check
the effective owner policy at new admission boundaries, even for an already
running Engine. A stale allocation cannot undo an accepted owner command or
assign fresh work to a departing participant. Handle concurrent completion,
automatic recovery and owner replacement with revision/transaction checks.

The owner may explicitly revise pinned membership in the same session. Record
that decision as a new team-policy revision; preserve the immutable startup
capture and historical attribution. Automatic policies obey the current explicit
owner constraints. Fixed size, pinned roster, eligibility restriction and current
membership remain distinct. Validate inconsistent commands atomically. A valid
owner choice that temporarily leaves no admissible independent reviewer may
produce a waiting state, but must not invent review feasibility or acceptance.

Initial `SessionPolicy.eligible_pool` is evidence, not itself an enforced immutable
allowlist: current eligibility comes from native metadata/configuration under
explicit constraints. Preserve this distinction when accepting newly selected
native participants. Team changes do not amend token/time/invocation limits,
change task acceptance criteria or create new historical agent identities.

Strategy IDs/configuration must remain attributable to their decisions. The
future placement of a strategy selector on a team screen is open and must not
be settled by a backend placeholder UI.

## Acceptance and verification

Use public runtime consumers with injected/scripted providers and temporary
application homes. Do not invoke installed real providers, modify ~/.ymp2, or
resume session 3223c6a9. Establish meaningful failing-before controls, then run
focused tests and one required final sequence: cargo fmt --all --check,
cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace.
Do not rerun successful chains without a code change or unresolved concern.

Cover at least:

1. Saved plan review fails, resumes after restart, and invokes no duplicate
   planner; negative and malformed reviews remain distinguishable.
2. Bounded transient recovery and a different injected manual-wait policy cause
   different observable calls/statuses while obeying the same limits.
3. Correlated failures and an unavailable reviewer retain useful results, expose
   a finite waiting condition and never fabricate independent acceptance.
4. Interrupted write uncertainty, explicit pause and exhausted/unknown-accounting
   cases preserve access and resource invariants before any repeat.
5. Owner add, idle remove and active replace work in the same session, with
   pending departure, preserved result attribution and no new assignment to the
   departing agent. Stale/idempotent requests and completion races are exercised.
6. An explicit owner change revises a pinned roster, while an agent-originated
   attempt cannot. Historical startup policy, usage and earlier conclusions
   remain unchanged; no-reviewer choices stay unaccepted.

Retain evidence under ymp-docs/evidence/ymp-146/backend/. Report unverified
platform behavior, unsupported recovery cases and any contract disagreement
explicitly. No UI completion, installed release or comparative quality claim is
implied by backend tests.

## Assignment write boundary

Backend author may change ymp-core, ymp-runtime, ymp-storage and narrowly required
ymp-providers failure classification, their tests, and the assigned evidence
directory. Purely additive fixture adaptations to new public fields elsewhere
are allowed and must be listed. Do not edit ymp-tui, Git/navigation/highlighting,
task metadata, this contract, intent.md, release files, Cargo dependencies,
installed binaries or unrelated request documents. Parent will sequence the UI
and independent acceptance after the actual typed API is available.
