# Bounded allocation implementation

YMP-110 supplies the runtime selection boundary described by the approved
[team and effort policy](team-and-effort-policy.md). It does not implement the
revision board, knowledge lifecycle, concurrent task scheduler, or a new UI.

## Captured constraints and observable state

`Config.team_constraints` contains `fixed_size`, `fixed_roster`, `eligible_agents`
and `max_members` (default 4). Omitted optional fields remain adaptive. For example:

```toml
[team_constraints]
max_members = 4
fixed_size = 2
eligible_agents = ["writer", "reviewer", "researcher"]
```

A fixed size permits replacement. A fixed roster pins identities. An eligible
list limits selection without forcing participation. `limits.parallel` remains
the independent active-invocation ceiling. At least two eligible identities are
needed for independent review; fixed size 1 can schedule different eligible
identities in sequence, while a one-identity fixed roster is infeasible.
`Config.team` remains a starting preference, not an implicit roster pin.

`SessionPolicy.team_constraints` and `captured_team` preserve the initial input.
Legacy policies have no invented initial team constraints. `Store.team_state`
and `SessionTrace.team_state` expose current membership, refreshed eligibility,
revision, selected method and update time. `Session.team` preserves every captured
participant: joins append profiles in the same transaction as the membership
decision. Leaving never erases identity, assignments, usage or provenance.
`SessionAgentView::from_captured` therefore resolves current and historical names.

`reserved_final_reviewer` means an eligible identity remains available for future
independent review. It grants no standing role, tools, permissions or active
invocation. A policy may change that reservation to another eligible nonproducer.
The runtime rejects production that consumes the last reserved reviewer. It also
rejects self-review, unavailable pinned members, contradictions, ceilings, stale
membership revisions and departures with active assignments. Public admission
wrappers check current membership and the latest observed eligibility in the same
transaction as admission. `Engine.refresh_team_eligibility` refreshes local
metadata without invoking a model; normal admission performs that refresh.

## Replaceable proposal boundaries

`Engine.with_allocation_policy(Arc<dyn AllocationPolicy>)` installs joint method,
team, executor, model and effort selection. `Engine.with_resource_policy` installs
`ResourceAllocationPolicy` for timeout, native-turn and output allowances.
Each interface accepts typed immutable input and returns a proposal. Neither gets
a mutable store or a permission-grant API. The runtime validates settings against
captured pins and available model metadata, and existing native adapters retain
final native capability validation. Unknown native support is not invented.

`Engine.reconsider_allocation(session, boundary, demand)` runs and commits a
validated allocation without inference. The engine calls the same boundary for
ready work, result review, failed work, conversation and changes in availability.
No token event runs a selection model. Incoming board task proposals are bounded
inputs, alongside recent review/check/failure evidence, task complexity and risk,
ready work, current resource accounting and configuration-specific qualified
experience. Future coordination policies can call this boundary after validating
their task revisions.

`DecisionRecord.links.allocation` and `.resource_allocation` retain implementation
ID/version, inputs, proposed choices, acceptance and rationale. Implementations
are captured on injection. `Store.allocation_decisions` returns typed selection
history. Chosen settings, membership and the decision commit together; assignment
and invocation records retain actual requested/sent/reported settings separately.
Resource proposals cannot enlarge captured ceilings or reset the admission ledger.

## Built-in behavior and limits

The built-ins are `ymp.bounded-allocation` version 1 and `ymp.bounded-resources`
version 1. They are explicit heuristics, not claims of calibrated or optimal
allocation. Ordinary startup requests one plan; a request classified as complex
may request two, bounded by active and startup allowances. Complexity at startup
uses an explicit `complex` marker or more than 1,000 prompt bytes; later work uses
its declared task difficulty. Risk detection conservatively recognizes security,
credentials, payments, migrations, deletion, publication and production terms.
Elevated risk or complex work can select a decomposition method and a third
member when useful work and resources justify it. The chosen method is supplied
in assignment context. The initial decision records the captured resource limits.

Default native turn proposals are at most 4 for planning, 8 for ordinary work,
and 16 for complex or elevated-risk work, always capped by session controls.
Output and timeout use captured ceilings. These are requested native controls;
adapter support and opaque native loops retain the limitations recorded by the
budget and provider subsystems. A historical session without resource fields gets
an explicitly recorded bounded proposal using these defaults; its historical
spend and original invocation/time limits remain unchanged.

Selection ranks exact agent/model/effort configurations by confirmation-qualified
experience, retaining current participants on equal evidence. It does not sort
native effort labels or presume a universal reasoning scale. Defaults fill absent
choices; captured pins and explicit assignment rules constrain alternatives.
Failures cause reconsideration with retained evidence and spend, without automatic
effort escalation. There is no mandatory pool-wide bidding or proposal round.

Each distinct user task creates a session. Follow-up classification distinguishes
same-task steering/clarification from a distinct task, retaining the former in the
same conversation; committing task revisions belongs to YMP-112. Common explicit
location questions read stored paths/current file metadata without a model call
or repeat production. Other questions receive a bounded read-only conversation
assignment. UI integration consumes these shared DTOs through YMP-118; no TUI
source is changed by this backend outcome.

## Verification

The runtime consumer checks exercise a ten-profile pool, fixed size replacement,
fixed roster, dynamic reduction, unavailable participants, active responsibilities,
independent final review across two tasks, model-specific effort changes, captured
pins, qualified versus unconfirmed experience, failed-check reconsideration, and
two distinct resource policies. Scripted backends make native requests observable
without provider authentication or inference. They establish protocol behavior;
real-provider quality or efficiency remains unmeasured.

Observed failing controls and final command exits are retained in
[allocation verification evidence](../research/evidence/ymp110-allocation-checks.json).
