# Assignment, budget and confirmation views

What the interface shows of the records a run writes, in the words it is allowed to use.
Every page named here is a read of data that already exists. None of them starts a turn,
asks a provider anything, reads a credential or writes to the working directory.

## Where the records come from, and when they are read

The controller takes one snapshot per session and one per machine, and the pages present
those snapshots. A page never reads the store, the filesystem or `PATH` while it is being
painted, because painting happens on every keystroke and a read that can fail or block does
not belong there. Each page states when its records were read.

| Snapshot | Read through | Taken when |
| --- | --- | --- |
| Session records | `Store::trace`, which now also carries the session's roster, then `Store::team_grant` for each assignment's grants, `Store::result_is_current` for each decision that carries a result, and `Store::outcomes` for the accepted results and the directory each was recorded in | the window opens, a session is loaded, a run reports a task or a turn while a record page is open, a run finishes or stops, and a record page is opened |
| Local pool | `ymp_providers::discovery::inspect_pool` and `::inspect` | the window opens, a configuration change is saved, and the team, profile or provider page is opened |

`Store::trace` is a full read of one session's records. Refreshing it while a run reports is
how `/assignments` stays live; the cost is a scan per reporting turn, which at present
record counts is not measurable. `Store::result_is_current` compares recorded digests
against the files on disk, which is why it is taken in the controller and stated as of the
moment it was read. `Store::outcomes` does the same comparison and builds absolute artifact
paths from the directory each result was captured in; it answers for a whole session or not
at all, so a single unreadable location is reported as a failed read and never as an absence
of accepted work.

The memory page is the one page that reads the store while it is built, which it did before
this work as well. It asks two questions: every entry this project has, through
`Store::memory_inventory` or `Store::search_memory` with candidates included, and which of
them a run would actually be given, through `Store::search_memory` in the supported mode with
the default scope. Both are `SELECT`s over ymp's own storage.

## The vocabulary, and what each word is allowed to mean

**Requested, sent, reported.** Requested is what the run asked for. Sent is what the adapter
passed to the installation. Reported is what the installation said it used. A column the
record leaves empty stays empty: nothing is copied from one column into another, and a value
that was sent and never reported is shown as unconfirmed, never as applied. Where nothing was
requested and nothing reported, the page says the installation used its own default rather
than naming a level.

**Fixed against adaptable.** A fixed model or effort is a constraint the configuration
states, and it is the only value allowed for that agent. Where nothing is fixed, the run
chooses and may choose differently on the next turn. A loaded session reads its constraints
from the policy it captured; with no session loaded, the page says it is describing the
configuration as it stands now.

**Running against left open.** A record with no end is a turn in flight only while a run is
active in this window. In a stored session the same record means a turn that was left open,
which is what an interrupted run leaves behind, and the page says so in those words.

**Committed against graded.** A membership decision and a per-turn resource bound record
their own outcome inside the record they carry, and no grade is ever written for them. Those
rows read committed, refused, set or refused from that field; they are never reported as
decisions recorded without an outcome, and the grade words below are not applied to them.

**Accepted, unconfirmed, confirmed, unknown.** An acceptance carries a grade. Confirmed means
the evidence the acceptance bound passed for every criterion it applies to. Unconfirmed means
the result was accepted on an independent review alone. Unknown means the record predates
grading or the runtime did not classify it, and it is never read as a pass. A task marked
accepted with no acceptance decision read for it is reported as exactly that: the state alone
does not say who accepted it or against what.

**Current against superseded.** Accepted work stays accepted. Where a file the result named
has changed since, the acceptance is still shown and the page also says that what it was
accepted against is no longer what is on disk.

**Credited.** Competence credit is a record of its own, written only where confirmed evidence
supported a single producer. Only a credited confirmed acceptance counts toward who a later
run may pick. An acceptance without credit is not evidence of unreliability, and an agent
without observations is not thereby unreliable.

**Captured against next run.** A session captures the limits and the team it starts with. The
editable values on the limits page apply to a later session; they are never presented as what
a finished session ran under, and the turn counter in the header and the sidebar uses the
captured bound when a session is loaded.

**Pool, member, worked here.** The pool is who may be drawn on this machine; a session's team
is who its run actually formed. An agent that worked in a session keeps its place in that
session's list after its profile leaves the pool, and an agent with records that the current
list does not name is shown apart rather than dropped.

**Roster against captured identities.** A session keeps every identity it ever admitted, and
it holds a roster of the members a turn may be given to now. A run that replaces a member
leaves both records, so the two lists are shown separately: the members, and the identities
the roster no longer lists. An identity that left the roster keeps the turns recorded under
it, and the page says which roster revision it is measured against. A session that recorded no
roster at all says so, and every identity it captured is presented as a member, because that
is what such a record states. The reserved final reviewer is named as availability: one
eligible agent is kept out of production so that something other than the producer can review
the result. It is not a rank, it carries no permission, and the page says so.

