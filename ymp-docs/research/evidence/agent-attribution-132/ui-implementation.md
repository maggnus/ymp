# YMP-132 UI/CLI presentation: implementation report

- Worktree: `/private/tmp/ymp132-agent-attribution`, branch `fix/message-attribution`
- Commit: `1bf8d307b8b5e121266605a8ebbd6c85b12c19ef`
  ("Name agents by their concrete model and invocation effort (YMP-132)"), parent `dcaebd1`.
  The working tree was clean after the commit.
- Follow-up commit: `6e9b396` ("Show none for an effort the invocation did not report (YMP-132)"),
  described under "Follow-up" below.
- Implementation agent: Claude Code, model `claude-opus-5`.

## Behavior

`ymp_tui::label` (`crates/ymp-tui/src/label.rs`) is the single naming rule. The terminal uses it
everywhere it names an agent, and headless `ymp run` uses `label::heading`.

- **Discovery and selection** show the concrete raw model only. This applies to agents page rows,
  team page member and pool rows, the welcome team line, idle sidebar members, token rows and the
  sessions list. The label is `identity.resolved_model` when it is concrete, otherwise the
  concrete `identity.model`. Pools are not expanded into model and effort combinations. Local
  fixture identities keep their captured name.
- **Work** is shown as `MODEL EFFORT` for one invocation. This applies to terminal transcript
  entries, headless headings, live streams, working sidebar members, assignment rows and tool
  activity in the status row. The model is chosen in this order:
  1. The concrete model the invocation reported.
  2. The sent or requested model. The internal `default` alias is resolved only through the
     identity captured with that assignment, and only when that identity's model matches.
  3. The identity's resolution when nothing was requested.

  The effort shown is only the reported one. Without it, the label reads
  `MODEL none`.
- **`default` is never shown as a model.** Without a concrete resolution it reads
  `unknown model`. Pool actors whose identity is the unresolved alias are left out of pool choices
  unless they are already members. In that case they stay listed and read `unknown model`.
- **Captions, provider IDs and actor IDs appear only in details.** Examples of captions are
  `Default (recommended)` and `Latest release`. The inspector shows an origin line in the form
  `agent ID · provider ID · invocation SHORT-ID`. The agents page shows `native label`,
  `configured as`, `asks for`, `resolved to` and `metadata from`.
- **Unlinked agent messages read `unknown model`.** They are never named from another invocation,
  the configuration or the current provider kind, as the review note required. Bound local
  identities keep their captured name. `you` and `ymp` are unchanged.
- **Labels are made safe for the terminal.** Escape sequences and control characters are removed,
  whitespace is collapsed, and labels are cut at 120 cells.
- **Freshness in the terminal.** The terminal reads the session trace again before it presents a
  new agent message, when a stream starts and on every `AgentStatus` event. Message links are
  resolved once per message in a single pass over the history (`provenance::Attribution`).
- **Freshness in headless output.** The CLI clones the `Store` before `Engine::new` and resolves
  each agent heading through `SessionTrace::message_attribution`. Events still queued when the run
  future returns are printed before the outcome, not dropped.
- **Runtime.** The authorized narrow change is limited to the `ProviderEvent::Tool` branch in
  `crates/ymp-runtime/src/engine.rs`. That branch now sends `Status("Using NAME")` and
  `UiEvent::AgentStatus { agent: <stable id>, status: "tool: NAME" }`. The terminal shows
  `MODEL EFFORT · NAME` in the status row and keeps the member's purpose. Grant, cancellation and
  inference logic is untouched.
- **Nothing stored changes.** No stored identity, execution setting, permission, history row or
  message body is modified.
