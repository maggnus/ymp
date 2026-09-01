# Research

This directory owns the human-readable scientific layer of ymp: falsifiable hypotheses,
experimental protocols, intervention designs, analyses, results, and negative findings.

It does not redefine product behavior. Product requirements, architecture, protocol, security,
invariants, and roadmap remain in the parent directory. Machine-executable study manifests,
corpora, frozen assignments, compliance checks, and analysis programs remain beside their owning
Rust tool; research prose links to their exact evidence instead of copying it.

## Current protocols

- [CALIBRATION.md](CALIBRATION.md) — development profile ladder, measured runs, and promotion
  boundary.
- [WEAK_DIAGNOSTIC.md](WEAK_DIAGNOSTIC.md) — diagnostic matched-budget comparison using weak
  participant profiles before the frozen primary study.
- [MECHANISM_MAP.md](MECHANISM_MAP.md) — falsifiable coordination mechanisms, interventions,
  expected null strata, and scientific stop rules.

## Standing review rule

The read-only Sol max scientific researcher is consulted before a hypothesis, oracle regime, arm,
budget comparison, metric, causal claim, or POC conclusion changes. Its analysis informs a work
contract but cannot substitute for executable checks, controlled interventions, or accepted
artifacts.
