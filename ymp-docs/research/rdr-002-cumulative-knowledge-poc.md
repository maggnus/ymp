# RDR-002 — Cumulative-knowledge POC

Status: current research decision; no live collection or POC conclusion yet.
Decision date: 6 September 2026.

## Decision and sources

The primary scientific objective of ymp is to accumulate reusable knowledge across tasks and test
whether a frozen, provenance-bound memory transfers to genuinely new tasks. The product must also
provide a coherent terminal experience in which a person can understand the goal, progress,
evidence, remaining uncertainty, and exact result. These objectives follow the
[owner direction](https://github.com/maggnus/ymp/blob/27f974ce92541fdcee1427f0dbaa670ee176689d/ymp-docs/work/backlog/OWNER-DIRECTION-20260906.md)
and the approved
[user journey](https://github.com/maggnus/ymp/blob/27f974ce92541fdcee1427f0dbaa670ee176689d/ymp-docs/USER_JOURNEY.md).

The
[LIT-001 consultation](https://github.com/maggnus/ymp/blob/27f974ce92541fdcee1427f0dbaa670ee176689d/ymp-docs/research/lit-001-cumulative-knowledge-minimum.md)
is advisory input. Its two-task note carrier is a useful first feasibility probe, not the whole
cumulative POC. The suggested task pairs, corpus reuse, memory representation, sample size, and
budget are not approved experimental choices.

This decision supersedes same-budget superiority, independent best-of-`11`, 1,992 tasks per
stratum, and the mandatory weak-to-strong sequence as current delivery or POC-completion gates.
The frozen
[study protocol](https://github.com/maggnus/ymp/blob/27f974ce92541fdcee1427f0dbaa670ee176689d/ymp-rust/tools/ymp-corpus/corpus/study/PROTOCOL.md),
[manifest](https://github.com/maggnus/ymp/blob/27f974ce92541fdcee1427f0dbaa670ee176689d/ymp-rust/tools/ymp-corpus/corpus/study/manifest-v1.json),
[power report](https://github.com/maggnus/ymp/blob/27f974ce92541fdcee1427f0dbaa670ee176689d/ymp-rust/tools/ymp-corpus/corpus/study/power-report-v1.json),
and [execution records](https://github.com/maggnus/ymp/blob/27f974ce92541fdcee1427f0dbaa670ee176689d/ymp-docs/research/README.md#execution-records)
remain immutable historical evidence. Cost remains an outcome and a constraint; ymp need not beat
another condition at equal cost to complete this bounded POC.

## Operational distinctions

- **Candidate knowledge** is a bounded memory item derived from an observed source task. It records
  its exact bytes and digest, source task, attempt or candidate, supporting evidence, authoring
  procedure, intended scope, and known limits. Storage or delivery does not establish usefulness
  or truth.
- **Accumulation** means that successive source tasks produce frozen memory snapshots whose retained,
  revised, or superseded items remain traceable to their sources. A larger archive alone is not
  accumulation evidence: a later snapshot must contain usable provenance from more than one earlier
  point in the sequence.
- **Demonstrated transfer** means that access to a pre-frozen memory produces a favourable change in
  a predeclared observable on a target task that did not contribute to selecting, writing, or tuning
  that memory. An independent verifier judges the exact target candidate.
- **Negative transfer** means that, in a valid predeclared contrast, the experience-derived memory
  worsens the target outcome relative to its control. A runtime failure, absent instrumentation, or
  a participant choosing not to read available memory is not negative transfer.
- **Collective benefit** is a later and separate claim. It requires a control in which one agent with
  the same receiver profile receives the exact same memory bytes. More participants, a larger
  archive, or a fluent conversation cannot establish it.
- **Cost** includes model, execution, verification, storage, and human-review resources. It is
  recorded for every condition and failure but is not an equal-cost victory threshold.

## Prospective validity and handling rules

Before any live collection, one versioned experiment package freezes:

1. source and target tasks, their order, eligibility, compatibility, and permitted corpus reuse;
2. the memory representation, extraction rule, snapshots, digests, and provenance chain;
3. receiver and runtime profiles, fresh-session policy, conditions, order or randomization, exact
   trial count, resource limits, and stopping rule;
4. public requirements, protected target information, independent oracle, primary observables,
   decision rule, exclusions, and treatment of missing data; and
5. promised observations for delivery, availability, read or tool access, candidate production,
   verification, cost, and every terminal outcome.

Target tasks and protected inputs are frozen before the experience-derived memory is exposed. They
must be genuinely new to memory construction: overlap in repository, task family, fixtures, or
dependency lineage is declared and either excluded or treated as a stated limit. No target result
may alter the memory, task set, outcome rule, or stopping rule for that collection.

The minimum target contrast uses the same receiver profile in fresh sessions with: the
experience-derived memory; no memory; and a similarly sized generic memory. A predeclared disjoint
or irrelevant-memory condition may additionally test susceptibility to negative transfer. The
no-memory and generic-memory conditions are the required negative-transfer controls. Allowed
resources are frozen per condition and actual use is reported; unequal use is a limitation to
interpret, not an automatic win or loss.

All infrastructure failures remain in cost and outcome logs. A loss of runtime, isolation,
candidate, oracle, digest, or evidence integrity makes the affected observation
`infrastructure_invalid`; it is never recoded as a failed solution. Missing promised instrumentation
makes only the dependent claim unobservable and is reported separately. If memory is mechanically
available and the participant voluntarily does not read or use it, the run is behaviourally valid
and shows no demonstrated use. Read-back or echo proves byte access only, not understanding or
causal use.

## Staged proving sequence

1. **Executable preparation without models.** Select the smallest supported carrier after inspecting
   existing facilities; create the versioned experiment package; and use a deterministic runtime to
   reject changed digests, protected-input leakage, missing provenance, broken condition assignment,
   lost observations, and falsely successful infrastructure terminals.
2. **Carrier feasibility.** Under fresh authorization, run a bounded source-task → target-task probe
   with one frozen item. It can establish delivery, observability, and a first transfer observation,
   but cannot close the cumulative POC.
3. **Accumulation and new-task transfer.** Run the pre-frozen ordered source sequence and target
   conditions. A positive cumulative claim requires later memory to retain attributable knowledge
   from more than one earlier point and independently verified favourable transfer on genuinely new
   targets. The exact sequence length and repetition count are decided only by executable
   preparation and available resources.
4. **Bounded conclusion.** Report all valid observations, infrastructure-invalid runs, exclusions,
   costs, uncertainty, and negative-transfer checks. The conclusion is positive, negative, or
   inconclusive for the frozen task, memory, receiver, and oracle regime; it is never generalized
   from one compelling episode.

## Claim-to-evidence checks

| Claim | Required observation | Insufficient substitute |
|---|---|---|
| Candidate knowledge exists | Frozen bytes, digest, provenance, scope, and evidence | A note or archive entry without provenance |
| Delivery works | Target received the expected digest under the assigned condition | A source-side write or later byte echo alone |
| Accumulation occurred | Successive snapshots preserve attributable content from multiple earlier points | Archive size or task count |
| Transfer occurred | Predeclared favourable target contrast and independent verification | Fluent conversation, self-report, or passing source task |
| Negative transfer occurred | Valid predeclared harmful target contrast | Runtime or instrumentation failure |
| Collective benefit occurred | Multi-participant effect beyond one agent with identical memory bytes | More agents or messages |
| POC concluded | Valid frozen sequence, honest terminals, controls, costs, and bounded interpretation | Fake runtime, old frozen study, one probe, or a polished TUI |

## Unapproved choices and stop boundary

No task pair, corpus reuse, memory representation, sample size, live profile, total budget, or
decision threshold is approved by this record. Live collection remains closed until executable
preparation supplies those exact choices and receives fresh authorization. Preparation stops if it
would require open-ended research infrastructure instead of the smallest mechanism needed for the
next observable decision.
