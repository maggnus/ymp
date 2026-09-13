# YMP-149 implementation evidence: concise interface language

Implemented by Claude Code claude-opus-5 high in an isolated worktree, branch
`fix/ymp149-ui-language`, baseline `27f8454`, following the maintainer decisions in
[audit.md](audit.md) and the [language guide](../../guides/ui-language.md). Every runtime fixture
used temporary homes and projects with mock providers or a provider whose executable does not
exist. No provider was queried and no model was invoked. The source is now independently accepted and integrated; see
[final acceptance](acceptance-final.md). It is not an installed release.

The first implementation was `07025c5`. An independent review returned it; the rework and each
finding's disposition are recorded under [Independent review rework](#independent-review-rework).
The glossary below describes the reworked source.

## Glossary

| Before | After | Reason |
| --- | --- | --- |
| `READING` column with `read from the installation`, `not read recently`, `not in the catalog`, `no native model`, `a local provider`, `what is installed could not be read` | Column removed from `/agents` and `/team` members. The AGENT cell keeps the model label and adds a short warning qualifier only where it matters: `· stale`, `· not in catalog`, `· metadata unavailable` | Owner decision: no provenance narration and no replacement metadata column in primary rows. Unresolved identities still read `unknown model`. The last qualifier is about missing pool data, not a provider failure |
| Captured member READING value (recorded model or `model not recorded`) | Follows the name in the AGENT cell (`name · model`) unless it is exactly the same identifier | Historical model information is data, not provenance prose; distinct identifiers that share a prefix both stay visible |
| `TEAM`: `in team` / blank | `PREFERRED`: `true` / `false`; Inspect `preferred: true · starting selection preference, not guaranteed membership` (or `false · starting selection preference, not an exclusion`) | The known `Config.team` flag. Team selection ranks preferred agents first among the eligible (`ymp-runtime/src/engine/allocation.rs`, `eligible_from`). It is neither current-session membership (UI150) nor a guaranteed roster |
| `{id} joined the team.` / `{id} left the team.` | `{id} is now preferred.` / `{id} is no longer preferred.` | The action edits the preference, not a roster |
| `{id} is already a member` / `outside the team` | `{id} is already preferred` / `not preferred.` | Same |
| `/team` help `Team membership for new sessions.` | `Agents preferred when a session selects its team.` | Same |
| Team hint `Space add or remove`; agents hint `t team` | `Space toggle preferred`; `t toggle preferred` | Same; the key behaviour is unchanged |
| `/team` without a session: `Members of the next run`, `N for the next run`, roster note `the team the next run would form …`, member detail `membership: the configuration as it stands now` | `Preferred agents, enabled`, `N preferred and enabled`, `… the enabled agents the configuration prefers. A new session prefers them when it selects its team, within eligibility and the roster rules; they are not a fixed roster.`, `preferred in the configuration · not a fixed roster` | The list is `Config::members`: preferred agents whose profile and provider are enabled. Captured-session headings (`Members of this session`) are unchanged |
| Sidebar `TEAM next run`, `empty · /team add ID`; welcome `team`, `none configured · /team add ID` | Sidebar `TEAM preferred · enabled`, `none · /team add ID` (a captured session keeps `this session` and `empty`); welcome `preferred agents`, `none enabled · /team add ID` | Both list `Config::members`, which leaves out disabled profiles and providers |
| `the profile is disabled`, `its provider is disabled`, `its program was not found`, `the model is not in the catalog`, `the catalog lists no model` | `profile disabled`, `provider disabled`, `executable not found`, `model not in catalog`, `no models in catalog`; `no native model` kept | Concise refusal causes. The full `exclusion_sentence` stays in Inspect |
| Inspect `metadata from: the installation, read by M at T[, which is no longer current]` | `source: native scan · M · T[ · stale]` | Method and time retained in details |
| Other source values (`nowhere yet …`, `the configuration · …`, `a stored reading whose source was not recorded`) | `not resolved · no model is set and no stored catalog reports a default`, `configuration · …`, `stored scan · method and time not recorded` | Shorter; unknown stays unknown, and missing default metadata is not said to prove no default |
| Catalog field (`read from the installation by …`, `the attempt at … ended as F; the reading … is kept`, `nothing stored; …`, `written by configuration; …`) | `native scan · M · T`, `last attempt A failed: F · keeping native scan · M · T`, `not scanned`, `configured · not scanned`, `native claim · no stored scan matches` | Retains method, time, attempt and the bounded failure code |
| Controls `unknown · the catalog does not say which it offers`; `none · the catalog says it offers none`; `no default is named`; provider default `none was reported; …` | `not reported`; `none`; `default not reported`; `not reported` | `not reported` for absent native fields; `unconfirmed` was not used |
| Catalog completeness `the whole list` / `not known to be the whole list`; `none, and the list is complete: …` | `complete list` / `completeness not reported`; `none · complete list` | Same facts, shorter |
| `Changing the model or the instructions starts a new experience identity. …` | `Experience is recorded with the profile configuration in use, including its model and instructions. …` (details only) | No invented profile-version event or guaranteed reset |
| `Instructions for {id} saved. This starts a new experience identity.` | `Instructions for {id} saved.` | Factual notice |
| `r` notice `Re-read what is installed … This asks no provider anything: reading a provider's own offerings is R.` | `Reloaded local providers and stored catalogs. No provider was queried; press R to scan.` | Keeps the local-reload versus native-scan distinction |
| Hints `r re-read` / `re-read what is stored`; `R ask the installations` / `ask this installation` | `r reload`; `R scan catalogs` (agents) / `scan catalog` (one provider) | Stable action names |
| Enter hints `open the record`, `show the record`, `show the recorded run`, `show the full path`, `read the evidence`, `read` | `inspect` | All open the same read-only Inspect surface. Distinct actions kept: `run the command`, `open read-only`, `open`, `diff`, limits `edit or inspect` |
| `Space enable` (agents) | `Space enable or disable` | Matches providers; it is a toggle |
| Agents detail `enabled: yes/no` | `enabled: on/off` | Matches the ENABLED column |
| `None. Press i to write them.` | `None. Press i to edit.` | Imperative |
| `N turn(s) here`, `N turn(s) recorded here`, `N member(s) · revision R`, `N profiles` | Singular or plural by count | Existing inline `if n == 1` convention |
| TUI scan notice `Read R of N installation(s): … offering(s) stored, … actor(s) added, … existing actor(s) now resolved …` | `Catalog scan: R of N providers updated; catalog entries: E; agents added: A; existing agents resolved to a native model: M. Configured names unchanged; no model was invoked.` | `agent` instead of `actor`. `N` counts every attempted provider and `R` only those whose status is `updated`, so failed attempts are not reported as successes. `E` is a neutral total that may include entries retained from earlier scans or unresolved aliases |
| TUI scan failure `Nothing was read: …` | `Catalog scan failed: …` | Cause retained |
| CLI first-run scan line (same wording as the TUI notice) | `Catalog scan: R of N providers updated; catalog entries: E; agents added: A; existing agents resolved to a native model: M. Configured names unchanged. Run \`ymp catalog\` for details.` | Same counts as the TUI; the first version's `providers scanned: N` could read as N successes |
| CLI health `installed` / `unavailable` | `available` / `unavailable` | `health.available` includes in-process providers; it is not an installation claim |
| CLI `N provider probe(s) failed` | `1 provider probe failed` / `N provider probes failed` | Count convention |