**Supported, candidate, unknown.** A memory entry is a projection of a result this project
accepted, or a candidate a run proposed. Confirmed means the acceptance it names carried
passing checks; unconfirmed means no passing evidence is attached; unknown means the entry
predates provenance entirely. Supported is a different question again, and the store answers
it: whether a run assembling a prompt would be given the entry, which depends on re-reading
the source record, so an entry recorded as confirmed reads as confirmed and not offered once
the task, the files or the criteria behind it change. The page lists retired entries too,
labelled, rather than letting them disappear.

**Declared against enforced.** A task declares whether it may write; a backend enforces what
a turn can actually do. The page keeps the two apart: the declaration is shown on the task as
declared in the plan, the enforced access is shown on the assignment as what the backend
enforces, and a coordination policy may describe the enforced access as broader but never as
narrower, because the runtime refuses a policy that claims to narrow it.

**Recorded location.** An accepted result names the directory it was accepted in. That
location is historical: changing the project's current path does not move it, and the page
states that ymp keeps no copy of the artifact anywhere else. Artifact paths are shown in full
and wrapped when they are longer than the pane, never truncated.

## What these pages will not say

- A grant is permission to use one coordination call for one assignment. It is never
  described as a sandbox, an approval or a limit on what a turn may do in the working
  directory: by the time a turn runs it already has the access of the user who started ymp.
- No page claims that a provider does not support a control. Nothing in the records says so,
  and absence of a report is reported as absence.
- Eligibility is about this machine: the profile is enabled, its provider is enabled, and the
  program was found on `PATH`. Model lists are labelled as coming from the configuration.
  Authentication and quota belong to the installation and are never stated here.
- There is no deliberation to show. A decision carries the reason its actor stated and the
  records it links; no reasoning trace is stored, and none is reconstructed.
- No acceptance, observation or grade is created by the interface. The pages read what the
  runtime wrote.

## How the working directory is used

The interface states the effective policy rather than implying a protection. For this release
a run works directly in the selected directory: files agents create, change or delete are the
real ones, nothing is staged or copied, and there is no review step between a turn and the
directory.

What bounds that is the access each turn actually holds, and the run records it. The page
states the three facts behind it.

The access is the execution backend's own, not a purpose the interface reads. The native
adapter enforces read-only access for a turn it asks read-only, except over the ACP protocol,
where a mode name is not a filesystem guarantee and the turn is therefore recorded as writing
the whole directory; any other backend is taken to write the whole directory unless it states
otherwise. A task declared read-only in the plan is asked read-only, a task that declares
nothing may write, and the declaration is explicit rather than concluded from the work.

Turns overlap only where their recorded access does not conflict. A turn that writes the whole
directory excludes every other turn in it, two readers do not exclude each other, and declared
disjoint paths do not. A run admits at most as many turns at once as its own parallelism
allows, every turn that waits records why it waited, and the checks and the acceptance that
judge a candidate hold the whole directory while they run.

One run uses a project's directory at a time, because a run holds an exclusive lock in ymp's
metadata home and another run refuses to start while it is held; the page says that this lock
is about ymp and says nothing about other programs or a command run by hand.

ymp keeps its own metadata and evidence in its home directory and the deliverables in the
working directory, and it does not create a hidden copy of the tree or move the directory that
was selected. Isolated execution with a reviewed publication step is not part of this release,
and the page says that too rather than leaving it to be assumed.

These sentences are the first row of `/diff`, together with the statement that ymp cannot
restore a previous version of a file, so they are what the first frame of that page shows.
The opening screen carries the short form: where a run works, that no copy is kept, and that
`/diff` is where the record and its limits are stated. None of this adds a question before an
action, a confirmation or a block, and nothing recurring was introduced: the statements are
read where the reader already is.

Per-turn access is disclosed from the records, not inferred. Each assignment carries the
access the run recorded for it: what the backend enforces, what the coordination policy used
where the two differ, which backend and which policy, when the reservation was taken, when the
turn was admitted under it, and when it ended. A reservation with no release record was still
held when the records were read, and a turn with no access record says that instead of showing
a default. Waits are listed with the code the runtime recorded them under, and they are
recorded against the agent, so the page says that a wait belongs to the agent and not
necessarily to the turn it is listed with. The permission mode an assignment requested, the
adapter sent and the installation reported stays a separate field, with the same three-column
honesty as model and effort, because a mode name is not an enforcement guarantee.

The four lifecycle records are kept apart and named in their own words: a turn waited, the
directory was reserved, the turn was admitted under that reservation, the reservation ended.
None of them carries a grade, so none of them is reported as a decision recorded without an
outcome.

## One width per page

A page is built once, for one width, and every surface that paints it uses that width:
`frame::page_content_width` for the row list, the detail pane and the empty state,
`frame::inspect_width` and `frame::inspect_content_width` for the read-only surface `Enter`
opens. Before this, the list was wrapped two columns wider than the pane it was painted into
and the record `Enter` opened was wrapped for the whole terminal while the surface showing it
was at most eighty-six columns, so prose lost words at the right edge on every page. The
independent YMP-107 review reproduced this on the checks page and on the untouched memory
page; it is fixed here for every page at once.

