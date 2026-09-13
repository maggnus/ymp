# YMP-132 Claude effort follow-up: independent backend review

**Accept `449e2fef1ac3cf046ec0de3bc08334963a175a60`, integrated unchanged in `5899589`. No findings requiring correction.** This review covers SDK hook metadata and Rust tool forwarding, not UI or release installation.

API fidelity was checked against the installed, pinned Claude Agent SDK **0.3.246** declarations. `BaseHookInput.effort.level` is documented as the applied turn effort after native downgrade. `agent_id` identifies subagent hooks; the main thread may have `agent_type` without `agent_id`. The filters in [effort.ts:10](/tmp/ymp132-claude-effort/ymp-bridges/claude/src/effort.ts:10) respect that distinction. Assistant-model observations similarly exclude frames with a parent tool-use ID, as documented by `SDKAssistantMessage`.

Matching or missing metadata returns an empty hook result. Registration through public `Options.hooks` leaves existing permission mode, restricted tool list, `canUseTool`, MCP capability and native settings sources unchanged. It never returns a permission approval or copies tool arguments, transcript paths or the full hook input into metadata. Only effort levels and primary tool names are emitted. No private settings API is used.

An explicit effort mismatch first retains the actually reported level, returns the SDK's supported `continue: false` result and marks the observation permanently mismatched. [index.ts:99](/tmp/ymp132-claude-effort/ymp-bridges/claude/src/index.ts:99) checks this before accepting subsequent model/result events, so a native success response cannot erase the mismatch. Missing effort stays unknown; repeated identical hook levels do not produce repeated updates. Metadata discovery still withholds the user-message generator while calling `supportedModels()`.

The Rust addition at [rpc.rs:200](/tmp/ymp132-claude-effort/ymp-rust/crates/ymp-providers/src/rpc.rs:200) forwards only the tool-name notification into the existing `ProviderEvent::Tool` channel. Native execution metadata is sent before the corresponding tool notification and is retained by the ordinary invocation consumer; it creates no separate actor or authority.

Independent validation passed:

- Type checking and six selected bridge tests covering effort, native children, discovery and resumed permission restrictions.
- An additional actual-SDK/fake-CLI **Stop-only mismatch**: requested max, reported high, native hook response `continue: false`, terminal RPC error, and no tool event.
- An additional capabilities request: zero native user messages and no fabricated applied effort.
- Direct observer controls for a named main agent, ignored children, permission-neutral matching output and a sticky mismatch after a later matching level.

The retained hook-removal control fails the physical effort-observation test. The Rust forwarding control and parent integration logs were inspected: the integrated source passes **458 Rust tests**, with two existing ignores, and **18 bridge tests**. Relevant production files were independently verified identical between the reviewed commit and main. Commands, source/SDK type hashes and log hashes are in [review evidence](effort-review-evidence.json); additional wire results are in [SDK controls](effort-sdk-controls.json).

Limitations: applied effort cannot be known from this mechanism before the first `PreToolUse` or `Stop` hook. A turn without tools may expose it only at completion, and hosts omitting hook effort remain unknown. No real native inference or UI timing test was run. No source was edited, no broad suite was rerun, and no Cargo target was created during this review.