- **Documentation.** `ymp-docs/guides/interface.md` was updated (the layout sample and "Reading
  the conversation"). `ymp-docs/architecture/provider-named-agent-catalog.md` gained a "Names in
  the interface" section, which supersedes the earlier use of captions as names.

## Tests

New tests:

- **`ymp-cli/tests/message_labels.rs`, with `tests/fixtures/attribution_acp.py`.** This runs the
  real `ymp run` through a scripted ACP fixture. The fixture uses the captions
  `Default (recommended)` and `Recommended descriptive caption` and the actor IDs `transport-*`.
  Every agent heading (chat, plan, execute and others) must start with `native-model-z max · `,
  and the number of headings must equal the number of stored messages.
- **`ymp-tui/src/tests.rs`:**
  - `every_message_names_the_model_and_effort_of_the_invocation_that_wrote_it`. One actor has two
    invocations. The first resolves `default` and reports no effort, so it reads
    `claude-opus-5 none`. The second reads `glm-5.2 max`. An unlinked message
    reads `unknown model`, and `you` and `ymp` keep their names.
  - `a_live_stream_and_the_sidebar_name_the_invocation_that_is_running`
  - `tool_activity_names_the_running_invocation_and_keeps_the_members_purpose`
  - `a_choice_of_agent_is_its_concrete_model_and_never_a_caption_or_an_unresolved_alias`
  - `local_fixture_agents_keep_their_names_and_the_user_and_ymp_keep_theirs`
  - `an_unlinked_native_message_stays_unknown_after_its_actor_runs_as_a_local_fixture`, the review
    case: an old unlinked native message, then a provider change to Mock and a bound local turn.
    The old message stays `unknown model`, and the new one reads `Local fixture`.
- **`ymp-tui/src/label.rs`:** five unit tests covering choices, work labels, unlinked and local
  cases, rosters, and terminal-safe output.
- **`ymp-runtime/src/backend_tests.rs`:** `tool_activity_names_the_actor_by_id_and_never_by_its_caption`.
  It uses the scripted backend `Behavior::Tool`, with every actor caption set to
  `Default (recommended)`.

Ten existing terminal tests were updated to the new contract:

- `the_sidebar_shows_the_team_the_session_captured_not_the_edited_configuration`
- `the_session_total_and_the_agents_survive_a_small_terminal`
- `a_profile_with_no_native_reading_is_not_presented_as_a_named_agent`
- `the_whole_window_names_a_member_by_what_the_installation_returned`
- `a_finished_session_is_named_by_the_identity_its_turns_captured`
- `an_actor_whose_model_comes_from_its_execution_policy_is_still_named_natively`
- `a_reading_that_is_no_longer_current_says_so_rather_than_reading_as_native`
- `a_model_no_reading_lists_is_unknown_and_never_native`
- `a_scanned_model_is_shown_exactly_as_the_installation_resolved_it`, renamed from
  `a_native_name_is_shown_exactly_as_the_installation_returned_it`
- `turns_recorded_without_a_captured_identity_are_not_renamed_from_the_present_catalog`, renamed
  from `turns_recorded_without_a_captured_identity_keep_the_name_they_ran_under`. An
  assignment row without a captured identity now reads `unknown model none`, and
  the present catalog still never renames it.

`routine_coordination_is_collapsed_and_routing_is_hidden` changed only to match the new
`transcript::build` call. Model names appear only as test fixture values. Production code
hard-codes no incident model.

## Controls shown to fail before the fix

- **`/tmp/ymp132-ui-negative.log`, run before the presentation fix.** The terminal tests exited
  with 101: the message, stream and sidebar, and choice controls failed. The local-fixture control
  passed; it guards existing behavior. The CLI `message_labels` test also exited with 101.
- **`/tmp/ymp132-ui-negative-review.log`, run against temporary mutations.** All three cases
  exited with 101:
  - Pre-review unlinked-message fallback: the review case failed with `"Default (recommended)"`.
  - Tool branch disabled in the terminal: the status row read `"Using Bash"`.
  - Original engine tool branch (`"{agent.name} · {name}"`, no typed event): the runtime control
    received no tool activity.

  The mutated files were restored from backups before the final checks, and their SHA-1 checksums
  matched the backups.

## Final checks

Environment: `CARGO_TARGET_DIR=/tmp/ymp132-test-target CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. The checks ran on the tree
that was then committed as `1bf8d30`, with no edits in between.

- **Formatting.** `cargo fmt --all --check` exited with 0 (`/tmp/ymp132-ui-fmt.log`).
- **Lints.** `cargo clippy --workspace --all-targets -- -D warnings` exited with 0
  (`/tmp/ymp132-ui-clippy.log`).
- **Tests.** `cargo test --workspace` exited with 0 (`/tmp/ymp132-ui-test.log`). There were 33 test
  target runs, including doc-test runs: 441 passed, 0 failed, 2 ignored.
- **Parent check.** `python3 /tmp/ymp132-cli-label-check.py /tmp/ymp132-test-target/debug/ymp`
  passed (`/tmp/ymp132-ui-parent-check.log`). It printed 12 headers, all in the form
  `native-model-z max · KIND`, and six of them are `chat`. The binary was built at 10:55:57, after
  the sources were restored at 10:55:01.
- **Whitespace.** `git diff --check` found no problems.

Everything ran offline, with scripted fixtures and temporary application homes. No native provider
inference was made. The real `~/.ymp2`, the Downloads project, the task register, `README`, the
release notes and the version were not touched.

## Limitations

1. **Runtime text outside the tool branch is unchanged, because it is parent-owned.**
   `engine.rs:2097` sends the status `"{agent.name} proposal unavailable: …"`, and `engine.rs:2188`
   posts the ymp notice `"{author.name}'s proposal exhausted its revision attempts…"`. Both can
   still contain a caption. The notice is stored ymp message text and is not rewritten.
2. **Execution content still carries captions.** This is by design, because it is not
   presentation. Examples are the provider prompt `You are {agent.name}` (`engine.rs:1097`, with
   `engine.rs:966`) and `ymp ask` (`main.rs:535`). `ymp doctor --probe` prints `profile.name`
   (`main.rs:228`, `244`, `247` and `686`). `ymp catalog` output was out of scope and is unchanged.
3. **Some historical records have no known model.** Agent messages written before messages were
   linked, and turns recorded without a captured identity, read `unknown model`. Rosters (idle
   members, token rows and the sessions page) for actors whose latest turn captured no identity may
   still use the current configuration to recognize a local Mock fixture. Messages never do.
4. **A running turn's label reflects the latest trace read.** Before the installation reports its
   settings, the label shows the sent or requested concrete model with `none`. It
   updates on the next agent message, stream start or agent status event. There is no timer.
5. **The trace is read from the store often.** It is read again for each agent message, stream
   start and agent status. No Rust provider currently emits `ProviderEvent::Tool`, so this cost has
   not been exercised with real tool traffic or measured on very long sessions.

## Not verified

- The owner's store. For example, message 163 in session `0b24eea4` should read
  `claude-opus-5 none · chat` if its link resolves as described.
- An interactive terminal with real installations.
- An installed or release build.

## Follow-up: `none` for an unreported effort

At the owner's request, `6e9b396` changes the wording only. An invocation that reported no effort
now reads `MODEL none`, for example `claude-opus-5 none`, instead of `MODEL (effort not reported)`.
Effort still comes only from what the invocation reported. The assignment details keep requested,
sent and reported effort in separate columns. An installation that genuinely reports an effort
named `none` produces the same label, and only those details tell the two cases apart.

- **Control before the change.** `/tmp/ymp132-ui-negative-none.log`: with the updated
  expectations and the old wording, `label::tests::work_is_named_by_the_invocations_own_model_and_reported_effort`
  and `every_message_names_the_model_and_effort_of_the_invocation_that_wrote_it` failed
  (exit 101; the actual value was `... (effort not reported)`, the expected value `... none`).
- **Checks after the change**, in the same environment:
  - `cargo fmt --all --check` exited with 0 (`/tmp/ymp132-ui-none-fmt.log`).
  - `cargo clippy --workspace --all-targets -- -D warnings` exited with 0
    (`/tmp/ymp132-ui-none-clippy.log`).
  - `cargo test --workspace` exited with 0: 441 passed, 0 failed, 2 ignored
    (`/tmp/ymp132-ui-none-test.log`).
  - The parent check passed with 12 headers in the form `native-model-z max · KIND`
    (`/tmp/ymp132-ui-none-parent-check.log`).
