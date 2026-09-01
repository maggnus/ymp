# Research

This directory owns the human-readable scientific layer of ymp: falsifiable hypotheses,
experimental protocols, intervention designs, analyses, results, and negative findings.

It does not redefine product behavior. Product requirements, architecture, protocol, security,
invariants, and roadmap remain in the parent directory. Machine-executable study manifests,
corpora, frozen assignments, compliance checks, and analysis programs remain beside their owning
Rust tool; research prose links to their exact evidence instead of copying it.

## File numbering

`README.md` is the sole exception. Every record uses `ccc-NNN-kebab-title.md`: a lowercase category
first for sorting, followed by the next unused three-digit index in that category. Each category has
its own permanent, non-reusable `001`–`999` sequence. Before creation, inspect current `main`; one
document has exactly one immutable primary category, and this index records its exact link and full
identifier.

The closed research categories are:

- `rdr` — research decision record;
- `hyp` — falsifiable hypothesis or counterhypothesis;
- `prt` — experimental protocol;
- `exp` — experiment design;
- `run` — one execution record;
- `ana` — analysis of observations;
- `res` — synthesized result or conclusion;
- `lit` — literature review;
- `cal` — calibration method or evidence;
- `map` — mechanism map or scientific taxonomy.

Architecture `adr` and general product `rfc` records remain outside this research directory.
`DECISIONS.md` stays canonical until an explicit architecture-record migration. Creating a research
record also updates this index.

## Current protocols and calibration

- **CAL-001** — [cal-001-calibration.md](cal-001-calibration.md) — development profile ladder,
  measured runs, and promotion boundary.
- **PRT-001** — [prt-001-weak-diagnostic.md](prt-001-weak-diagnostic.md) — diagnostic matched-budget
  comparison on a separately frozen held-out L4+ set using weak participant profiles before the
  frozen primary study.

## Current hypotheses

- **HYP-001** —
  [hyp-001-model-dependent-communication-representation.md](hyp-001-model-dependent-communication-representation.md)
  — directional sender-profile × representation × receiver-profile hypothesis, typed research
  envelope, counterhypotheses, and transfer boundary.

## Current mechanism maps

- **MAP-001** — [map-001-mechanism-map.md](map-001-mechanism-map.md) — falsifiable coordination
  mechanisms, interventions, expected null strata, and scientific stop rules.
- **MAP-002** —
  [map-002-coordination-research-queue.md](map-002-coordination-research-queue.md) — dependency-ordered
  coordination questions, transport and profile-admission gates, observable predictions, and
  scientific stops.

## Research decisions

- **RDR-001** —
  [rdr-001-evaluation-stage-boundaries.md](rdr-001-evaluation-stage-boundaries.md) — accepted
  separation of calibration, held-out weak diagnosis, stronger-profile transfer, interventions,
  and the frozen primary comparison.

## Execution records

- **RUN-001** —
  [run-001-manual-file-communication-pilot.md](run-001-manual-file-communication-pilot.md) —
  infrastructure-invalid exploratory file-communication pilot, its zero communication observations,
  and the resulting profile-admission stop.

## Standing review rule

The read-only Sol max scientific researcher is consulted before a hypothesis, oracle regime, arm,
budget comparison, metric, causal claim, or POC conclusion changes. Its analysis informs a work
contract but cannot substitute for executable checks, controlled interventions, or accepted
artifacts.
