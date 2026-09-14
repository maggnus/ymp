# Product decisions from the fifteen-puzzle investigation

The report must change the work plan. This document turns the retained findings
into sequenced decisions, acceptance conditions and research questions. The
[general system protocol](../guides/session-trial-and-acceptance.md) governs all
future evaluations; the game remains one diagnostic case.

## Immediate decision

Finish the authorized native repeat after accepting the cause repair. Do not
combine unrelated improvements into that repeat: otherwise the outcome will not
show whether the specific recovery path became usable. The first experiment
remains failed even though its unchanged game passed external checks.

The cause repair is integrated at `e063494`: independently reviewed generated
check replacement can be applied during the final permitted attempt, invalidates
obsolete evidence and rechecks the existing result. It preserves attempt counters,
accepted siblings, trusted contracts and the old semantics of other proposals.
The [implementation evidence](../evidence/ymp-163/validation.md) records the
passing reproduction, meaningful negative case and required project checks.

## Ordered work

| Order | Finding and user impact | Decision / expected change | Acceptance signal | Owning work |
| --- | --- | --- | --- | --- |
| 1 — now | A useful diagnosis and proposed check repair could not prevent a blocked session. | Integrate the accepted check-replacement mechanism, then run the authorized fresh case. | Native completion plus unchanged-artifact validation and truthful final state; failed repeat remains failed. | Check recovery YMP-163; repeat YMP-164 |
| 2 — next proposed correction | A deleted or unreadable temporary script made checks irreproducible and weakened review. | Make generated verification source persistent, versioned and narrowly accessible to the reviewer; bind it to its result. | A different reviewer can inspect and repeat the same check after the producer finishes; missing or changed source invalidates that evidence. | New YMP-166 |
| 3 — existing visibility work | Raw logs were needed to understand progress, blocked acceptance and actual contribution. | Integrate the existing statistics reader and make artifact availability, task acceptance, confirmation and actionable failure distinct in CLI/TUI. | A user can identify the unfinished requirement and next valid action without manually reading the database; per-agent totals reconcile with trace. | Statistics YMP-158; interface YMP-156 |
| 4 — policy candidate, not an assumed optimization | The mechanics task repeated production after the page already contained mechanics; useful verification and avoidable repair were mixed. | Use remaining requirements and evidence gaps to choose the next contribution. Allow explicit conversion to targeted verification, preserving independent acceptance. | On the same state, the candidate selects necessary remaining work; a comparison shows reduced unnecessary execution without extra escaped defects. | Coordination boundary YMP-148 |
| 5 — research after the mechanism works | Zero overlapping calls and eight stale proposals do not establish useful self-organization. | Compare actual adaptive coordination with a fixed workflow, holding team/tools/criteria/resource basis constant. Keep independent attempts as a distinct control when testing the benefit of communication. | Improved task-level outcomes, resources, time or intervention with uncertainty; proposals demonstrably change admitted work. Null or harmful effects are reportable outcomes. | YMP-148 mechanism; separately authorized comparative program |
| 6 — later experience study | All twenty knowledge retrievals were empty; no verified prior experience was available. | Test acquisition/reuse only with qualified history and held-out tasks, including absent/stale/irrelevant-history controls. | Better later results or amortized resource use attributable to usable verified knowledge; acquisition/review/correction costs included. | Existing YMP-202/YMP-204 research |

Rows after the authorized repair/repeat are a work proposal, not a claim of
implementation or authorization for additional native trials. Preserve existing
dependencies and separate decisions about budgets and experimental treatments.

## What each proposed change must preserve

The first repeat adds a concrete delivery finding: a copied build referred to a
Claude adapter absent from its build worktree, and failed only after Luna had
already planned. Native readiness needs the binary's external adapter paths and
dependencies, not just a successful Rust build. Preserve a safe specific missing-
adapter error rather than a generic provider failure. The next run binds the
existing compatible adapter through the supported environment override; a product
readiness/diagnostic correction is proposed separately from this launcher fix.
See the [repeat diagnosis](/Users/maggnus/ymp-research/fifteen-2026-09-14/repeat-20260914T084441Z-8cl8je80/failure-diagnosis.md).

**Verification artifacts.** Use existing workspace/evidence mechanisms rather
than opening general access to `/tmp`. Preserve declared access and native
authentication. Storage alone is insufficient: the reviewer must inspect the
specific code whose output supports acceptance. Start with one lost-check
reproduction and one changed-source case; do not create another large framework.

**Useful contribution selection.** Unchanged files are not automatically wasted
work. The baseline mechanics review found a real evidence defect. A candidate
must preserve that assurance while avoiding a redundant production assignment.
The measurable unit is a contribution to an unmet requirement, with its evidence
and cost, not the number of completed task cards.

**Observable completion.** Show that the artifact exists separately from whether
its required verification finished. A blocked status can be honest but unusable
without an actionable cause. Preserve the distinction between accepted and
confirmed; do not relabel every returned model answer as successful work.

**Self-organization.** Separate initiative, admission and value. An agent asking
to change work is evidence of initiative; a recorded runtime decision establishes
admission; only a comparison of outcomes can establish benefit. Count substantive
criticism that changed action, accepted/rejected proposals and their causes,
reallocation, obsolete-work avoidance, waits and intervention. Do not maximize
message volume, proposal acceptance or parallelism as ends in themselves.

The fixed roster does not imply fixed roles in a future study, but the baseline
reserved one of two agents for independent final review. Any changed role,
communication or access policy is a declared intervention. A greater team size,
effort change or extra hidden judge would test a different hypothesis.

## Priorities rejected for now

- Increasing attempts, effort or participant count does not repair an unusable
  check-revision path and would obscure the diagnosed cause.
- Installing a browser alias makes this local check pass but hides the product's
  inability to revise an environment-dependent method.
- Disabling independent review or accepting every replacement can remove the
  blocked symptom while weakening the original requirement.
- A global rewrite of task-version rules could silently admit old responsibility
  proposals and change effort; the current repair stays operation-specific.
- A learning-benefit claim from an empty knowledge baseline is unsupported.
- A performance claim based only on a faster second model run ignores stochastic
  plans, native context and changed environment; it is an observation, not an
  estimated causal saving.

## Evidence and update rule

Primary evidence is the durable
[baseline report](/Users/maggnus/ymp-research/fifteen-2026-09-14/baseline/report.md),
[requirement coverage](/Users/maggnus/ymp-research/fifteen-2026-09-14/baseline-native-evidence.md),
[quantitative analysis](/Users/maggnus/ymp-research/fifteen-2026-09-14/analysis/baseline-analysis.json)
and [reviewed repeat plan](fifteen-puzzle-repeat-plan.md).

After the repeat, update each decision as retained, revised or rejected, with the
new evidence and reason. Record whether the repair was exercised naturally or
only in its deterministic reproduction. Do not replace the first failed outcome
with the eventual successful one or silently promote proposed work to done.
