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

## Current cumulative-knowledge decision

- **RDR-002** —
  [rdr-002-cumulative-knowledge-poc.md](rdr-002-cumulative-knowledge-poc.md) — current definitions,
  staged executable preparation, transfer and negative-transfer controls, honest outcome classes,
  and unapproved choices for the cumulative-knowledge POC.

No live cumulative-memory result exists yet. Exact tasks, corpus reuse, memory representation,
sample, budget, and decision rule remain open until the zero-model executable preparation is
complete and a new live authorization is granted.

## Historical protocols and calibration

The following records and their executable artifacts are immutable research history. They may
supply mechanisms or prior observations, but their same-budget, best-of-`11`, corpus-size, or
weak-to-strong requirements do not gate the current POC.

- **CAL-001** — [cal-001-calibration.md](cal-001-calibration.md) — development profile ladder,
  measured runs, and promotion boundary.
- **PRT-001** — [prt-001-weak-diagnostic.md](prt-001-weak-diagnostic.md) — diagnostic matched-budget
  comparison on a separately frozen held-out L4+ set using weak participant profiles before its
  frozen historical primary study.

## Supporting hypotheses

These hypotheses remain available for future bounded studies. They do not replace RDR-002 or set a
current delivery sequence.

- **HYP-001** —
  [hyp-001-model-dependent-communication-representation.md](hyp-001-model-dependent-communication-representation.md)
  — directional sender-profile × representation × receiver-profile hypothesis, typed research
  envelope, counterhypotheses, and transfer boundary.

## Supporting mechanism maps

These maps preserve earlier coordination questions and stop rules as research context. Any new
experiment selects only the part needed for the next cumulative-knowledge decision.

- **MAP-001** — [map-001-mechanism-map.md](map-001-mechanism-map.md) — falsifiable coordination
  mechanisms, interventions, expected null strata, and scientific stop rules.
- **MAP-002** —
  [map-002-coordination-research-queue.md](map-002-coordination-research-queue.md) — dependency-ordered
  coordination questions, transport and profile-admission gates, observable predictions, and
  scientific stops.

## Earlier research decisions

- **RDR-001** —
  [rdr-001-evaluation-stage-boundaries.md](rdr-001-evaluation-stage-boundaries.md) — accepted
  separation of calibration, held-out weak diagnosis, stronger-profile transfer, interventions,
  and the frozen primary comparison; superseded as a current delivery sequence by RDR-002.

## Execution records

- **RUN-001** —
  [run-001-manual-file-communication-pilot.md](run-001-manual-file-communication-pilot.md) —
  infrastructure-invalid exploratory file-communication pilot, its zero communication observations,
  and the resulting profile-admission stop.
- **RUN-002** —
  [run-002-first-authorized-live-tool-host-probe.md](run-002-first-authorized-live-tool-host-probe.md)
  — infrastructure-invalid first authorized live probe, its indeterminate failure phase, spent
  reservation, and diagnostic STOP.
- **RUN-003** —
  [run-003-w1-evl-04s-prelive-stop.md](run-003-w1-evl-04s-prelive-stop.md) — authorized
  W1-EVL-04s attempt stopped in deterministic preflight with zero live/model calls because product
  process-cleanup conformance remained unproven.

## Standing review rule

The read-only scientific researcher selected by current project instructions is consulted before a
hypothesis, oracle regime, experimental condition, metric, causal claim, or POC conclusion changes.
Its analysis informs a work contract but cannot substitute for executable checks, controlled
observations, or accepted artifacts.

## Current literature consultations

- **LIT-001** — [lit-001-cumulative-knowledge-minimum.md](lit-001-cumulative-knowledge-minimum.md) — Fable 5.1 advisory input for the revised cumulative-knowledge POC; no frozen experiment or observed result.
