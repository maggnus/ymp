# Agent pool and native capability metadata

`ymp_providers::discovery::inspect_pool(&config)` exposes the configured agent profiles, local eligibility reasons and provider model offerings. It reads executable metadata and configuration only. It does not launch an agent, make a model request, open a native conversation, inspect credential contents or check quota/authentication. Native authentication is checked by the installed agent when it is used.

Profiles retain their existing stable IDs, display names, instructions and provider/model bindings. Several profiles may share the same provider and model. Each ID owns its own native continuation keys, board authorship, task attribution and usage. A name edit does not change the profile's execution version or rewrite a captured session. There are no role or authority fields in the pool.

## Eligibility and participation

| Data | Meaning |
| --- | --- |
| `Config.agents` | All configured individual profiles, including disabled profiles |
| `AgentPool.eligible()` | Locally eligible profiles, before session admission constraints |
| `Config.team` / `Config.members()` | Configured starting roster, filtered by enabled profile/provider settings |
| `SessionAgentView.captured_participants` | Profiles captured in that session, retaining their names and bindings |
| `SessionAgentView.current_members` | Explicit current membership supplied by the runtime; `None` means unknown |
| `SessionAgentView.active_invocations` | Explicit live invocation references supplied by the runtime; `None` means unknown |

Pool exclusions identify disabled profiles, disabled providers, missing executables, complete catalogs with no models, and configured models absent from an explicitly complete model catalog. An incomplete or missing catalog leaves an unlisted model's availability unknown. A profile without an explicit model retains its inherited native default; inspection does not claim to resolve it. An executable's presence does not establish that every dependency of a configured wrapper is installed.

`SessionAgentView::from_captured` validates session/agent references and rejects duplicate or foreign invocation references. Use `captured_name(agent_id)` for session chat/task attribution. The builder has no reference to today's configuration, so a renamed or deleted profile cannot replace a historical name. Callers must supply authoritative current membership and actual live invocation references; saved native continuation IDs and durable usage rows do not prove a process is running. Membership changes and their persistence remain runtime work in YMP-110; this API does not grant or transfer authority.

GLM cache discovery can enable the configured GLM installation and its default profile. It preserves IDs and names and does not add anyone to `Config.team`. The initial roster and pool availability are separate decisions.

## Configuring native offerings

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

Validation rejects duplicate/empty identifiers, empty or duplicate choice lists, reversed ranges, out-of-range or incorrectly typed defaults, and a default model missing from a complete catalog. Pool output labels all configuration-supplied catalogs as `configured`, even if a configuration claims a native metadata source. The `native_metadata` source variant is reserved for future adapters that capture a real metadata API response with its method and observation time. No such API is called by this implementation.

These are capability claims, not execution evidence. Assignment-level choice validation, control combinations, transmission, acknowledgement and provider-reported effective settings belong to YMP-111. Existing invocation behavior is unchanged by adding a catalog.

## Offline evidence

Core tests exercise old configuration round trips, identity/name preservation, exact control values, unknown metadata and invalid catalog/session references. Discovery tests first execute a synthetic sentinel provider to prove it detects a launch, then show config save/load and pool inspection never launch it. Runtime integration executes two mock agents sharing one backend/model and verifies separate continuation IDs, independent review attribution, per-agent usage and captured names after edits and storage reopen. These tests make no real provider request and do not establish real model availability.
