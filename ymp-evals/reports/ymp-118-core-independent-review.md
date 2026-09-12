# YMP-118 core visibility independent review

13/09 01:39 HKT, 2026-09-13 — **ACCEPT, 8/10** on `7038a4cebed88d44f0f684764020ab7b5b2c458f`,
against backend base `1b76133c78de65d0504cb056c96fd57191f2e23f`.

Code 8/10; evidence 9/10; consumer experience 7/10. The contracted YMP-118 core visibility is
delivered against actual records, the YMP-110 and YMP-113 corresponding views are consistent
with the accepted backends, and every required check passes at the reviewed HEAD. Four defects
remain, none of them a misstatement about what a run recorded. Two of them are moderate and
should be closed before YMP-118 is marked done: a membership or resource decision contradicts
its own row when the record is opened, and the Limits page loses its title and truncates its
subtitle at 80 columns. The remaining two are small overreaches in pages whose stated purpose
is not to overreach. The consumer axis carries the verdict down from 9; nothing found is a
blocker.

Reviewed as a fresh reviewer, not as the author. The checkout was read-only throughout: no
tracked file was modified, and `git status --porcelain` reports exactly one untracked entry, this
report. No application source, `intent.md` or task registry was touched, and nothing was
committed. Mutation checks were run on a separate `git archive` copy under `/tmp/ymp118-mutate`
so that the reviewed tree was never edited.

## Authority and scope inspected

- Checkout `AGENTS.md` and the unchanged approved `intent.md` at the reviewed HEAD.
- Current main `/Users/maggnus/Code/ymp2/ymp-docs/tasks/tasks.json`: YMP-118 acceptance, and the
  YMP-110 and YMP-113 clauses requiring corresponding views on shared runtime data contracts.
- The author's own record, `ymp-evals/reports/ymp-118-core-visibility.md`, as evidence to check
  rather than as a result to accept.
- The complete delta versus `1b76133`, not only the last slice `673aa96`: 17 files,
  5107 insertions, 369 deletions. Source read in full for
  `ymp-tui/src/provenance.rs` (new, 403 lines), `views.rs`, `state.rs`, `ui.rs`, `frame.rs`,
  `sidebar.rs`, `commands.rs`, and the geometry and size assertions in `tests.rs`.
- The backend behaviour each new statement claims: the scheduler ready set, the read-only
  purpose mapping, the workspace lock, the settings transport, and the competence-credit guards.
- The documentation delta: the new
  `ymp-docs/architecture/assignment-and-confirmation-views.md`, the interface and usage guides,
  `README.md`, and the YMP-118 status section appended to
  `ymp-docs/research/evidence/agent-profile-ui-contract.md`.

## Commands run, and what they returned

All commands were run in the review worktree at `7038a4c`. The `target/` directory was empty at
the start, so the first build was a full one; no rebuild loop was repeated for its own sake.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace` | exit 0, **290 passed**, 0 failed |
| `cargo test -p ymp-tui --lib` | exit 0, 96 passed, 0 failed |
| `cargo build --bin ymp` | exit 0 |
| pseudo-terminal walk at 80x24 and 120x40, mock provider only | completed |
| four independent source mutations on a separate copy | each failed its named test, exit 101 |

The workspace figure is 290, not the 289 the author recorded. The per-binary sum is
1+10+5+12+4+1+10+2+6+89+13+1+1+1+28+96+10. The 96 figure for `ymp-tui` is exact.

## Deterministic workloads only

Everything here is deterministic and in process. The configuration used for the walk names a
single provider of `kind = "mock"`, which `ymp-providers::mock_turn` answers inside ymp without
spawning anything. No provider process was started, no credential was read, and no model was
asked anything at any effort level. `Pool::read` was exercised and does what its comment says:
`discovery::inspect` resolves `PATH` entries and reads file metadata, and `inspect_pool` reads
the configured catalog. Neither launches a process. No real-provider compatibility probe was
run, and no quota was consumed.

## What was verified, and how

### The statements about the working directory are true at this source

Each sentence of `DIRECT_WORKSPACE` was checked against the code rather than against a document.

- Only an executing turn is asked with write permission. `engine.rs:2034` passes `read_only =
  false` for the `execute` purpose; `plan`, `review_plan`, `review`, `final_review`, `synthesis`,
  `learn` and `review_memory` all pass `true`. `engine.rs:301` turns that into the requested
  permission mode, and `providers/src/rpc.rs:250` declines the permission requests of a
  read-only turn.
- One task per round. `engine.rs:1510-1519` truncates the ready set with `.take(1)`.
- The exclusive lock. `ymp-storage/src/lib.rs:210,223` take `try_lock_exclusive()` on
  `workspace.lock` in ymp's own metadata directory.

This snapshot predates accepted YMP-115, and it truthfully describes the serial execution its
own source performs. Per the review brief this composition is known integration work being
handled separately and is not counted against this snapshot. Production isolation, YMP-124, is
likewise out of scope, and the pages state its absence rather than describing the direct mode as
a protection. The new YMP-125 trusted-config ingress is backend work and is not a blocker here.

### The location correction is genuinely consumed

This is the strongest independent check in the review and it was run against the real binary,
not against a fixture. A mock session was executed to completion, the project record was then
relocated in a cloned store and the directory moved on disk, and the reader was opened at the
new path.

```
 Changed files …  /private/tmp/118walk/project-moved      <- current path, in the subtitle
   /private/tmp/118walk/project/greeting…  created        <- recorded change, original path
  │recorded in    /private/tmp/118walk/project            <- opened record, original path
