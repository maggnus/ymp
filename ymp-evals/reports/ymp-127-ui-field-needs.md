# YMP-127: what the views need from the catalog, and what they already do

13/09/2026 02:58 HKT, from the YMP-118 interface worktree. This is the interface side's request
to the catalog author, written against the DTO proposal of `AgentIdentity`,
`ModelCapabilities.display_name/aliases/resolved_model`, `PoolAgent.identity` and
`discovery::refresh_catalog`. Nothing here was merged: no backend source is in this worktree.

## Answers the views must be able to give

A reader of `/team`, `/agents`, `/providers`, `/assignments` and `/usage` must be able to tell
these apart, and no current field distinguishes all of them:

1. This agent would run as this exact native model, and this is the native display name for it.
2. This agent pins a model the configuration names, and the catalog does not list it.
3. Nothing has been read from this installation, so no native name is known.
4. A catalog was read and reported no default, so this agent has no resolved model.
5. The catalog is stale: it was read before the installation changed.
6. The turn that already ran used this exact model and these exact native control values.

## What the proposed DTO gives, and what is still missing

The proposal covers 1 and 2. `display_name: Option<String>` with the identifier as the factual
fallback is exactly right, and `AgentIdentity { name, configured_name, model, effort,
resolved_model, source, status }` gives the views one place to read. Four gaps remain.

**A. The status enumeration must name the unresolved cases separately.** `status` needs to
distinguish *nothing read for this provider*, *read and no default reported*, *read and the
pinned model is not in the list*, and *the read failed*. A single `unknown` collapses four
different sentences into one, and the interface would have to infer the difference, which is
what this task forbids. The interface currently derives these from `CapabilitySource` plus
`default_model` and will switch to `status` as soon as it carries them.

**B. A scan needs its method and observation time on the path the views read.** The views must
say when a name was read, and from what. `CapabilitySource::NativeMetadata { method,
observed_at }` already carries both; please keep them reachable through `AgentIdentity` or
`PoolAgent.identity` rather than only on the provider catalog, so a row can state its own
provenance without a second lookup.

**C. Native control values need a home on the recorded settings.** `ExecutionSettings` has a
single `effort: Option<String>`. A native control is an exact name with an exact value, and GLM's
`thought_level` is not an effort ladder. Without a map of control id to recorded value in `sent`
and `reported`, `/assignments` can only print a value under the word `effort`, which is the
mislabelling the task rejects. Requested: `controls: BTreeMap<String, NativeControlValue>` on
`ExecutionSettings`, or an equivalent field, written from what the adapter actually sent and
actually read back.

**D. The pool must be allowed to carry a native catalog.** `discovery::inspect_pool` currently
stamps every catalog `Configured`, which is the right guard against a configuration claiming an
observation it never made, and the interface deliberately reads the pool rather than
`Config::capabilities` for exactly that reason. A stored scan therefore has to reach the pool
snapshot with its native source intact, or no view can ever report a scan. Whatever holds the
stored snapshot, please keep the same rule: a configuration-written catalog stays configured.

## What the interface already does, before any of this lands

- `/team` and `/agents` rows read `name · model · provider`. The model is the profile's pin, or
  the default the pool's catalog reports, and otherwise one of `not scanned`, `model unknown` or
  `not read`. The provider's label is never placed where a model name belongs.
- Each profile's detail says where its answer came from: the pin, the scan with its method and
  observation time, a scan that reported no default, a catalog written by configuration, or
  nothing read at all.
- `/providers` shows per provider where its catalog came from, the identifiers it lists, whether
  the list claims to be complete, and the default it reported or that none was reported.
- `/usage` names, per agent, the exact models that agent's recorded turns ran with, and counts
  the recorded turns whose model nothing reported rather than filling them in from the profile.
- `/assignments` already prefers what the installation reported, then what was sent, then what
  was requested, and names both ends when they differ.
- `r` on `/agents` and `/providers` re-reads what is installed and the stored catalog. Its notice
  states that it asks no provider anything and that reading a provider's own offerings is a
  separate explicit scan. That is where `refresh_catalog` attaches; the interface will not offer
  a scan action that cannot scan.
- The three shipped placeholder profiles are therefore presented as `Codex · not scanned · codex`
  and not as named agents, which is the explicit unresolved state the owner asked for.

No name is derived from an identifier anywhere: nothing is title-cased, no marketing label is
mapped, and no effort ladder is assumed. The pool page's own words are taken from the stored
catalog or reported as absent.

## Evidence

Interface tests, mock only, no provider process and no credential read:
`a_profile_with_no_scanned_catalog_is_not_presented_as_a_named_agent`,
`a_scanned_default_is_named_exactly_as_the_scan_reported_it`,
`a_scan_that_resolved_no_model_says_unknown_and_never_the_provider`,
`a_usage_row_names_the_model_its_turns_actually_ran_with`,
`re_reading_the_catalog_is_an_action_that_asks_no_provider_anything`. The scanned cases supply a
pool snapshot the way the controller will hold one, because the supported path cannot produce a
native catalog yet; the test says so in its own comment. Five mutations were applied and
reverted, each failing its named test, including the one that makes the configuration the source
of provenance.

The catalog is not done and is not claimed to be: no native name has been read from an
installation in this worktree, and none will be until the scan exists and is run.
