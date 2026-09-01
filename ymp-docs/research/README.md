# Research

This directory owns the human-readable scientific layer of ymp: falsifiable hypotheses,
experimental protocols, intervention designs, analyses, results, and negative findings.

It does not redefine product behavior. Product requirements, architecture, protocol, security,
invariants, and roadmap remain in the parent directory. Machine-executable study manifests,
corpora, frozen assignments, compliance checks, and analysis programs remain beside their owning
Rust tool; research prose links to their exact evidence instead of copying it.

## File numbering

`README.md` is the only unclassified file. Every record uses `TYPE-NNN-kebab-title.md`, where the
three-digit number is the next unused index inside its type and is never reused:

- `RES-NNN-*` — research ideas, hypotheses, protocols, analyses, and results in this directory;
- `ADR-NNN-*` — accepted architecture decision records, reserved for a future ADR directory.

The scientific researcher creates only `RES-*` records. `DECISIONS.md` remains the canonical
decision log until an explicit migration; this naming rule does not silently turn research into an
architecture decision. Creating any record also updates its directory index.

## Current protocols

- [RES-001-calibration.md](RES-001-calibration.md) — development profile ladder, measured runs, and promotion
  boundary.
- [RES-002-weak-diagnostic.md](RES-002-weak-diagnostic.md) — diagnostic matched-budget comparison using weak
  participant profiles before the frozen primary study.
- [RES-003-mechanism-map.md](RES-003-mechanism-map.md) — falsifiable coordination mechanisms, interventions,
  expected null strata, and scientific stop rules.

## Standing review rule

The read-only Sol max scientific researcher is consulted before a hypothesis, oracle regime, arm,
budget comparison, metric, causal claim, or POC conclusion changes. Its analysis informs a work
contract but cannot substitute for executable checks, controlled interventions, or accepted
artifacts.
