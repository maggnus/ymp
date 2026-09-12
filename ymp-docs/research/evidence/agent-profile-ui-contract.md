# Agent and team UI contract

Design contract only. No product code was written, no provider or model was exercised, and no other
report or task file was touched. Source of truth for behaviour: [intent.md](../../../intent.md),
final version, 22 clauses. Baseline: working tree at `c443ad0`; references are to that source.

## What the product is

YMP is an autonomous, self-organizing pool of agents. The user states a goal and constraints and
does not direct the internal work. For each task a session forms a team out of the eligible pool,
and the team chooses the method, the decomposition, who executes each piece, which model runs it
and which native effort level it runs at, revising all of that mid-session if it needs to. Every
agent enters equal: no provider or profile holds standing authority, and roles last only as long as
the work that created them. Agents address the whole team or one member through the shared board.
Results are independently verified before acceptance, one agent's failure must not destroy work
already accepted, and the team works inside the user's constraints and a shared budget. Decisions
and knowledge accumulate as the session runs, knowledge can be corrected or superseded by newer
verified data, and verified knowledge and reputation carry into later sessions.

Manual configuration therefore expresses a constraint or a default, not the normal workflow. The
interface must stay legible when the user configures nothing beyond the pool, must never imply a
team is assembled by hand, and must make the team's work observable and explainable without the
user intervening in it (`intent.md`, final clause).

## What the present UI already supports

- **A pool and a captured team.** `Config.agents` is the library and `Config.team` lists eligible
  ids (`config.rs:76-83`, `:196-203`); `Session.team` stores what a run captured (`model.rs:26`).
  `App::active_team` prefers the capture, and the sidebar labels it "this session" or "next run".
- **Provider is metadata, never a grouping key.** Rows read `name · model · provider` on `/agents`
  and `/team`, and `name · provider` on `/usage`, the name always first; `usage::agent_rows` keys on
  agent id, locked in by `tests.rs::two_agents_on_one_provider_are_counted_apart`.
- **Independent verification is enforced and named.** `Task::review` rejects self-acceptance
  (`model.rs:100-102`); `/tasks` shows assignee, reviewer, attempts, checks and results; `/memory`
  shows author and reviewer; `/reputation` warns that a high rate from few observations is not
  evidence.
- **Work and spend survive a turn.** Tasks, messages, usage and knowledge are stored as they
  happen, failed and cancelled invocations keep their usage, and every page is a read-only
  projection that starts nothing.
- **A shared turn budget is visible.** The header and sidebar show `turns used / limit`, and
  `/limits` explains each limit (`ui.rs:160`, `sidebar.rs:208`, `views.rs::limits`).

## What is missing

