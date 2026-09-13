# YMP-146 backend acceptance and integration

Accepted candidate: `743543fe1c3f6c11e9bb7663e89c5b79605d1ee8`.
Executable source: `edc779a2da5b101ad50a5803427b4fe4354d46f3`.
Parent source integration: `c28f501`; all 13 commits from 15f7c22..743543f were
cherry-picked in order without conflicts. Runtime, core, storage, providers and
Cargo files match the accepted candidate exactly. UI, CLI, bridges and evaluation
source match accepted YMP-149 source at 0d3f9f4. No source repair was needed during
integration. The unrelated owner request edit was not staged or altered.

## Independent acceptance

The [authority review](authority-round2.md) accepted the scoped owner boundary,
idempotency, racing owner controls, hold preservation and failure coverage.
The executable reviewer independently accepted the recovery/inspection behavior:
[full result and hashes](independent-round2/review-result.json).

Forty-nine checks passed: 12 default-native local protocol scenarios, 33 recovery
regressions and four external controls. The latter additionally preserve nonzero
historical usage together with unknown ACP usage and require plan revision after
an actual negative verdict, even when current-files authorization exists. The
reviewer verified 15 source hashes, 16 earlier artifacts, the preserved R4 files
and the author's restored failing mutation. Original external artifacts were
copied only after matching each SHA-256 in the review result.

A genuine legacy-shaped ACP review with no local_effect_scope can receive a fresh
independent read-only plan review and then, after the explicit owner decision,
execute tasks in the same session across restart. Positive review does not cause
another planning call; no old ACP invocation is replayed. Historical uncertainty,
limits and attribution remain intact and no reputation is invented. A valid
negative verdict remains a revision obligation. Missing authorization, unknown or
active prior work, stale context, owner holds, exhausted budget and new uncovered
failures retain their relevant refusals.

The author's complete fmt/strict-Clippy/workspace sequence passed with 624 tests
and two existing ignored tests. The parent assigned one additional complete
integration sequence because main combines this backend with separately accepted
UI changes. That sequence passed: fmt and strict Clippy exited zero, and 627 workspace
tests passed with zero failures and two existing ignored tests. The reviewer
verified 218 unchanged execution inputs; this is integration evidence, not a
repeated standalone acceptance of the same candidate. Exact commands, source
bindings and raw logs are retained under [integrated/](integrated/).

## Delivery limits

Backend source is accepted and integrated. YMP-146 remains in progress until its
minimal owner-facing UI is implemented and independently accepted. The local APIs
are not yet available through the current TUI. Follow
[the UI contract](../../architecture/session-recovery-ui.md) and the final
[API description](rework/round2/API.md); earlier author API notes are historical.
YMP-154's longer slash-command list follows the current task, as requested.

All recovery checks used temporary data and local protocol fixtures on macOS
arm64. Linux, real provider processes and the owner's actual failed session were
not tested or modified. Current-files context size/exclusion limits and the
separate conversation admission behavior remain as recorded by the authority
review. Owner authorization is not effect confirmation or a filesystem freeze.
The installed executable remains 0.4.6. No release build, installation, native
comparative trial or real-session continuation is implied by this acceptance.
