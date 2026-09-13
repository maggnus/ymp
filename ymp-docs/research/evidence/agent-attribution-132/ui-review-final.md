# YMP-132 UI model naming: independent review

**Verdict: accept, with one Medium correctness finding (F1).** F1 never shows a wrong model or a caption and changes no records, so I would not block the 0.4.2 installation on it. It should still get a narrow fix before YMP-132 is declared complete.

## Scope

- **Files reviewed.** `label.rs` and its wiring in `provenance.rs`, `state.rs`, `transcript.rs`, `views.rs`, `sidebar.rs`, `usage.rs` and `ui.rs`, the headless `ymp run` output in `ymp-cli/src/main.rs`, and the tests for all of these.
- **Revision.** `5899589`, and all line numbers refer to it.
  - The naming source is `1bf8d30` + `6e9b396`, integrated as `744a982` + `8f5af28`.
  - `5899589` itself changes only the Claude bridge and the tool forwarding in `rpc.rs`.
- **Method.** The review was read-only. I wrote YMP-131, not YMP-132.

## Findings

### F1 (Medium, confirmed by reading): a follow-up that opens a new session makes the previous session's agent messages read `unknown model`

**How it happens:**

1. `Route::FollowUp` keeps the transcript (`state.rs:1932-1938`). Only `Route::NewRun` clears it (`state.rs:1939-1944`).
2. `engine.follow_up` answers a `task` decision with `run_internal(path, &task, None, Some(previous), None)`, which records a new child session (`engine.rs:572-582`, `:667`, `:696`).
3. The first `UiEvent::Message` from the child session sets `opened`. It keeps the old messages and calls `refresh_session_facts`, which calls `refresh_attribution` (`state.rs:490-511`, `:685`, `:692-695`).
4. On a session change, `Attribution::refresh` clears every cached link (`provenance.rs:47-53`). `resolve` then resolves only messages from the new trace's session (`provenance.rs:70`).
5. `transcript.rs:173-174` therefore passes `None` to `label::author`.

**Result:**

- Every agent message from the previous session that is still on screen reads `unknown model · KIND`, even though its link is stored.
- Its origin line says the message is unlinked (`transcript.rs:166`).
- Before the follow-up, the same message read its model, for example `claude-opus-5 none · chat`.

**What is not affected:**

- Loading either session again, or restarting and resuming, shows the right names. `load_session` replaces the messages with that one session's own.
- Headless output is correct, because it reads the trace of each message's own session (`main.rs:636-639`).

**Tests and fix:**

- No test covers a session change within one window.
- Suggested fix: key the cached links by session and message sequence, and resolve each pending message against its own session's trace.
- Suggested regression test: a linked message from the first session keeps its label after a message from a second session arrives.

### F2 (Low; the author's limitation 4): labels follow the last trace read

- `ProviderEvent::Execution` records reported settings without sending a UI event (`engine.rs:1346-1351`). The terminal reads the trace again only on:
  - an agent message (`state.rs:505-511`);
  - a stream start (`:525-528`);
  - an `AgentStatus` event (`:530-532`).
- **Stream and working labels.** They can show the sent model with `none` until the next read. Since `5899589`, Claude tool hooks add reads for turns that use tools.
- **Messages already linked.** `provenance.rs` caches the whole `AgentAttribution`, including the reported settings, and never resolves a cached message again in the same session (`provenance.rs:72`).
  - If an invocation reports its settings after its message was linked, that message keeps the earlier label until another session is loaded or ymp restarts.
  - I did not establish that any current provider reports in that order.

### F3 (Low; the author's limitation 3): some names come from the current configuration, not captured evidence

- **Local detection.** When an actor's latest assignment captured no identity, `local_actor` decides that it is a local fixture from the current provider configuration (`label.rs:275-277`).
  - `label::profile` does the same on the sessions page (`views.rs:800`, `label.rs:80-81`).
  - It then prints the captured profile name, which can be a caption for a provider-named actor.
- **Aliases on the sessions page.** In every other case, `label::profile` prints the configured `profile.model` (`label.rs:83-87`).
  - An alias other than `default`, such as `opus`, is shown as the alias.
  - The team and agents pages show the identity's `resolved_model` for the same actor.
- Messages are not affected.

### F4 (Low): decision and board rows use the roster rule

`actor_name` (`views.rs:5778-5788`) calls `label::agent`, which names the actor by its latest assignment. An older decision can therefore show the model of the same actor's later turn. Messages are not affected.

### Test gap

