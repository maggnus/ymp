# Outcome evidence, not grade

The original design proposed a persistent per-skill grade that both summarized acceptance outcomes
and limited which tasks an agent could claim. That mechanism is removed.

A grade-based claim gate is central allocation even when the number is learned rather than declared:
the kernel still converts a global estimate into permission. More importantly, the available signal
is too sparse, confounded, non-stationary, and gameable to justify that authority.

ymp retains immutable outcome evidence. Participants may inspect it when making local offers, bids,
awards, challenges, and verification decisions. The kernel neither compresses it into rank nor
enforces it as access control.

## Quantitative cold-start problem

Assume, unrealistically favourably, that one runtime profile has a fixed success probability `p` on
independent, identically distributed tasks and that acceptance is a perfect binary observation. With
a uniform `Beta(1, 1)` prior and approximately 70% observed success, the posterior remains broad:

| Comparable outcomes | Successes | Posterior mean | 95% credible interval |
|---:|---:|---:|---:|
| 5 | 4 | 0.71 | 0.36–0.96 |
| 10 | 7 | 0.67 | 0.39–0.89 |
| 20 | 14 | 0.68 | 0.48–0.85 |
| 50 | 35 | 0.69 | 0.56–0.81 |
| 100 | 70 | 0.70 | 0.60–0.78 |

Under the usual normal approximation for a two-proportion test, distinguishing true pass rates of
60% and 75% with a two-sided 5% error rate and 80% power requires about **152 comparable outcomes
per runtime profile**. Estimating a 70% pass rate to roughly ±10 percentage points requires about 81
outcomes; ±5 points requires about 323.

A per-skill split makes the data thinner. Real outcomes are also less informative than this model
assumes because:

- tasks differ in difficulty, decomposability, environment, and oracle strength;
- participants choose tasks rather than receiving a random sample;
- one result may depend on a sponsor's decomposition, another agent's finding, and an integrator;
- agent runtime, harness policy, model route, tool projection, and provider versions change;
- repeated protected-check feedback changes later attempts;
- a pass may be an oracle exploit and a failure may be infrastructure error; and
- skill tags are partly self-declared and do not define statistically exchangeable populations.

The proposed grade would therefore express false precision long before it carried useful
information.

## Selection feedback would reinforce early noise

A gate creates a feedback loop: an early success yields more opportunities, those opportunities
yield more evidence, and untried participants remain uncertain because they are denied work. Random
initial outcomes become durable inequality. Symmetric `+1/-1` movement does not fix the exposure
bias, and weighting by a task complexity score introduces another unvalidated estimate.

This is also why a generic exploration algorithm is not placed in the kernel. Multi-armed-bandit
methods assume a defined chooser, reward, context, and stationarity model. Making the kernel that
chooser would reintroduce the semantic dispatcher the design rejects. Participants may explore and
use evidence locally within their own delegated budgets.

## Evidence ledger

Raw outcome records may persist across runs. Each record includes enough context to prevent silent
pooling:

- agent-runtime kind, driver version, external harness version and digest where applicable;
- harness and prompt-policy versions, model provider and deployment, endpoint class, wire
  protocol, model identifier or snapshot, account or quota scope, authentication mode, and
  data-disclosure class;
- coordination-tool projection and schema version;
- participant, attempt, task, parent obligation, source, and base-candidate digests;
- project class and declared task attributes;
- what prior findings, candidate rationales, and protected feedback were visible;
- sponsor and contributor lineage;
- contract, environment, verifier, oracle, and diagnostic-disclosure regimes;
- `passed`, `failed`, `infrastructure_error`, `cancelled`, or `abstained` outcome;
- resource vector consumed and wall-clock latency;
- structured failure observations where independently established; and
- timestamp and supersession relationships.

Peer praise, self-reported confidence, bid selection, and consensus are not acceptance outcomes.
They remain separate attributed events.

## Communication evidence is not reputation

The communication observatory may record that a published finding was cited by a later decision,
appeared in an accepted artifact lineage, survived a challenge, or changed behaviour under a
controlled message intervention. These observations belong to an episode, group, task, and study
condition. They are not stable personal traits.

In particular, ymp does not score participants for:

