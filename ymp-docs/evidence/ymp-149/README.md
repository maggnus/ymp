# YMP-149 implementation evidence: concise interface language

Implemented by Claude Code claude-opus-5 high in an isolated worktree, branch
`fix/ymp149-ui-language`, baseline `27f8454`, following the maintainer decisions in
[audit.md](audit.md) and the [language guide](../../guides/ui-language.md). Every runtime fixture
used temporary homes and projects with mock providers or a provider whose executable does not
exist. No provider was queried and no model was invoked. This is reviewable source, not parent
acceptance or an installed release.

## Glossary

| Before | After | Reason |
| --- | --- | --- |
| `READING` column with `read from the installation`, `not read recently`, `not in the catalog`, `no native model`, `a local provider`, `what is installed could not be read` | Column removed from `/agents` and `/team` members. The AGENT cell keeps the model label and adds a short warning qualifier only where it matters: `· stale`, `· not in catalog`, `· unavailable` | Owner decision: no provenance narration and no replacement metadata column in primary rows. Unresolved identities still read `unknown model` |
| Captured member READING value (recorded model or `model not recorded`) | Follows the name in the AGENT cell (`name · model`) only where the name does not already carry it | Historical model information is data, not provenance prose; it stays visible |
| `TEAM`: `in team` / blank | `NEXT TEAM`: `true` / `false`; Inspect `next team: true · next-session preference` | Known `Config.team` flag, scoped to the next session. Not current-session membership (UI150) |
| `{id} joined the team.` / `{id} left the team.` | `{id} added to the next-session team.` / `{id} removed from the next-session team.` | The action edits `Config.team`, which new sessions read (`Config::members`, `App::active_team`) |
| `{id} is already a member` / `outside the team` | `{id} is already in` / `not in the next-session team.` | Same scope |
| `/team` help `Team membership for new sessions.` | `Next-session team membership.` | Same scope |
| Team hint `Space add or remove`; agents hint `t team` | `Space next-session team`; `t next-session team` | Same scope; the key behaviour is unchanged |
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
| TUI scan notice `Read R of N installation(s): … offering(s) stored, … actor(s) added, … existing actor(s) now resolved …` | `Catalog scan: R of N providers updated; entries stored: E; agents added: A; existing agents resolved to a native model: M. Configured names unchanged; no model was invoked.` | `agent` instead of `actor`; `entries` because they may include unresolved aliases |
| TUI scan failure `Nothing was read: …` | `Catalog scan failed: …` | Cause retained |
| CLI first-run scan line (same wording as the TUI notice) | `Catalog scan: providers scanned: N; entries stored: E; agents added: A; existing agents resolved to a native model: M. Configured names unchanged. Run \`ymp catalog\` for details.` | Same |
| CLI health `installed` / `unavailable` | `available` / `unavailable` | `health.available` includes in-process providers; it is not an installation claim |
| CLI `N provider probe(s) failed` | `1 provider probe failed` / `N provider probes failed` | Count convention |

## Retained intentionally

- The `/team` page titles and body (`Members of the next run`, `Available on this machine`, the
  roster explanations) and the limits scope `next run`: the limits consumer was not changed and
  UI150 owns the team-page restructuring.