This table describes the baseline named above. What the delivered interface now does about
each row is recorded in [Status after YMP-118](#status-after-ymp-118) at the end of this
document; the table itself is left as it was written.


| Gap | Evidence |
| --- | --- |
| The session team is simply the whole enabled pool, so the interface has no vocabulary for a team the run formed, or for a member added mid-session | `engine.rs:210-214` copies `config.members()` |
| No effort control exists anywhere, and no surface shows which model or effort an agent actually ran a turn with, so the team's own choices are invisible | no `effort`/`reasoning`/`thinking` control in `ymp-core`, `ymp-providers` or the Claude bridge, only reasoning **token counts**; `/usage`, `/tasks` and the sidebar show name and provider only |
| Board messages carry a recipient, but the interface never distinguishes team-wide from addressed | `Message.recipient` (`model.rs:36`), `team_post` (`mcp.rs:116-120,171`), unused in `ymp-tui` |
| Chat and Tasks resolve names from the live configuration, not from the capture, so editing a profile rewrites how a finished session reads | `transcript.rs:142-152`, `views.rs::display_name` |
| The display name cannot be set: `/agent add` copies the id into `name`, and no rename exists | `state.rs:1865-1867` |
| Joining a team silently enables the profile and its provider while the notice reports only the join | `state.rs:1341-1360` |
| The budget denominator is read live, so a stored session is measured against today's limit rather than the one it ran under | `ui.rs:160`, `sidebar.rs:208`; `Session` stores no limit |
| A failure is reported as an event, but nothing states which accepted work it left intact | no surface relates a failed turn to the accepted tasks that survived it |
| Knowledge shows author and reviewer but not its origin session or what it superseded | `views.rs:1149-1159`; `MemoryEntry.source_session` and `.supersedes` (`model.rs:205,211`) unused |
| No vocabulary for "requested but unconfirmed" or "this backend has no such control" | absent everywhere |

## The contract

**1. The eligible pool and the session team are different objects, and both are named.** The pool
is who may be drawn on; the session team is who the run actually formed. Membership is always a
reference to an agent id.
*Acceptance:* every surface states which of the two it shows; a page about the pool never claims to
describe a finished run; an agent that worked in a session stays in its team after leaving the
pool; no surface creates membership from a provider or a model.

**2. Identity is the id; the name is a label.** The id is stable and is what attribution, tasks,
usage and reputation key on. The name is an editable label with no other meaning. Provider and
model are metadata rendered after the name, never in place of it and never as a rank.
*Acceptance:* a rename changes `name` only, leaving the id, past attribution and recorded usage
untouched; no page edits an id; an empty name falls back to the id, not to the provider; no
ordering, emphasis or default selection favours one provider over another.

**3. Agents sharing a provider or a model stay separately identified.** Two agents differing only in
instructions or in the effort they were given are two visible entries everywhere.
*Acceptance:* with two agents on one provider and one model, each surface lists both and the
distinguishing label is visible without opening a detail; nothing aggregates by provider.

**4. Effort is a provider-native value with an honest state.** A setting is one of the values the
backend actually names; no cross-provider low/medium/high ladder is invented and no value is
normalised. Whoever chose it, the team during the run or a user-set default, is part of the record.
*Acceptance:* the interface distinguishes these states and never conflates them:

| State | Meaning | Rule |
| --- | --- | --- |
| native default | nothing was requested | shown as the provider's default, not as a level |
| requested | a value was asked for | shown with the exact native value and who asked |
| applied | the backend confirmed the turn ran with it | may be stated as in effect |
| unconfirmed | requested, and no confirmation came back | marked; never stated as in effect |
| not supported | this backend exposes no such control | said plainly, never silently substituted |

A value the provider does not offer is never saved silently or carried across a provider change,
and the interface makes no claim about a setting's effect on quality or cost.

**5. Adaptation is recorded, not rewritten, and the history reads end to end.** A change of plan, a
reassignment, a raised effort or an added member is an appended fact with a time and an author, and
earlier turns keep the membership, model and effort they ran with.
*Acceptance:* a mid-session change never restates history; a completed turn's recorded model and
effort do not change when a later turn uses different ones; from the session's own pages a reader
can follow proposal, review, assignment, reassignment and result in order, without opening the
store or asking an agent.

**6. A captured session is read with what it captured, and old data still loads.** Name, provider,
model, effort and the limits a run worked under come from the session record; the live
configuration is consulted only for what the capture lacks.
*Acceptance:* renaming or deleting a profile does not change how a finished session reads; an agent
in the capture but not the library resolves to its captured name, one in neither resolves to its
id; a record with no effort field reads as native default; no stored id is rewritten.

**7. Board messages state their audience.** A message to the whole team and one addressed to a
single agent are visibly different, and the addressee is named.
*Acceptance:* an addressed message shows author and recipient; a team-wide message shows no false
recipient; neither is presented as a channel the user cannot see.

**8. Acceptance is visibly independent.** A result is accepted by an agent other than the one that
produced it, and the interface shows who verified what against which evidence.
*Acceptance:* every accepted task names its reviewer, never the assignee; a rejected attempt keeps
its reason and evidence rather than disappearing; nothing is shown as verified on the producer's
own claim.

**9. A failure is scoped, and accepted work survives it.** When an agent fails, times out or is
cancelled, the interface says what failed and what remains accepted.
*Acceptance:* accepted results, recorded usage, board history and accepted knowledge stay visible
after a failure; a failed turn is never rendered as invalidating the session; a blocked task keeps
its attempts and last evidence.

**10. The budget is shared and shown against the constraint it was spent under.** Turns and tokens
belong to the session as a whole; per-agent figures are attribution, not separate allowances.
*Acceptance:* consumption is shown against the limit the run actually worked under, not against a
limit edited afterwards; no per-provider or per-account budget is implied; unknown spend stays
unknown and no monetary figure is invented.

**11. Knowledge is provenanced and correctable.** Knowledge and reputation shown in a later session
say where they came from and what verified them, and a correction reads as a correction.
*Acceptance:* an entry names its origin session, its reviewer and the scope it applies to; an entry
that supersedes another says which one, and the superseded entry is never shown as current; a
competence figure names the observations behind it; retirement is explicit and confirmed, never a
silent deletion.

**12. A phase is an assignment, not a role.** Planning, bidding, execution, review and synthesis say
what an agent is doing in a turn. They are never identity, never a profile field, and never a badge
suggesting a permanent specialist or a lead.
*Acceptance:* activity words are bound to the current turn and disappear with it; no surface names
an agent by phase; no text implies a standing hierarchy.

## Non-goals

No large configuration dashboard, no cross-provider effort ladder, no money display, and no step
that asks the user to approve the team's internal choices. Automatic team formation and automatic
effort selection are core behaviour, not non-goals; only the comparative question of which
selection policy performs better stays outside this contract, as a separate measured experiment.

## Recommendation

Land it in this order, each step independently verifiable.

1. **Capture-faithful name resolution** in chat and tasks: a correctness fix for a promise the
   interface already makes elsewhere, and a precondition for clauses 5, 6 and 10.
2. **The decision record.** Per session, show who the team formed, what each agent ran with, when
   the team adapted, and what survived a failure. This is the observability the intent asks for,
   and it needs no new control.
3. **The native effort field** with its five states, once the backend can report which controls a
   provider exposes and whether a requested value was accepted.
4. **Provenance and audience:** origin and supersession on knowledge, recipient on board messages,
   then the rename path and disclosure of the switches joining a team already flips.

## Status after YMP-118

Recorded 12 September 2026 and revised 13 September 2026, when the accepted membership and
knowledge records arrived and the pages were coordinated with them. The contract above is
unchanged; this is what the delivered pages now do about it, and what they still do not.
Evidence is in [assignment, budget and confirmation views](../../architecture/assignment-and-confirmation-views.md)
and in the interface tests named there.

| Clause | Delivered | Still open |
| --- | --- | --- |
| 1. Pool and session team are different objects | `/team` shows the members of this session or of the next run, says which, and lists the locally eligible pool with the reason each profile is excluded. An agent that worked in a session stays in its list after its profile leaves the pool. A roster that replaced a member shows the replaced identity apart, with the turns recorded under it and the revision it is measured against; the roster record names the final reviewer it keeps free as availability, and the bounds the roster was formed under are shown beside it. | Membership cannot be changed from the interface, and nothing here proposes a roster: the pages read the decisions a run recorded. |
| 2. Identity is the id; the name is a label | Unchanged and still held: rows read the name, then the model the agent would run as, then the provider; records key on the id, and the pages show the id in the detail. A model the catalog does not name is shown as not read, not scanned or unknown, and the provider's own label is never put where a model name belongs. | Renaming is still not possible from the interface, and no page can yet name a model that no stored catalog reports. |
| 3. Agents sharing a provider stay separate | Held on every new page: assignment rows key on the agent id and name the agent, never the provider. | — |
| 4. Effort is native, with an honest state | `/assignments` shows requested, sent and reported for model, effort and permission mode, and says when a value was sent and nothing was reported. Nothing requested reads as the installation's own default. | No page claims a control is unsupported, because no record says so. Choosing an effort from the interface is still not possible. |
| 5. Adaptation is recorded, not rewritten | Assignments and decisions are appended records, shown in the order the store keeps them, each with its own time and actor. A later turn does not restate an earlier one. | Plan revisions are shown as decisions rather than as a diff between plans. |
| 6. A captured session is read with what it captured | The turn counter in the header and the sidebar uses the limit the session captured; `/limits` separates captured limits from the next run's and refuses to edit a record; `/team` reads the captured profiles. | Names in the transcript still resolve through the live configuration. |
| 7. Board messages state their audience | Not addressed here. | Unchanged from the baseline. |
| 8. Acceptance is visibly independent | `/decisions` names the reviewer an acceptance links, its grade, its evidence count, and the result version and criteria version it rests on. `/tasks` carries the grade on the row. | — |
| 9. A failure is scoped | A failed, cancelled or interrupted turn is shown as such beside the turns that completed, and an accepted task keeps its acceptance. | No page yet states, in one sentence, what a given failure left intact. |
| 10. The budget is shown against its own constraint | `/limits` shows the captured bound, what the session admitted, what it reserved for review, what it observed and any recorded stop, with unreported spend distinguished from nothing spent. | Money is still not shown, by design. |
| 11. Knowledge is provenanced and correctable | `/memory` lists every entry, including candidates and retired ones, and labels each: the confirmation its provenance records, the acceptance, result version and criteria version it projects, the applicability conditions it carries, the policy that retained it, and whether a run would actually be given it as context. An entry written before provenance was recorded says its confirmation is unknown. `/reputation` shows each observation's evidence status and what credit requires. | Retiring an entry is still the only correction available from the interface; nothing here edits an entry's text or its provenance. |
| 12. A phase is an assignment, not a role | The purpose of a turn is shown on the assignment that created it, and a grant is described as permission held by one assignment. No page names an agent by phase. | — |
