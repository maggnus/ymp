# Paseo model discovery reference

Inspected the local `/Users/maggnus/Code/paseo` checkout on 2026-09-13 at the
owner's request. Checkout HEAD: `fa93c4290eaa87ae58452ab6e2012f85ae6e0c6b`.
This is a source-reading reference for YMP-132; no Paseo implementation or model
manifest was copied into ymp.

## Findings

- Model definitions keep `id`, `label`, `description`, aliases, selectable state
  and thinking options as separate fields. The app resolves references against
  IDs and aliases, then filters nonselectable entries.
- The snapshot manager returns model IDs when resolving an unspecified choice.
  Agent configuration normalizes the literal `default` before resolving it. This
  is configuration selection, distinct from observing the actual runtime model.
- Claude catalog discovery here combines a version-filtered static manifest with
  configured model names read from Claude settings. It is not entirely a native
  model-list scan. Its runtime separately captures the SDK initialization model;
  observed assistant-frame model handling can preserve unfamiliar raw identifiers.
- Codex discovery starts an app-server, calls `model/list` and reads configured
  defaults through `config/read`, then disposes the temporary client.
- ACP derives model definitions from `availableModels`/`currentModelId`, with
  configuration selectors as a fallback. The raw ID, descriptive name and effort
  options are distinct.

Relevant local source files:

- [Claude models](/Users/maggnus/Code/paseo/packages/server/src/server/agent/providers/claude/models.ts)
- [Claude manifest](/Users/maggnus/Code/paseo/packages/server/src/server/agent/providers/claude/model-manifest.ts)
- [Claude runtime](/Users/maggnus/Code/paseo/packages/server/src/server/agent/providers/claude/agent.ts)
- [Codex adapter](/Users/maggnus/Code/paseo/packages/server/src/server/agent/providers/codex-app-server-agent.ts)
- [ACP adapter](/Users/maggnus/Code/paseo/packages/server/src/server/agent/providers/acp-agent.ts)
- [Snapshot manager](/Users/maggnus/Code/paseo/packages/server/src/server/agent/provider-snapshot-manager.ts)
- [Agent configuration](/Users/maggnus/Code/paseo/packages/server/src/server/agent/agent-manager.ts)
- [App model references](/Users/maggnus/Code/paseo/packages/app/src/provider-selection/model-catalog.ts)

## Decision for ymp

Keep native discovery and its existing provenance. Present raw concrete model IDs
in scan/selection lists. During execution show the model and observed effort of
that invocation. Preserve requested, sent and reported settings separately.
Descriptions remain metadata; provider names remain transport details.

Resolve an alias through factual native metadata or the originating invocation's
model report. The installed ymp catalog already records Claude `default` resolving
to `claude-opus-5[1m]`; the reported poker invocation identifies `claude-opus-5`.
Use the actual invocation report for that message. Never hard-code that every
`default` means Opus, select the first catalog row as evidence of actual execution,
or start inference merely to paint the interface.

No change to stored actor IDs or historical source metadata is needed. Existing
team-operation events already link chat messages to invocations. New runtime
responses and decoded follow-up answers receive equivalent atomic origin links.
This allows the shared CLI/TUI presentation to use each message's own model and
effort rather than the actor's latest catalog caption.
