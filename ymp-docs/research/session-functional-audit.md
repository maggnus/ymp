# Functional session audit before model comparisons

Owner direction, 2026-09-14: first establish whether ordinary ymp sessions behave
as expected. Comparative weak/strong-model trials are deferred. The first observed
case is an already running owner session, not a new paid attempt started by the
maintainer. Read-only observation is authorized for that session, its configuration
and the originating native usage records. It does not authorize intervention,
changing the team/settings, resuming the session or operating on unrelated data.

## Case and first observations

Session: 6e286a3a-af64-4897-8209-87ee82026ee6.
Directory: ~/Downloads/_ymp3. Installed executable: ymp 0.4.6.
Recorded task: `Create browser Texas Holdem pocker game for me and 4 virtual players`.
Preserve this exact task text when repeating that integration case. Each future
creation run starts in a genuinely new empty directory, never a cleared or reused
previous result directory. Existing case data is preserved.

The session started at 2026-09-14T00:16:39UTC with Luna and GLM 4.5 Air already selected.
Current configuration prefers Luna and a Sonnet profile, has max_members 4 and no
fixed roster/size. Preference alone does not establish why the algorithm chose
GLM; the allocation record and native eligibility still require inspection.

At 00:57UTC five of six persisted tasks were accepted and one was under review.
Accepted tasks were not deleted: sidebar.rs counts them, then filters them from
its visible task rows. Which list should remain, and whether any lists can be
combined, depends on distinguishing the persisted graph, native-agent todos and
plan messages. Accepted/confirmed and model-claimed completion must remain distinct.

At 01:13:44UTC the session was blocked, with five accepted tasks, one review task,
14 completed invocations and no active invocations. Its last event was 2579 at
01:00:51UTC. Summary message 234 reports a malformed-JSON parse failure. Review
message 233 contains CSS braces in commentary and a valid terminal JSON object
with approved:true/reason/lesson. The parent independently parsed that suffix;
this is not runtime acceptance and does not authorize modifying the session.
The exact parser/provider boundary is under source investigation.

## Token audit: provisional application figures

Summing per-invocation usage.counts, not cumulative native_total, gives Luna:
11,084,090 input, 88,216 output, 10,825,984 cache-read and 36,991 reasoning tokens.
Input+output is 11,172,306; input excluding recorded cache is 258,106. Reasoning is
part of output. These are figures from ymp, not yet independent confirmation of
native accounting and not a currency-cost calculation.

The largest execute invocation records 6,203,061 input, 6,139,136 cache-read and
14,873 output during final verification. Audit exact corresponding native logs
for cumulative-counter deltas, duplicate snapshots, resume baselines, reset events,
request/tool counts and context growth. Attribute useful work versus repeated
inspection/browser/debug cycles without guessing monetary costs. GLM has seven
partial usage records because ACP exposes only the final native request; their
sum is not complete session expenditure.

## Expected behavior and evidence to collect

- Stable chronological message/activity placement using recorded ordering and
  stable identities; updating one invocation must not reorder unrelated entries.
- Completed work remains inspectable with its real state and confirmation scope.
  Do not erase progress from the owner's view or merge unrelated task concepts.
- Manual preferences, fixed membership, selected current members and actual
  invocations are clearly distinguished. Explain departures or substitutions
  through recorded decisions; do not infer a fixed roster from preferences.
- Report requested, sent and reported model/effort separately. Luna currently
  reports xhigh while requested/sent effort is absent; determine its origin.
- Measure actual overlapping invocation intervals against dependencies, ready
  work, reservations and effective access. No parallelism is not itself a bug
  when there is no independent work or unbounded access requires serialization.
- Inspect inter-agent board use, repetitive stale proposals, waiting conditions,
  denied checking tools, negative versus malformed verdicts and final acceptance.
- Separate trustworthy command execution, model judgments and actual game quality.
  A passing producer-written test suite alone does not prove every poker rule.

Read the live database only with SQLite mode=ro/query_only and short transactions,
including current WAL data. Export minimal task-relevant projections to a private
/tmp audit folder, not credentials or unrelated sessions. The active directory
and user processes remain untouched. Stop monitoring when terminal state and
ended invocations are confirmed; analysis may continue on the retained projections.

## Recommended functional progression

1. A single index.html counter, no dependency/install/server step: initial 0,
   a +1 button and Reset to 0. Verify exact file/output behavior externally. This
   exercises the whole session lifecycle and exposes disproportionate coordination,
   repeated calls, parsing errors, waiting and lost task visibility cheaply.
2. A small independent-work scenario for concurrency, with explicit expected
   dependencies and access. The counter does not establish parallel scheduling.
3. A standalone fifteen-puzzle with valid moves, reset/shuffle and solvability.
   It adds application state and interaction while keeping the rules bounded.
4. Repeat the fixed Texas Holdem case for the more complex end-to-end workflow.
   Battleship is an intermediate option, not necessary for the first smoke test.

These are separate functional cases, not evidence about model superiority. Fix
one prompt per case and capture directory, source version, selected configuration,
actual participants, settings, history, checks, timing and usage for every run.
New native runs still need a concrete bounded allowance under the project's
provider-check rule; no quota is inherited from the deferred YMP-201 proposal.
The already owner-run 6e286a3a observation requires no new model invocation.

## Current work

52111c72 observes runtime selection, stages, access, board traffic and source-native
usage. Claude 8d8b7748 investigates the UI projection using source and offline
falsifiers. Parent owns expectations, task recording and the next minimal run.
The delivered UI146 candidate 19370d5 remains subject to independent acceptance;
it must not be confused with the installed 0.4.6 observations. Its author is not
being asked to alter that frozen candidate during this audit.
