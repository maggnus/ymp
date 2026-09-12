# YMP-118 core visibility: the roster, the knowledge basis and the working directory

Recorded 13 September 2026, 00:49 HKT, in the `118` worktree. Delivering session: model
`claude-opus-5`, permission mode `bypassPermissions`, thinking level requested as `max` by the
assignment. The model identifier and the permission mode are observable in the session; the
thinking level is not, so it is recorded as requested rather than as measured.

## What was delivered, and where

| Commit | What it is |
| --- | --- |
| `7d65e656e237c490a64d22ff96b1da051d12844d` | Merge of sibling `integrate_state` `addba636a56653b1f1d2f14932cd3672e528f515`, the combined YMP-110 membership and YMP-113 knowledge work, into the interface branch. Both ancestries kept. |
| `673aa96a18e2f174f1e1e899079e9308aa32f381` | The interface slice for those records: the roster on `/team`, membership and resource decisions on `/decisions`, the knowledge basis on `/memory`, and how the working directory is used plus the recorded locations on `/diff` and the opening screen. |
| `f754b84` | Merge of `1b76133c78de65d0504cb056c96fd57191f2e23f`, the head the backends were independently accepted at, which keeps the captured output directory in a location follow-up. |
| `bd7299882acfc7f16f8834c02430a10cc9584330` | The size control for the new statements, with its own width mutations. |

Ancestry is separate throughout: nothing was squashed or cherry-picked, and no upstream line
was altered. The two conflicts in the first merge were resolved by taking the upstream side,
which only added the allocation and resource policy fields, their bounded defaults, the
allocation test include, and one add/add document whose merge base predates both files.

Neither backend commit changes the interface. The correction in `1b76133` is used by the
interface as it stands: the status line after a run reads the structured workspace of the run
outcome, so it names the directory the work was recorded in rather than a relocated path.

## Commands and results

