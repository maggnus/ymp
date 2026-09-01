# Research

This directory owns the human-readable scientific layer of ymp: falsifiable hypotheses,
experimental protocols, intervention designs, analyses, results, and negative findings.

It does not redefine product behavior. Product requirements, architecture, protocol, security,
invariants, and roadmap remain in the parent directory. Machine-executable study manifests,
corpora, frozen assignments, compliance checks, and analysis programs remain beside their owning
Rust tool; research prose links to their exact evidence instead of copying it.

## File numbering

`README.md` is the only unnumbered file. Every research idea, hypothesis, protocol, analysis, or
result uses `NNN-kebab-title.md`, where `NNN` is the next unused three-digit index from `001` to
`999`. Numbers are permanent and never reused; creating a file also updates this index.

## Current protocols

- [001-calibration.md](001-calibration.md) — development profile ladder, measured runs, and promotion
  boundary.
- [002-weak-diagnostic.md](002-weak-diagnostic.md) — diagnostic matched-budget comparison using weak
  participant profiles before the frozen primary study.
- [003-mechanism-map.md](003-mechanism-map.md) — falsifiable coordination mechanisms, interventions,
  expected null strata, and scientific stop rules.

## Standing review rule

The read-only Sol max scientific researcher is consulted before a hypothesis, oracle regime, arm,
budget comparison, metric, causal claim, or POC conclusion changes. Its analysis informs a work
contract but cannot substitute for executable checks, controlled interventions, or accepted
artifacts.