- sending many messages or producing long explanations;
- persuading another participant or causing its action to change;
- agreeing with a later consensus;
- claiming a role, insight, confidence level, or theory of mind; or
- receiving citations from peers.

Causal influence is measured only as a research variable. Rewarding it would let manipulation,
authority laundering, and theatrical dialogue compete directly with useful work. Contribution
credit remains uncertain when several participants share context or build on the same artifact;
the ledger preserves provenance rather than inventing a scalar allocation of merit.

## Evidence cards

A query may derive a context-specific **evidence card** from the ledger. A card must show:

- the exact filter and grouping dimensions;
- successes, failures, excluded outcomes, and effective sample size;
- estimate plus interval, never the mean alone;
- recency and version coverage;
- task-selection and verifier-strength caveats;
- cost distribution, not only pass rate; and
- whether observations share model family, context, or other sources of correlation.

A beta-binomial summary is permitted only as a descriptive view over genuinely comparable binary
observations. It does not solve task-difficulty adjustment, causal credit assignment, or context
transfer. When behavior may change, recent-window or discounted views may supplement the full
history; the raw record is never discarded.

The evidence service returns facts and uncertainty. It does not return `eligible`, `required_level`,
or a single globally ordered score.

## Cross-project persistence

What persists is evidence, not reputation. A result from one project may be displayed for another
only with its original context visible. Derived estimates are separated at least by agent-runtime
and harness-policy version, model route, project class, task attribute set, and verification
regime. Automatic transfer across skills or project classes is prohibited until a preregistered
transfer study demonstrates calibration.

This still permits learning over time. It refuses to pretend that “passed tests on small Rust
repairs” is a stable trait that authorizes architecture work on an unrelated repository.

## Cold start

New participants are not assigned an average grade or blocked from difficult work. They may bid on
offers under the same mechanical rules as others. A sponsor sees that evidence is absent and decides
locally whether the task, proposal budget, and risk justify exploration.

Configured benchmark claims may appear as explicitly external evidence, never as observed ymp
outcomes. Proposal-stage budgets make audition cost visible. Per-principal fair admission prevents
an established participant from winning merely by flooding, while no rule promises equal semantic
opportunity.

## Task difficulty and unmet demand

An unsolved task does not become more complex. Complexity is a latent property; failure adds
evidence about the current combination of task, method, agent runtime, model route, environment,
and oracle.

The board therefore exposes raw unmet-demand observations:

- offer age and time to deadline;
- number and diversity lineage of attempts;
- typed failure and infrastructure outcomes;
- bids, withdrawals, and dead-end findings;
- remaining escrow and verification queries; and
- dependency and obligation state.

Participants may infer that decomposition, recruitment, a new method, clarification, or abstention
is appropriate. The kernel does not raise a global “complexity” value, unlock a stronger tier, or
prescribe decomposition first.

## Diversity and independence

Different runtime and model-route configurations may be useful, but vendor or model labels are weak proxies for
independent error. Shared training data, prompts, visible findings, tools, and evaluation feedback
can make nominally different judges behave like a much smaller panel.

ymp records an **independence lineage** rather than awarding a diversity bonus. For an independent
proposal or review phase, a participant can be blinded from earlier answers and rationales until it
has committed an initial digest. Later critique is allowed, but the initial record remains
observable. This preserves disagreement evidence and reduces anchoring.

Majority vote and consensus never accept a candidate. Machine checks, protected observations, and
authorized human judgments remain the oracle.

## Conditions for any future adaptive allocation feature

No learned allocation mechanism enters the trusted kernel. An advisory mechanism may be tested
only after all of the following are true:

1. a task context taxonomy has demonstrated predictive validity rather than merely sounding
   plausible;
2. sample-size and calibration thresholds are preregistered;
3. model and environment drift are handled explicitly;
4. selection bias and new-participant exploration are measured;
5. credit remains attached to the causal contribution and verifier regime;
6. an ablation beats local decisions using raw evidence under the same budget; and
7. the mechanism cannot deny permission, force an award, or hide its uncertainty.

Until then, evidence cards are an observability and local-decision aid. They are not “the one
adaptive element” that makes the network intelligent.