```

The page names the current directory where it means the current directory, and the directory the
run recorded where it means the record. The correction in `1b76133` reaches the consumer.

### Negative controls are meaningful

The author lists eleven mutations. All twelve named test functions exist. Rather than take the
table on trust, four inversions were applied independently, one at a time, on the separate copy,
each followed by a byte-for-byte restore:

| Inversion | Test | Result |
| --- | --- | --- |
| `team()` ignores the roster and treats every captured identity as a member | `a_member_the_roster_replaced_is_not_presented_as_one_and_keeps_its_records` | failed, exit 101 |
| `outcome_row` reads the location from `ctx.cwd` | `a_relocated_project_does_not_move_where_an_outcome_was_recorded` | failed, exit 101 |
| `page_content_width` returns the full width | `the_record_pages_paint_their_statements_whole_at_every_supported_size` | failed at 80x24, exit 101 |
| `Acceptance::word` reports unconfirmed as confirmed | whole `ymp-tui` suite | `an_acceptance_on_review_alone_is_not_presented_as_confirmed` failed, 95 passed |

The size tests are real rendering tests: they paint into a `TestBackend` buffer and read the
painted cells back, and they recompute the expected geometry independently instead of calling the
production helper. `SUPPORTED_SIZES` covers 80x24, 100x30, 120x40 and 160x48.

### The wrapping contract holds

`page_width` in `state.rs` and `page_view` in `ui.rs` now derive the same number from the same
two helpers. At 80 columns: `sidebar_width(80) = 26`, `main_width(80, 26) = 53`,
`page_content_width(53) = 49`, and the list rect at `body.x + 1` and the detail rect at
`body.x + 2` are both 49 wide inside a 53-wide column. At 120 columns the same arithmetic gives
85 inside 89. Key handling therefore reads the page the frame painted, which is what the
inspect-surface test measures directly. `split_body` was refactored through `main_width` without
changing its behaviour.

### Read-only navigation, proven rather than asserted

After roughly fifty binary launches and several hundred keystrokes across every page at both
sizes, the working directory still contained only `greeting.txt` at its original modification
time, and the store still held exactly one session and one project. Opening a page starts nothing
and writes nothing.

### Typography, sidebar and the black default

`theme.rs` is untouched by this delta. The default background is pure black at both sizes: every
one of the 1920 cells at 80x24 and 4800 cells at 120x40 on the opening screen has background
`000000`, and on a page 1877 of 1920 are black with 43 carrying the selection accent `d8a34c`.
The right sidebar is present at both sizes with its navigation, session, token, team and task
sections, and the sidebar appears from 80 columns as `README.md` states.

### The pages, walked at 80x24 and 120x40

`/team` at 80x24, on the records of a real mock run with `fixed_size = 1`:

```
 Team   1 in the roster · 2 captured by this session
 › ◇ What membership means here  what this page is
   MEMBERS OF THIS SESSION
   ✓ One · mock                     5 turn(s) here
   CAPTURED HERE, NO LONGER A MEMBER
   ◇ Two                        no longer a member
   HOW THE ROSTER IS BOUNDED
   ✓ roster               1 member(s) · revision 9
   • roster rules                        exactly 1