- `a turn was left open` and `running now`: the uncertainty is unchanged.
- `in process` for the provider executable column, `accepted, unconfirmed`, confirmation and
  reputation wording, `actor` on the decisions page (a decision's actor is not always an agent),
  native model IDs, native control and effort values and captions, `on or off` for boolean
  controls, the `ENABLED` on/off switch, all JSON and MCP output, stored records, user and agent
  messages, raw diagnostics, and the CLI about line.
- `WHAT_MEMBERSHIP_MEANS`, `pool_row` and `exclusion_sentence` detail prose, and the pool/catalog
  distinction.

## Behaviour-sensitive consequences

- Removing READING moves sort keys on `/agents`: ENABLED is now `E` (was `N`) and NEXT TEAM is
  `N` (was `T` for TEAM). `R` stays reserved. The unit test and `check-table-interface.py` were
  updated to press `E`.
- The stale/unknown qualifiers are part of the AGENT cell, so the filter and the AGENT sort also
  match them.
- `/agents` `t`, `/team` `Space` and `/team add|remove` still edit `Config.team` exactly as
  before, including enabling the profile and provider on add. Only the notices and hints name the
  scope.

## Tests

Existing tests changed only where they compared renamed text; each keeps its claim:

- `a_profile_with_no_native_reading_is_not_presented_as_a_named_agent`: the row starts with
  `unknown model`, the detail explains why and reports `not scanned`.
- `a_scanned_model_is_shown_exactly_as_the_installation_resolved_it`: `native scan · models.list`
  in Inspect. Added the one semantic regression here: no `read from` in the row; the NEXT TEAM cell
  is exactly `true` or `false` according to `Config.team`, `t` flips it and back, and the notice
  names the next-session scope.
- stale, not-in-catalog, failed-attempt, policy-model and `r` reload tests: new qualifier, source
  and notice wording.
- the recorded-checks Enter hint test compares `inspect`; the sort edit test presses `E`.

## Terminal checks

[check-agent-rows.py](check-agent-rows.py), with tmux, the debug build and a temporary home: an
enabled mock profile in the next-session team, a disabled profile outside it, and a team profile
whose provider executable does not exist. `/agents` and `/team` at 140×40, 100×30, 80×24 and
60×20, then Inspect and `t` twice. Reports and captures are in [terminal/](terminal/); paths are
replaced by `<output>`, `<target>` and `<worktree>`.

| Run | Result |
| --- | --- |
| `check-agent-rows.py` | passed, 27 report entries, 0 failed. No READING or `read from the installation` in `/agents` or `/team`, and never `in team`, at any size. NEXT TEAM reads atlas `true`, boreal `false`, cygnus `true` at all four sizes (the column fits at 60×20). `/team` shows `executable not found` and `profile disabled`. Inspect shows `source`, `catalog: not scanned` and `next team: true · next-session preference`. `t` flips the page flag and `config.toml` to `false`, and a second `t` restores `true` |
| `check-agent-rows.py --ascii` (`LC_ALL=C`) | passed, 27 entries, 0 failed |
| `ymp-evals/scripts/check-table-interface.py` | passed, 19 cases, including the `/agents` sort-and-edit case with `E` and the pages at 140×45, 80×24 and 60×24 |
| `check-table-interface.py --ascii` | passed, 19 cases |

At 80 columns the `/team` AGENT cell is cut (`unknown mod…`) with the PROVIDER and STATE columns
kept. At 60 columns the page shows the member table and its refusal causes in full.

## Required workspace checks

Run sequentially, once, on the final source, with the incremental debug cache
`CARGO_TARGET_DIR=/Users/maggnus/Code/ymp2/target`. No release build.

| Check | Result | Output |
| --- | --- | --- |
| `cargo fmt --all --check` | first run exit 1: two assertions in `tests.rs` needed rewrapping. After `cargo fmt --all` (which changed only `tests.rs` whitespace), exit 0 | [fmt.txt](fmt.txt) (the exit 0 recheck) |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | [clippy.txt](clippy.txt) |
| `cargo test --workspace` | exit 0; 579 passed, 0 failed, 2 ignored | [workspace-tests.txt](workspace-tests.txt) |

Clippy and the workspace tests ran before the whitespace-only formatting change. Afterwards, the
two reformatted tests (`a_truncated_record_says_the_rest_is_not_shown_anywhere` and
`a_scanned_model_is_shown_exactly_as_the_installation_resolved_it`) passed when run on their own.
The full chain was not repeated for that change.

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
- Current-session membership is still not shown on `/agents`; that and the `/team` restructuring
  belong to UI150. The 60×8 palette defect (YMP-152) was not addressed.
- Independent review, parent acceptance and installation have not happened.
