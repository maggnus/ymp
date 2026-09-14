# YMP-146 recovery UI acceptance

Accepted candidate: `19370d53b906ce54d53332f3abf8763d47e96f65`.
Integrated into main as `a8367c4` without conflicts. The TUI and the terminal
scenario source match the accepted candidate exactly. Backend, core, runtime,
storage, providers, CLI, workspace, Cargo manifests and bridges were unchanged
by the UI commit.

The author passed formatting, strict Clippy and the workspace test suite: 632
tests passed and two existing tests were ignored. The author also passed 11 of
11 terminal checks in Unicode at 100x32 and ASCII at 60x18. Mutations cover stale
reads, hidden-session continuation, ignored holds and accidental edits of
starting preferences.

The independent reviewer accepted the candidate after five focused recovery
tests and both 11-case terminal scenarios on a separately built binary. The
review confirmed current-session add/remove/replace, pending departures, fresh
saved-plan review, effect inspection, one current-files confirmation followed by
ordinary continuation of the same session, idempotent retry, holds, cancellation,
session binding and run/Git conflict handling. Fixtures used temporary data and
no real provider.

Remaining low-severity limits are recorded rather than hidden: some event-loop
branches were accepted by source inspection plus terminal coverage; a project
lock conflict after authorization needs a dedicated future scenario; `Pick`
relies on surrounding session binding; and command IDs are not retained in TUI
memory across an application restart although durable outcomes remain in Store.
These do not invalidate the supported owner actions.

The current owner's poker session was not changed by this work. The installed
binary remains 0.4.6 and does not include the accepted UI. Linux, Windows, real
providers and real provider-process shutdown were not verified. Release and
installation follow the separate structured-response parser correction.