```

Current and historical membership are two lists, and the replaced identity keeps the turns
recorded under it. The roster record names the reserved final reviewer, the eligibility observed
when the revision was written, and the method. The roster rules row reads the bounds the session
captured and says so, so editing the configuration changes the next run and not the record.

`/assignments` shows agent identity, provider, purpose, task attempt, directory and turn timeout,
then the settings block, then the native session and turn identifiers, then context and grants.
Incomplete coverage is preserved as such: `native turn  not reported`,
`tokens  no count was reported for this turn`. `/usage` attributes to agent identities and never
groups by provider, as `AGENTS.md` requires.

`/decisions` reads `26 recorded · 0 confirmed` for the walked session, with rows reading
`membership committed`, `bound set`, `recorded without an outcome` and `accepted, unconfirmed`.
The opened membership record names the members proposed, the roster it replaced with its
revision, the reviewer kept free, the boundary it was decided at, the method, and the policy and
version that decided it.

`/memory` labels each entry with the confirmation its provenance records, whether a run would
actually be given it, the acceptance, result version and criteria version it projects, its
applicability conditions and the policy that retained it. The walked entry reads
`confirmation  unconfirmed: no passing evidence is attached` and
`given to a run  no; it can be read here and is not offered as support`, which is the store's
answer under the default scope rather than a reading of the text.

`/limits` separates `THIS SESSION, AS CAPTURED` from `The next run`, and the header and sidebar
turn counters use the captured bound. Reopening a historical session from `/sessions` works at
both sizes and repopulates every record page.

## Findings

### F1 — Moderate. A committed membership or resource decision contradicts its own record.

`views.rs:3410` writes the `outcome` field of the opened record from `acceptance.word()`, which
returns `"recorded without an outcome"` whenever `DecisionRecord.outcome` is `None`.
`bounded_outcome` is consulted only for the row marker at `views.rs:3391` and for the right
column. Membership and resource decisions never carry a grade, so every one of them reads
committed on the row and ungraded in the record. Observed at 80x24:

```
   ✓ allocation committed ·…  membership committed     <- the row
  │outcome        recorded without an outcome          <- the same record, opened
```

The author's report states that these records "read committed, refused or set instead of
recorded without an outcome". That is true of the row and false of the record. The covering test
`a_membership_decision_says_what_it_changed_and_what_it_reserved` asserts the row word and the
allocation fields, and never asserts the detail's outcome line, which is why the gap survived.

Fix: in `decision_row`, derive the `outcome` field from `bounded_outcome(decision)` when the
decision carries an allocation or a resource allocation, and extend the test to assert the
opened record's outcome line, not only `right_of_key`.

### F2 — Moderate. The Limits page loses its title and cuts its subtitle at 80 columns.

The subtitle is new in this delta: `views.rs:2636` replaced `"Applied to the next run"`
(23 cells) with a captured-versus-next sentence. At the default limits both of its variants are
62 cells, 63 with the trailing space `ui.rs` appends; the captured variant grows further with
larger numbers. The main column at 80 columns is 53. `text::row` truncates the
left side to fit the right and never the right side, so `budget = width - (right + 2)` saturates
to zero, the title and the command are dropped entirely, and the subtitle itself is clipped by
the paragraph render. Observed at 80x24:

```
this session: 200 turns, 3 at a time · the next run:
```

The page identity is gone and the statement ends on a colon. At 120x40 it fits and reads
correctly. No other new subtitle overflows the whole row: `/team` at 45, `/assignments` at 36 and
`/decisions` at 26 all fit or degrade to an ellipsis. This is the same class of defect YMP-118
was chartered to correct, and the new size tests do not cover the header row.

Fix: shorten the subtitle to something that fits 53 cells, for example
`captured: 200 turns, 3 at a time · next run: 200`, and add a size assertion that the page title
survives at the smallest supported size.

### F3 — Minor. The requested column is dropped whenever a value was sent.

`setting_state` at `views.rs:3183` matches `(_, Some(sent), _)` before any arm that mentions
`requested`, so the requested value is never shown once a value was sent. The about-row prose on
`/assignments` and the new clause 4 of the UI contract both state that three columns are shown.
For model and effort the two values coincide in practice, because
`providers/src/settings.rs:97-118` validates an effort against the advertised control and rejects
an unsupported one rather than remapping it. Permission mode demonstrably differs:
`engine.rs:301` requests `read_only` or `write`, while `rpc.rs:284-311` records what was actually
written to the transport, which is the Codex `sandbox` value or the ACP `modeId`. Under the mock
this is invisible because the mock echoes the request:

```
  │permissions    write sent · unconfirmed, the installation reported nothing
