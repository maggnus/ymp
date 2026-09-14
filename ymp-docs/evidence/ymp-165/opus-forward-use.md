# Evaluation plan: proposed knowledge-retrieval policy for source-grounded document work

Prepared with the `ymp-trials` skill as a bounded forward-use check. The request is synthetic. Nothing was executed.

Skill inputs, read once from the uncommitted working tree on HEAD `ef1fc9065bb23d8c98d903f9109f5660e0846a70`:

| File | SHA-256 |
| --- | --- |
| `.agents/skills/ymp-trials/SKILL.md` | `89d79f5f283a6da9fbbadf362a3669722b0fb861949b9d4e73e223419439f63c` |
| `.agents/skills/ymp-trials/assets/evaluation-plan.md` | `6144e6184aa5e977d5fd422d54695aa45ffc96fdc9a9b1a2aab5c6fb04e7bbdb` |
| `ymp-docs/guides/session-trial-and-acceptance.md` | `19f23eb342f1e80608474a5e42eb762cf6b8fef378490f261b17b1e43662c976` |
| `AGENTS.md` | `e874d280365610c8e582800fd3861d9e85e41beb769d50c74965c4a30c4cb5b9` |

The only domain document consulted was `ymp-docs/architecture/memory-retrieval.md`. It describes the current policy `ymp.fts5@1`: the first 12 query terms joined with OR, at most 5 entries, and at most 8,000 supplied characters, recorded in the `memory_retrieval` events.

## Decision and scope

- **Question.** Does proposed retrieval policy P1 make document work better grounded in its sources than the current policy P0, under the same context allowance?
- **Decision informed.** Whether P1 deserves an authorized discriminating check. This plan does not decide adoption.
- **Mode.** Policy comparison is the primary question. It cannot be executed under the current authority, so the only work allowed now is a read-only retrospective appraisal of retained evidence, a diagnostic mode without new execution.
- **Primary endpoint for the future comparison.** On a frozen labelled task set, P1 supplies more relevant source entries than P0 and no more irrelevant, stale, foreign or unconfirmed entries, within an identical allowance of 5 entries and 8,000 characters.
- **Acceptance layers.**
  - Now: policy compliance (knowledge integrity) and evidence appraisal.
  - Later: policy effectiveness.
  - Out of scope until native execution is authorized: artifact quality, native completion and product experience.
- **Status.** Not executed. The product outcome is unknown. The analysis is limited to the appraisal below.

## Baseline and authority

- **Authorized.** Reading the repository, retained traces and the one past session with its document.
- **Not authorized.** Native calls, publication, repository or state changes, and seeding knowledge or reputation. Planning grants no further authority.
- **Phase B.** Offline deterministic replay requires explicit authorization for local execution, even without native calls.
- **Phase C.** A native paired comparison requires separate authorization for native calls and a resource policy. It is described here only as a design.
- **Bindings to record.** Captured retrieval implementation ID and version of the past session; source and executable of P0 and P1; corpus snapshot and hashes; scope rules for project, confirmation and retirement.
- **Confounders.**
  - The past session's native model knowledge and native context.
  - A corpus or labels that favour one query form.
  - A larger effective allowance.
  - Label leakage into the queries.

## System coverage

| Dimension | Scope | Reason |
| --- | --- | --- |
| Knowledge acquisition and retrieval | direct | Candidate, selected, supplied and referenced stages are the question |
| Verification and confirmation | direct | Grounding must be checked deterministically at claim/span level; accepted/unconfirmed is retained |
| Reputation and knowledge correction | indirect | Only qualified, unretired, in-scope entries may be supplied as knowledge; no credit is inferred |
| Goal and requirements | indirect | Requirements of the past document; full requirement integrity needs Phase C |
| Resources and efficiency | indirect | Supplied characters and entries under the same allowance; no token-cost claim |
| Persistence and reproducibility | indirect | Replay from recorded queries and a corpus snapshot |
| Planning, execution adapters, user experience | relevant-unobserved | Whether supplied knowledge changes agent decisions needs native work |
| Catalog, team, communication, concurrency, recovery, publication, packaging | outside-scope | No native execution or publication; retrieval does not change them in this design |

## Criteria

