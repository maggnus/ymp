# Team strategies and the agent catalog

Owner scope, 2026-09-13: place the discussed policy selection only in `/team`.
Do not add a policy selector/default control to `/settings` or a separate
strategies page. Keep the scope to coordination, participant/model/effort
allocation and invocation-resource allocation; do not expose all existing
policies, recovery settings or generic algorithm-parameter forms.

## Distinct responsibilities

`/agents` is the catalog of available native agents and editable profile defaults.
It owns native discovery/refresh, eligibility and refusal explanations, supported
capabilities, default instructions and applicable profile configuration. It does
not define or silently mutate current-session membership. Preserve existing native
identity and historical configuration rules; changing a default never relabels
past work or changes an already-issued invocation.

`/team` owns the session's current participants, temporary responsibilities,
activity and pending departures/replacements, together with the selected
coordination/allocation/resource implementations. Show only real registered
implementations with short behavioral descriptions. Team changes use the trusted
YMP-146 command API, revision checks and active-responsibility handling.

Remove the permanently repeated full `Available on this machine` catalog from
the team page. Adding a participant opens a filtered chooser over the same catalog
data and row/detail component used by `/agents`; no second agent registry or
eligibility algorithm is introduced. A catalog shortcut may open the team flow,
but must not retain a separate preference-only membership mutation. Membership
badges, if retained in the catalog, read actual session state and name their scope.

Without a selected session, `/team` clearly edits the next session's draft
configuration. With a selected session, actions address that session rather than
silently changing defaults for a future run. Preserve current versus historical
membership and explicit continuation semantics for completed or paused sessions.

## Strategy scope and persistence

Strategy configuration is versioned on the session even though the control is
located on `/team`. Coordination governs phases, temporary roles and information
sharing; AllocationPolicy chooses compatible participants/settings, while
ResourceAllocationPolicy proposes bounded invocation resources. Actual resource
ceilings and independent acceptance remain runtime-owned constraints.

Strategy changes apply at a validated decision boundary. Show pending application
where necessary, retain active assignments, and bind subsequent decisions to the
effective policy revision. Historical strategy IDs/versions and outcomes must not
be rewritten. The experiment runner selects the same implementations directly;
the GUI location does not define a second execution mechanism.

There is no complete replaceable CoordinationPolicy in current main. Existing
BoardProposalPolicy only selects pending proposals. Implement and verify the
coordination consumer before presenting a selector as working functionality.
No public selector is needed for provider, confirmation, knowledge or workspace
policies in this scope. Recovery remains standard behavior, not a new user mode.

## Current source evidence and delivery

At main before this specification, `views::team` includes current/historical
members and repeats the complete pool. `views::agents` also exposes a team flag;
Agents `t`, Team `Space` and `/team add|remove` all call `toggle_membership`, which
changes `Config.team` starting preferences. The source explains the perceived
duplication and the misleading current-session action semantics.

Implement this as a separate follow-up after the P0 recovery/backend command
slice. Delegate UI to Claude Code `claude-opus-5 high`, reuse the shared popup/list
and catalog components, and independently verify selected-session versus next-run
behavior, active replacement, actual strategy effects, historical attribution,
small terminals and all required checks. Do not expand or delay the existing
YMP-145 popup correction or YMP-146 recovery fork.