## Retained intentionally

- The captured-session `/team` headings and labels (`Members of this session`, `this session`),
  `Available on this machine`, the captured roster explanations and the limits scope `next run`:
  the limits consumer was not changed and UI150 owns the team-page restructuring.
- `a turn was left open` and `running now`: the uncertainty is unchanged.
- `in process` for the provider executable column, `accepted, unconfirmed`, confirmation and
  reputation wording, `actor` on the decisions page (a decision's actor is not always an agent),
  native model IDs, native control and effort values and captions, `on or off` for boolean
  controls, the `ENABLED` on/off switch, all JSON and MCP output, stored records, user and agent
  messages, raw diagnostics, and the CLI about line.
- `WHAT_MEMBERSHIP_MEANS`, `pool_row` and `exclusion_sentence` detail prose, and the pool/catalog
  distinction.

## Behaviour-sensitive consequences

- Removing READING moves sort keys on `/agents`. ENABLED is now `E` (was `N`), and PREFERRED
  takes `F`, because `P` belongs to PROVIDER, `R` is reserved and `E` is taken (TEAM was `T`). The
  unit test and `check-table-interface.py` press `E`.
- The stale, not-in-catalog and metadata-unavailable qualifiers are part of the AGENT cell, so the
  filter and the AGENT sort also match them.
- `/agents` `t`, `/team` `Space` and `/team add|remove` still edit `Config.team` exactly as
  before, including enabling the profile and provider on add. Only the notices, hints and headings
  now call it a preference. A preferred agent can be absent from a session and a non-preferred
  eligible agent can join one: the preference ranks, it does not pin.

## Tests

Existing tests changed only where they compared renamed text; each keeps its claim:

- `a_profile_with_no_native_reading_is_not_presented_as_a_named_agent`: the row starts with
  `unknown model` and the detail explains why. One detail line must be exactly the `catalog` label
  with the value `not scanned`.
- `a_scanned_model_is_shown_exactly_as_the_installation_resolved_it`: `native scan · models.list`
  in Inspect. The semantic regression added here: no `read from` in the row; the PREFERRED cell is
  exactly `true` or `false` according to `Config.team`; `t` flips it and back; and the notice calls
  the change a preference without mentioning a team.
- `a_captured_member_shows_what_its_turns_ran_with_before_what_its_profile_said`: added a
  shared-prefix case. The captured identity asks for `gpt-5.6-sol`, the installation reported
  `gpt-5.6-sol-0913`, and both must be separate parts of the row.
- The sidebar test expects `preferred · enabled` without a session, and the window-naming test
  finds the `preferred agents` welcome line.
- The stale, not-in-catalog, failed-attempt, policy-model and `r` reload tests use the new
  qualifier, source and notice wording.
- The recorded-checks Enter hint test compares `inspect`; the sort edit test presses `E`.

## First-round checks on `07025c5`

These results belong to the returned first version. The reworked source's results are in
[Independent review rework](#independent-review-rework), and the capture files under
[terminal/](terminal/) were replaced by the rework run.

[check-agent-rows.py](check-agent-rows.py) runs with tmux, the debug build and a temporary home
containing: an enabled preferred mock profile, a disabled non-preferred profile, and a preferred
profile whose provider executable does not exist. It checks `/agents` and `/team` at 140×40,
100×30, 80×24 and 60×20, then Inspect and `t` twice. Paths in reports are replaced by `<output>`,
`<target>` and `<worktree>`.

| Run | Result |
| --- | --- |
| `check-agent-rows.py` | passed, 27 report entries, 0 failed. No READING or `read from the installation` in `/agents` or `/team`, and never `in team`, at any size. NEXT TEAM reads atlas `true`, boreal `false`, cygnus `true` at all four sizes (the column fits at 60×20). `/team` shows `executable not found` and `profile disabled`. Inspect shows `source`, `catalog: not scanned` and `next team: true · next-session preference`. `t` flips the page flag and `config.toml` to `false`, and a second `t` restores `true` |
| `check-agent-rows.py --ascii` (`LC_ALL=C`) | passed, 27 entries, 0 failed |
| `ymp-evals/scripts/check-table-interface.py` | passed, 19 cases, including the `/agents` sort-and-edit case with `E` and the pages at 140×45, 80×24 and 60×24 |
| `check-table-interface.py --ascii` | passed, 19 cases |

At 80 columns the `/team` AGENT cell is cut (`unknown mod…`) with the PROVIDER and STATE columns
kept. At 60 columns the page shows the member table and its refusal causes in full.

First-round workspace checks on `07025c5`: `cargo fmt --all --check` initially exited 1 (two test
assertions needed rewrapping) and exited 0 after `cargo fmt --all`. Strict Clippy exited 0, and
`cargo test --workspace` exited 0 with 579 passed, 0 failed, 2 ignored. The files `fmt.txt`,
`clippy.txt` and `workspace-tests.txt` now hold the rework's final run.

## Independent review rework

The review materials, including the raw probe and `source.diff`, remain unchanged outside this
repository at `/tmp/ymp149-independent-review`. The parent resolved the scope as follows.

| Finding | Disposition |
| --- | --- |
| 1. `NEXT TEAM` overstated the flag. The review observed a disabled preferred `boreal` shown `true` on `/agents` yet missing from the sidebar `TEAM next run`, and a `ymp demo` session that captured `atlas, dorado` with `Config.team = [atlas, boreal]` | Fixed. The column is now `PREFERRED`; Inspect, notices, hints, `/team` help, the no-session `/team` heading, subtitle and roster note, the sidebar (`preferred · enabled`) and the welcome line all describe a starting selection preference. Captured-session labels are unchanged. The review's claim that the runtime never reads `Config.team` was checked by the parent and rejected: `eligible_from` in `ymp-runtime/src/engine/allocation.rs` sorts eligible agents by `Config.team` position. `dorado` joining without a preference and `boreal` staying preferred while disabled are therefore consistent with a preference, not a pinned roster. Runtime behaviour was not changed |
| 2. `identity_cell(None)` labelled an agent `unavailable` when only its pool metadata was missing | Fixed. The qualifier is now `metadata unavailable`, which is about data and not a provider failure. No new column; details are unchanged |
| 3. `name.contains(model)` hid a distinct recorded model sharing a prefix with the name | Fixed with an exact identifier comparison (`name != model`). Both values are cleaned model identifiers, and the name carries no effort (`label::agent` uses `Turn::model`). No effort or history was invented. Failing control [rework/prefix-before.txt](rework/prefix-before.txt): the row read `● · gpt-5.6-sol-0913 · mock · a turn was left open`. After the change, [rework/prefix-after.txt](rework/prefix-after.txt) passes |
| 4. The strengthened catalog assertion matched the label word `catalog`, which is always present | Fixed. The test reads the detail line by line and requires exactly `catalog` followed by `not scanned` on one line. The first attempt matched against the single-line `detail_of_key` output and failed, which confirmed the assertion is not vacuous |
| Review probe P2: sort letters | Adapted. ENABLED remains `E` and PREFERRED takes `F`. Hints are generated from the column titles |
| Scan counts | The CLI line now reports `R of N providers updated` like the TUI; `entries stored` became `catalog entries` |

Focused checks before the final run, with the debug cache `CARGO_TARGET_DIR=/Users/maggnus/Code/ymp2/target`:

- The shared-prefix case failed before the exact comparison and passed after it (files above).
- The first `ymp-tui` run after the rework had 269 passed and 1 failed: the field-level catalog
  assertion had been written against the single-line helper. After reading the detail line by line,
  270 passed, 0 failed, 1 ignored.

### Final checks on the reworked source

Run once, sequentially, in one command, with no release build. `cargo fmt --all` had been applied
before it.

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | exit 0 | [fmt.txt](fmt.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [clippy.txt](clippy.txt) |
| `cargo test --workspace` | exit 0; 579 passed, 0 failed, 2 ignored (the new checks are assertions inside existing tests) | [workspace-tests.txt](workspace-tests.txt) |
| `check-agent-rows.py` on the rebuilt debug binary | passed, 29 entries, 0 failed. PREFERRED reads atlas `true`, boreal `false`, cygnus `true` at 140×40, 100×30, 80×24 and 60×20, and the column fits at 60×20. The sidebar reads `TEAM preferred · enabled` at 140×40 and 100×30. No READING, provenance narration or `in team`. Without a session, `/team` shows the refusal causes; its table heading `PREFERRED AGENTS, ENABLED` is visible in the captures (the script does not assert it). Inspect: `catalog not scanned`, `preferred true · starting selection preference, not guaranteed membership`. `t` flips the flag in the page and in `config.toml`, and restores it | [terminal/rows/](terminal/rows/) |
| `check-agent-rows.py --ascii` | passed, 29 entries, 0 failed | [terminal/rows-ascii/](terminal/rows-ascii/) |
| `check-table-interface.py` and `--ascii` | passed, 19 cases each, including the `/agents` sort-and-edit case with `E` | [terminal/table/](terminal/table/), [terminal/table-ascii/](terminal/table-ascii/) |

At 80 columns the `/team` AGENT cell is still cut (`unknown mod…`) while PROVIDER and STATE stay.
The review materials were not modified: `source.diff` still has its 01:02 timestamp and SHA-256
`c6995a1551f90de46b2131030bda36c22c637fe7058df88528972bd93cefae49`.

## Not covered

- The stale, not-in-catalog and failed-attempt row and detail states were verified by unit tests
  against constructed catalogs, not in a real terminal, because a terminal fixture would need a
  native scan snapshot.
- The CLI scan line, `ymp doctor` health labels and the probe failure count were compiled and
  reviewed but not executed: running them would scan or probe providers.
- The TUI scan-completion notice and the `Catalog scan failed` failure path need a catalog scan and
  were not exercised.
- The `r` reload notice and the membership notices were verified in unit tests. The terminal check
  confirms the flag change but does not read the notice from the screen.
- The `metadata unavailable` qualifier, shown when the pool snapshot has no identity for a profile,
  was changed and reviewed in source only. No unit or terminal case builds a pool without that
  profile's identity, and no label-only test was added for it.
- The preference wording describes the existing runtime ranking in `eligible_from`. Session
  formation through that ranking was not re-exercised here, and the runtime was not changed.
- Current-session membership is still not shown on `/agents`; that and the `/team` restructuring
  belong to UI150. The 60×8 palette defect (YMP-152) was not addressed.
- Independent review and parent acceptance are recorded in [final acceptance](acceptance-final.md). Installation has not happened.
