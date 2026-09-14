# YMP-201 preparation and native-consumer acceptance

Accepted candidate d12d40a997c3d3356991c389b022615e8042eb20, executable consumer
75f60a6972d8d58cec654330af60a6041a2c5274. Parent integrated it into main with normal
merge d48b081, preserving the implementation history. A full comparison of Cargo,
ymp-rust, ymp-evals and ymp-bridges against the accepted candidate is empty.
No code repair was needed during integration. The owner request edit was preserved.

The [independent final review](independent-final/review-result.json) accepts C1,
C2 and C3. Sixteen targeted Rust tests, ten restricted-Python tests, the nine
previous outcomes and independent paired private-canary controls passed. Valid
negative outcomes continue the matrix; unknown accounting and broken controls
stop. Usable time and metadata rules are shared. Candidate execution is confined
in selection/scoring/direct probing, expected values stay with the trusted scorer,
and supervised child processes terminate. Underuse retains requested and actual
participant counts. The one-phase authorization marker survives output removal.

The reviewer verified final native/executable/wrapper/Python/sandbox and exact
proposal bindings, and both unapproved native commands refused before provider
work. The author previously passed fmt, strict Clippy, 643 Rust tests (two existing
ignored) and 62 Python tests on this source. An unchanged source integration does
not require repeating those complete suites. The parent copied the independent
artifacts only after checking each recorded SHA-256. Original before controls
remain unchanged.

This accepts the preparation and executable measurement path. It does not measure
model quality, allocate quota, operate on the original user session or install a
release. macOS is verified; Linux, native outcome quality, monetary cost and
in-flight billed-token overshoot are unmeasured. The installed ymp remains 0.4.6.
YMP-201 is not complete until the authorized study produces its scoped result.

The next action is the owner's explicit decision on the two
[exact phase proposals](proposed-run/README.md). Until that answer, no real approval
record, native phase marker, calibration or measured call is created. Execution
must use the accepted frozen checkout and executable; do not silently rebuild,
substitute models, alter manifests, repeat a phase or enlarge its allowance.

## Subsequent owner model correction

The owner selected Sonnet 5 instead of Astra on 2026-09-14. The acceptance above
remains evidence for the previous Codex-only runner and its source. The old exact
manifests are no longer the active spending proposal. The same isolated author
is adapting the strong baseline to the existing Claude provider; no native study
has started and the model correction itself allocates no quota.
