# YMP-149 independent review: bounded RETURN

Candidate: `07025c5aaa0202481d80985187f8b3f52b589641`, baseline `27f8454`.
Reviewer: `332e0b99-79a7-4255-82ae-be642ff7707e`, Claude Code claude-opus-5 high.
The parent reviewed the actual source and resolves the findings below. No native
provider inference or installed-binary modification was part of the review.

## Required semantic corrections

1. **Preferences are not a future roster.** Replace NEXT TEAM with PREFERRED and
   retain known true/false values for Config.team. Details and action messages
   describe a starting selection preference. Keep no-session headings/sidebar
   consistent with that scope, including filtering of disabled profiles; keep
   actual captured-session membership labels intact. Do not change runtime
   allocation or implement UI150 in this copy-edit task.
2. **Missing metadata is not agent unavailability.** identity_cell(None) must
   identify a metadata/read problem, not label the agent unavailable. Retain
   diagnostic details and no replacement source-status column.
3. **Historical model prefixes are distinct.** Replace substring suppression
   with exact or structured model comparison so gpt-X and gpt-X-suffix cannot
   hide a different recorded model. Keep origin and actual effort unchanged.
4. **Catalog assertions must inspect values.** The word catalog in a field label
   is not evidence for that field's content. Preserve a meaningful consumer-level
   assertion after shortening the value.

## Correction to the review's runtime premise

The review claimed runtime never reads Config.team. This is false at the reviewed
commit: runtime/engine/allocation.rs90–129 uses self.config.team to sort eligible
candidates by starting preference. The source access spans lines. The observed
session [atlas,dorado] from preferences [atlas,disabled boreal] does not contradict
that behavior: preference does not constrain the roster or require disabled
participants. The observed discrepancy does show why NEXT TEAM promises too much.

The parent rejects the proposed `not used to form sessions` explanation. PREFERRED
states the actual weaker semantics without exposing an implementation-field name.
The reviewer was asked to reconcile this specific source fact; other findings and
original probe artifacts remain preserved.

## Retained successful evidence and limits

The reviewer confirmed READING removal, actual ENABLED on/off, sort keys A/P/E/N
for this candidate, provider filtering, Inspect routing, missing-field/default
wording, doctor availability and unchanged JSON/MCP/runtime/storage behavior.
Nine targeted semantic tests passed; the author had already passed the workspace
and Clippy checks before a whitespace-only test rewrap. The review's first filter
failure was observer input accumulation, not a product defect.

Unverified: pool-read failure and stale/unlisted metadata in a real terminal,
independent ASCII walk, native scan/init output and whether the CLI count means
attempted versus successful providers. Use truthful neutral count wording where
needed, without launching real native scans. Preserve a real source-backed
distinction instead of a shorter but unsupported status.

Rework stays in the existing UI fork. Repeat targeted checks and required final
checks after actual source changes, retain prior evidence, and return for review.
The parent has not accepted or installed this candidate.