- TUI tests link messages only through `invocation_message` (`tests.rs:7252`, `:7266`, `:7352`, `:7687`).
- No TUI test covers the `message_seq` grouping for `TeamOperationCommitted` links, which are the shared chat posts (`provenance.rs:81-95`).
- The CLI test reaches those links only through `SessionTrace::message_attribution`.
- The owner's store confirms the grouping key: message 163 is named by exactly one event, `provenance`/`team_operation_committed`, which carries `message_seq` at the top level.

## Verified against the contract

- **Scans and choices** show raw resolved IDs (`label.rs:35`, `:70`). Pool entries still on the unresolved `default` alias are left out unless they are already members (`views.rs:1531`).
- **Work labels** are the model plus the reported effort only (`label.rs:218`, `:243`, `:249`).
  - A missing effort reads `none` (`label.rs:26`).
  - The details keep stored absence apart from a reported value (`views.rs`, `setting_state`).
- **`default`** is never a primary name (`label.rs:29`, `:47`). In the details it reads "the installation's default alias" (`views.rs:1827`).
- **Captions, provider IDs and actor IDs** appear only in the details and the origin line (`transcript.rs:162-166`).
- **Messages:**
  - Each message is named by its own linked invocation (`transcript.rs:173-174`, `label.rs:121`).
  - Unlinked history reads `unknown model`.
  - A local fixture name in a message requires a linked `Local` identity (`label.rs:106`).
- **Streams and working members** use the unique running invocation (`label.rs:135`, `state.rs:797`).
- **The tool branch** sends the stable `agent.id` and `tool: NAME`, with no caption (`engine.rs:1360-1368`). The status row shows the running invocation's label (`state.rs:533-538`).
- **Owner's store** (read-only), session `0b24eea4-0c01-4d9e-b137-0f432cd7d2ac`, message 163:
  - reported model `claude-opus-5`, no reported effort;
  - identity model `default`, resolved to `claude-opus-5[1m]`, caption `Default (recommended)`;
  - by these rules the heading is `claude-opus-5 none · chat`, as expected.

## Limits

- I ran no builds or tests, so F1 is confirmed by reading only.
- `/tmp/ymp042-tests-final.log` shows 458 passed and 0 failed. I did not re-check the CLI heading or PTY logs.
- For `4f613fd` I checked only the diff statistics: 2 lines in `engine.rs`. Its content is outside this review.
- Performance was not assessed.
- No native inference was run, and no interactive terminal with real installations or installed build was used.

## Addendum: cache correction `28075bf`, integrated as `0c1c135`

**F1 and the message-cache part of F2 are closed.** I inspected only this commit, which changes `provenance.rs`, `transcript.rs` and `tests.rs`. `git patch-id --stable` gives the same ID for `28075bf` and `0c1c135` (`d552cd37…`).

- **F1 is fixed.**
  - Links are now keyed by session and message sequence (`Attribution::message(&Message)`).
  - Each refresh drops the links of messages no longer shown.
  - A shown message from another session is resolved once, from that session's own `store.trace`.
  - After a follow-up opens a new session, the earlier messages keep their labels.
- **F2, message cache, is fixed.**
  - Each link records whether its invocation was still `Running` when it was read.
  - A link in the active session is resolved again on every refresh until its invocation stops, so settings reported after the message was linked replace the first read.
  - Links in other sessions are not read again. A follow-up starts only when no run is active (`lib.rs:249`).
  - A failed read keeps the links already read and leaves unresolved messages for the next refresh.
- **Unchanged.** Stream and working-member labels still reflect the last trace read, because `ProviderEvent::Execution` sends no UI event. This is the author's documented limitation 4, not a cache defect.
- **Tests.** I read these tests but did not run them.
  - `a_conversation_that_moves_to_a_new_session_keeps_naming_the_earlier_messages`:
    - drives the F1 path through `UiEvent::Message`;
    - also resolves an earlier-session message from an empty cache.
  - `a_model_and_effort_reported_after_a_message_was_linked_rename_that_message`:
    - covers a late report picked up through `AgentStatus`;
    - checks that the label stays after the invocation completes.
  - `a_shared_chat_post_is_named_by_the_invocation_its_team_operation_committed`:
    - posts through the real `team_call`;
    - asserts that only the `provenance` event names the post;
    - closes the test gap noted above.
  - Reading the old code, the first two tests would fail on `5899589`: the first with `unknown model`, the second with `glm-5.2 none`.
- **Limits.**
  - F3 and F4 are tracked in YMP-133.
  - I ran no builds or tests. The fmt, Clippy and test results come from the author and the parent's integration.
