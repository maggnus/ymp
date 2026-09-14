# Owner messages to session participants

Owner decision, 2026-09-14: support `@agent` addressing in the session composer.
Addressing selects a concrete session participant, not a model name. This extends
the existing durable shared chat; it does not create private communication,
assign work, interrupt a native call or grant authority.

## Identity and disambiguation

The stored recipient is the full stable `agent_id`. Model/provider/display text
is presentation metadata and never replaces that ID. Typing `@` opens a chooser
from the selected session's effective participants. Each row shows the exact
captured model label, current assignment role or state when known, and a short
stable ID suffix, for example:

```text
gpt-5.6-luna · implementation · 8da25f
gpt-5.6-luna · review         · c17b42
```

The inserted mention resolves to one full ID before the message is stored. A
short prefix must be unique in that session; collisions reopen the chooser or
are rejected. Do not use mutable ordinals such as `luna-1`: membership changes
must not redirect an old mention.

`@gpt-5.6-luna` may resolve directly only when exactly one effective participant
matches that exact model identifier. With two or more matches, require a chooser;
never broadcast or choose the first silently. `@team` explicitly selects no
recipient and sends to the shared team chat.

## Delivery semantics

Addressed messages remain visible to every team member and are rendered as
`You -> recipient`. The recipient field marks intended attention. The message
becomes context at an existing turn/assignment boundary; sending it alone does
not start a native invocation or promise a reply. The UI states this when the
target has no active or pending work.

Validate the recipient against the selected session's effective team when the
owner submits the message. Bind the action to session and team revision so a
stale chooser cannot redirect it after membership changes. If the participant
leaves before delivery, retain the original addressed history and report that no
future delivery is scheduled; do not rewrite it to another participant.

The owner path is a trusted local action. Provider tools cannot impersonate the
owner. Existing agent `team_post` keeps its own authority checks and optional
recipient field. All participants may read the shared history; the UI must not
describe an address as confidential.

## Interface and acceptance

Mention completion uses the shared list-popup component and preserves normal
slash-command completion. Keyboard selection inserts a structured mention token
without losing the surrounding draft. Inspect shows the full recipient ID and
captured label; the primary transcript keeps the concise disambiguated label.

Tests cover one matching model, three copies of one model, short-ID collision,
model change under one stable ID, membership revision after opening the chooser,
recipient departure, `@team`, message persistence/restart, shared visibility,
no automatic invocation and no owner impersonation through team MCP. Message
chronology follows the YMP-156 sequence/notice corrections.
