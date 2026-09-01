# Scientific mechanism map

ymp does not hypothesize that conversation makes agents generally intelligent. It tests whether
specific coordination mechanisms improve independently accepted results, calibration, recovery,
or cost beyond both a single participant and independent candidate selection under the same total
budget. Fluent transcripts and agent count are never evidence.

This map refines the behavioural hypotheses in [CONCEPT.md](../CONCEPT.md), the study boundary in
[PROJECT-CONTRACT.md](../PROJECT-CONTRACT.md), and the decisions in
[DECISIONS.md](../DECISIONS.md). It changes no frozen primary outcome.

## Falsifiable mechanisms

| Mechanism | Observable prediction | Controlled intervention | Primary outcome | Rejection condition |
|---|---|---|---|---|
| Useful evidence transfer | Correct message content changes the receiver's next action | Original message, no message, neutral replacement, shuffled message, and the same raw evidence delivered directly | Preregistered action change plus accepted outcome or calibrated abstention | No action change means no listening; action change without improved outcome, calibration, recovery, or cost means no task value |
| Decomposition and synthesis | The accepted candidate causally requires at least two attributable non-redundant contributions | Remove one branch; give the same artifacts to one blinded synthesizer | Exact candidate acceptance and contribution ablation | The best individual branch passes, branch removal changes nothing, or a single synthesizer matches the result at the same budget |
| Critique with preserved dissent | An independent challenge causes evidence-supported revision or abstention without erasing a correct minority position | Commit initial assessments, then remove/replace/shuffle the challenge or inject a false observation | Acceptance, audited false acceptance, and calibrated abstention | No listening, increased false acceptance, or revision driven equally by unsupported social pressure |
| Adaptive local allocation | Local offers and commitments reduce duplicate work and move remaining budget toward unresolved parts | Fixed or random assignment with the same participants, routes, and budget | Accepted-result rate and duplicated-work cost | No gain over fixed/random allocation, unequal resources, or roles that appear only because they were prescribed |
| Recovery after perturbation | Work and information paths reform after delay, false information, or participant loss | Delay, false observation, or participant removal against a neutral sham intervention | Acceptance and bounded recovery time/cost | No transfer or re-formation, hidden restart/budget, or independent search performs as well or better |

## Expected null and harmful strata

L1-L3 are calibration instruments, not experimental null strata. The separately frozen held-out
L4+ set contains sequential/null tasks on which no benefit or a negative effect is expected, as
well as decomposable tasks on which the mechanism could be visible. High single-agent ceilings,
tightly globally coupled changes, and merge or communication costs that consume the available
reasoning budget remain plausible null conditions. They are required controls, not inconvenient
exclusions. A weak oracle invalidates the whole experiment rather than forming another outcome
category.

## Spend-minimizing sequence

1. Validate L1-L3 oracles, planted defects, profile admission, assignment, budget conservation, and
   evidence accounting without treating any result as a three-arm observation.
2. Freeze the held-out L4+ decomposable and sequential/null task set before any diagnostic model
   call.
3. Run one exact weak-profile cohort through the three-arm protocol in
   [prt-001-weak-diagnostic.md](prt-001-weak-diagnostic.md), then stop negatively unless it exceeds
   both controls under the frozen diagnostic rule.
4. Repeat a qualifying weak effect on the same held-out task set with a stronger admitted profile;
   failure to transfer is an explicit weak-only stop.
5. Only after transfer, freeze eligible message episodes before inspecting intervention outcomes.
   Start with absence and neutral replacement; after positive listening, add shuffling and
   direct-evidence controls, then false information, delay, and participant removal.
6. Keep the frozen primary comparison, its strong-single baseline, outcomes, thresholds, seeds, and
   budgets unchanged.

## Change and stop rules

A coordination mechanism changes only after a reproducible failure of that mechanism on development
data, under a new version and fresh tasks. It is never tuned after seeing primary outcomes.

- A completed primary comparison below its frozen minimum effect against independent selection
  rejects the reliability mechanism claim.
- No positive listening rejects the communication-use claim.
- Listening without task value rejects the collective-reasoning claim.
- Improvement restricted to weak participants records a bounded weak-profile result and stops
  before message interventions.

The accepted ordering and ownership boundary are recorded in
[rdr-001-evaluation-stage-boundaries.md](rdr-001-evaluation-stage-boundaries.md).

No intervention stand, sampled episode set, or outcome evidence exists yet. The current status is
therefore a falsifiable research design, not evidence that self-organization occurs.