```

Under a real backend the requested word disappears and the reader sees only
`danger-full-access sent`. This review could not exercise a real backend, so the consequence is
argued from the transport code rather than observed.

Fix: when `requested` is `Some` and differs from `sent`, name both, for example
`write requested · danger-full-access sent · the installation reported nothing`.

### F4 — Minor. `/reputation` states a credit it has not read.

For a confirmed observation outside the loaded session, the credit field at `views.rs:2525` reads
`credited by the session that accepted the work, which this page does not read`. Credit is a
separate record written under several guards in `ymp-storage/src/confirmation.rs:855-887`:
interrupted work is refused, an observation whose task is no longer the accepted current attempt
is refused, an attribution mismatch is refused, and a duplicate returns without writing. A
confirmed observation can therefore exist with no credit, and the page asserts the credit as
fact. In a delta whose governing rule is that absence of a record is absence of a record, this
sentence should report rather than assert.

Fix: `whether it was credited is recorded by the session that accepted the work, which this page
does not read`.

### F5 — Minor, documentation. The module comment overstates its own scope.

`provenance.rs:5-7` says pages "never read the store, the filesystem or `PATH` while painting".
That is true of the pages listed by `View::reads_records`, which present the controller's
snapshot and state its read time in the about row. It is not true of `/memory`, `/diff`,
`/checks`, `/files`, `/sessions` and `/reputation`, which read the store or the filesystem inside
`build`. The claim should be scoped to the snapshot it describes.

### F6 — Minor, report accuracy. The workspace test count is 289 in the report and 290 here.

Not a functional defect, but the report's evidence table should match the tree it describes. The
`ymp-tui` figure of 96 is exact.

## YMP-110 and YMP-113 corresponding views

Both tasks require the corresponding existing views to stay consistent with the new state on
shared runtime data contracts, coordinated with YMP-118 and without a separate dashboard. Judged
delivered.

- **YMP-110.** Current membership, historical participants, the roster revision, the reserved
  final reviewer, the eligibility observed when the revision was written and the captured bounds
  are all present and distinguished on `/team`, and the committing decision is readable on
  `/decisions` with its boundary, method and deciding policy. A fixed size, a pinned roster, an
  eligible-pool restriction and the ceiling are four separate lines on the roster rules row. The
  pages read committed decisions and propose nothing, which matches the clause that the runtime
  commits membership.
- **YMP-113.** The knowledge inventory lists candidates and retired entries rather than hiding
  them, labels each with its provenance, source acceptance, result version and criteria version,
  applicability and retaining policy, and answers separately whether a run would be given it.
  Reputation shows each observation's evidence status and what credit requires, subject to F4.
  Captured outcome locations survive relocation, proven above.

The two new destinations are pages inside the existing frame and sidebar, sharing the same
selection model, keyboard rules and empty state as every other page. That satisfies "without
requiring a separate dashboard"; readers who take "through existing views" literally should note
that `/assignments` and `/decisions` are new entries in the existing navigation rather than
additions to pre-existing pages.

## Remaining YMP-118 contracted visibility

Listed separately, as requested. None of these is a defect in the delivered slice.

- The requested column of clause 2, per F3.
- Task-level declared access is not shown, because this snapshot's records do not carry it. It
  arrives with YMP-115; the fixture has a single `Task {` literal in `seed_task`, which is the
  one edit point.
- `Store::outcomes` answers for a whole session, so one unreadable captured location leaves the
  entire list unavailable. The page reports that as a failed read and not as an absence, which is
  the correct behaviour for the interface; the narrowing belongs to the storage layer.
- Competence credit recorded by another session is still not read, and the page says so.
- An agent with assignment records and no captured membership cannot be produced by this version,
  because storage refuses an assignment for an agent outside the captured team. That section of
  `/team` exists for records written by earlier versions and its test supplies a controller
  snapshot rather than writing one into the store.
- A long recorded path wraps across lines in the detail and is never truncated.

## Known limits of this review

- No real provider was contacted. F3's consequence under a real backend is argued from
  `rpc.rs` and `engine.rs` rather than observed.
- The walk used a terminal emulator built on `pyte`. Two artefacts in an early pass, a
  replacement character and a stray prefix, were my decoder splitting UTF-8 across reads, not the
  application; with an incremental decoder the captures contain zero replacement characters. Every
  screen reading above agrees with an interface test.
- Mutation checks were run on a `git archive` copy, so they exercise the same source but not the
  same build directory as the reviewed worktree.
- Only the 80x24 and 120x40 sizes were walked by hand. The 100x30 and 160x48 sizes are covered by
  the suite's own assertions and were not walked.
- `Cargo.lock` carries a one-line change in this delta; `cargo` was run without `--locked` and did
  not modify it.
