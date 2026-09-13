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

## Remaining second-round work

The separately authorized fresh saved-plan review is implemented. Next implement
the owner-approved Continue with current files authorization. It must not create effect_resolution or historical local scope, reuse
native context, run production, or bypass holds/budgets/unknown dependencies.
The parent has been asked to confirm the concrete downstream dependency boundary:
current tasks do not model dependencies on unknown effects of older invocations.
The final required check chain is reserved for the complete second-round source.