## Evidence

Interface tests drive the real engine against the in-process mock provider and assert what
the pages then say: that a setting nothing reported is not presented as applied, that an
acceptance on review alone is not shown as confirmed, that a confirmed acceptance names its
evidence and its credit, that an accepted result whose files changed stays accepted and says
what changed, that an observation written before grading is not read as a pass, that a
reopened session is measured against the limits it captured, that a session which captured
none says so rather than showing today's, that an agent stays in a session's list after its
profile leaves the pool, and that a turn left open is not called running until a run is
active. Three further tests assert that prose is painted whole at 80x24, 100x30, 120x40 and
160x48, in the detail pane, in the empty state and in the surface `Enter` opens.

Nine tests cover what the merged membership and knowledge records added. A run constrained to
one member at a time replaces its member to reach an independent reviewer, which is the
runtime's own path to that state and needs nothing written by hand: the page then shows one
member, shows the replaced identity apart with the turns recorded under it and the roster
revision it is measured against, and its subtitle separates the roster from what the session
captured. A snapshot with the same assignments, no captured team and no roster stands in for a
record this version cannot write, and the page states the absence instead of presenting an
empty roster. A run with a trusted check recorded both a confirmed projection and candidates:
the page lists all of them, names the supported one as support, names the candidate a reviewer
agreed with as unconfirmed and not offered, and reports a retired entry as retired instead of
dropping it. A run without a contract recorded three candidates and no support at all, which
the previous page would have shown as an empty memory; the test asserts every entry is listed
and that the subtitle says none of it would be given to a run. One test reads the directory an
accepted result was recorded in and the statement that no copy exists elsewhere, and another
relocates the project and asserts that the recorded location does not move with it. One
replaces the controller's snapshot with a failed read of those locations and asserts the page
reports a failed read rather than an absence. One asserts that a committed membership decision
is reported as committed rather than as ungraded, and that it names the members it admitted,
the reviewer it kept free and the moment it was taken; the same test asserts that a recorded
per-turn bound reports what it allowed and says that an allowance is not a measurement. One asserts the opening screen states
how the directory is used and what cannot be put back, at 80x24.

Three more cover the access records. A mock run's own turns give both kinds of access: the
executing turn is recorded as writing the whole directory and the final review as reading it
and writing nothing, and the test asserts both, the backend that enforced them, and the
reservation from its admission to its end. A plan of two tasks where the second depends on the
first is the runtime's own reason for a wait, so that run records one, and the test asserts
that the task says it waited under the code the runtime recorded and that the decision reads
as a wait rather than as an ungraded decision. A task declared read-only is asserted to read
as declared in the plan, beside a task that declares nothing and may write.

Each of those claims was also inverted in a scratch copy of the source, outside the
repository, and the test that covers it failed in the expected direction.

The interface was then walked in a pseudo-terminal against the deterministic demonstration
agents, which answer inside ymp: no provider process was started, no credential was read and
no model was asked anything at any effort level. The walk covered 80x24, 120x40, 160x48 and
40x12, which is below the documented minimum; a frame caught while a run was still working,
with the header reading running and the page counting the turn in flight; a run stopped by
Ctrl+C, which leaves the session paused; a run refused by a one-turn budget, where the limits
page named the recorded stop and the assignments page reported that nothing was assigned
rather than that nothing was read; the record each page opens with Enter, wrapped inside the
surface that shows it; and all five palettes, with the default still painting a pure black
background.

## Limits

- Scoped access is shown as the records carry it, and no backend in this release declares one:
  the native adapter records either the whole directory or read-only, so the scoped rows are
  reachable only through another execution backend. The words and the path lists are in place
  for one.
- A wait is recorded against an agent or a task, never against an assignment, so the page
  cannot say which turn a wait delayed. It says so rather than implying the pairing.
- Isolated execution and a recoverable publication step are YMP-124. The pages state that they
  are absent rather than describing the direct mode as a protection.
- `Store::outcomes` answers for a whole session. One accepted result whose captured directory
  is missing makes the entire list unavailable, and the page then reports that failed read
  instead of showing the other locations. The records behind it are not changed by the
  attempt, and the fix belongs to the storage layer rather than here.
- An agent with assignment records and no captured membership cannot be produced by this
  version: the storage refuses an assignment for an agent outside the captured team. That
  section remains for records written by earlier versions, and its test supplies such a record
  as a controller snapshot rather than writing one into the store.
- Competence credit recorded by another session is not read here. The reputation page says
  which observations this session credited and says plainly that credit recorded elsewhere is
  not read.
- `Store::result_is_current` is a read of the files as they are now. A page that has not been
  refreshed states the time of its read rather than implying the answer is current.
