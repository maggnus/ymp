# Native assignment settings

YMP-111 supplies model and effort transmission and validation. It does not choose an optimization policy, issue grants, or enforce a token/currency budget.

## Configuration and runtime API

`Config.execution` maps stable agent IDs to `AgentExecutionPolicy`. Both `defaults` and `fixed` contain optional `model` and `effort` strings. Existing `AgentProfile.model` remains a model default. A present fixed value is a singleton allowed set: a conflicting assignment choice fails before an invocation is allocated. Otherwise an assignment choice wins, then the configured default, then the profile model. Missing values remain omitted so the native runtime can supply its own defaults. There is no common effort ladder.

For example, in `~/.ymp2/config.toml`:

```toml
[execution.codex.defaults]
model = "gpt-6-astra"
effort = "high"

[execution.claude.fixed]
model = "claude-opus-5"
effort = "max"
```

The session captures these policies and its initial assignment rules. Later configuration-file edits cannot weaken captured pins. `Engine::set_assignment_settings(Vec<AssignmentSettingsRule>)` replaces choices for subsequent invocations on that engine and its clones. It does not reconfigure an active turn. A resumed engine inherits the captured rules unless the caller supplies replacements.

Each rule has `agent_id`, optional `purpose` and `task_id`, and `settings`. Task-specific rules take precedence over purpose-specific rules, which take precedence over an agent-wide rule. The most specific matching rule is selected as a whole; its missing values use configured defaults. Equally specific matching rules are rejected. The native request receives a separate `TurnRequest.settings`; adapters never substitute the profile model for the chosen assignment model.

```json
[
  {"agent_id":"codex","purpose":"plan","settings":{"model":"gpt-6-astra","effort":"xhigh"}},
  {"agent_id":"codex","purpose":"execute","settings":{"model":"gpt-5.6-sol","effort":"high"}},
  {"agent_id":"glm","purpose":"review","settings":{"model":"glm-4.7","effort":"on"}}
]
```

Save that array as `assignment-settings.json`, then run:

```sh
ymp run "Implement the requested change" --assignment-settings assignment-settings.json
ymp resume SESSION_ID --headless --assignment-settings assignment-settings.json
ymp ask codex "Inspect the implementation" --model gpt-6-astra --effort xhigh
```

`ask` also enforces configured fixed values. Assignment rules cover conversation, plan, review_plan, bid, execute, review, final_review, learn, review_memory, and synthesis. They cannot change permissions. CLI `ask` remains a standalone diagnostic; complete assignment/invocation traces belong to session engine runs.

## Native validation and metadata

`ymp capabilities AGENT [--model MODEL]` and `ymp_providers::inspect_capabilities` use native metadata/control interfaces and never release a model prompt. The normalized result is `ProviderCapabilities` with a native source and observation time. Configured catalogs remain claims: they are not substituted for observed native metadata. Runtime invocations retain catalog observations in `native_capabilities` history events for policy callers and audit.

| Backend | Validation and transmission |
| --- | --- |
| Codex | Paginated `model/list`, including hidden offerings, supplies exact wire model IDs and per-model `supportedReasoningEfforts`. The installed schema defines effort as an open string. `thread/start`/`thread/resume` receive the selected model and `config.model_reasoning_effort`; `turn/start` receives model and effort again. Any reported disagreement fails before the prompt. |
| Claude | The real SDK starts with a gated asynchronous prompt stream. `Query.supportedModels()` supplies `ModelInfo.supportedEffortLevels` before the stream releases any user message. `Options.model` and `Options.effort` carry the exact selection. The SDK's supported effort type is checked as well. Initialization and assistant model reports remain separate observations; conflicting model reports or a reported effort downgrade fail the invocation. |
| GLM ACP | `session/new` or `session/load` reports the active model and configuration. After `session/set_model`, the adapter uses refreshed `config_option_update` options, never the previous model's controls. It sends `session/set_config_option` with `configId: thought_level` and verifies `currentValue` before `session/prompt`. GLM's own silent clamping is therefore detected or avoided. |

An incomplete picker list does not make an unlisted explicit model unavailable. Claude forwards such a model when no effort override requires unknown capability information. Unknown effort support is reported as unknown and that override is rejected; unsupported advertised values are rejected as unsupported. GLM can obtain an unlisted selected model's actual controls through `session/set_model` and its configuration update. Models not inspected in ACP metadata retain unknown controls.

Claude's documented `[1m]` suffix changes the context window. Capability matching permits only this known suffix on an otherwise identical advertised alias/resolved ID; it does not infer capability from a model-family prefix, change the sent identifier, or label derived base IDs as observed catalog rows. Thus an observed `claude-opus-5[1m]` row supports validating the exact `claude-opus-5` version pin. See [Claude model configuration](https://code.claude.com/docs/en/model-config).

## Continuations and competence

ACP output exhaustion has a bounded review-only recovery route in 0.4.1; see
[incomplete native reviews](../guides/incomplete-native-review.md). Retry keeps
the selected settings and requires fresh admission. It does not select a stronger
effort or treat truncated content as a verdict.

The runtime reuses a native session only from a typed marker written after successful invocation completion, with matching requested settings, profile/provider configuration and read/write scope. Known default-to-default and identical explicit configurations remain reusable. A setting change, explicit-to-default reset, unknown legacy marker or incompatible profile starts a new native session. Beginning an invocation invalidates the old marker; failure cannot resurrect it. Unrelated scopes keep their own continuations. These markers preserve conversation context and do not issue or restore authority; YMP-120 owns assignment grants.

Requested settings, settings actually sent, and reported model/effort remain distinct in each invocation. A missing report remains unknown; an ACP empty `set_model` acknowledgement is not a provider-reported model. SDK options already sent during metadata initialization are recorded even if capability validation then prevents a user prompt.

`execution_config_version` separates requested execution configurations from legacy profile-only observations. Effective versions also include the actual sent/reported settings and observed native version. The runtime maps a request configuration to its most recently observed effective version for selection lookup and attributes competence to the original producing invocation, not to a later reviewer configuration. Unknown values stay explicit, and no legacy competence is silently relabeled. Selection before a new native observation uses the last known effective configuration; a newly observed backend/default change creates a separate version. Evidence qualification is YMP-117's responsibility.

## Verification

Offline wire fixtures use synthetic models and never call a paid backend. The initial Codex/ACP tests failed on omitted turn settings, accepted invalid effort and missing ACP configuration transmission. A deliberate removal of Claude option transmission fails the physical SDK/CLI test. Disabling continuation reuse fails the known-default reuse control. The restored implementation passes those controls, pin/default precedence, changing assignment settings, invalid/unknown effort, ACP clamping and missing refresh, native metadata-only requests, exact plain Opus 5 transmission, and consistent competence lookup/update attribution.

Metadata-only checks were also run against local Codex 0.153.4, Claude Code 2.1.269 / SDK 0.3.246, and GLM ACP 1.3.0. They observed Codex's per-model open efforts, Claude's native alias/resolved metadata, and GLM 5.2 `none/high/max` versus GLM 4.7 `none/on`. These are installation observations, not universal hard-coded defaults. No live inference, quota/budget guarantee, or native permission-enforcement guarantee is claimed by these checks.

The normalized installation observations are retained in [native metadata evidence](../research/evidence/assignment-settings-native-metadata.json). Integrated validation after independent review and the diagnostic correction passed `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (147 tests), `npm run check`, `npm test` (10 tests), and `npm run build` in `ymp-bridges/claude`, all with exit 0. A headless mock CLI run with an assignment-rule file produced the expected artifact and recorded distinct planning/execution models across nine invocations.
