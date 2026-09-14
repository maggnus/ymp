# YMP-163 independent implementation review

Date: 2026-09-14. Scope: read-only review of the final working-tree diff from
`c18fe31005fd5dec080936c4014ed321a129c7f3`. The reviewer made no file changes and
ran no native providers.

## Corrections required during review

The first pass found four issues that were corrected before the final verdict:

1. The definition digest excluded source `workspace` and `base_commit`; they now
   remain bound while lifecycle fields alone are excluded.
2. Old/new execution evidence was aggregated for the whole command set; it is now
   retained per command, every replaced old command must fail, and the complete
   proposed set must pass.
3. Owner pause or cancellation could be mistaken for a command failure; control
   errors are now rechecked and propagated without deciding the proposal.
4. Recovery matched saved review verdicts without the exact replacement evidence;
   the recovery key and lookup now include the proposal-specific evidence.

The final-attempt clarification also changed the result flow. An accepted replacement
does not create another production attempt. The board commit atomically invalidates
the previous result binding and creates a new, unique `ResultVersion.id` with the
same producer, artifact snapshots, trusted contract, `TaskAttemptRef` and attempt
counter. Revised ordinary checks and normal candidate review then run again.

## Final verdict

**ACCEPT — no required findings.** The final read confirmed:

- `ReplaceChecks` is the only lifecycle-insensitive action; legacy responsibility,
  assignment and additive `Revise` behavior is unchanged;
- proposal origin remains bound to the active executor, assignment, invocation,
  grant and exact task attempt;
- independent review is admitted once and a rejection does not select another
  reviewer for the same proposal;
- trusted contracts, budgets, owner controls, confirmation and reputation remain
  under their existing authority;
- the revised target and accepted siblings are not produced again;
- provenance, invalidation and rerun evidence use the exact replacement proposal and
  result binding.

The reviewer did not execute tests. Final formatting, Clippy and workspace tests are
owned by the implementing parent and recorded in `validation.md`.
