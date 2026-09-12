# Agent pool and native capability metadata

`ymp_providers::discovery::inspect_pool(&config)` exposes configured actors, local eligibility reasons and stored provider offerings. It reads executable metadata, configuration and the loaded native catalog snapshot; it does not launch a provider or check authentication. The separate `ymp catalog --refresh` operation queries native metadata without a model prompt and populates the actual pool. See [native catalog capture](provider-named-agent-catalog.md). Rendering or inspecting an existing pool does not initiate a scan.

Actors retain stable local IDs, instructions and distinct provider/model bindings; several actors may use the same offering. Selectable names come from the applicable native catalog's exact display name or model identifier. A configured label remains separate, and missing native metadata produces an explicit unknown or unresolved identity rather than a provider label masquerading as a model. Each actor owns its continuation keys, board authorship, task attribution and usage. Historical names/settings remain bound to captured session and assignment records. Pool membership grants no role or invocation authority.

## Eligibility and participation

| Data | Meaning |
| --- | --- |
| `Config.agents` | All configured individual profiles, including disabled profiles |
| `AgentPool.eligible()` | Locally eligible profiles, before session admission constraints |
| `Config.team` / `Config.members()` | Configured starting roster, filtered by enabled profile/provider settings |
| `SessionAgentView.captured_participants` | Profiles captured in that session, retaining their names and bindings |
| `SessionAgentView.current_members` | Explicit current membership supplied by the runtime; `None` means unknown |
| `SessionAgentView.active_invocations` | Explicit live invocation references supplied by the runtime; `None` means unknown |

Pool exclusions identify disabled actors/providers, missing executables, unresolved provider placeholders, complete catalogs with no models and effective models absent from a complete catalog. Inspection resolves existing fixed/default assignment settings before checking the model; an incomplete or missing catalog leaves an unlisted model's availability unknown. Refresh can migrate provider placeholders to a returned native default or an explicitly recorded offering while preserving existing pins. An executable's presence does not establish that every wrapper dependency is installed.

`SessionAgentView::from_captured` validates session/agent references and rejects duplicate or foreign invocation references. Use `captured_name(agent_id)` for session chat/task attribution. The builder has no reference to today's configuration, so a renamed or deleted profile cannot replace a historical name. Callers must supply authoritative current membership and actual live invocation references; saved native continuation IDs and durable usage rows do not prove a process is running. Membership changes and their persistence remain runtime work in YMP-110; this API does not grant or transfer authority.

GLM cache discovery can enable the configured GLM installation and its default profile. It preserves IDs and names and does not add anyone to `Config.team`. The initial roster and pool availability are separate decisions.

## Configured capability claims

Configuration version 1 accepts an optional `capabilities` table keyed by an existing provider ID. Older files without it remain valid and retain unknown native defaults. The following is a **synthetic schema example**, not a claim about any installed model. Replace the provider/model/control identifiers and values with the installation's actual metadata before use:

```toml
[capabilities.example-provider]
models_complete = false

[capabilities.example-provider.source]
kind = "configured"

[[capabilities.example-provider.models]]
id = "example-native-model"

[[capabilities.example-provider.models.controls]]
id = "native_thought_mode"
default = "adaptive"
values = { kind = "choices", options = ["off", "adaptive"] }

[[capabilities.example-provider.models.controls]]
id = "native_thinking_enabled"
values = { kind = "boolean" }

[[capabilities.example-provider.models.controls]]
id = "native_thinking_allowance"
values = { kind = "integer", min = 128, max = 1024 }
```

Each model has its own exact control names and supported values. String choices, booleans and integer ranges remain distinct. Missing `controls` means unknown; `controls = []` explicitly declares no controls. Missing `default_model` or control `default` means unknown. No universal effort ladder, model list or equivalence between identically named settings is provided.

Validation rejects duplicate/empty identifiers, empty or duplicate choice lists, reversed ranges, out-of-range or incorrectly typed defaults, and a default model missing from a complete catalog. Configuration-supplied catalogs remain `configured` claims even if their TOML names a native source; they cannot supply native presentation names. An applicable observed catalog from `provider-catalog.json` takes precedence and retains its native method, observation time, aliases and controls. `inspect_pool` reads that snapshot without querying a provider; refresh owns the metadata calls.

These are capability claims, not execution evidence. Assignment-level choice validation, control combinations, transmission, acknowledgement and provider-reported effective settings belong to YMP-111. Existing invocation behavior is unchanged by adding a catalog.

## Offline evidence

Core tests exercise old configuration round trips, identity/name preservation, exact control values, unknown metadata and invalid catalog/session references. Discovery tests first execute a synthetic sentinel provider to prove it detects a launch, then show config save/load and pool inspection never launch it. Runtime integration executes two mock agents sharing one backend/model and verifies separate continuation IDs, independent review attribution, per-agent usage and captured names after edits and storage reopen. These tests make no real provider request and do not establish real model availability.
