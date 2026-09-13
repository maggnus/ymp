# YMP-146 second-round bounded corrections

Parent contract: `acceptance-round2.md`, read from the parent workspace without
changes. Baseline: `1dff2cbcb558639a74f46fe009372a71652cc4e7`. R4 is independently
accepted and its implementation is preserved.

## Completed malformed response

The independent probe was copied with verified hashes. Only output/temp path
literals are redirected; assertions and dependency versions are unchanged.
`malformed-before.json` records exit **101** on 1dff2cb: Failed+Unknown passes;
Completed+MalformedResponse is wrongly rejected before inspection. Originals and
exact copies remain immutable in `reviewer-originals`.

Storage now accepts Completed only with the exact recorded runtime
MalformedResponse, matching assignment/invocation and obligation binding, terminal
records, and no valid accepted/negative verdict for that invocation. All scope,
independence and current-state checks remain. No failure history is cleared.

`malformed-after.json` records two zero exits: the unchanged external probe
(two passing controls) and eight sustainable strict-inspection scenarios, including
the new completed-malformed positive and ordinary accepted/negative verdict
rejections. `malformed-source.json` binds the checked implementation and tests.

## Fresh saved-plan review and hold release

`Engine::review_saved_plan_fresh` records a new independent actual read-only verdict
for the exact saved proposal. It does not replay the failed invocation, create
scope/effect_resolution, set manual_permit or run production. Continue consumes
that saved verdict through a distinct typed state. Fresh commands bind session,
stage revision, full proposal, chosen non-author/non-failed reviewer and command ID.
Atomic admission records each attempted command to prevent a repeated native call
under the same ID, including unsuccessful attempts. Normal origin/verdict validation,
resource admission, access and owner constraints remain in force.

`RecoveryControl::ReleaseHold` restores the condition preceding explicit Wait/Pause
without retry permission or calls. The native test demonstrates this reachable
path; a global hold uses the existing owner-team Continue API.

`fresh-native-focused.json` records four passing tests through default native
adapters, a local ACP/Codex protocol fixture and real fixture-only model discovery.
An existing native conversation context is deliberately populated; the new review
still uses thread/start with read-only sandbox, never thread/resume. Saved failures,
proposal, usage and old invocation attribution remain unchanged. No local scope
is supplied. The no-authorization continuation saves the graph and reports typed
unknown dependencies without production. Negative controls cover stale/foreign
inputs, self/failed reviewer, unknown termination, holds and exhausted resources.

Intermediate fixture failures are retained: initial allocation selected a healthy
reviewer until the initial roster was explicitly pinned; owner addition initially
lacked a resolved native model until the fixture used its actually scanned model.
Compile iterations caught connection-guard coercion and helper visibility; these
are not counted as successful behavioral controls.

## Owner-authorized new work from current files

The owner subsequently approved Continue with current files. The earlier typed
write refusal remains the default/negative control; a distinct scoped owner
command now permits ordinary new work without resolving old effects. The actual
API and reachable hold-release flow are documented in [API.md](API.md).

The reviewable context binds canonical workspace file metadata, full saved
stage/proposal/result, board/team revision, exact failures and actual budget.
File contents are hashed with the existing adapter and are not copied into
application storage. Project/session ownership and a storage transaction reject
changed context, active/unknown local execution, unreleased prior access and
owner holds. Durable idempotent receipts survive restart. Authorization sets
neither manual_permit nor effect_resolution and never invokes a provider itself.

Only exact acknowledged failures are covered. Subsequent uncertain failures
remain blocked in the runtime and at native admission. The latter also enforces
the no-authorization case once a fresh-plan-review stage exists. An acknowledged
uncertain actor cannot reuse its previous native context for new work.

`final-focused.json` binds the complete candidate source to four successful
commands: 12 default-native protocol scenarios, 33 session_recovery scenarios,
the unchanged two-case external strict-inspection probe, and both saved
revision/arbitration subcases of the preserved independent probe. All exits are
**0**. The accepted R4 storage implementation and its regression file remain
byte-identical to 1dff2cb.

The positive native scenario issues the owner command, reopens Store/Engine and
runs the same session to actual task completion with a real temporary file write.
It asserts one original planner, no old ACP replay, one saved fresh verdict,
unchanged historical invocations/uncertainty, no scope/resolution backfill,
unchanged limits and no reputation. Negative controls retain the default refusal,
unknown and actively running native work, unreleased access, changed files or
revision/context, foreign/reused commands, holds and explicit hold release,
resource exhaustion, negative verdict/malformed-attempt idempotence, native
backend-version changes and a later unacknowledged write-capable failure. The old
receipt is replayed after that later failure without expanding its scope.

`owner-authorization-falsifier.json` records a real source mutation ignoring the
owner authorization. The positive end-to-end test then exited **101**, stopping
at unknown dependencies without execution. Source was restored byte-for-byte
with matching SHA-256. After a subsequent admission hardening, the complete
focused sequence passed. No successful suite was repeated for documentation edits.

## Final verification and limits

The first final sequence passed fmt and strict Clippy but workspace tests exited
**101** at the existing location-export control with a stack overflow. The exact
focused test reproduced it. The full fresh-review proposal had enlarged every
cloned Engine/async frame; sharing that immutable command via Arc fixes the
regression. The unchanged test then passed without increasing the stack limit.
Logs and source hashes are in `export-stack-before.json`/`export-stack-after.json`.
The required final sequence was repeated only after this source correction.

Final executable source: `edc779a2da5b101ad50a5803427b4fe4354d46f3`.
All final commands exited **0**:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: **624 passed, 0 failed, 2 ignored**.

`final-checks.json` records the actual command arguments, times and logs.
`source-verification.json` binds the 15 source/test files changed in this round,
checks the exact write zone, confirms the preserved R4 files and project contract,
and verifies all 16 reviewer-owned original artifacts. The two ignored tests are
the existing fixture-only Claude SDK scan requiring bridge npm setup/build and
the optional fixture-retention helper. No real-provider probe was added.

Only evidence/documentation changes follow the checked source commit. Authored
command logs have trailing blank lines normalized; reviewer originals remain
byte-identical. No checks are repeated for these documentation-only edits.
The next step is independent parent acceptance and integration; this delivery
does not assert that the owner's real session has been resumed. No real provider inference, user
home/session access, integration, installation, UI, dependency or release change
is included. Linux and actual native process/external-effect guarantees remain
unverified. Parent owns independent final acceptance and UI/integration.
