# YMP-149 final acceptance

Accepted source: `46a5a5138aa1601854ebbd29cb493527ed588711`, independently checked
by Claude Code reviewer `8d8b7748-c2b4-4468-8585-4be62e5bbec0`. The parent integrated
`07025c5` and `46a5a51` into main as `781c5a2` and `0d3f9f4`. There were no conflicts.
A full Git comparison of Cargo.toml, Cargo.lock, ymp-rust, ymp-bridges and ymp-evals
between the candidate and integrated source was empty. No source was changed
while retaining the acceptance evidence. The unrelated owner request edit was
not staged or changed.

## Accepted behavior

The READING column and verbose provenance narration are removed from primary
agent/member rows; source and observation details remain in Inspect. PREFERRED
is exactly the known Config.team preference flag, displayed as true/false. It is
not a promise of current or future membership. The runtime already uses this
list to rank eligible candidates; its behavior was not changed. Without a session,
the sidebar identifies enabled preferred agents; a captured session continues to
show its recorded participants after the preference changes.

Missing metadata does not establish provider unavailability. Distinct recorded
model identifiers remain distinct when they share a prefix. Native names, effort,
raw messages, machine-readable output, usage and acceptance semantics are intact.
The implementation glossary lists retained detail and the new generated sort keys.

## Verification

The author's final fmt, strict Clippy and workspace test sequence passed: 579 tests
passed and two were ignored. The final terminal checks recorded 29 row cases and
19 table cases in each of Unicode and ASCII modes. See the implementation README.

The independent reviewer ran 21 focused tests and 21 additional terminal cases
in each marker mode. These exercised doctor without native probes, disabled but
preferred profiles, current-session attribution and configuration edits that do
not alter the captured team. A mutation restoring the faulty substring comparison
failed the historical-model test with exit 101. The parent inspected these logs,
reports and source scope; a byte-identical integration did not justify another
complete compilation/test sequence.

The retained [manifest](independent/manifest.json) binds the independent observer,
reports, captures and logs to their copied bytes. All runtime data came from
temporary mock fixtures. Initial observer runs had extraction defects, were not
product failures and are not counted as successful checks; their original
*-run1-extraction-error directories remain outside the repository.

## Limits and next delivery

No native catalog scan, model inference or real session operation was performed.
The missing-identity qualifier was reviewed in source; stale/unlisted states had
unit coverage but no independent terminal coverage. Native scan notices were not
executed. Linux was not tested for this change. Minor detail prose and existing
ASCII use of middle dots remain as recorded in the review; they do not invalidate
the accepted source behavior.

YMP-149 is source-complete. Live session membership and the /team strategy controls
remain YMP-146/YMP-148/YMP-150 work. The installed executable remains 0.4.6; release
builds, platform verification and installation have not been performed for this
change. No comparative product trial was authorized or run.
