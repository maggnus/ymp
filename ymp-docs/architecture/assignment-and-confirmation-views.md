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
| Session records | `Store::trace`, then `Store::team_grant` for each assignment's grants, and `Store::result_is_current` for each decision that carries a result | the window opens, a session is loaded, a run reports a task or a turn while a record page is open, a run finishes or stops, and a record page is opened |
| Local pool | `ymp_providers::discovery::inspect_pool` and `::inspect` | the window opens, a configuration change is saved, and the team, profile or provider page is opened |

`Store::trace` is a full read of one session's records. Refreshing it while a run reports is
how `/assignments` stays live; the cost is a scan per reporting turn, which at present
record counts is not measurable. `Store::result_is_current` compares recorded digests
against the files on disk, which is why it is taken in the controller and stated as of the
moment it was read.

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

- Dynamic membership is not here yet. Until a run can change its team mid-session, the
  "worked here" section appears only for records whose session captured no team, such as
  older ones. The section and its wording are in place for YMP-110.
- Knowledge has no provenance of its own yet. The memory page says what each entry records,
  which is its author, its reviewer if one was recorded, its origin session and what it
  supersedes, and it stops calling all memory verified. Source binding, applicability and
  candidate states arrive with YMP-113.
- Competence credit recorded by another session is not read here. The reputation page says
  which observations this session credited and says plainly that credit recorded elsewhere is
  not read.
- `Store::result_is_current` is a read of the files as they are now. A page that has not been
  refreshed states the time of its read rather than implying the answer is current.