All commands were run in the `118` worktree, on the merged tree, in this order.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace` | exit 0, 289 passed, 0 failed |
| `cargo test -p ymp-tui` | exit 0, 96 passed, 0 failed |
| `cargo build --bin ymp` | exit 0 |
| pseudo-terminal walk at 80x24 and 120x40 | completed, mock provider only |

`cargo` was run without `--locked` and used the worktree's own ignored `target/`. `Cargo.lock`
was not modified.

## Deterministic workloads only

Every workload in this work is deterministic and in process. The interface tests and the
terminal walk both use a configuration whose only provider is `kind = "mock"`, which answers
inside ymp before any process is spawned: no provider process was started, no credential was
read, and no model was asked anything at any effort level.

The distinction the owner asked to keep explicit holds here as well. The `max` and `xhigh`
values that appear in the repository are parameter-test data for validation and settings
plumbing, not requests sent to a paid model, and the SDK executable the bridge tests use is
the local Python fixture. Nothing in this report rests on a real provider, and no
real-provider compatibility probe was run.

## What the pages now say

**`/team`.** A session keeps every identity it admitted and holds a roster of the members a
turn may be given to now, so the page shows the members, then any identity the roster no
longer lists, with the turns recorded under it and the roster revision it is measured against.
A session that recorded no roster says so, and every identity it captured is then presented as
a member. The roster record names the final reviewer it keeps free as availability and not
authority, the eligibility observed when the revision was written, and the method recorded.
The bounds the roster was formed under are read from what the session captured, so editing the
configuration changes the next run and not the record.

**`/decisions`.** A membership decision and a per-turn resource bound record their outcome
inside the record they carry, and no grade is ever written for them, so they read committed,
refused or set instead of recorded without an outcome. A membership decision names the members
it admitted, the roster it replaced, the reviewer it kept free, the moment it was taken and the
backend that decided it. A resource decision names the time, native turns and output
characters one turn was allowed, and says that an allowance is not a report that it was
reached.

**`/memory`.** The supported reader hides candidates and legacy entries by design, so browsing
reads the inventory and labels every entry: the confirmation its provenance records, the
acceptance, result version and criteria version it projects, its applicability conditions, the
policy that retained it, and whether a run assembling a prompt would actually be given it.
That last question is the store's answer under the default scope, not a reading of the text.
Retired entries stay listed, labelled.

**`/diff` and the opening screen.** The first row of the change page states that agents work in
this directory itself, that only a turn executing a task is asked with permission to change
files while planning, review, final review and summary turns are asked in the provider's own
read-only mode and have their permission requests declined, that one task executes at a time
because the scheduler takes one ready task per round, that one run holds the directory through
an exclusive lock which says nothing about other programs, that ymp keeps no hidden copy of the
tree, and that isolated execution with a reviewed publication step is not part of this release.
It then carries the existing statement that no earlier content was recorded. A new section
lists the accepted results with the directory each was recorded in, states that the location is
historical and that no copy exists elsewhere, and reports a failed read of those locations as a
failed read rather than as an absence of accepted work. The opening screen carries the short
form. No dialog, confirmation or recurring question was added anywhere.

Every claim above was checked against the code rather than against a document: the read-only
flag is `false` only for the `execute` purpose, the ready set is truncated to one task per
round, and the lock is an exclusive lock on `workspace.lock` in the project's metadata
directory.

## Failing-before controls

New surfaces cannot fail before by construction, so each delivered claim was inverted in the
source, one mutation at a time, and the test covering it was run. The sources were restored
from a snapshot after every mutation and checked byte for byte afterwards; all eleven
mutations and their restorations are accounted for.

| Mutation | Test | Result |
| --- | --- | --- |
| The roster is ignored and every captured identity is a member | `a_member_the_roster_replaced_is_not_presented_as_one_and_keeps_its_records` | failed as intended |
| An absent roster is described as an empty one | `a_session_that_recorded_no_roster_says_so_and_still_shows_its_records` | failed as intended |
| Memory reads the supported-only reader again | `knowledge_a_run_recorded_without_evidence_is_listed_rather_than_hidden` | failed as intended |
| An entry is labelled by its reviewer field, as before | `a_confirmed_projection_and_a_candidate_are_not_shown_as_the_same_thing` | failed as intended |
| A recorded location is taken from the current directory | `a_relocated_project_does_not_move_where_an_outcome_was_recorded` | failed as intended |
| A failed read of the locations shows nothing | `locations_that_could_not_be_read_are_not_reported_as_none` | failed as intended |
| A membership decision takes its word from the absent grade | `a_membership_decision_says_what_it_changed_and_what_it_reserved` | failed as intended |
| The opening screen drops the recovery limit | `the_first_screen_says_how_the_directory_is_used_and_what_cannot_be_undone` | failed as intended |
| The workspace statement drops the access sentence | `an_accepted_outcome_names_the_directory_it_was_recorded_in` | failed as intended |
| A page is wrapped for the whole terminal | `the_record_pages_paint_their_statements_whole_at_every_supported_size` and `the_detail_pane_paints_prose_whole_at_every_supported_size` | failed as intended |
| The inspect surface is built for the terminal width | `the_inspect_surface_is_built_for_its_own_width_and_not_the_terminal` | failed as intended |

The last two are complementary: the prose tests catch a page wrapped too wide, and the
inspect-surface test measures the built lines directly and catches a surface wrapped too wide.

## Consumer walk

The real binary was driven in a pseudo-terminal with a configuration naming the mock provider
and `team_constraints.fixed_size = 1`, which is the runtime's own path to a replaced member:
with a roster of one, a run replaces its member to reach an independent reviewer. The capture
is `/tmp/118-walk/walk_slice.txt`.

- Opening frame at 80x24: the working directory, `how it is used  directly, no copy kept`, and
  the two sentences about where a run works and what cannot be put back.
- `/team` at 80x24: subtitle `1 in the roster · 2 captured by this session`; `MEMBERS OF THIS
  SESSION` with `Two · mock  5 turn(s) here`; `CAPTURED HERE, NO LONGER A MEMBER` with `One
  no longer a member`; `HOW THE ROSTER IS BOUNDED` with `roster  1 member(s) · revision 9` and
  `roster rules  exactly 1`; then the pool.
- `/diff` at 80x24 and 120x40: the leading row `How this directory is used, and what cannot be
  put back  direct · metadata only`, the recorded change, and `WHERE ACCEPTED WORK WAS
  RECORDED` with `Created greeting.txt and verified its content.  accepted, unconfirmed`.
- `/memory` at 80x24: `3 recorded · 0 supported as context`, three rows labelled `unconfirmed`,
  and the entry's confirmation on the first detail screen. Before this change the same run
  showed an empty page.
- `/decisions` at 80x24: the allocation rows read `membership committed`, and the resource rows
  their own bound.
- The surface `Enter` opens on the change page at 80x24 shows the whole workspace statement,
  wrapped inside the surface, with nothing lost at the right edge.
- The default palette paints a pure black background: 4766 cells of `0;0;0` against 34 of the
  selection accent.

The screen was read with a minimal terminal emulator written for this work. Two of its own
faults were fixed during the walk, because they would have been read as application faults: a
24-bit colour sequence split across two reads was printed as text, and a character split
across two reads was replaced. It reproduces cursor positioning, erasure, scrolling, text
placement and cell background, and it is not a full terminal; every reading here agrees with
an interface test.

## Remaining limits

- Task-level access is not shown, because it is not recorded yet. YMP-115 adds a declared
  read-only or writing access to a task; its candidate is in correction and was read for
  interface coordination only. The interface task fixture has a single `Task {` literal, in
  `seed_task`, which is the one edit point when the field lands.
- Isolated execution and recoverable publication are YMP-124. The pages state their absence
  rather than describing the direct mode as a protection.
- `Store::outcomes` answers for a whole session, so one accepted result whose captured
  directory is missing leaves the whole list unavailable; the page reports that failed read. The
  fix belongs to the storage layer and was not duplicated here.
- An agent with assignment records and no captured membership cannot be produced by this
  version, because the storage refuses an assignment for an agent outside the captured team.
  That section of `/team` remains for records written by earlier versions, and its test supplies
  such a record as a controller snapshot rather than writing one into the store.
- A long recorded path is wrapped across lines in the detail, never truncated. The whole path
  is present; it is not one line at 80 columns.
- Competence credit recorded by another session is still not read, and the reputation page says
  so.
- The public MCP work in YMP-123 touches no interface surface and was not read or relied upon.

## Round two: declared access, enforced access and the coordination records

Recorded 13 September 2026, 01:25 HKT, after the accepted concurrency work arrived. Same
session and same settings as above.

| Commit | What it is |
| --- | --- |
| `f034ddf` | Merge of integrated main `f02a8c2d`, the line YMP-115 was accepted on, keeping both ancestries. Ten conflicts were the same content arriving through two commit lines and were resolved by taking the integration side. |
| `2c4a89b` | Declared task access, the enforced access and reservation lifecycle on each assignment, the four coordination records on the decisions page, and the corrected policy text on the change page, the limits page and the README. |
| `5562495` | A member the runtime reports as waiting reads as waiting rather than as busy. |
| `18bf72f` | The acceptance criteria a run captured before the work, on the decisions page. |

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace` | exit 0, 306 passed, 0 failed |
| pseudo-terminal walk at 80x24 and 120x40, two phases | completed, mock provider only |

### What was corrected, and against what

Two sentences this interface carried became false with the accepted concurrency work, and both
were checked against the code rather than against a document before being rewritten.

The claim that one task executes at a time is gone. The scheduler now takes up to the run's
parallelism from the ready set, records a wait for every ready task it does not take, and lets
the access coordinator decide what actually overlaps. The page says that turns overlap only
where their recorded access does not conflict, that a turn writing the whole directory excludes
every other turn, that readers and declared disjoint paths do not exclude each other, and that
the checks and the acceptance that judge a candidate hold the whole directory.

The claim that only a task-executing turn is asked with permission to change files is gone. The
access a turn holds is the execution backend's own: the native adapter records read-only access
for a turn it asks read-only, except over the ACP protocol, where a mode name is not a
filesystem guarantee and the turn is recorded as writing the whole directory, and any other
backend is taken to write the whole directory unless it states otherwise. A task declared
read-only in the plan is asked read-only; the page names that as a declaration and never as a
measurement. The same correction was made to the README sentence about one task writing at a
time and to the parallelism description on the limits page.

### What the records now show

Each assignment carries the access the backend enforced, the paths a scoped access would name,
the backend and the coordination policy, and the reservation from the moment it was taken,
through the admission of the turn under it, to its end. A reservation with no release record is
reported as one. A turn with no access record says so rather than showing a default. Waits are
listed with the code the runtime recorded them under.

The decisions page reads the four lifecycle records in their own words: a turn waited, the
directory was reserved, the turn was admitted under that reservation, the reservation ended.
None carries a grade, so none is reported as a decision recorded without an outcome. A captured
acceptance contract reads the same way: the task it binds, each criterion, the checks bound to
them, the artifacts named, the inputs recorded by digest, the checker and the contract version,
and a statement that it is a binding rather than a result.

### Failing-before controls, round two

Nine further mutations were applied one at a time and reverted, with the sources checked byte
for byte afterwards.

| Mutation | Test | Result |
| --- | --- | --- |
| A writing access reads as read-only | `the_access_a_turn_held_is_shown_as_the_backend_enforced_it` | failed as intended |
| The admission record is not shown | same | failed as intended |
| A declared read-only task reads as writing | `a_read_only_task_is_shown_as_declared_and_never_as_measured` | failed as intended |
| A task does not say it waited | `a_task_that_waited_says_what_it_waited_for` | failed as intended |
| A wait takes its word from the absent grade | same | failed as intended |
| The corrected policy text is reverted | `an_accepted_outcome_names_the_directory_it_was_recorded_in` | failed as intended |
| A waiting member reads as busy | `an_agent_held_up_by_coordination_is_not_reported_as_working` | failed as intended |
| A captured contract is not shown | `a_captured_acceptance_contract_is_shown_as_a_binding_and_not_as_a_result` | failed as intended |
| A captured contract is described as evidence | same | failed as intended |

### Walk, round two

The second phase of the walk ran two dependent tasks, which is the runtime's own reason for a
recorded wait. It shows the declared access on a task, the access block on an assignment with
its backend, its coordination policy, the admission and the held window, and the reserved,
admitted and ended records on the decisions page with their own words. Both phases used the
in-process mock provider: no provider process started, no credential was read and no model was
asked anything at any effort.

## Remaining YMP-118 acceptance gaps

Stated plainly, because none of them is covered by the work above.

1. **Scoped access has no producer in this release.** The words and the path lists are in
   place, and the native adapter records only the whole directory or read-only, so the scoped
   rows are reachable on this build through another execution backend alone. The interface
   tests therefore do not exercise them; the runtime's own fixture backend does.
2. **A wait cannot be paired with the turn it delayed.** Waits are recorded against an agent or
   a task, never against an assignment. The assignment page lists an agent's waits and says
   exactly that rather than implying the pairing.
3. **`Store::outcomes` answers for a whole session.** One accepted result whose captured
   directory is missing makes the list unavailable, and the page reports that failed read. The
   fix belongs to storage.
4. **The section for records without captured membership is unreachable through supported
   writes.** Storage refuses an assignment for an agent outside the captured team, so that part
   of the team page exists for records written by earlier versions, and its test supplies such a
   record as a controller snapshot rather than writing one into the store.
5. **Competence credit recorded by another session is not read**, and the reputation page says
   so.
6. **Typed check outcomes and recorded shell checks are not joined on one page.** A captured
   contract names its typed checks; their outcomes appear as check evidence on the acceptance
   and rejection records; `/checks` lists the shell commands from the session log. No page
   relates the two, and none claims to.
7. **A long recorded path wraps across lines** in the detail pane rather than being truncated.
   The whole path is present, but it is not one line at 80 columns.
8. **The decision list is long.** A session records dozens of decisions, and a frame shows the
   first screenful, so the coordination records in the middle of a run were asserted by test
   rather than read on a captured frame in this round.
9. **No surface proposes or edits membership, access or criteria.** That is by design for this
   task: the pages read what a run recorded. Isolated execution and recoverable publication
   remain YMP-124, and the pages state their absence rather than describing the direct mode as a
   protection.
10. **The shared lock release correction under separate review is not duplicated here**, and
    nothing in these pages depends on it.

## Round three: one walk of a long record list, and what it found

Round two left item 8 of the list above open: the coordination records in the middle of a run
were asserted by test and never read on a captured frame. This round answers it by walking the
real binary, at 80x24, through a session that recorded 67 decisions. Commit `2124b53`,
13/09/2026 01:37 HKT; the source stood at `05e67df` until the walk found something to correct.

### How the walk was driven

The binary is `target/debug/ymp` built from this worktree, started in a pseudo-terminal with
its own home and its own working directory. Its configuration names the in-process mock
provider (`kind = "mock"`, `command = "internal"`), two mock agents and two dependent tasks, so
the recorded wait is the runtime's own and no provider process starts. No credential is read,
no model is asked anything, and no effort setting applies.

The first harness counted its position in the list by arithmetic over the row texts it had
swept. Dozens of rows share the same left text, so that arithmetic answered with the first
match, pressed Down too many times, and opened a later record than the one it named: a run it
labelled "an acceptance record" in fact opened a reservation release. The corrected harness
presses a key, reads the row the application itself marks as selected, and stops when that row
is the wanted one, which is what a reader does. Both captures are kept, the faulty one
included, at `/tmp/118-walk/walk_long_list2.txt` and `walk_long_list3.txt`.

### What the keyboard reached

| From | Keys | What became selected |
| --- | --- | --- |
| Top of the list | `Home` then 5x `PageDown` | row 35 of 68, the admission of One's turn |
| Top of the list | `Home` then `End` | the last row, a reservation release |
| Top of the list | `Home` then 16x `Down` | the wait record |
| Top of the list | `Home` then 30x `Down` | the task acceptance record |
| Top of the list | `Home` then 54x `Down` | the final acceptance record |
| `/checks` | `Home` then 1x `Down` | the first recorded shell check |

A sweep of the whole list with `Down` alone selected 68 rows: the explanatory first row plus
the 67 the subtitle counts. The sweep took 70 samples, the last two because `Down` on the last
row does not move, and two adjacent pairs read the same because the left text of a membership
commit truncates before the member's name. The session's own store holds 67 decisions, of which
ten are membership commits, which is the number of such rows the sweep selected, so the sweep
reached every record rather than stopping inside the list. `PageDown` advances seven rows, one viewport at this
height, and `End` reaches the last row from the top in one press. Two independent runs agreed
on the order of the list, so the positions above are the same rows in both. `Enter` on any of
them opens the whole record in a read-only surface that scrolls with `Up`/`Down` and states how
much is left; the task acceptance record ended with `0 more` after one `PageDown`.

The records read, in the words the pages now use:

- The wait: `a turn waited`, actor the runtime, `outcome the turn waited · dependencies`,
  the task and attempt, `waited because dependencies · a task it depends on was not accepted
  yet`, `holder none was named`, and the runtime's own reason.
- The reservation: `directory reserved`, its reservation short id, the directory, `effective
  reads the directory and writes nothing`, `enforced by ymp.native 0.3.0`, and the direct-mode
  statement that unbounded writers own the whole directory with no rollback or source
  isolation.
- The release: `reservation ended`, with `Runtime resource ownership ended; this is not result
  acceptance`.
- The task acceptance: `task accepted`, actor Two, `outcome accepted, unconfirmed`, `basis an
  independent review, with no applicable check evidence`, the reviewer, the result with its
  version and criteria version, `files still current (0 named by the result)`, the task and its
  attempt, and the assignment.
- The recorded check: the command whole across two lines, `outcome passed`, the task, the
  directory, when it was recorded, and the captured output with its exit status.

### The defect the walk found, and the correction

A row reading `reservation ended` opened a record reading `outcome recorded without an
outcome`. The detail read the acceptance grade field, which is never written for a membership
change, a per-turn bound, a wait or the three reservation records, while the row beside it read
the record's own field. One record, two statements, one of them false.

`recorded_outcome` in `views.rs` now reads the same field the row does, so the detail states
what the record decided: the membership was committed or refused, the bound was set or refused,
the turn waited with its code, the directory was reserved, the turn was admitted under that
reservation, the reservation ended, or criteria were captured before the work. Records that do
carry an acceptance grade are unchanged.

`a_record_that_carries_its_own_outcome_never_reads_as_ungraded` asserts it over every record of
a mock run whose outcome lives inside itself, and fails before the change. Reverting
`recorded_outcome` to the grade field fails that test as intended; this was the only mutation
of round three.

### Commands and results, round three

| Command | Exit |
| --- | --- |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0, no warning |
| `cargo test --workspace` | 0, 307 tests passed, 102 of them in `ymp-tui` |
| `python3 /tmp/118-walk/walk_long_list3.py` | walk written, 257 keystrokes |

## Acceptance state, stated in two separate lists

### Actually unmet YMP-118 criteria

None remain that this task owns. Item 8 above is closed by the walk: normal navigation reaches
every record of a long list, including the middle and the last row, and `Enter` opens each one
whole. The one visibility defect the walk exposed is corrected in `2124b53`. Frame clipping,
the other half of the task, is covered both by the test that paints every record page whole at
every supported size and by these captured frames, where long values wrap inside the record
surface and nothing is silently cut.

Two defects found while doing this work belong to layers this task may not change, and are
reported rather than fixed: a wait is recorded against an agent or a task and never against the
assignment it delayed, so the pages list an agent's waits and say exactly that; and
`Store::outcomes` answers for a whole session, so one accepted result with a missing captured
directory makes the list unavailable and the page reports that failed read.

### Tested limitations, which are not unmet criteria

1. **A captured acceptance contract cannot appear in a walk driven through the binary.** No
   configuration this build accepts supplies typed criteria; that ingress is YMP-125. The walk
   searched the whole list for such a record and reported its absence rather than passing over
   it: 120 `Down` presses from the top never selected one. The display path is covered by
   `a_captured_acceptance_contract_is_shown_as_a_binding_and_not_as_a_result` and by the test
   added this round, both against a contract supplied in process.
2. **Scoped access rows have no producer on this build.** The native adapter records the whole
   directory or read-only and nothing between, so the scoped words and path lists are exercised
   by interface tests and by the runtime's fixture backend, not by a walk.
3. **The section for records without captured membership is unreachable through supported
   writes**, because storage refuses an assignment for an agent outside the captured team. Its
   test supplies such a record as a controller snapshot.
4. **A long recorded path wraps across lines** in the detail pane at 80 columns. The whole path
   is present and reachable; it is not one line.
5. **The emulator is not a terminal.** The screen model in the harness handles positioning,
   erase, scrolling, colour and split sequences, and every reading above is cross-checked
   against an interface test that asserts the same text.

### Explicit feature boundaries, unchanged by this round

Cross-session competence credit is not read, and the reputation page says so. Typed check
outcomes and recorded shell checks are not joined on one page, and none claims to. No surface
proposes or edits membership, access or criteria. Isolated execution and recoverable
publication remain YMP-124. The shared lock release correction and the public MCP work are in
main at `3b4045f` and are not duplicated here.