| ID / intent | Role | Requirement or hypothesis | Evidence method / counter-explanation | Verdict now |
| --- | --- | --- | --- | --- |
| E0 / evidence | Required now | Classify each existing evidence item as discriminating, inspection-only, non-discriminating or absent for this question | Map the 12 tests to criteria E1–E3. Because they are unrelated, the expected class is absent or non-discriminating | Pending read-only mapping |
| E1 / G5, knowledge integrity | Required | P1 supplies no foreign-project, retired, unconfirmed-as-confirmed or irrelevant entries | Phase B: P1 through the existing production-prompt-assembly negative fixtures. Counter-explanation: the corpus lacks hard negatives | Not exercised |
| E2 / G1, G5 | Primary (Phase B) | More relevant supplied entries per task than P0 at an identical allowance, with no increase in bad entries | Labels frozen before replay; paired per-task counts with uncertainty; no combined score. Counter-explanations: allowance change, label leakage, a corpus tuned to P1 | Not exercised |
| E3 / G1 | Conditional (Phase C) | Documents produced with P1 have fewer claims unsupported by supplied source spans | Deterministic claim-to-span check as confirmation; paired native tasks with P0/P1 under matched conditions. Counter-explanation: native model knowledge | Not exercised; not authorized |
| E4 / G2 | Exploratory | Supplied characters and retrieval latency are comparable | Values from the `memory_retrieval` events. Counter-explanation: a latency difference caused by the corpus rather than the policy | Not exercised |
| E5 / diagnostic | Required now | Retrospective analysis of the past session. Record the captured retrieval policy version; the supplied entries and included characters; whether any supplied entry is referenced in the document; whether the document's citations resolve to source spans | Read-only `ymp trace` and the document. Counter-explanation: grounding came from the prompt or native knowledge, not retrieval | Pending read-only analysis |
| G3, G4 | Outside scope | Time and intervention | Not affected by an offline retrieval replay; observable only in Phase C | Not exercised |

## Minimal sequence

1. **Now, read-only.** Complete E0 and E5, freeze E1–E3 and the labelling rules, and write this plan to durable project documentation once repository changes are authorized.
2. **Phase B, after authorization for local execution.** Replay recorded queries and a frozen corpus through the production prompt assembly with scripted providers. Run P0 and P1 on identical inputs, retain failures, and do not tune labels after seeing results.
3. **Phase C, only if Phase B passes and native calls are authorized.** Run a small paired comparison of document tasks with a declared resource basis and controls, including a retrieval-disabled control. Retain every attempt.

## What the current evidence supports now

- **12 unrelated retrieval unit tests passed.** This supports only the behaviors those tests assert at their source version. It says nothing about the effect of P1, and not even about P1's correctness unless a test exercises P1. For this question it is non-discriminating.
- **One past session completed with an accepted/unconfirmed document.** This supports a single native completion under whichever policy was captured. The document stays accepted but unconfirmed: no trusted confirmation and no reputation effect. It shows neither grounding quality (unknown until E5) nor any contribution from retrieval. If the session ran P0, it is baseline evidence; if it ran P1, it is n=1 without a comparator.
- **No controlled comparator.** Improvement and effect size are untested. Intent goals G1 (quality) and G5 (experience) are unsupported, not failed. G2–G4 are not assessed.
- **Supported decision.** There is no basis to claim improvement or to adopt P1 on effectiveness grounds. The next justified step is the read-only E0/E5 appraisal, followed by Phase B only after explicit authorization.

## Skill forward-use notes (not part of the plan)

- **Adaptation.** The skill fitted a non-game request without imposing a browser, publication or a native run (SKILL "Choose the scope before tools"; template preamble; protocol mode table and coverage labels).
- **F1, inconsistency.** The protocol study card still reads "Study mode: diagnostic case, regression replication or controlled comparison" (three modes), while its mode table, SKILL.md and the template use seven. The stale line pushes this request towards "controlled comparison", which is not authorized.
- **F2, gap.** No mode or step covers a plan-only or retrospective appraisal under limited authority. The discriminating/inspection-only/non-discriminating/absent classification is defined only for native acceptance evidence, not for pre-existing evidence against a question. It had to be applied by analogy (E0).
- **F3, minor.** The template's "Required, conditional or exploratory" uses "conditional", which the protocol does not define; the protocol distinguishes required from exploratory.
- **F4, minor.** The completion step requires updating `tasks.json` and `manage.py`, but gives no rule for an ad-hoc plan without a register entry or write authorization.
